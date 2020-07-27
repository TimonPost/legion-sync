use legion::systems::{Builder, Resource};

use crate::register::ComponentRegistration;
use legion::{world::SubWorld, Entity, World};
use net_sync::compression::CompressionStrategy;

pub mod client;
pub mod server;
pub mod world_instance;

pub trait WorldBuilder {
    type BuildResult;

    fn default_resources<C: CompressionStrategy + 'static>(self) -> Self;

    fn default_systems(self) -> Self;

    fn with_resource<R: Resource>(self, resource: R) -> Self;

    fn register_systems(self, user_system_builder: fn(Builder) -> Builder) -> Self;

    fn build(self) -> Self::BuildResult;
}

pub trait WorldAbstraction {
    fn has_component(&self, entity: Entity, component: &ComponentRegistration) -> bool;
}

impl WorldAbstraction for World {
    fn has_component(&self, entity: Entity, component: &ComponentRegistration) -> bool {
        component.exists_in_world(&self, entity)
    }
}

impl<'a> WorldAbstraction for SubWorld<'a> {
    fn has_component(&self, entity: Entity, component: &ComponentRegistration) -> bool {
        component.exists_in_subworld(&self, entity)
    }
}
