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

    fn main_builder<F>(self, builder: F) -> Self
    where
        F: Fn(Builder) -> Builder;

    fn remote_builder<F>(self, builder: F) -> Self
    where
        F: Fn(Builder) -> Builder;

    fn build(self) -> Self::BuildResult;
}
