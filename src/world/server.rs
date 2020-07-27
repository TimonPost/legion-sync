use std::net::TcpListener;

use legion::{
    any,
    systems::{Builder, Resource},
    Entity, Resources, Universe, World,
};
use serde::export::PhantomData;

use net_sync::{
    compression::{lz4::Lz4, CompressionStrategy},
    synchronisation::{
        CommandFrameTicker, ComponentData, ModifiedComponentsBuffer, NetworkCommand,
        NetworkMessage, WorldState,
    },
    transport,
    transport::PostOffice,
    uid::UidAllocator,
};

use crate::{
    event::{LegionEvent, LegionEventHandler},
    resources::{EventResource, RegisteredComponentsResource, ResourcesExt},
    systems::BuilderExt,
    world::{world_instance::WorldInstance, WorldBuilder},
};
use bincode::Options;
use net_sync::re_exports::bincode;
use std::time::Instant;

pub struct ServerConfig {}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {}
    }
}

pub struct ServerWorldBuilder<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand> {
    resources: Resources,
    system_builder: Builder,
    config: ServerConfig,

    stcm: PhantomData<ServerToClientMessage>,
    ctsm: PhantomData<ClientToServerMessage>,
    ctsc: PhantomData<ClientToServerCommand>,
}

impl<
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
    > Default
    for ServerWorldBuilder<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    fn default() -> Self {
        ServerWorldBuilder {
            resources: Default::default(),
            system_builder: Builder::default(),
            config: ServerConfig::default(),

            stcm: PhantomData,
            ctsm: PhantomData,
            ctsc: PhantomData,
        }
        .default_systems()
        .default_resources::<Lz4>()
    }
}

impl<
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
    > WorldBuilder
    for ServerWorldBuilder<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    type BuildResult =
        ServerWorld<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>;

    fn default_resources<C: CompressionStrategy + 'static>(self) -> Self {
        let mut s = self;
        s.resources
            .insert_server_resources::<C, ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>(C::default());
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

        s.resources.insert(EventResource::new(&mut main_world));
        s.resources.insert(universe);

        let world = WorldInstance::new(main_world, s.system_builder.build());

        ServerWorld::new(s.resources, world)
    }
}

impl<
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
    > ServerWorldBuilder<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    pub fn with_tcp(mut self, listener: TcpListener) -> Self {
        listener
            .set_nonblocking(true)
            .expect("Cannot set non-blocking on TCP socket.");
        self.resources.insert_tcp_listener_resources(listener);
        self.system_builder = self.system_builder.add_tcp_server_systems::<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>();
        self
    }

    pub fn with_config(mut self, config: ServerConfig) -> Self {
        self.config = config;
        self
    }
}

pub struct ServerWorld<
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
> {
    pub(crate) world: WorldInstance,
    config: ServerConfig,
    pub(crate) resources: Resources,
    pub(crate) state_update_sequence: u16,

    pub(crate) last_tick: Instant,

    stcm: PhantomData<ServerToClientMessage>,
    ctsm: PhantomData<ClientToServerMessage>,
    ctsc: PhantomData<ClientToServerCommand>,
}

impl<
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
    > ServerWorld<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    pub fn new(
        resources: Resources,
        world: WorldInstance,
    ) -> ServerWorld<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand> {
        ServerWorld {
            world,
            resources,
            config: ServerConfig::default(),
            state_update_sequence: 0,

            last_tick: Instant::now(),

            stcm: PhantomData,
            ctsm: PhantomData,
            ctsc: PhantomData,
        }
    }

    pub fn tick(&mut self) {
        let resources = &mut self.resources;

        self.world.execute(resources);

        let mut command_ticker = resources.get_mut::<CommandFrameTicker>().unwrap();

        if command_ticker.try_tick() {
            let last_tick = self.last_tick;

            // This state packet is for the previous command frame.
            let previous_command_frame = command_ticker.command_frame() - 1;
            let mut world_state = WorldState::new(previous_command_frame);

            // Setup resources
            let mut allocator = resources.get_mut::<UidAllocator<Entity>>().unwrap();
            let components = resources.get::<RegisteredComponentsResource>().unwrap();
            let event_resource = resources.get_mut::<EventResource>().unwrap();
            let mut modified_buffer = resources.get_mut::<ModifiedComponentsBuffer>().unwrap();

            // Add the serializes differences to the world state.
            add_differences_to_state(
                &components,
                &mut world_state,
                &mut modified_buffer,
                &self.world.world,
                &allocator,
            );

            handle_world_events(
                &self.world.world,
                &mut allocator,
                &components,
                &event_resource,
                &mut world_state,
            );

            let mut postoffice =
                resources
                    .get_mut::<PostOffice<
                        ServerToClientMessage,
                        ClientToServerMessage,
                        ClientToServerCommand,
                    >>()
                    .unwrap();

            // First do an state update to each new client.
            let new_clients = postoffice
                .clients()
                .filter(|x| x.1.connected_at() > last_tick)
                .count();

            if new_clients != 0 {
                let new_clients = postoffice
                    .clients_mut()
                    .filter(|x| x.1.connected_at() > last_tick);

                let bytes = bincode::serialize(
                    &self
                        .world
                        .world
                        .as_serializable(any(), components.legion_registry()),
                )
                .unwrap();

                if bytes.len() != 0 {
                    let universe = resources.get_mut::<Universe>().unwrap();

                    let registry = components.legion_registry();

                    for (_id, client) in new_clients {
                        client.postbox_mut().send(
                            transport::ServerToClientMessage::InitialStateSync(bytes.clone()),
                        )
                    }
                }
            }

            // Sent state update to all clients.
            if !world_state.is_empty() {
                // Then broadcast the world state to all clients.
                postoffice.broadcast(transport::ServerToClientMessage::StateUpdate(world_state));
            }

            self.last_tick = Instant::now();
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
                // let identifier = allocator
                //     .deallocate(to_remove)
                //     .expect("Entity should be allocated.");
            }
            LegionEvent::EntityInserted(entity, _component_count) => {
                let identifier = allocator.get(&entity);

                let mut entity_components = Vec::new();

                for component in components.slice_with_uid().iter() {
                    component
                        .1
                        .serialize_if_exists_in_world(&world, entity, &mut |serialize| {
                            let mut buffer = Vec::new();
                            let serializer = &mut bincode::Serializer::new(
                                &mut buffer,
                                bincode::DefaultOptions::new()
                                    .with_fixint_encoding()
                                    .allow_trailing_bytes(),
                            );

                            if let Ok(_) = erased_serde::serialize(&serialize, serializer) {
                                entity_components.push(ComponentData::new(component.0, buffer));
                            }
                        });
                }

                world_state.insert_entity(identifier, entity_components);
            }
        }
    }
}

fn add_differences_to_state(
    components: &RegisteredComponentsResource,
    world_state: &mut WorldState,
    modification_buffer: &mut ModifiedComponentsBuffer,
    world: &World,
    allocator: &UidAllocator<Entity>,
) {
    let entries = modification_buffer.drain_entries();

    for entry in entries {
        for ((entity_id, component_type), unchanged) in entry.1 {
            let component_id = components.get_uid(&component_type).expect("Should exist");
            let entity = allocator.get_by_val(&entity_id);

            let components = components.by_type_id();
            let registered_component = components.get(&component_type).expect("Should exist");

            let mut buffer = Vec::new();
            let serializer = &mut bincode::Serializer::new(
                &mut buffer,
                bincode::DefaultOptions::new()
                    .with_fixint_encoding()
                    .allow_trailing_bytes(),
            );

            let unchanged = &mut bincode::Deserializer::from_slice(
                &unchanged,
                bincode::DefaultOptions::new()
                    .with_fixint_encoding()
                    .allow_trailing_bytes(),
            );

            let is_different = registered_component
                .serialize_difference_with_current(
                    world,
                    *entity,
                    &mut erased_serde::Deserializer::erase(unchanged),
                    &mut erased_serde::Serializer::erase(serializer),
                )
                .unwrap();

            if is_different {
                world_state.change(entity_id, ComponentData::new(*component_id, buffer));
            }
        }
    }
}
