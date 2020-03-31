use crate::tracking::SerializationStrategy;
use legion::systems::{resource::Resource, schedule::Builder};
use net_sync::compression::CompressionStrategy;
pub mod client;
pub mod network;
pub mod server;

pub trait UniverseBuilder {
    type BuildResult;

    fn default_resources<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        self,
    ) -> Self;

    fn default_systems(self) -> Self;

    fn with_resource<R: Resource>(self, resource: R) -> Self;

    fn register_remote_systems(self, user_system_builder: fn(Builder) -> Builder) -> Self;

    fn register_main_systems(self, user_system_builder: fn(Builder) -> Builder) -> Self;

    fn build(self) -> Self::BuildResult;
}
