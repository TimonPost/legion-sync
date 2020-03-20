use crate::{
    resources::{EventResource, ResourcesExt, TickResource},
    systems::SchedulerExt,
    tracking::SerializationStrategy,
    universe::{
        network::{NetworkUniverse, WorldInstance},
        UniverseBuilder,
    },
};
use legion::{
    prelude::{Resources, Universe},
    systems::{resource::Resource, schedule::Builder},
};
use net_sync::{compression::CompressionStrategy, state::WorldState};
use std::net::TcpListener;

pub struct ServerConfig {
    /// The tick rate in milliseconds.
    pub tick_rate: u8,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig { tick_rate: 10 }
    }
}

pub struct ServerUniverseBuilder {
    resources: Resources,
    remote_builder: Builder,
    main_builder: Builder,
    config: ServerConfig,
}

impl Default for ServerUniverseBuilder {
    fn default() -> Self {
        ServerUniverseBuilder {
            resources: Default::default(),
            remote_builder: Builder::default(),
            main_builder: Builder::default(),
            config: ServerConfig::default(),
        }
    }
}

impl UniverseBuilder for ServerUniverseBuilder {
    type BuildResult = ServerUniverse;

    fn default_resources<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        self,
    ) -> Self {
        let mut s = self;
        s.resources
            .insert_server_resources(S::default(), C::default());
        s
    }

    fn default_systems(self) -> Self {
        let mut s = self;
        s.remote_builder = s.remote_builder.add_server_systems();
        s
    }

    fn with_resource<R: Resource>(self, resource: R) -> Self {
        let mut s = self;
        s.resources.insert(resource);
        s
    }

    fn main_builder<F>(self, builder: F) -> Self
    where
        F: Fn(Builder) -> Builder,
    {
        let mut s = self;
        s.main_builder = builder(s.main_builder);
        s
    }

    fn remote_builder<F>(self, builder: F) -> Self
    where
        F: Fn(Builder) -> Builder,
    {
        let mut s = self;
        s.remote_builder = builder(s.remote_builder);
        s
    }

    fn build(self) -> Self::BuildResult {
        let mut s = self;

        let universe = Universe::new();
        let mut main_world = universe.create_world();
        let remote_world = universe.create_world();

        s.resources.insert(EventResource::new(&mut main_world));

        let main_world = WorldInstance::new(main_world, s.main_builder.build());
        let remote_world = WorldInstance::new(remote_world, s.remote_builder.build());

        ServerUniverse::new(
            s.resources,
            NetworkUniverse::new(universe, main_world, remote_world),
        )
    }
}

impl ServerUniverseBuilder {
    pub fn with_tcp<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        mut self,
        listener: TcpListener,
    ) -> Self {
        self.resources.insert_tcp_listener_resources(listener);
        self.remote_builder = self.remote_builder.add_tcp_listener_systems::<S, C>();
        self
    }

    pub fn with_config(mut self, config: ServerConfig) -> Self {
        self.config = config;
        self
    }
}

pub struct ServerUniverse {
    config: ServerConfig,
    universe: NetworkUniverse,
    pub(crate) resources: Resources,
}

impl ServerUniverse {
    pub fn new(resources: Resources, universe: NetworkUniverse) -> ServerUniverse {
        ServerUniverse {
            resources,
            config: ServerConfig::default(),
            universe,
        }
    }

    pub fn tick(&mut self) {
        let resources = &mut self.resources;

        self.universe.remote.execute(resources);
        self.universe.main.execute(resources);

        let server_tick = resources.get_mut::<TickResource>().unwrap().tick();

        if server_tick % 10 == 0 {
            let mut world_state = WorldState::new();
            self.universe.merge_into(resources, &mut world_state);
        }

        resources.get_mut::<TickResource>().unwrap().increment();
    }
}
