use std::{any::TypeId, collections::HashMap};

use legion::{
    storage::{ComponentMeta, ComponentTypeId},
    systems::SystemBuilder,
    world::{EntityStore, SubWorld, World},
    Entity,
};

use serde::{
    export::{
        fmt::{Debug, Error},
        Formatter,
    },
    Deserialize, Serialize,
};

use net_sync::{
    error::ErrorKind,
    re_exports::serde_diff,
    track_attr::serde_diff::{Config, FieldPathMode, SerdeDiff},
    uid::{Uid, UidAllocator},
};

inventory::collect!(ComponentRegistration);

pub type ComponentRegistrationRef = &'static ComponentRegistration;
pub type HashmapRegistry = HashMap<ComponentTypeId, ComponentRegistrationRef>;

#[derive(Clone)]
pub struct ComponentRegistration {
    pub(crate) component_type_id: ComponentTypeId,
    pub(crate) meta: ComponentMeta,
    pub(crate) type_name: &'static str,

    pub(crate) components_clone: fn(*const u8, *mut u8, usize),

    pub(crate) exists_in_world: fn(world: &World, entity: Entity) -> bool,

    pub(crate) exists_in_subworld: fn(world: &SubWorld, entity: Entity) -> bool,

    pub(crate) serialize_if_exists_in_world: fn(
        world: &World,
        entity: Entity,
        serialize_fn: &mut dyn FnMut(&dyn erased_serde::Serialize),
    ),

    pub(crate) serialize_difference: fn(
        unchanged: &mut dyn erased_serde::Deserializer,
        changed: &mut dyn erased_serde::Deserializer,
        serializer: &mut dyn erased_serde::Serializer,
    ) -> Result<bool, ErrorKind>,

    pub(crate) serialize_difference_with_current: fn(
        world: &World,
        entity: Entity,
        unchanged: &mut dyn erased_serde::Deserializer,
        serializer: &mut dyn erased_serde::Serializer,
    ) -> Result<bool, ErrorKind>,

    pub(crate) grand_write_access: fn(system_builder: SystemBuilder) -> SystemBuilder,
    pub(crate) grand_read_access: fn(system_builder: SystemBuilder) -> SystemBuilder,

    pub(crate) add_component:
        fn(world: &mut World, entity: Entity, data: &mut dyn erased_serde::Deserializer),

    pub(crate) register_into_registry: fn(world: &mut legion::Registry<String>),

    pub(crate) register_into_merger: fn(world: &mut legion::world::Duplicate),

    pub(crate) remove_component: fn(world: &mut World, entity: Entity),

    pub(crate) apply_changes:
        fn(world: &mut World, entity: Entity, changes: &mut dyn erased_serde::Deserializer),
}

impl Debug for ComponentRegistration {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str(self.type_name)
    }
}

impl ComponentRegistration {
    pub fn ty(&self) -> TypeId {
        self.component_type_id.type_id()
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

    pub fn exists_in_subworld(&self, world: &SubWorld, entity: Entity) -> bool {
        (self.exists_in_subworld)(world, entity)
    }

    pub fn exists_in_world(&self, world: &World, entity: Entity) -> bool {
        (self.exists_in_world)(world, entity)
    }

    pub fn serialize_if_exists_in_world(
        &self,
        world: &World,
        entity: Entity,
        serialize_fn: &mut dyn FnMut(&dyn erased_serde::Serialize),
    ) {
        (self.serialize_if_exists_in_world)(world, entity, serialize_fn)
    }

    pub fn serialize_difference(
        &self,
        unchanged: &mut dyn erased_serde::Deserializer,
        changed: &mut dyn erased_serde::Deserializer,
        serializer: &mut dyn erased_serde::Serializer,
    ) -> Result<bool, ErrorKind> {
        (self.serialize_difference)(unchanged, changed, serializer)
    }

    pub fn serialize_difference_with_current(
        &self,
        world: &World,
        entity: Entity,
        unchanged: &mut dyn erased_serde::Deserializer,
        serializer: &mut dyn erased_serde::Serializer,
    ) -> Result<bool, ErrorKind> {
        (self.serialize_difference_with_current)(world, entity, unchanged, serializer)
    }

    pub fn grand_read_access(&self, system_builder: SystemBuilder) -> SystemBuilder {
        (self.grand_read_access)(system_builder)
    }

    pub fn grand_write_access(&self, system_builder: SystemBuilder) -> SystemBuilder {
        (self.grand_write_access)(system_builder)
    }

    pub fn register_into_registry(&self, registry: &mut legion::Registry<String>) {
        (self.register_into_registry)(registry)
    }

    pub fn register_into_merger(&self, merger: &mut legion::world::Duplicate) {
        (self.register_into_merger)(merger)
    }

    pub fn add_component(
        &self,
        world: &mut World,
        entity: Entity,
        component_raw: &mut dyn erased_serde::Deserializer,
    ) {
        (self.add_component)(world, entity, component_raw)
    }

    pub fn remove_component(&self, world: &mut World, entity: Entity) {
        (self.remove_component)(world, entity)
    }

    pub fn apply_changes(
        &self,
        world: &mut World,
        entity: Entity,
        data: &mut dyn erased_serde::Deserializer,
    ) {
        (self.apply_changes)(world, entity, data)
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
    >() -> Self {
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
            exists_in_subworld: |world, entity| -> bool {
                if let Some(entry) = world.entry_ref(entity) {
                    entry.get_component::<T>().is_ok()
                } else {
                    false
                }
            },
            exists_in_world: |world, entity| -> bool {
                if let Some(entry) = world.entry_ref(entity) {
                    entry.get_component::<T>().is_ok()
                } else {
                    false
                }
            },
            serialize_if_exists_in_world: |world, entity, serializer_fn| {
                if let Some(entry) = world.entry_ref(entity) {
                    if let Ok(component) = entry.get_component::<T>() {
                        serializer_fn(&*component);
                    }
                }
            },
            serialize_difference: |unchanged, changed, serializer| {
                let unchanged = erased_serde::deserialize::<T>(unchanged)
                    .expect("failed to deserialize component");

                let changed = erased_serde::deserialize::<T>(changed)
                    .expect("failed to deserialize component");

                let diff = Config::new()
                    .with_field_path_mode(FieldPathMode::Index)
                    .serializable_diff(&unchanged, &changed);

                <serde_diff::Diff<T> as serde::ser::Serialize>::serialize(&diff, serializer)
                    .expect("failed to serialize diff");

                Ok(diff.has_changes())
            },
            serialize_difference_with_current: |world, entity, unchanged, serializer| {
                let unchanged = erased_serde::deserialize::<T>(unchanged)
                    .expect("failed to deserialize component");

                if let Some(entry) = world.entry_ref(entity) {
                    let changed = entry.get_component::<T>().expect("failed to get component");

                    let diff = Config::new()
                        .with_field_path_mode(FieldPathMode::Index)
                        .serializable_diff(&unchanged, &changed);

                    <serde_diff::Diff<T> as serde::ser::Serialize>::serialize(&diff, serializer)
                        .expect("failed to serialize diff");

                    return Ok(diff.has_changes());
                }

                Ok(false)
            },
            grand_read_access: |system_builder| system_builder.read_component::<T>(),
            grand_write_access: |system_builder| system_builder.write_component::<T>(),
            register_into_registry: |registry| {
                registry.register::<T>(std::any::type_name::<T>().to_string());
            },
            register_into_merger: |registry| {
                registry.register_clone::<T>();
            },
            add_component: |world, entity, data| {
                let component =
                    erased_serde::deserialize::<T>(data).expect("failed to deserialize component");

                if let Some(mut entry) = world.entry(entity) {
                    entry.add_component::<T>(component);
                }
            },
            remove_component: |world, entity| {
                if let Some(mut entry) = world.entry(entity) {
                    entry.remove_component::<T>();
                }
            },
            apply_changes: |world, entity, data| {
                if let Some(mut entry) = world.entry(entity) {
                    let mut component = entry
                        .get_component_mut::<T>()
                        .expect("Can not apply changes to component.");

                    <serde_diff::Apply<T> as serde::de::DeserializeSeed>::deserialize(
                        serde_diff::Apply::deserializable(&mut component),
                        data,
                    );
                };
            },
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
    ($component_type:ty) => {
        inventory::submit! {
             $crate::register::ComponentRegistration::of::<$component_type>()
        }
    };
}

#[cfg(test)]
pub mod test {
    use std::any::TypeId;

    use legion::storage::{ComponentMeta, ComponentTypeId};

    use crate::{
        components::UidComponent,
        register::{ComponentRegister, ComponentRegistration, ComponentRegistrationRef},
        tracking::{re_exports::serde_diff::*, track_attr::*},
    };

    #[derive(Clone, Default, Debug, Serialize, Deserialize, SerdeDiff)]
    struct Component {}

    crate::register_component_type!(Component, Bincode);

    #[test]
    fn registered_by_component_id_should_be_filled_test() {
        let registered = ComponentRegister::by_component_id();

        assert_eq!(registered.len(), 2);
    }

    #[test]
    fn registered_by_uid_should_be_filled_test() {
        let registered = ComponentRegister::by_unique_uid();

        assert_eq!(registered.len(), 2);
    }

    #[test]
    fn uid_should_start_count_at_one_test() {
        let registered = ComponentRegister::by_unique_uid();

        assert!(registered.get(&1).is_some());
        assert!(registered.get(&2).is_some());
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
