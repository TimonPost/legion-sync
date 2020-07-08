use std::net::TcpListener;

use legion::{
    prelude::{any, Entity, Resources, Universe, World},
    systems::{resource::Resource, schedule::Builder},
};
use log::debug;
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
use net_sync::re_exports::bincode;
use bincode::DefaultOptions;

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

    fn default_resources<C: CompressionStrategy + 'static>(
        self,
    ) -> Self {
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

        s.resources
            .insert(EventResource::new(&mut main_world, any()));

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
    pub fn with_tcp<
//        C: CompressionStrategy + 'static
    >(
        mut self,
        listener: TcpListener,
    ) -> Self {
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

            // Sent state update to all clients.
            if !world_state.is_empty() {
                let mut postoffice =
                    resources
                        .get_mut::<PostOffice<
                            ServerToClientMessage,
                            ClientToServerMessage,
                            ClientToServerCommand,
                        >>()
                        .unwrap();
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
                            let mut serializer =  &mut bincode::Serializer::new(&mut buffer, DefaultOptions::default());

                           if let Ok(_) = erased_serde::serialize(&serialize, serializer) {
                               entity_components
                                   .push(ComponentData::new(component.0, buffer));
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
        for ((entity_id, component_type), mut unchanged) in entry.1 {
            let component_id = components.get_uid(&component_type).expect("Should exist");
            let entity = allocator.get_by_val(&entity_id);

            let components = components.by_type_id();
            let registered_component = components.get(&component_type).expect("Should exist");

            let mut buffer = Vec::new();
            let mut serializer = &mut bincode::Serializer::new(&mut buffer, DefaultOptions::default());
            let mut unchanged = &mut bincode::Deserializer::from_slice(&unchanged, DefaultOptions::default());


            let difference = registered_component
                .serialize_difference_with_current(world, *entity,&mut erased_serde::Deserializer::erase(unchanged), &mut erased_serde::Serializer::erase(serializer))
                .unwrap()
                .unwrap();

            world_state.change(entity_id, ComponentData::new(*component_id, difference));
        }
    }
}
