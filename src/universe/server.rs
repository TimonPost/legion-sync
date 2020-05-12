use std::net::TcpListener;

use legion::prelude::{Entity, World, any};
use legion::{
    prelude::{Resources, Universe},
    systems::{resource::Resource, schedule::Builder},
};
use log::debug;

use net_sync::compression::lz4::Lz4;
use net_sync::synchronisation::CommandFrameTicker;
use net_sync::transport::{PostOffice, NetworkCommand, NetworkMessage};
use net_sync::uid::UidAllocator;
use net_sync::{compression::CompressionStrategy, state::WorldState, ComponentData, transport};

use crate::event::{LegionEvent, LegionEventHandler};
use crate::filters::filter_fns::registered;
use crate::resources::RegisteredComponentsResource;
use crate::tracking::Bincode;
use crate::{
    resources::{EventResource, ResourcesExt},
    systems::BuilderExt,
    tracking::SerializationStrategy,
    universe::{
        network::{WorldInstance},
        UniverseBuilder,
    },
};
use serde::export::PhantomData;

pub struct ServerConfig {
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig { }
    }
}

pub struct ServerUniverseBuilder<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    resources: Resources,
    system_builder: Builder,
    config: ServerConfig,

    stcm: PhantomData<ServerToClientMessage>,
    ctsm: PhantomData<ClientToServerMessage>,
    ctsc: PhantomData<ClientToServerCommand>
}

impl<ServerToClientMessage: NetworkMessage,ClientToServerMessage: NetworkMessage,ClientToServerCommand: NetworkCommand> Default for ServerUniverseBuilder<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    fn default() -> Self {
        ServerUniverseBuilder {
            resources: Default::default(),
            system_builder: Builder::default(),
            config: ServerConfig::default(),

            stcm: PhantomData,
            ctsm: PhantomData,
            ctsc: PhantomData
        }
        .default_systems()
        .default_resources::<Bincode, Lz4>()
    }
}

impl<ServerToClientMessage: NetworkMessage,ClientToServerMessage: NetworkMessage,ClientToServerCommand: NetworkCommand> UniverseBuilder for ServerUniverseBuilder<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    type BuildResult = ServerUniverse<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>;

    fn default_resources<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        self,
    ) -> Self {
        let mut s = self;
        s.resources
            .insert_server_resources::<S, C, ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>(S::default(), C::default());
        s
    }

    fn default_systems(self) -> Self {
        let mut s = self;
        s.system_builder = s.system_builder.add_server_systems();
        s
    }

    fn with_resource<R: Resource>(self, resource: R) -> Self {
        let mut s = self;
        s.resources.insert(resource);
        s
    }

    fn register_systems(self, user_system_builder: fn(Builder) -> Builder) -> Self {
        let mut s = self;
        s.system_builder = user_system_builder(s.system_builder);
        s
    }

    fn build(self) -> Self::BuildResult {
        let mut s = self;

        let universe = Universe::new();
        let mut main_world = universe.create_world();

        s.resources
            .insert(EventResource::new(&mut main_world, any()));

        let world = WorldInstance::new(main_world, s.system_builder.build());

        ServerUniverse::new(
            s.resources,
            world,
        )
    }
}

impl<ServerToClientMessage: NetworkMessage,ClientToServerMessage: NetworkMessage,ClientToServerCommand: NetworkCommand> ServerUniverseBuilder<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    pub fn with_tcp<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        mut self,
        listener: TcpListener,
    ) -> Self {
        listener.set_nonblocking(true).expect("Cannot set non-blocking on TCP socket.");
        self.resources.insert_tcp_listener_resources(listener);
        self.system_builder = self.system_builder.add_tcp_server_systems::<S, C, ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>();
        self
    }

    pub fn with_config(mut self, config: ServerConfig) -> Self {
        self.config = config;
        self
    }
}

pub struct ServerUniverse<ServerToClientMessage: NetworkMessage,ClientToServerMessage: NetworkMessage,ClientToServerCommand: NetworkCommand>
{
    pub(crate) world: WorldInstance,
    config: ServerConfig,
    pub(crate) resources: Resources,
    pub(crate) state_update_sequence: u16,

    stcm: PhantomData<ServerToClientMessage>,
    ctsm: PhantomData<ClientToServerMessage>,
    ctsc: PhantomData<ClientToServerCommand>
}



impl<ServerToClientMessage: NetworkMessage,ClientToServerMessage: NetworkMessage,ClientToServerCommand: NetworkCommand> ServerUniverse<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    pub fn new(resources: Resources, world: WorldInstance) -> ServerUniverse<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand> {
        ServerUniverse {
            world,
            resources,
            config: ServerConfig::default(),
            state_update_sequence: 0,

            stcm: PhantomData,
            ctsm: PhantomData,
            ctsc: PhantomData
        }
    }

    pub fn tick(&mut self) {
        let resources = &mut self.resources;

        self.world.execute(resources);

        let mut command_ticker = resources.get_mut::<CommandFrameTicker>().unwrap();

        if command_ticker.try_tick() {
            let mut world_state = WorldState::new(command_ticker.command_frame());

            // Setup resources
            let mut allocator = resources.get_mut::<UidAllocator<Entity>>().unwrap();
            let components = resources.get::<RegisteredComponentsResource>().unwrap();
            let event_resource = resources.get_mut::<EventResource>().unwrap();

            // Add the serializes differences to the world state.
            add_differences_to_state(&event_resource, &components, &mut world_state);

            handle_world_events(&self.world.world, &mut allocator, &components, &event_resource, &mut world_state);

            // Sent state update to all clients.
            if !world_state.is_empty() {
                let mut postoffice = resources.get_mut::<PostOffice<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>>().unwrap();
                postoffice.broadcast(transport::ServerToClientMessage::StateUpdate(world_state));
            }
        }
    }

    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }
}

// Handle the events from above merge operation.
fn handle_world_events(
    world: &World,
    allocator: &mut UidAllocator<Entity>,
    components: &RegisteredComponentsResource,
    event_resource: &EventResource,
    world_state: &mut WorldState,
) {
    let mut event_handler = LegionEventHandler::new();

    let events = event_handler.handle(&event_resource.legion_receiver(), world, &components);

    for legion_event in events {
        debug!("{:?}", legion_event);
        match legion_event {
            LegionEvent::ComponentAdded(entity, _component_count) => {
                let identifier = allocator.get(&entity);
                world_state.add_component(identifier, ComponentData::new(0, vec![]))
            }
            LegionEvent::ComponentRemoved(entity, _component_count) => {
                let identifier = allocator.get(&entity);
                world_state.remove_component(identifier, 0);
            }
            LegionEvent::EntityRemoved(entity) => {
                let identifier = allocator.get(&entity);
                world_state.remove_entity(identifier);

                // TODO?
//                let identifier = allocator
//                    .deallocate(to_remove)
//                    .expect("Entity should be allocated.");
            }
            LegionEvent::EntityInserted(entity, _component_count) => {
                let identifier = allocator.get(&entity);

                let mut entity_components = Vec::new();

                for component in components.slice_with_uid().iter() {
                    if let Some(serialized_component) = component.1.serialize_if_exists_in_world(&world, entity).unwrap() {
                        entity_components.push(ComponentData::new(component.0, serialized_component));
                    }
                }

                world_state.insert_entity(identifier, entity_components);
            }
        }
    }
}

fn add_differences_to_state(
    event_resource: &EventResource,
    components: &RegisteredComponentsResource,
    world_state: &mut WorldState,
) {
    for event in event_resource.changed_components() {
        let register_id = components.get_uid(&event.type_id).expect("Should exist");
        world_state.change(
            event.identifier,
            ComponentData::new(*register_id, event.modified_fields),
        );
    }
}

