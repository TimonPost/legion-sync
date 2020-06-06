use legion::systems::{resource::Resource, schedule::Builder};

use crate::register::ComponentRegistration;
use legion::prelude::{Entity, World};
use net_sync::{compression::CompressionStrategy, serialization::SerializationStrategy};

pub mod client;
pub mod server;
pub mod world_instance;

pub trait WorldBuilder {
    type BuildResult;

    fn default_resources<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        self,
    ) -> Self;

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

impl WorldAbstraction for legion::systems::SubWorld {
    fn has_component(&self, entity: Entity, component: &ComponentRegistration) -> bool {
        component.exists_in_subworld(&self, entity)
    }
}
