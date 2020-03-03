use legion::{
    command::CommandBuffer,
    storage::{ComponentMeta, ComponentTypeId},
    systems::SystemBuilder,
};
use net_sync::uid::{Uid, UidAllocator};
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
use track::{error::ErrorKind, serialization::SerializationStrategy};
use std::mem::MaybeUninit;

inventory::collect!(ComponentRegistration);

pub type ComponentRegistrationRef = &'static ComponentRegistration;
pub type HashmapRegistery = HashMap<ComponentTypeId, ComponentRegistrationRef>;

#[derive(Clone)]
pub struct ComponentRegistration {
    pub(crate) component_type_id: ComponentTypeId,
    pub(crate) meta: ComponentMeta,
    pub(crate) type_name: &'static str,
    pub(crate) comp_clone_fn: fn(*const u8, *mut u8, usize),
    pub(crate) serialize_if_in_entity: Arc<
        dyn Fn(
                &legion::systems::SubWorld,
                legion::entity::Entity,
            ) -> Result<Option<Vec<u8>>, ErrorKind>
            + Send
            + Sync,
    >,
    pub(crate) deserialize_single_fn:
        Arc<dyn Fn(&CommandBuffer, legion::entity::Entity, &[u8]) + Send + Sync>,
    pub(crate) add_to_system: fn(SystemBuilder) -> SystemBuilder,
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
        world: &mut legion::systems::SubWorld,
        command_buffer: &CommandBuffer,
        entity: legion::entity::Entity,
        data: &[u8],
    ) {
        (&*self.deserialize_single_fn)(command_buffer, entity, data)
    }

    pub fn serialize_if_in_entity(
        &self,
        world: &legion::systems::SubWorld,
        entity: legion::entity::Entity,
    ) -> Result<Option<Vec<u8>>, ErrorKind> {
        (&*self.serialize_if_in_entity)(world, entity)
    }

    pub fn compare(&self, component_type: ComponentTypeId) -> bool {
        self.type_id() == component_type.0
    }

    pub fn add_to_system(&self, system_builder: SystemBuilder) -> SystemBuilder {
        (self.add_to_system)(system_builder)
    }

    pub unsafe fn clone_components(&self, src: *const u8, dst: *mut u8, num_components: usize) {
        (self.comp_clone_fn)(src, dst, num_components);
    }

    pub fn of<
        T: Clone + Debug + Serialize + for<'de> Deserialize<'de> + Send + Sync + Default + 'static,
        S: SerializationStrategy + 'static + Clone,
    >(
        serialisation: S,
    ) -> Self {
        let serialize = serialisation.clone();
        let deserialize = serialisation.clone();

        Self {
            component_type_id: ComponentTypeId::of::<T>(),
            meta: ComponentMeta::of::<T>(),
            type_name: std::any::type_name::<T>(),
            comp_clone_fn: move |src, dst, num_components| unsafe {
                for i in 0..num_components {
                    let src_ptr = (src as *const T).add(i);
                    let dst_ptr = (dst as *mut T).add(i);
                    std::ptr::write(dst_ptr, <T as Clone>::clone(&*src_ptr));
                }
            },
            serialize_if_in_entity: Arc::new(
                move |world, entity| -> Result<Option<Vec<u8>>, ErrorKind> {
                    if let Some(component) = world.get_component::<T>(entity) {
                        return Ok(Some(serialize.serialize(&*component)?));
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
            add_to_system: |system_builder| system_builder.read_component::<T>(),
        }
    }
}

pub struct ComponentRegister;

impl ComponentRegister {
    pub fn by_component_id() -> HashMap<ComponentTypeId, ComponentRegistrationRef> {
        let mut allocated_components = HashMap::new();

        for component in ComponentRegister.iter() {
            allocated_components.insert(component.component_type_id(), component);
        }

        allocated_components
    }

    pub fn by_unique_uid() -> HashMap<Uid, ComponentRegistrationRef> {
        let mut uid_allocator = UidAllocator::new();
        let mut allocated_components = HashMap::new();

        for component in ComponentRegister.iter() {
            let id = uid_allocator.allocate(None);
            allocated_components.insert(id, component);
        }

        allocated_components
    }

    pub fn iter(&self) -> impl Iterator<Item =ComponentRegistrationRef> {
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
        tracking::Bincode,
    };
    use legion::storage::{ComponentMeta, ComponentTypeId};
    use net_sync::uid::Uid;
    use serde::{Deserialize, Serialize};
    use std::any::{Any, TypeId};

    #[derive(Clone, Default, Debug, Serialize, Deserialize)]
    struct Component;

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

        assert!(registered.get(&Uid(1)).is_some());
        assert!(registered.get(&Uid(2)).is_some());
        assert!(registered.get(&Uid(3)).is_some());
    }

    #[test]
    fn uid_should_be_registered_test() {
        let registered = ComponentRegister::by_component_id()
            .into_iter()
            .filter(|f| f.1.ty() == TypeId::of::<UidComponent>())
            .map(|(k, v)| v)
            .collect::<Vec<ComponentRegistrationRef>>();

        assert_eq!(registered.len(), 1);
    }

    #[test]
    fn registered_component_has_correct_information_test() {
        let registered = ComponentRegister::by_component_id()
            .into_iter()
            .filter(|f| f.1.ty() == TypeId::of::<UidComponent>())
            .map(|(k, v)| v.clone())
            .collect::<Vec<ComponentRegistration>>();

        assert!(registered[0].type_name() == std::any::type_name::<UidComponent>(););
        assert!(registered[0].meta() == &ComponentMeta::of::<UidComponent>());
        assert_eq!(
            registered[0].component_type_id(),
            ComponentTypeId::of::<UidComponent>()
        );
    }
}
