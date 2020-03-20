use crate::resources::PostBoxResource;
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
    prelude::{Entity, Resources, Universe},
    systems::{resource::Resource, schedule::Builder},
};
use net_sync::transport::PostBox;
use net_sync::{
    compression::CompressionStrategy,
    state::WorldState,
    uid::{Uid, UidAllocator},
    ClientMessage, ServerMessage,
};
use std::net::SocketAddr;

pub struct ClientUniverse {
    universe: NetworkUniverse,
    pub(crate) resources: Resources,
}

impl ClientUniverse {
    pub fn new(resources: Resources, universe: NetworkUniverse) -> ClientUniverse {
        ClientUniverse {
            universe,
            resources,
        }
    }

    pub fn universe(&mut self) -> &mut NetworkUniverse {
        &mut self.universe
    }

    pub fn tick(&mut self) {
        let resources = &mut self.resources;

        self.universe.remote.execute(resources);
        self.universe.main.execute(resources);

        let tick = resources.get_mut::<TickResource>().unwrap().tick();
        let mut postbox = resources.get_mut::<PostBoxResource>().unwrap();

        if tick % 10 == 0 {
            let inbox = postbox.drain_inbox(|m| match m {
                ServerMessage::StateUpdate(_) => true,
                _ => false,
            });

            for packet in inbox {
                match packet {
                    ServerMessage::StateUpdate(update) => {}
                }
            }

            let mut world_state = WorldState::new();
            self.universe.merge_into(resources, &mut world_state);
        }

        resources.get_mut::<TickResource>().unwrap().increment();
    }

    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    pub fn new_entity_id(&self, entity: Entity) -> Uid {
        let mut borrow = self.resources.get_mut::<UidAllocator<Entity>>().unwrap();
        borrow.allocate(entity, None)
    }
}

pub struct ClientUniverseBuilder {
    resources: Resources,
    remote_builder: Builder,
    main_builder: Builder,
}

impl Default for ClientUniverseBuilder {
    fn default() -> Self {
        ClientUniverseBuilder {
            resources: Default::default(),
            remote_builder: Builder::default(),
            main_builder: Builder::default(),
        }
    }
}

impl UniverseBuilder for ClientUniverseBuilder {
    type BuildResult = ClientUniverse;

    fn default_resources<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        self,
    ) -> Self {
        let mut s = self;
        s.resources
            .insert_client_resources(S::default(), C::default());
        s.resources.insert_required(S::default(), C::default());
        s
    }

    fn default_systems(self) -> Self {
        let mut s = self;
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
        s.main_builder = s.main_builder.add_client_systems();
        s.main_builder = builder(s.main_builder);
        s
    }

    fn remote_builder<F>(self, builder: F) -> Self
    where
        F: Fn(Builder) -> Builder,
    {
        let mut s = self;
        s.remote_builder = builder(Builder::default());
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

        ClientUniverse::new(
            s.resources,
            NetworkUniverse::new(universe, main_world, remote_world),
        )
    }
}

impl ClientUniverseBuilder {
    pub fn with_tcp<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        mut self,
        addr: SocketAddr,
    ) -> Self {
        self.remote_builder = self.remote_builder.add_tcp_client_systems::<S, C>();
        self.resources.insert_tcp_client_resources(addr);
        self
    }
}
