use std::{marker::PhantomData, net::SocketAddr};

use itertools::Itertools;
use legion::{
    prelude::{CommandBuffer, Entity, Resources, World},
    systems::{resource::Resource, schedule::Builder},
    world::Universe,
};
use log::debug;

use net_sync::{
    compression::{lz4::Lz4, CompressionStrategy},
    serialization::{bincode::Bincode, SerializationStrategy},
    synchronisation::{
        ClientCommandBuffer, ClientCommandBufferEntry, CommandFrame, CommandFrameTicker,
        ComponentChanged, ComponentData, NetworkCommand, NetworkMessage, ResimulationBuffer,
        WorldState,
    },
    transport,
    transport::PostBox,
    uid::UidAllocator,
};

use crate::{
    filters::filter_fns::registered,
    resources::{EventResource, RegisteredComponentsResource, ResourcesExt},
    systems::BuilderExt,
    world::{world_instance::WorldInstance, WorldBuilder},
};

pub struct ClientWorldBuilder<
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
> {
    resources: Resources,
    system_builder: Builder,

    stcm: PhantomData<ServerToClientMessage>,
    ctsm: PhantomData<ClientToServerMessage>,
    ctsc: PhantomData<ClientToServerCommand>,
}

impl<
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
    > Default
    for ClientWorldBuilder<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    fn default() -> Self {
        ClientWorldBuilder {
            resources: Default::default(),
            system_builder: Builder::default(),

            stcm: PhantomData,
            ctsm: PhantomData,
            ctsc: PhantomData,
        }
        .default_resources::<Bincode, Lz4>()
        .default_systems()
    }
}

impl<
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
    > WorldBuilder
    for ClientWorldBuilder<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    type BuildResult =
        ClientWorld<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>;

    fn default_resources<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        self,
    ) -> Self {
        let mut s = self;
        s.resources
            .insert_client_resources::<S, C, ClientToServerCommand>(S::default(), C::default());
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

        ClientWorld::new(s.resources, main_world)
    }
}

impl<
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
    > ClientWorldBuilder<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
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

pub struct ClientWorld<
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
> {
    pub(crate) world: WorldInstance,
    pub(crate) resources: Resources,
    // TODO: HACK, REMOVE!
    has_received_first_message: bool,

    stcm: PhantomData<ServerToClientMessage>,
    ctsm: PhantomData<ClientToServerMessage>,
    ctsc: PhantomData<ClientToServerCommand>,
}

impl<
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
    > ClientWorld<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>
{
    pub fn new(
        resources: Resources,
        world: WorldInstance,
    ) -> ClientWorld<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand> {
        ClientWorld {
            world,
            resources,
            has_received_first_message: false,

            stcm: PhantomData,
            ctsm: PhantomData,
            ctsc: PhantomData,
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
                .get_mut::<PostBox<
                    transport::ServerToClientMessage<ServerToClientMessage>,
                    transport::ClientToServerMessage<ClientToServerMessage, ClientToServerCommand>,
                >>()
                .unwrap();

            let mut uid_allocator = resources.get_mut::<UidAllocator<Entity>>().unwrap();
            let registered = resources.get_mut::<RegisteredComponentsResource>().unwrap();
            let mut client_buffer = resources
                .get_mut::<ClientCommandBuffer<ClientToServerCommand>>()
                .unwrap();
            let mut resimulation_buffer = resources
                .get_mut::<ResimulationBuffer<ClientToServerCommand>>()
                .unwrap();

            debug!("Draining Inbox Tick");
            let inbox = postbox.drain_inbox(|m| match m {
                transport::ServerToClientMessage::StateUpdate(_) => true,
                _ => false,
            });

            for packet in inbox {
                match packet {
                    transport::ServerToClientMessage::StateUpdate(update) => {
                        if !self.has_received_first_message {
                            debug!("Initial Status Update");
                            self.has_received_first_message = true;
                            command_ticker.set_command_frame(update.command_frame + 2);
                        }

                        let mut state_updater = StateUpdater::new(
                            &mut uid_allocator,
                            &mut self.world.world,
                            &registered,
                            &update,
                            &mut client_buffer,
                            &mut resimulation_buffer,
                            command_ticker.command_frame(),
                        );

                        state_updater.apply_entity_removals();
                        state_updater.apply_entity_inserts();
                        state_updater.apply_removed_components();
                        state_updater.apply_added_components();
                        state_updater.apply_changed_components();
                    }
                    _ => {}
                }
            }

            // Sent commands to server

            for command in client_buffer.iter_history(1) {
                debug!("Send Command");

                postbox.send(transport::ClientToServerMessage::Command(
                    command.command_frame.clone(),
                    command.command.clone(),
                ));

                command.is_sent = true;
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
    registry: &'a RegisteredComponentsResource,
    update: &'a WorldState,
    client_buffer: &'a mut ClientCommandBuffer<C>,
    resimmulation_buffer: &'a mut ResimulationBuffer<C>,
    current_command_frame: CommandFrame,
}

impl<'a, C: NetworkCommand> StateUpdater<'a, C> {
    pub fn new(
        allocator: &'a mut UidAllocator<Entity>,
        world: &'a mut World,
        registry: &'a RegisteredComponentsResource,
        update: &'a WorldState,
        client_buffer: &'a mut ClientCommandBuffer<C>,
        resimmulation_buffer: &'a mut ResimulationBuffer<C>,
        current_command_frame: CommandFrame,
    ) -> StateUpdater<'a, C> {
        StateUpdater {
            allocator,
            world,
            registry,
            update,
            client_buffer,
            current_command_frame,
            resimmulation_buffer,
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

        let registry_by_id = self.registry.by_uid();

        for to_insert_entity in self.update.inserted.iter() {
            let mut buffer = CommandBuffer::new(&self.world);
            let entity = buffer.start_entity().build();

            for component in to_insert_entity.components() {
                let component_registration = registry_by_id
                    .get(&component.component_id())
                    .expect("Component should be registered.");
                component_registration.deserialize(&buffer, entity, component.data());
            }

            buffer.write(self.world);

            self.allocator
                .allocate(entity, Some(to_insert_entity.entity_id()));
        }
    }

    fn apply_removed_components(&mut self) {
        debug!("State Update; Removing components...");

        let registry_by_id = self.registry.by_uid();

        for to_remove_component in self.update.component_removed.iter() {
            let entity = self.allocator.get_by_val(&to_remove_component.entity_id());
            let component_registration = registry_by_id
                .get(&to_remove_component.component_id())
                .expect("Component should be registered.");
            component_registration.remove_component(self.world, *entity);
        }
    }

    fn apply_added_components(&mut self) {
        debug!("State Update; Adding components...");

        let registry_by_id = self.registry.by_uid();

        for to_add_component in self.update.component_added.iter() {
            let entity = self.allocator.get_by_val(&to_add_component.entity_id());
            let component_data = to_add_component.component_data();
            let component_registration = registry_by_id
                .get(&component_data.component_id())
                .expect("Component should be registered.");
            component_registration.add_component(self.world, *entity, component_data.data());
        }
    }

    fn apply_changed_components(&mut self) {
        debug!("State Update; Changing components...");

        let mut to_resimmulate = Vec::new();
        let registry_by_type = self.registry.by_type_id();

        let update_command_frame = self.update.command_frame;
        for (grouped_entity_id, group) in &self
            .client_buffer
            .iter()
            .filter(|x| x.command_frame == update_command_frame)
            .group_by(|x| x.entity_id)
        {
            let group: Vec<&ClientCommandBufferEntry<C>> = group.collect();

            // Take the first first and last change.
            // The buffer stores entries from newest to oldest changes therefore, the newest change is the first result.
            let oldest_change: &&ClientCommandBufferEntry<C> = group
                .last()
                .expect("Should have at least one element because of the filter.");
            let newest_change: &&ClientCommandBufferEntry<C> = group
                .first()
                .expect("Should have at least one element because of the filter.");

            let entity = self.allocator.get_by_val(&grouped_entity_id);
            let registration = registry_by_type
                .get(&oldest_change.component_type)
                .expect("Should exist");

            match registration
                .serialize_difference(&oldest_change.unchanged_data, &newest_change.changed_data)
            {
                Ok(Some(client_difference)) => {
                    let client_state = ComponentData::new(
                        *self
                            .registry
                            .get_uid(&oldest_change.component_type)
                            .expect("Should exist"),
                        client_difference,
                    );
                    let client_state_match = self
                        .update
                        .changed
                        .get(&ComponentChanged(oldest_change.entity_id, client_state));

                    if client_state_match.is_none() {
                        debug!("Predicted Wrong");
                        let server_difference = self
                            .update
                            .changed
                            .iter()
                            .find(|val| val.0 == oldest_change.entity_id)
                            .expect("");

                        to_resimmulate.push(oldest_change.entity_id);

                        registration.apply_changes(self.world, *entity, server_difference.1.data());
                    } else {
                        debug!("Predicted Same");
                    }
                }
                Ok(None) => debug!("None returned"),
                Err(e) => debug!("{:?}", e),
            }
        }

        if to_resimmulate.len() != 0 {
            let to_resimulate = self
                .client_buffer
                .iter_history(self.current_command_frame - self.update.command_frame)
                .filter(|val| to_resimmulate.contains(&val.entity_id))
                .map(|val| val.clone())
                .collect::<Vec<ClientCommandBufferEntry<C>>>();

            self.resimmulation_buffer.push(
                self.update.command_frame,
                self.current_command_frame,
                to_resimulate,
            );
        }
    }
}
