use legion::{filter::filter_fns::any, prelude::*, systems::schedule::Builder};
use legion_sync::{
    components::UidComponent,
    filters::filter_fns::{all, modified, removed},
    resources::{
        tcp::{TcpClientResource, TcpListenerResource},
        BufferResource, EventResource, Packer, PostOfficeResource, ReceiveBufferResource,
        RegisteredComponentsResource, ResourcesExt, TrackResource,
    },
    systems::{
        insert_received_entities_system,
        tcp::{tcp_client_sent_system, tcp_connection_listener, tcp_server_receive_system},
        track_modifications_system, SchedulerExt,
    },
    tracking::*,
    ReceivedPacket,
};
use net_sync::{
    compression::{lz4::Lz4, CompressionStrategy},
    uid::UidAllocator,
};
use std::{
    net::{SocketAddr, TcpListener},
    thread,
    thread::JoinHandle,
    time::Duration,
};

#[sync]
#[derive(Debug)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

impl Position {
    pub fn set(&mut self, pos: (u16, u16)) {
        self.x = pos.0;
        self.y = pos.1;
    }
}

impl Default for Position {
    fn default() -> Self {
        Position { x: 0, y: 0 }
    }
}

fn main() {
    start_server();
    start_client().join();
}

/// Start and initialize the legion server logic.
fn start_server() {
    thread::spawn(|| {
        let universe = Universe::new();
        let mut world = universe.create_world();

        // Create TCP listener on port local host port 1999.
        let listener = TcpListener::bind("127.0.0.1:1119".parse::<SocketAddr>().unwrap()).unwrap();
        listener.set_nonblocking(true);

        // Insert the needed resources for receiving component synchronizations.
        let mut resources = Resources::default();
        resources.insert(TcpListenerResource::new(Some(listener)));
        resources.insert_server_resources(Bincode, Lz4);

        let mut schedule = initialize_server_systems();

        loop {
            schedule.execute(&mut world, &mut resources);

            thread::sleep(Duration::from_millis(0));
        }
    });
}

/// Start and initialize the legion client logic.
fn start_client() -> JoinHandle<()> {
    thread::spawn(|| {
        let universe = Universe::new();
        let mut world = universe.create_world();

        let tcp_client = TcpClientResource::new("127.0.0.1:1119".parse().unwrap()).unwrap();

        let mut resources = Resources::default();
        resources.insert(tcp_client);
        resources.insert_client_resources(Bincode, Lz4);
        resources.insert(EventResource::new(&mut world));

        // Custom resource that we need in this example.
        let mut allocator = UidAllocator::new();
        initial_data(&mut world, &mut allocator);
        resources.insert(ExampleResource {
            counter: 0,
            uid_allocator: allocator,
        });

        let mut schedule = initialize_client_systems();

        loop {
            schedule.execute(&mut world, &mut resources);
            thread::sleep(Duration::from_millis(20));
        }
    })
}

/// Initializes the systems needed for TCP network communication receiving entity updates.
fn initialize_server_systems() -> Schedule {
    Schedule::builder()
        .add_tcp_server_systems::<Bincode, Lz4>()
        .add_server_systems()
        .add_system(apply_position_modifications_system())
        .add_system(remove_entities_system())
        .flush()
        .build()
}

/// Initializes the systems needed for TCP network communication and entity client sync with server.
fn initialize_client_systems() -> Schedule {
    Schedule::builder()
        .add_client_systems()
        .add_system(tcp_client_sent_system::<Bincode, Lz4>())
        .add_system(make_modification_system())
        .flush()
        .build()
}

pub fn remove_entities_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("read_received_system")
        .write_resource::<ReceiveBufferResource>()
        .write_resource::<TrackResource>()
        .with_query(<(Read<UidComponent>, legion::prelude::Write<Position>)>::query())
        .build(|command_buffer, mut world, resource, query| {
            let filter = query.clone().filter(removed(&resource.1));
            let removed_packets: Vec<ReceivedPacket> = resource.0.drain_removed();

            for (identifier, pos) in filter.iter_mut(&mut world) {
                for packet in removed_packets.iter() {
                    println!("Removed {:?}", *pos);
                }
            }
        })
}

pub fn apply_position_modifications_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("apply_modifications_system")
        .write_resource::<ReceiveBufferResource>()
        .write_resource::<TrackResource>()
        .read_resource::<RegisteredComponentsResource>()
        .with_query(<(legion::prelude::Write<Position>, Read<UidComponent>)>::query())
        .build(|command_buffer, mut world, resource, query| {
            let by_uid = resource.2.slice_with_uid();

            let filter = query.clone().filter(modified(&resource.1));

            for (mut pos, identifier) in filter.iter_mut(&mut world) {
                let uid = resource
                    .2
                    .get_uid(&pos.id().0)
                    .expect("Type should be registered, make sure to implement `sync` attribute.");

                let modified_packets: Vec<ReceivedPacket> =
                    resource.0.drain_modified(identifier.uid(), *uid);

                for packet in modified_packets.iter() {
                    if let legion_sync::ClientMessage::ComponentModified(_entity_id, record) =
                        packet.event()
                    {
                        println!("Modified {:?} from {:?}", *pos, *identifier);
                        Apply::apply_to(&mut *pos, &record.data(), Bincode);
                    }
                }
            }
        })
}

/// Inserts, modifies, removes components for demonstration purposes.
pub fn make_modification_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("move player")
        .write_resource::<ExampleResource>()
        .read_resource::<EventResource>()
        .with_query(<(legion::prelude::Write<Position>, Read<UidComponent>)>::query())
        .build(|command, mut world, resources, query| {
            // Every 5th tick modify all entities.
            if resources.0.counter % 5 == 0 {
                for (mut pos, identifier) in query.iter_mut(&mut world) {
                    let mut pos = pos.track(resources.1.notifier(), identifier.uid());
                    let new_pos = (pos.x + 1, pos.x + 1);
                    pos.set(new_pos);
                }
            }

            // Every 20th tick insert new component.
            if resources.0.counter % 20 == 0 {
                command.insert((), test_components(&mut resources.0.uid_allocator));
            }

            // Every 100th tick remove all entities.
            if resources.0.counter % 100 == 0 {
                for (entity, (pos, identifier)) in query.iter_entities_mut(&mut world) {
                    // TODO: fix remove component.
                }

                resources.0.counter = 1;
            }

            resources.0.counter += 1;
        })
}

/// Inserts initial world data.
fn initial_data(world: &mut World, allocator: &mut UidAllocator) {
    world.insert((), test_components(allocator));
}

/// Returns some test components which can be added to an entity.
fn test_components(allocator: &mut UidAllocator) -> Vec<(Position, UidComponent)> {
    vec![(
        Position { x: 10, y: 10 },
        UidComponent::new(allocator.allocate(None)),
    )]
}

/// Resource used in this example for storing some global state.
struct ExampleResource {
    // Variable used to do some flexible simulation things.
    pub counter: u32,
    /// Allocator used to allocate entity ids.
    pub uid_allocator: UidAllocator,
}
