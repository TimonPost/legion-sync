use crate::register::RegisteredComponent;
use legion::{
    prelude::*,
    storage::{ComponentMeta, ComponentStorage, ComponentTypeId},
};
use std::collections::HashMap;

/// A trivial clone merge impl that does nothing but copy data. All component types must be
/// cloneable and no type transformations are allowed
pub struct CopyCloneImpl {
    components: HashMap<ComponentTypeId, RegisteredComponent>,
}

impl CopyCloneImpl {
    pub fn new(components: HashMap<ComponentTypeId, RegisteredComponent>) -> Self {
        Self { components }
    }
}

impl legion::world::CloneMergeImpl for CopyCloneImpl {
    fn map_component_type(
        &self,
        component_type: ComponentTypeId,
    ) -> (ComponentTypeId, ComponentMeta) {
        let comp_reg = &self.components[&component_type];
        (ComponentTypeId(comp_reg.ty(), 0), comp_reg.meta().clone())
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
