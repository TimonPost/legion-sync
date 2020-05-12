use std::net::SocketAddr;
use std::time::Instant;

use legion::prelude::{CommandBuffer, World};
use legion::world::Universe;
use legion::{
    prelude::{Entity, Resources},
    systems::{resource::Resource, schedule::Builder},
};
use log::debug;

use net_sync::compression::lz4::Lz4;
use net_sync::synchronisation::{CommandFrameTicker, ClientCommandBuffer, ResimulationBuffer, ClientCommandBufferEntry, CommandFrame};
use net_sync::transport::{PostBox, NetworkCommand, NetworkMessage};
use net_sync::{compression::CompressionStrategy, state::WorldState, uid::{Uid, UidAllocator}, transport};
use track::serialization::bincode::Bincode;

use crate::filters::filter_fns::registered;
use crate::resources::HashmapRegistry;
use crate::resources::RegisteredComponentsResource;
use crate::{
    resources::{EventResource, ResourcesExt},
    systems::BuilderExt,
    tracking::SerializationStrategy,
    universe::{network::WorldInstance, UniverseBuilder},
};
use std::marker::PhantomData;

pub struct ClientUniverseBuilder<ServerToClientMessage: NetworkMessage,ClientToServerMessage: NetworkMessage,ClientToServerCommand: NetworkCommand>
{
    resources: Resources,
    system_builder: Builder,

    stcm: PhantomData<ServerToClientMessage>,
    ctsm: PhantomData<ClientToServerMessage>,
    ctsc: PhantomData<ClientToServerCommand>
}

impl<ServerToClientMessage: NetworkMessage,ClientToServerMessage: NetworkMessage,ClientToServerCommand: NetworkCommand>  Default for ClientUniverseBuilder<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    fn default() -> Self {
        ClientUniverseBuilder {
            resources: Default::default(),
            system_builder: Builder::default(),

            stcm: PhantomData,
            ctsm: PhantomData,
            ctsc: PhantomData
        }
        .default_resources::<Bincode, Lz4>()
        .default_systems()
    }
}

impl<ServerToClientMessage: NetworkMessage,ClientToServerMessage: NetworkMessage,ClientToServerCommand: NetworkCommand,> UniverseBuilder for ClientUniverseBuilder<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    type BuildResult = ClientUniverse<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand> ;

    fn default_resources<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        self,
    ) -> Self {
        let mut s = self;
        s.resources
            .insert_client_resources::<S,C,ClientToServerCommand>(S::default(), C::default());
        s
    }

    fn default_systems(self) -> Self {
        let mut s = self;
        s.system_builder = s.system_builder.add_client_systems();
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
            .insert(EventResource::new(&mut main_world, registered()));

        let main_world = WorldInstance::new(main_world, s.system_builder.build());

        ClientUniverse::new(s.resources, main_world)
    }
}

impl<ServerToClientMessage: NetworkMessage, ClientToServerMessage: NetworkMessage, ClientToServerCommand: NetworkCommand>  ClientUniverseBuilder<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    pub fn with_tcp<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        mut self,
        addr: SocketAddr,
    ) -> Self {
        self.system_builder = self.system_builder.add_tcp_client_systems::<S, C, ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>();
        self.resources.insert_tcp_client_resources::<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>(addr);
        self
    }
}

pub struct ClientUniverse<ServerToClientMessage: NetworkMessage,ClientToServerMessage: NetworkMessage,ClientToServerCommand: NetworkCommand>
{
    pub(crate) world: WorldInstance,
    pub(crate) resources: Resources,
    // TODO: HACK, REMOVE!
    has_received_first_message: bool,

    stcm: PhantomData<ServerToClientMessage>,
    ctsm: PhantomData<ClientToServerMessage>,
    ctsc: PhantomData<ClientToServerCommand>
}

impl<ServerToClientMessage: NetworkMessage,ClientToServerMessage: NetworkMessage,ClientToServerCommand: NetworkCommand> ClientUniverse<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{

    pub fn new(resources: Resources, world: WorldInstance) -> ClientUniverse<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand> {
        ClientUniverse {
            world,
            resources,
            has_received_first_message: false,

            stcm: PhantomData,
            ctsm: PhantomData,
            ctsc: PhantomData
        }
    }

    pub fn world(&mut self) -> &mut World {
        &mut self.world.world
    }

    pub fn tick(&mut self) {
        let resources = &mut self.resources;

        self.world.execute(resources);

        let mut command_ticker = resources.get_mut::<CommandFrameTicker>().unwrap();

        if command_ticker.try_tick() {
            debug!("Universe Tick");
            let mut postbox = resources
                .get_mut::<PostBox<transport::ServerToClientMessage<ServerToClientMessage>, transport::ClientToServerMessage<ClientToServerMessage, ClientToServerCommand>>>()
                .unwrap();

            let mut uid_allocator = resources.get_mut::<UidAllocator<Entity>>().unwrap();
            let registered = resources.get_mut::<RegisteredComponentsResource>().unwrap();
            let mut client_buffer = resources.get_mut::<ClientCommandBuffer<ClientToServerCommand>>().unwrap();
            let mut resimulation_buffer = resources.get_mut::<ResimulationBuffer<ClientToServerCommand>>().unwrap();

            debug!("Draining Inbox Tick");
            let inbox = postbox.drain_inbox(|m| match m {
                transport::ServerToClientMessage::StateUpdate(_) => true,
                _ => false,
            });

            let registered_by_uuid = registered.by_uid();

            for packet in inbox {
                match packet {
                    transport::ServerToClientMessage::StateUpdate(update) => {
                        if !self.has_received_first_message {
                            self.has_received_first_message = true;
                            command_ticker.set_command_frame(update.command_frame + 4);
                        }

                        let mut state_updater = StateUpdater::new(
                            &mut uid_allocator,
                            &mut self.world.world,
                            &registered_by_uuid,
                            &update,
                            &mut client_buffer,
                            &mut resimulation_buffer,
                            command_ticker.command_frame()
                        );

                        state_updater.apply_entity_removals();
                        state_updater.apply_entity_inserts();
                        state_updater.apply_removed_components();
                        state_updater.apply_added_components();
                        state_updater.apply_changed_components();

                        println!("{}", update.command_frame_offset);
                    }
                    _ => {}
                }
            }

            // Sent commands to server

            if let Some(command) = client_buffer
                .iterate_frames(command_ticker.command_frame())
                .last()
                .cloned()
            {
                postbox.send(transport::ClientToServerMessage::Command(
                    command.command_frame,
                    command.command,
                ))
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

struct StateUpdater<'a, C: NetworkCommand> {
    allocator: &'a mut UidAllocator<Entity>,
    world: &'a mut World,
    registration_by_uuid: &'a HashmapRegistry<'a, Uid>,
    update: &'a WorldState,
    client_buffer: &'a mut ClientCommandBuffer<C>,
    resimmulation_buffer: &'a mut ResimulationBuffer<C>,
    current_command_frame: CommandFrame,
}

impl<'a, C: NetworkCommand> StateUpdater<'a, C> {
    pub fn new(
        allocator: &'a mut UidAllocator<Entity>,
        world: &'a mut World,
        registration_by_uuid: &'a HashmapRegistry<'a, Uid>,
        update: &'a WorldState,
        client_buffer: &'a mut ClientCommandBuffer<C>,
        resimmulation_buffer: &'a mut ResimulationBuffer<C>,
        current_command_frame: CommandFrame,
    ) -> StateUpdater<'a, C> {
        StateUpdater {
            allocator,
            world,
            registration_by_uuid,
            update,
            client_buffer,
            current_command_frame: current_command_frame,
            resimmulation_buffer
        }
    }

    // Handle remove events, and clear mappings to prevent merge of removed entities and delete entity from worlds.
    fn apply_entity_removals(&mut self) {
        debug!("State Update; Removing Entities...");
        for to_remove_entity in self.update.removed.iter() {
            let entity = self.allocator.get_by_val(to_remove_entity).clone();

            self.world.delete(entity);

            self.allocator
                .deallocate(entity)
                .expect("Entity should be allocated.");
        }
    }

    fn apply_entity_inserts(&mut self) {
        debug!("State Update; Inserting entities...");
        for to_insert_entity in self.update.inserted.iter() {
            let mut buffer = CommandBuffer::new(&self.world);
            let entity = buffer.start_entity().build();

            for component in to_insert_entity.components() {
                let component_registration = self
                    .registration_by_uuid
                    .get(&component.component_id())
                    .expect("Component should be registered.");
                component_registration.deserialize(&buffer, entity, component.data());
            }

            buffer.write(self.world);

            self.allocator.allocate(entity, Some(to_insert_entity.entity_id()));
        }
    }

    fn apply_removed_components(&mut self) {
        debug!("State Update; Removing components...");
        for to_remove_component in self.update.component_removed.iter() {
            let entity = self.allocator.get_by_val(&to_remove_component.entity_id());
            let component_registration = self
                .registration_by_uuid
                .get(&to_remove_component.component_id())
                .expect("Component should be registered.");
            component_registration.remove_component(self.world, *entity);
        }
    }

    fn apply_added_components(&mut self) {
        debug!("State Update; Adding components...");
        for to_add_component in self.update.component_added.iter() {
            let entity = self.allocator.get_by_val(&to_add_component.entity_id());
            let component_data = to_add_component.component_data();
            let component_registration = self
                .registration_by_uuid
                .get(&component_data.component_id())
                .expect("Component should be registered.");
            component_registration.add_component(self.world, *entity, component_data.data());
        }
    }

    fn apply_changed_components(&mut self) {
        debug!("State Update; Changing components...");

        for changed in self.update.changed.iter() {
            let entity = self.allocator.get_by_val(&changed.entity_id());
            let server_component_changes = changed.component_data();

            let frame = self.client_buffer.frame(self.update.command_frame);

            if let Some(frame) = frame {
                let data = changed.component_data().data();

                if data == &frame.changed_data {
                    println!("Predicted Same")
                }else {
                    println!("Predicted Wrong");

                    let to_resimulate = self.client_buffer.iterate_frames(self.current_command_frame - self.update.command_frame)
                        .cloned()
                        .collect::<Vec<ClientCommandBufferEntry<C>>>();

                    self.resimmulation_buffer.push(self.update.command_frame, self.update.command_frame, data.to_owned(), to_resimulate);
                }
            }

            let component_registration = self
                .registration_by_uuid
                .get(&server_component_changes.component_id())
                .expect("Component should be registered.");

            component_registration.apply_changes(
                self.world,
                *entity,
                server_component_changes.data(),
            );
            //            component_registration.apply_changes(self.world, *entity, client_component_changes.data());
        }
    }
}
