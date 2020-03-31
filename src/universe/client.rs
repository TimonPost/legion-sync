use crate::filters::filter_fns::registered;
use crate::register::{ComponentRegistration, HashmapRegistery};
use crate::resources::HashmapRegistry;
use crate::resources::{PostBoxResource, RegisteredComponentsResource, TrackResource};
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
use legion::prelude::{CommandBuffer, World};
use legion::world::{HashMapCloneImplResult, HashMapEntityReplacePolicy, Universe};
use legion::{
    prelude::{Entity, Resources},
    systems::{resource::Resource, schedule::Builder},
};
use log::debug;
use net_sync::compression::lz4::Lz4;
use net_sync::state::ComponentRemoved;
use net_sync::transport::PostBox;
use net_sync::{
    compression::CompressionStrategy,
    state::WorldState,
    uid::{Uid, UidAllocator},
    ClientMessage, ServerMessage,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use track::serialization::bincode::Bincode;

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
        .default_resources::<Bincode, Lz4>()
        .default_systems()
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
        s
    }

    fn default_systems(self) -> Self {
        let mut s = self;
        s.main_builder = s.main_builder.add_client_systems();
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
        s.remote_builder = user_system_builder(Builder::default());
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

        ClientUniverse::new(s.resources, main_world)
    }
}

impl ClientUniverseBuilder {
    pub fn with_tcp<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
        mut self,
        addr: SocketAddr,
    ) -> Self {
        self.main_builder = self.main_builder.add_tcp_client_systems::<S, C>();
        self.resources.insert_tcp_client_resources(addr);
        self
    }
}

pub struct ClientUniverse {
    pub(crate) world: WorldInstance,
    pub(crate) resources: Resources,
}

impl ClientUniverse {
    pub fn new(resources: Resources, world: WorldInstance) -> ClientUniverse {
        ClientUniverse { world, resources }
    }

    pub fn world(&mut self) -> &mut World {
        &mut self.world.world
    }

    pub fn tick(&mut self) {
        let resources = &mut self.resources;

        self.world.execute(resources);

        let tick = resources.get_mut::<TickResource>().unwrap().tick();

        if tick % 10 == 0 {
            let mut postbox = resources.get_mut::<PostBoxResource>().unwrap();
            let mut uid_allocator = resources.get_mut::<UidAllocator<Entity>>().unwrap();
            let mut registered = resources.get_mut::<RegisteredComponentsResource>().unwrap();
            let mut track_resource = resources.get_mut::<TrackResource>().unwrap();

            let inbox = postbox.drain_inbox(|m| match m {
                ServerMessage::StateUpdate(_) => true,
                ServerMessage::EntityInsertAck(_, _) => true,
                _ => false,
            });

            let registered_by_uuid = registered.by_uid();

            for packet in inbox {
                match packet {
                    ServerMessage::StateUpdate(update) => {
                        let mut state_updater = StateUpdater::new(
                            &mut uid_allocator,
                            &mut self.world.world,
                            &registered_by_uuid,
                            &update,
                            &mut track_resource,
                        );
                        state_updater.apply_entity_removals();
                        state_updater.apply_entity_inserts();
                        state_updater.apply_removed_components();
                        state_updater.apply_added_components();
                        state_updater.apply_changed_components();
                    }
                    ServerMessage::EntityInsertAck(client_entity_id, server_entity_id) => {
                        uid_allocator.replace_val(client_entity_id, server_entity_id);
                    }
                }
            }
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

struct StateUpdater<'a> {
    allocator: &'a mut UidAllocator<Entity>,
    world: &'a mut World,
    registration_by_uuid: &'a HashmapRegistry<'a, Uid>,
    update: &'a WorldState,
    track_resource: &'a mut TrackResource,
}

impl<'a> StateUpdater<'a> {
    pub fn new(
        allocator: &'a mut UidAllocator<Entity>,
        world: &'a mut World,
        registration_by_uuid: &'a HashmapRegistry<'a, Uid>,
        update: &'a WorldState,
        track_resource: &'a mut TrackResource,
    ) -> StateUpdater<'a> {
        StateUpdater {
            allocator,
            world,
            registration_by_uuid,
            update,
            track_resource,
        }
    }

    // Handle remove events, and clear mappings to prevent merge of removed entities and delete entity from worlds.
    fn apply_entity_removals(&mut self) {
        debug!("State Update; Removing Entities...");
        for entity_id in self.update.removed.iter() {
            let entity = self.allocator.get_by_val(entity_id).clone();

            self.world.delete(entity);

            self.allocator
                .deallocate(entity)
                .expect("Entity should be allocated.");

            self.track_resource.remove(*entity_id as usize);
        }
    }

    fn apply_entity_inserts(&mut self) {
        debug!("State Update; Inserting entities...");
        for to_insert in self.update.inserted.iter() {
            let entity = self.allocator.get_by_val(&to_insert.entity_id()).clone();
            self.world.delete(entity);

            let mut buffer = CommandBuffer::new(&self.world);
            let entity = buffer.start_entity().build();

            for component in to_insert.components() {
                let component_registration = self
                    .registration_by_uuid
                    .get(&component.component_id())
                    .expect("Component should be registered.");
                component_registration.deserialize(&buffer, entity, component.data());
            }

            buffer.write(self.world);

            self.allocator.allocate(entity, Some(to_insert.entity_id()));

            self.track_resource.insert(to_insert.entity_id() as usize);
            self.track_resource.remove(to_insert.entity_id() as usize);
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

            self.track_resource
                .component_unset(to_remove_component.entity_id() as usize);
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

            self.track_resource
                .component_add(to_add_component.entity_id() as usize);
        }
    }

    fn apply_changed_components(&mut self) {
        debug!("State Update; Changing components...");
        for changed in self.update.changed.iter() {
            let entity = self.allocator.get_by_val(&changed.entity_id());
            let component_data = changed.component_data();
            let component_registration = self
                .registration_by_uuid
                .get(&component_data.component_id())
                .expect("Component should be registered.");
            component_registration.apply_changes(self.world, *entity, component_data.data());
        }
    }
}
