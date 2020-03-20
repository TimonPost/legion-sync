use crate::{
    components::UidComponent,
    tracking::serde_diff::{Config, Diff, FieldPathMode, SerdeDiff},
};
use legion::{
    command::CommandBuffer,
    prelude::Entity,
    storage::{ComponentMeta, ComponentTypeId},
    systems::{SubWorld, SystemBuilder},
    world::World,
};
use log::error;
use net_sync::{
    uid::{Uid, UidAllocator},
    ComponentId,
};
use serde::{
    export::{
        fmt::{Debug, Error},
        Formatter,
    },
    Deserialize, Serialize,
};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};
use track::{error::ErrorKind, serialization::SerializationStrategy, ModificationEvent};
inventory::collect!(ComponentRegistration);

pub type ComponentRegistrationRef = &'static ComponentRegistration;
pub type HashmapRegistery = HashMap<ComponentTypeId, ComponentRegistrationRef>;

#[derive(Clone)]
pub struct ComponentRegistration {
    pub(crate) component_type_id: ComponentTypeId,
    pub(crate) meta: ComponentMeta,
    pub(crate) type_name: &'static str,
    pub(crate) components_clone: fn(*const u8, *mut u8, usize),
    pub(crate) serialize_if_different: Arc<
        dyn Fn(
                &legion::world::World,
                legion::entity::Entity,
                &legion::world::World,
                legion::entity::Entity,
                &crate::tracking::Sender<ModificationEvent<ComponentId>>,
            ) + Send
            + Sync,
    >,
    // the following functions are duplicated, alternative would be to put them in a Arc Fn.
    pub(crate) exists_in_world: fn(world: &World, entity: Entity) -> bool,
    pub(crate) exists_in_subworld: fn(world: &SubWorld, entity: Entity) -> bool,
    pub(crate) serialize_if_in_world: Arc<
        dyn Fn(&World, legion::entity::Entity) -> Result<Option<Vec<u8>>, ErrorKind> + Send + Sync,
    >,
    pub(crate) serialize_if_in_subworld: Arc<
        dyn Fn(&SubWorld, legion::entity::Entity) -> Result<Option<Vec<u8>>, ErrorKind>
            + Send
            + Sync,
    >,
    pub(crate) deserialize_single_fn:
        Arc<dyn Fn(&CommandBuffer, legion::entity::Entity, &[u8]) + Send + Sync>,
    pub(crate) add_write_to_system: fn(SystemBuilder) -> SystemBuilder,
    pub(crate) add_read_to_system: fn(SystemBuilder) -> SystemBuilder,
}

impl Debug for ComponentRegistration {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str(self.type_name)
    }
}

impl ComponentRegistration {
    pub fn ty(&self) -> TypeId {
        self.component_type_id.0
    }

    pub fn component_type_id(&self) -> ComponentTypeId {
        self.component_type_id
    }

    pub fn meta(&self) -> &ComponentMeta {
        &self.meta
    }

    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn deserialize_single(
        &self,
        command_buffer: &CommandBuffer,
        entity: legion::entity::Entity,
        data: &[u8],
    ) {
        (&*self.deserialize_single_fn)(command_buffer, entity, data)
    }

    pub fn serialize_if_different(
        &self,
        src_world: &legion::world::World,
        src_entity: legion::entity::Entity,
        dst_world: &legion::world::World,
        dst_entity: legion::entity::Entity,
        notifier: &crate::tracking::Sender<ModificationEvent<ComponentId>>,
    ) {
        (&*self.serialize_if_different)(src_world, src_entity, dst_world, dst_entity, notifier)
    }

    pub fn exists_in_subworld(&self, world: &SubWorld, entity: legion::entity::Entity) -> bool {
        (&self.exists_in_subworld)(world, entity)
    }

    pub fn exists_in_world(&self, world: &World, entity: legion::entity::Entity) -> bool {
        (&self.exists_in_world)(world, entity)
    }

    pub fn serialize_if_in_world(
        &self,
        world: &World,
        entity: legion::entity::Entity,
    ) -> Result<Option<Vec<u8>>, ErrorKind> {
        (&*self.serialize_if_in_world)(world, entity)
    }

    pub fn serialize_if_in_subworld(
        &self,
        world: &SubWorld,
        entity: legion::entity::Entity,
    ) -> Result<Option<Vec<u8>>, ErrorKind> {
        (&*self.serialize_if_in_subworld)(world, entity)
    }

    pub fn compare(&self, component_type: ComponentTypeId) -> bool {
        self.type_id() == component_type.0
    }

    pub fn add_read_to_system(&self, system_builder: SystemBuilder) -> SystemBuilder {
        (self.add_read_to_system)(system_builder)
    }

    pub fn add_write_to_system(&self, system_builder: SystemBuilder) -> SystemBuilder {
        (self.add_write_to_system)(system_builder)
    }

    pub unsafe fn clone_components(&self, src: *const u8, dst: *mut u8, num_components: usize) {
        (self.components_clone)(src, dst, num_components)
    }

    pub fn of<
        T: Clone
            + Debug
            + Serialize
            + for<'de> Deserialize<'de>
            + Send
            + Sync
            + SerdeDiff
            + Default
            + 'static,
        S: SerializationStrategy + 'static + Clone,
    >(
        serialisation: S,
    ) -> Self {
        // The functions below need to own the serializer because they are standalone functions.
        // This is a bit awkward, and should probably be done on an other way.
        //
        // I thought about passing the serializer as a function parameter instead from here.
        // The problem with this is that the serde traits can't be trait objects because of it's generic parameters.
        // And therefore the `SerializationStrategy` can't be as well.
        // We can not pass an serde implementation as an function argument.
        // The trait object problem could be solved by using `ereased_serde` but
        // `bincode`, `rmp` aren't compilable with this since they don't expose `(Se/Dese)rializer`.

        let serialize1 = serialisation.clone();
        let serialize2 = serialisation.clone();
        let serialize3 = serialisation.clone();
        let deserialize = serialisation.clone();

        Self {
            component_type_id: ComponentTypeId::of::<T>(),
            meta: ComponentMeta::of::<T>(),
            type_name: std::any::type_name::<T>(),
            components_clone: move |src, dst, num_components| unsafe {
                for i in 0..num_components {
                    let src_ptr = (src as *const T).add(i);
                    let dst_ptr = (dst as *mut T).add(i);

                    std::ptr::write(dst_ptr, <T as Clone>::clone(&*src_ptr));
                }
            },
            serialize_if_different: Arc::new(
                move |src_world, src_entity, dst_world, dst_entity, notifier| {
                    let src_comp = src_world.get_component::<T>(src_entity);
                    let dst_comp = dst_world.get_component::<T>(dst_entity);

                    if let (Some(src_comp), Some(dst_comp)) = (src_comp, dst_comp) {
                        let diff = Config::new()
                            .with_field_path_mode(FieldPathMode::Index)
                            .serializable_diff(&*src_comp, &*dst_comp);

                        match serialize3.serialize::<Diff<T>>(&diff) {
                            Ok(data) => {
                                if diff.has_changes() {
                                    let identifier = src_world
                                        .get_component::<UidComponent>(src_entity)
                                        .expect("Serializing difference, uid should exit.");
                                    notifier
                                    .send(ModificationEvent::new(data, identifier.uid(), TypeId::of::<T>()))
                                    .expect("The sender for modification events panicked. Is the receiver still alive?");
                                }
                            }
                            Err(e) => {
                                error!(
                                    "Could not serialize modification information because: {:?}",
                                    e
                                );
                            }
                        };
                    }
                },
            ),
            exists_in_subworld: |world, entity| -> bool {
                // TODO: World supports a check if an component is in an entity.
                // Maybe we should open A PR for allowing this via the system world as well.
                world.get_component::<T>(entity).is_some()
            },
            exists_in_world: |world, entity| -> bool {
                // TODO: World supports a check if an component is in an entity.
                // Maybe we should open A PR for allowing this via the system world as well.
                world.get_component::<T>(entity).is_some()
            },
            serialize_if_in_world: Arc::new(
                move |world, entity| -> Result<Option<Vec<u8>>, ErrorKind> {
                    if let Some(component) = world.get_component::<T>(entity) {
                        return Ok(Some(serialize1.serialize(&*component)?));
                    }
                    Ok(None)
                },
            ),
            serialize_if_in_subworld: Arc::new(
                move |world, entity| -> Result<Option<Vec<u8>>, ErrorKind> {
                    if let Some(component) = world.get_component::<T>(entity) {
                        return Ok(Some(serialize2.serialize(&*component)?));
                    }
                    Ok(None)
                },
            ),
            deserialize_single_fn: Arc::new(move |command_buffer, entity, data| {
                // TODO propagate error
                let comp = deserialize
                    .deserialize::<T>(data)
                    .expect("failed to deserialize component");

                command_buffer.add_component(entity, comp);
            }),
            add_read_to_system: |system_builder| system_builder.read_component::<T>(),
            add_write_to_system: |system_builder| system_builder.write_component::<T>(),
        }
    }
}

pub struct ComponentRegister;

impl ComponentRegister {
    pub fn by_component_id() -> HashMap<ComponentTypeId, ComponentRegistrationRef> {
        let mut registered_components = HashMap::new();

        for component in ComponentRegister.iter() {
            registered_components.insert(component.component_type_id(), component);
        }

        registered_components
    }

    pub fn by_unique_uid() -> HashMap<Uid, ComponentRegistrationRef> {
        let mut uid_allocator = UidAllocator::new();
        let mut registered_components = HashMap::new();

        for component in ComponentRegister.iter() {
            let id = uid_allocator.allocate(component.ty(), None);
            registered_components.insert(id, component);
        }

        registered_components
    }

    pub fn iter(&self) -> impl Iterator<Item = ComponentRegistrationRef> {
        inventory::iter::<ComponentRegistration>.into_iter()
    }
}

#[macro_export]
macro_rules! register_component_type {
    ($component_type:ty, $serialisation:expr) => {
       inventory::submit!{
            $crate::register::ComponentRegistration::of::<$component_type, $serialisation>($serialisation)
       }
    };
}

#[cfg(test)]
pub mod test {
    use crate::{
        components::UidComponent,
        register::{ComponentRegister, ComponentRegistration, ComponentRegistrationRef},
        tracking::{serde_diff, Bincode, SerdeDiff},
    };
    use legion::storage::{ComponentMeta, ComponentTypeId};
    use serde::{Deserialize, Serialize};
    use std::any::TypeId;

    #[derive(Clone, Default, Debug, Serialize, Deserialize, SerdeDiff)]
    struct Component {}

    crate::register_component_type!(Component, Bincode);

    #[test]
    fn registered_by_component_id_should_be_filled_test() {
        let registered = ComponentRegister::by_component_id();

        assert_eq!(registered.len(), 3);
    }

    #[test]
    fn registered_by_uid_should_be_filled_test() {
        let registered = ComponentRegister::by_unique_uid();

        assert_eq!(registered.len(), 3);
    }

    #[test]
    fn uid_should_start_count_at_one_test() {
        let registered = ComponentRegister::by_unique_uid();

        assert!(registered.get(&1).is_some());
        assert!(registered.get(&2).is_some());
        assert!(registered.get(&3).is_some());
    }

    #[test]
    fn uid_should_be_registered_test() {
        let registered = ComponentRegister::by_component_id()
            .into_iter()
            .filter(|f| f.1.ty() == TypeId::of::<UidComponent>())
            .map(|(_k, v)| v)
            .collect::<Vec<ComponentRegistrationRef>>();

        assert_eq!(registered.len(), 1);
    }

    #[test]
    fn registered_component_has_correct_information_test() {
        let registered = ComponentRegister::by_component_id()
            .into_iter()
            .filter(|f| f.1.ty() == TypeId::of::<UidComponent>())
            .map(|(_k, v)| v.clone())
            .collect::<Vec<ComponentRegistration>>();

        assert!(registered[0].type_name() == std::any::type_name::<UidComponent>());
        assert!(registered[0].meta() == &ComponentMeta::of::<UidComponent>());
        assert_eq!(
            registered[0].component_type_id(),
            ComponentTypeId::of::<UidComponent>()
        );
    }
}
