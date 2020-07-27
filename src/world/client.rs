use std::{marker::PhantomData, net::SocketAddr};

use itertools::Itertools;
use legion::{
    any,
    systems::{Builder, Resource},
    world::{Entity, Universe, World},
    Resources,
};

use net_sync::{
    compression::{self, lz4::Lz4},
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
    resources::{EventResource, RegisteredComponentsResource, ResourcesExt},
    systems::BuilderExt,
    tracking::re_exports::bincode,
    world::{world_instance::WorldInstance, WorldBuilder},
};
use bincode::Options;
use serde::de::DeserializeSeed;
use std::{borrow::BorrowMut, ops::DerefMut};

pub struct ClientWorldBuilder<
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
    CompressionStrategy: compression::CompressionStrategy,
> {
    resources: Resources,
    system_builder: Builder,

    cs: PhantomData<CompressionStrategy>,
    stcm: PhantomData<ServerToClientMessage>,
    ctsm: PhantomData<ClientToServerMessage>,
    ctsc: PhantomData<ClientToServerCommand>,
}

impl<
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
        CompressionStrategy: compression::CompressionStrategy,
    > Default
    for ClientWorldBuilder<
        ServerToClientMessage,
        ClientToServerMessage,
        ClientToServerCommand,
        CompressionStrategy,
    >
{
    fn default() -> Self {
        ClientWorldBuilder {
            resources: Default::default(),
            system_builder: Builder::default(),

            cs: PhantomData,
            stcm: PhantomData,
            ctsm: PhantomData,
            ctsc: PhantomData,
        }
        .default_resources::<Lz4>()
        .default_systems()
    }
}

impl<
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
        CompressionStrategy: compression::CompressionStrategy,
    > WorldBuilder
    for ClientWorldBuilder<
        ServerToClientMessage,
        ClientToServerMessage,
        ClientToServerCommand,
        CompressionStrategy,
    >
{
    type BuildResult = ClientWorld<
        ServerToClientMessage,
        ClientToServerMessage,
        ClientToServerCommand,
        CompressionStrategy,
    >;

    fn default_resources<C: compression::CompressionStrategy + 'static>(self) -> Self {
        let mut s = self;
        s.resources
            .insert_client_resources::<C, ClientToServerCommand>(C::default());
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

        s.resources.insert(EventResource::new(&mut main_world));
        s.resources.insert(universe);

        let main_world = WorldInstance::new(main_world, s.system_builder.build());

        ClientWorld::new(s.resources, main_world)
    }
}

impl<
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
        CompressionStrategy: compression::CompressionStrategy,
    >
    ClientWorldBuilder<
        ServerToClientMessage,
        ClientToServerMessage,
        ClientToServerCommand,
        CompressionStrategy,
    >
{
    pub fn with_tcp(mut self, addr: SocketAddr) -> Self {
        self.system_builder = self.system_builder.add_tcp_client_systems::<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>();
        self.resources.insert_tcp_client_resources::<ServerToClientMessage, ClientToServerMessage, ClientToServerCommand>(addr);
        self
    }
}

pub struct ClientWorld<
    ServerToClientMessage: NetworkMessage,
    ClientToServerMessage: NetworkMessage,
    ClientToServerCommand: NetworkCommand,
    CompressionStrategy: compression::CompressionStrategy,
> {
    pub(crate) world: WorldInstance,
    pub(crate) resources: Resources,
    // TODO: HACK, REMOVE!
    has_received_first_message: bool,

    c: PhantomData<CompressionStrategy>,
    stcm: PhantomData<ServerToClientMessage>,
    ctsm: PhantomData<ClientToServerMessage>,
    ctsc: PhantomData<ClientToServerCommand>,
}

impl<
        ServerToClientMessage: NetworkMessage,
        ClientToServerMessage: NetworkMessage,
        ClientToServerCommand: NetworkCommand,
        CompressionStrategy: compression::CompressionStrategy,
    >
    ClientWorld<
        ServerToClientMessage,
        ClientToServerMessage,
        ClientToServerCommand,
        CompressionStrategy,
    >
{
    pub fn new(
        resources: Resources,
        world: WorldInstance,
    ) -> ClientWorld<
        ServerToClientMessage,
        ClientToServerMessage,
        ClientToServerCommand,
        CompressionStrategy,
    > {
        ClientWorld {
            world,
            resources,
            has_received_first_message: false,

            c: PhantomData,
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
            let mut postbox = resources
                .get_mut::<PostBox<
                    transport::ServerToClientMessage<ServerToClientMessage>,
                    transport::ClientToServerMessage<ClientToServerMessage, ClientToServerCommand>,
                >>()
                .unwrap();

            let mut uid_allocator = resources.get_mut::<UidAllocator<Entity>>().unwrap();
            let registered = resources.get_mut::<RegisteredComponentsResource>().unwrap();
            let universe = resources.get_mut::<Universe>().unwrap();

            let mut client_buffer = resources
                .get_mut::<ClientCommandBuffer<ClientToServerCommand>>()
                .unwrap();
            let mut resimulation_buffer = resources
                .get_mut::<ResimulationBuffer<ClientToServerCommand>>()
                .unwrap();

            let inbox = postbox.drain_inbox(|m| match m {
                transport::ServerToClientMessage::StateUpdate(_) => true,
                transport::ServerToClientMessage::InitialStateSync(_) => true,
                _ => false,
            });

            for packet in inbox {
                match packet {
                    transport::ServerToClientMessage::StateUpdate(mut update) => {
                        adjust_simulation_speed(
                            update.command_frame_offset,
                            update.command_frame,
                            &mut command_ticker,
                        );

                        if !self.has_received_first_message {
                            self.has_received_first_message = true;
                            command_ticker.set_command_frame(update.command_frame + 3);
                        }

                        let mut state_updater = StateUpdater::new(
                            &mut uid_allocator,
                            &mut self.world.world,
                            &registered,
                            &mut update,
                            &mut client_buffer,
                            &mut resimulation_buffer,
                            command_ticker.command_frame(),
                            Lz4,
                        );

                        state_updater.apply_entity_removals();
                        state_updater.apply_entity_inserts();
                        state_updater.apply_removed_components();
                        state_updater.apply_added_components();
                        state_updater.apply_changed_components();
                    }
                    transport::ServerToClientMessage::InitialStateSync(world_state) => {
                        let registry = registered.legion_registry();
                        match registry.as_deserialize(&universe).deserialize(
                            &mut bincode::Deserializer::from_slice(
                                &world_state,
                                bincode::DefaultOptions::new()
                                    .with_fixint_encoding()
                                    .allow_trailing_bytes(),
                            ),
                        ) {
                            Ok(world) => {
                                let mutex = registered.legion_merger();
                                let mut merger = mutex.lock().unwrap();
                                let merge_result = self
                                    .world
                                    .world
                                    .clone_from(&world, &any(), merger.deref_mut())
                                    .expect("Should have merged");

                                for merge in merge_result.iter() {
                                    let entry = self.world.world.entry(*merge.1).unwrap();
                                    let uid = entry.get_component::<net_sync::uid::Uid>();
                                }

                                let serialized = serde_json::to_value(
                                    self.world
                                        .world
                                        .as_serializable(any(), registered.legion_registry()),
                                )
                                .unwrap();
                            }
                            Err(e) => {
                                panic!("{:?}", e);
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Sent commands to server
            for command in client_buffer.iter_history(1) {
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

/// Adjust the simulation speed based on the client offset with the server.
/// The client offset is calculated by subtracting the `server command frame` from the `client command frame`.
/// The result indicates the client offset from the server command frame.
/// In normal situations the client should run a few command frames ahead of the server.
/// However, the client should run not to far ahead nor to far behind.
///
/// In cases the offset is to big either negative or positive we should tune the simulation speed.
///
/// If the client command frame is to far ahead of the server command frame slow down the simulation speed.
/// If the client command frame is behind the server command frame then increase the simulation speed.
fn adjust_simulation_speed(
    offset: i32,
    server_command_frame: CommandFrame,
    current_command_frame: &mut CommandFrameTicker,
) {
    static DEFAULT_LAG: i32 = 200; // TODO: replace with real lag distance from server to client.

    if DEFAULT_LAG == offset {
        return;
    }

    let mut speed_factor = 0.;

    if offset < -30 || offset > 30 {
        speed_factor = 1 as f32;
        current_command_frame.set_command_frame(server_command_frame + DEFAULT_LAG as u32);
    } else if offset < -15 {
        speed_factor = 0.875;
    } else if offset < 0 {
        speed_factor = 0.9375;
    } else if offset > 15 {
        speed_factor = 1.125;
    } else if offset > 8 {
        speed_factor = 1.0625;
    } else {
        speed_factor = 1 as f32;
    }

    let new_rate = current_command_frame.default_simulation_speed() as f32 * speed_factor;
    current_command_frame.adjust_simulation(new_rate);
}

struct StateUpdater<
    'a,
    C: NetworkCommand,
    CompressionStrategy: compression::CompressionStrategy = Lz4,
> {
    allocator: &'a mut UidAllocator<Entity>,
    world: &'a mut World,
    registry: &'a RegisteredComponentsResource,
    update: &'a mut WorldState,
    client_buffer: &'a mut ClientCommandBuffer<C>,
    resimmulation_buffer: &'a mut ResimulationBuffer<C>,
    current_command_frame: CommandFrame,

    phantom: PhantomData<CompressionStrategy>,
}

impl<'a, C: NetworkCommand, CompressionStrategy: compression::CompressionStrategy>
    StateUpdater<'a, C, CompressionStrategy>
{
    pub fn new(
        allocator: &'a mut UidAllocator<Entity>,
        world: &'a mut World,
        registry: &'a RegisteredComponentsResource,
        update: &'a mut WorldState,
        client_buffer: &'a mut ClientCommandBuffer<C>,
        resimmulation_buffer: &'a mut ResimulationBuffer<C>,
        current_command_frame: CommandFrame,
        _compression: CompressionStrategy,
    ) -> StateUpdater<'a, C, CompressionStrategy> {
        StateUpdater {
            allocator,
            world,
            registry,
            update,
            client_buffer,
            current_command_frame,
            resimmulation_buffer,
            phantom: PhantomData,
        }
    }

    // Handle remove events, and clear mappings to prevent merge of removed entities and delete entity from worlds.
    fn apply_entity_removals(&mut self) {
        for to_remove_entity in self.update.removed.iter() {
            let entity = self.allocator.get_by_val(to_remove_entity).clone();

            self.world.remove(entity);

            self.allocator
                .deallocate(entity)
                .expect("Entity should be allocated.");
        }
    }

    fn apply_entity_inserts(&mut self) {
        let registry_by_id = self.registry.by_uid();

        for to_insert_entity in self.update.inserted.iter() {
            let entity = self.world.extend(vec![()])[0].clone();

            for component in to_insert_entity.components() {
                let component_registration = registry_by_id
                    .get(&component.component_id())
                    .expect("Component should be registered.");

                let deserializer =
                    &mut bincode::Deserializer::from_slice(component.data(), default_options());
                component_registration.add_component(
                    &mut self.world,
                    entity,
                    &mut erased_serde::Deserializer::erase(deserializer),
                );
            }

            self.allocator
                .allocate(entity, Some(to_insert_entity.entity_id()));
        }
    }

    fn apply_removed_components(&mut self) {
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
        let registry_by_id = self.registry.by_uid();

        for to_add_component in self.update.component_added.iter() {
            let entity = self.allocator.get_by_val(&to_add_component.entity_id());
            let component_data = to_add_component.component_data();
            let component_registration = registry_by_id
                .get(&component_data.component_id())
                .expect("Component should be registered.");

            let deserializer =
                &mut bincode::Deserializer::from_slice(component_data.data(), default_options());

            component_registration.add_component(
                self.world,
                *entity,
                &mut erased_serde::Deserializer::erase(deserializer),
            );
        }
    }

    fn apply_changed_components(&mut self) {
        // In this buffer the wrong client predicted state is stored.
        let mut to_resimmulate = Vec::new();

        let registry_by_type = self.registry.by_type_id();

        let command_frame = self.update.command_frame;

        // Loop trough all client-side predicted state for the current server-authorizing command frame.
        for (grouped_entity_id, group) in &self
            .client_buffer
            .iter()
            .filter(|x| x.command_frame == command_frame)
            .group_by(|x| x.entity_id)
        {
            // The buffer stores entries from latest to oldest changes therefore, the newest change is the first result.
            let group: Vec<&ClientCommandBufferEntry<C>> = group.collect();

            // Take the oldest change, which is the last entry.
            let oldest_change: &&ClientCommandBufferEntry<C> = group
                .last()
                .expect("Should have at least one element because of the filter.");

            // Take the latest change, which is the first entry.
            let latest_change: &&ClientCommandBufferEntry<C> = group
                .first()
                .expect("Should have at least one element because of the filter.");

            // Get allocated entity id.
            let entity = self.allocator.get_by_val(&grouped_entity_id);

            // Now find the component registration needed for (se/dese)rializing.
            let registration = registry_by_type
                .get(&oldest_change.component_type)
                .expect("Should exist");

            // Create deserializer of the oldest changed component.
            let oldest_change_deserializer = &mut bincode::Deserializer::from_slice(
                &oldest_change.unchanged_data,
                default_options(),
            );

            // Create deserializer for the unchanged component.
            let latest_change_deserializer = &mut bincode::Deserializer::from_slice(
                &latest_change.changed_data,
                default_options(),
            );

            // Those deserializers are used to find the difference between the the oldest unchanged and latest changed data.
            // This difference should be the same as calculated on the server.

            let mut buffer = Vec::new();

            let mut bincode = bincode::Serializer::new(&mut buffer, default_options());
            let serialized = &mut erased_serde::Serializer::erase(&mut bincode);

            match registration.serialize_difference(
                &mut erased_serde::Deserializer::erase(latest_change_deserializer),
                &mut erased_serde::Deserializer::erase(oldest_change_deserializer),
                serialized.borrow_mut(),
            ) {
                // There is a difference, lets figure out if this is the same as on the server.
                Ok(true) => {
                    // Create entry, when hashed, should also be in the server authority sate.
                    let client_state = ComponentData::new(
                        *self
                            .registry
                            .get_uid(&oldest_change.component_type)
                            .expect("Should exist"),
                        buffer,
                    );

                    // Try to find this entry in the state, if the client-perdition is not found, the calculation is wrong.
                    let client_state_match = self
                        .update
                        .changed
                        .remove(&ComponentChanged(oldest_change.entity_id, client_state));

                    if !client_state_match {
                        // There is a wrong client-perdition.

                        // Take the authoritative server state
                        let server_difference = self
                            .update
                            .changed
                            .iter()
                            .find(|val| val.0 == oldest_change.entity_id)
                            .expect("");

                        // Add the oldest state change entry to the resimmulation buffer.
                        // The client should resimmulate the world state from this state.
                        to_resimmulate.push(oldest_change.entity_id);

                        let mut bincode = bincode::Deserializer::from_slice(
                            &mut server_difference.1.data(),
                            default_options(),
                        );

                        // Create deserializer of the server-difference.
                        let mut server_difference_deserializer =
                            erased_serde::Deserializer::erase(&mut bincode);

                        // Now apply the authoritative server-differences.
                        registration.apply_changes(
                            self.world,
                            *entity,
                            &mut server_difference_deserializer,
                        )
                    }
                }
                Ok(false) => {}
                Err(e) => panic!("{:?}", e),
            }
        }

        let registry_by_uid = self.registry.by_uid();

        for change in self.update.changed.iter() {
            if let Some(registration) = registry_by_uid.get(&change.component_data().component_id())
            {
                // Get allocated entity id.
                let entity = self.allocator.get_by_val(&change.entity_id());

                let mut bincode =
                    bincode::Deserializer::from_slice(&mut change.1.data(), default_options());

                // Create deserializer of the server-difference.
                let mut server_difference_deserializer =
                    erased_serde::Deserializer::erase(&mut bincode);

                // Now apply the authoritative server-differences.
                registration.apply_changes(self.world, *entity, &mut server_difference_deserializer)
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

fn default_options() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .allow_trailing_bytes()
}
