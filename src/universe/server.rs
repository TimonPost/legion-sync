use crate::event::{LegionEvent, LegionEventHandler};
use crate::filters::filter_fns::registered;
use crate::resources::{PostOfficeResource, RegisteredComponentsResource, RemovedEntities};
use crate::tracking::Bincode;
use crate::universe::network::WorldMappingResource;
use crate::{
    resources::{EventResource, ResourcesExt, TickResource},
    systems::SchedulerExt,
    tracking::SerializationStrategy,
    universe::{
        network::{NetworkUniverse, WorldInstance},
        UniverseBuilder,
    },
};
use legion::prelude::{Entity, SystemBuilder, World};
use legion::{
    prelude::{Resources, Universe},
    systems::{resource::Resource, schedule::Builder},
};
use log::debug;
use net_sync::compression::lz4::Lz4;
use net_sync::transport::PostOffice;
use net_sync::uid::UidAllocator;
use net_sync::{compression::CompressionStrategy, state::WorldState, ComponentData, ServerMessage};
use std::net::{SocketAddr, TcpListener};
use std::collections::HashMap;
use legion::world::{HashMapEntityReplacePolicy, HashMapCloneImplResult};

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
        .default_systems()
        .default_resources::<Bincode, Lz4>()
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

    fn register_main_systems(self, user_system_builder: fn(Builder) -> Builder) -> Self {
        let mut s = self;
        s.main_builder = user_system_builder(s.main_builder);
        s
    }

    fn register_remote_systems(self, user_system_builder: fn(Builder) -> Builder) -> Self {
        let mut s = self;
        s.remote_builder = user_system_builder(s.remote_builder);
        s
    }

    fn build(self) -> Self::BuildResult {
        let mut s = self;

        let universe = Universe::new();
        let mut main_world = universe.create_world();
        let remote_world = universe.create_world();

        s.resources
            .insert(EventResource::new(&mut main_world, registered()));

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
        listener.set_nonblocking(true);
        self.resources.insert_tcp_listener_resources(listener);
        self.remote_builder = self.remote_builder.add_tcp_server_systems::<S, C>();
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

            // Setup resources
            let mut allocator = resources.get_mut::<UidAllocator<Entity>>().unwrap();
            let mut removed_entities = resources.get_mut::<RemovedEntities>().unwrap();
            let components = resources.get::<RegisteredComponentsResource>().unwrap();
            let event_resource = resources.get_mut::<EventResource>().unwrap();
            let mut world_mappings = resources.get_mut::<WorldMappingResource>().unwrap();

            // First remove entities.
            handle_remove_entities(
                &mut removed_entities,
                &mut self.universe.main.world,
                &mut allocator,
                &mut world_mappings,
                &mut world_state,
            );

            // Serialize the entity differences, from the remote and main world.
            serialize_entity_differences(
                &mut self.universe,
                &components,
                &event_resource,
                &world_mappings,
            );

            // Add the serializes differences to the world state.
            add_differences_to_state(&event_resource, &components, &mut world_state);

            // Merge the remote world into the main world.
            let clone_impl = crate::create_copy_clone_impl();

            let mut result_mappings = HashMap::new();

            // Clone remote world into main world.
            self.universe.main.world.clone_from(
                &self.universe.remote.world,
                &clone_impl,
                &mut HashMapCloneImplResult(&mut result_mappings),
                &HashMapEntityReplacePolicy(&world_mappings.replace_mappings),
            );

            // Handle the insert, component add, component remove events caused by the merge operation.
            handle_events_from_merge(
                &self.universe.main.world,
                &mut allocator,
                &mut world_mappings,
                &components,
                &event_resource,
                &mut world_state,
                &result_mappings
            );

            world_mappings.refresh_mappings(result_mappings);

            // Sent state update to all clients.
            if !world_state.is_empty() {
                let mut postoffice = resources.get_mut::<PostOfficeResource>().unwrap();
                sent_state_update_to_clients(&mut postoffice, world_state);
            }
        }

        resources.get_mut::<TickResource>().unwrap().increment();
    }
}

// Handle the events from above merge operation.
fn handle_events_from_merge(
    main_world: &World,
    allocator: &mut UidAllocator<Entity>,
    world_mappings: &mut WorldMappingResource,
    components: &RegisteredComponentsResource,
    event_resource: &EventResource,
    world_state: &mut WorldState,
    result_mappings: &HashMap<Entity, Entity>
) {
    let mut event_handler = LegionEventHandler::new();

    let events = event_handler.handle(&event_resource.legion_receiver(), main_world, &components);

    for (remote, main) in result_mappings.iter() {
        let identifier = allocator.get(&remote);

        let mut serialized_components: Vec<ComponentData> = Vec::new();

        for component in components.slice_with_uid().iter() {
            if let Some(data) = component
                .1
                .serialize_if_exists_in_world(main_world, *main)
                .unwrap()
            {
                let record = ComponentData::new(component.0, data);
                serialized_components.push(record);
            }
        }

        world_state.insert_entity(identifier, serialized_components);
    }

    for legion_event in events {
        debug!("{:?}", legion_event);
        match legion_event {
            LegionEvent::ComponentAdded(entity, _component_count) => {
                let entity = world_mappings.remote_representative(entity).unwrap();
                let identifier = allocator.get(&entity);
                world_state.add_component(identifier, ComponentData::new(0, vec![]))
            }
            LegionEvent::ComponentRemoved(entity, _component_count) => {
                let entity = world_mappings.remote_representative(entity).unwrap();
                let identifier = allocator.get(&entity);
                world_state.remove_component(identifier, 0);
            }
            _ => {}
        }
    }
}

// Handle remove events, and clear mappings to prevent merge of removed entities and delete entity from worlds.
fn handle_remove_entities(
    removed_entities: &mut RemovedEntities,
    main_world: &mut World,
    allocator: &mut UidAllocator<Entity>,
    world_mappings: &mut WorldMappingResource,
    world_state: &mut WorldState,
) {
    for to_remove in removed_entities.drain() {
        let removed = world_mappings
            .replace_mappings
            .remove(&to_remove)
            .expect("Tried to remove entity while it didn't exist.");

        main_world.delete(removed);

        let identifier = allocator
            .deallocate(to_remove)
            .expect("Entity should be allocated.");

        world_state.remove_entity(identifier);
    }
}

// All (syncable) entities are mapped, retrieve the entity id and get its registration instance.
// Then compare and serialize the changes from the the remote and main world component.
fn serialize_entity_differences(
    universe: &mut NetworkUniverse,
    components: &RegisteredComponentsResource,
    event_resource: &EventResource,
    world_mappings: &WorldMappingResource,
) {
    let slice = components.slice_with_uid();
    for (remote, main) in world_mappings.replace_mappings.iter() {
        for (_id, comp) in slice.iter() {
            comp.serialize_if_changed(
                &universe.main.world,
                *main,
                &universe.remote.world,
                *remote,
                event_resource.notifier(),
            );
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

fn sent_state_update_to_clients(postoffice: &mut PostOfficeResource, world_state: WorldState) {
    for client in postoffice.clients_mut().iter_mut() {
        let postbox = client.1.postbox_mut();
        debug!("Sending State Update");
        postbox.send(ServerMessage::StateUpdate(world_state.clone()));
    }
}
