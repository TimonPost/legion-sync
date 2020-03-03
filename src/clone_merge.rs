use crate::register::ComponentRegistrationRef;
use legion::{
    prelude::*,
    storage::{ComponentMeta, ComponentStorage, ComponentTypeId},
};
use std::collections::HashMap;

/// A trivial clone merge impl that does nothing but copy data. All component types must be
/// cloneable and no type transformations are allowed
pub struct CopyCloneImpl {
    components: HashMap<ComponentTypeId, ComponentRegistrationRef>,
}

impl CopyCloneImpl {
    pub fn new(components: HashMap<ComponentTypeId, ComponentRegistrationRef>) -> Self {
        Self { components }
    }
}

impl legion::world::CloneImpl for CopyCloneImpl {
    fn map_component_type(
        &self,
        component_type: ComponentTypeId,
    ) -> (ComponentTypeId, ComponentMeta) {
        let comp_reg = &self.components[&component_type];
        (comp_reg.component_type_id(), comp_reg.meta().clone())
    }

    fn clone_components(
        &self,
        src_world: &World,
        src_component_storage: &ComponentStorage,
        src_component_storage_indexes: core::ops::Range<usize>,
        src_type: ComponentTypeId,
        src_entities: &[Entity],
        dst_entities: &[Entity],
        src_data: *const u8,
        dst_data: *mut u8,
        num_components: usize,
    ) {
        let comp_reg = &self.components[&src_type];
        unsafe {
            comp_reg.clone_components(src_data, dst_data, num_components);
        }
    }
}
