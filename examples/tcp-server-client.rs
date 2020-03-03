use legion::{filter::filter_fns::any, prelude::*};
use legion_sync::{
    components::UidComponent,
    filters::filter_fns::{all, modified, removed},
    resources::{
        tcp::{TcpClientResource, TcpListenerResource},
        BufferResource, EventResource, Packer, ReceiveBufferResource, RegisteredComponentsResource,
        SentBufferResource, TrackResource,
    },
    systems::{
        tcp::{tcp_connection_listener, tcp_receive_system, tcp_sent_system},
        track_modifications_system,
    },
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
use track::{preclude::*, Apply};

#[track]
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

fn main() {
    start_server();
    start_client().join();
}

/// Start and initialize the legion server logica.
fn start_server() {
    thread::spawn(|| {
        let universe = Universe::new();
        let mut world = universe.create_world();

        // Create TCP listener on port local host port 1999.
        let listener = TcpListener::bind("127.0.0.1:1119".parse::<SocketAddr>().unwrap()).unwrap();
        listener.set_nonblocking(true);

        // Insert the needed resources for receiving component synchronizations.
        let mut resources = Resources::default();
        resources.insert(TrackResource::new());
        resources.insert(ReceiveBufferResource::default());
        resources.insert(TcpListenerResource::new(Some(listener)));
        resources.insert(Packer::<Bincode, Lz4>::default());
        resources.insert(BufferResource::from_capacity(1500));

        let mut schedule = initialize_server_systems();

        loop {
            schedule.execute(&mut world, &mut resources);

            thread::sleep(Duration::from_millis(10));
        }
    });
}

/// Start and initialize the legion client logica.
fn start_client() -> JoinHandle<()> {
    thread::spawn(|| {
        let universe = Universe::new();
        let mut world = universe.create_world();

        let tcp_client = TcpClientResource::new("127.0.0.1:1119".parse().unwrap()).unwrap();
        let mut event_resource = EventResource::new();
        event_resource.subscribe_to_world(&mut world, any());

        let mut resources = Resources::default();
        resources.insert(tcp_client);
        resources.insert(event_resource);
        resources.insert(SentBufferResource::new());
        resources.insert(Packer::<Bincode, Lz4>::default());
        resources.insert(RegisteredComponentsResource::new());

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
            thread::sleep(Duration::from_millis(10));
        }
    })
}

/// Initializes the systems needed for TCP network communication receiving entity updates.
fn initialize_server_systems() -> Schedule {
    Schedule::builder()
        .add_system(tcp_connection_listener())
        .add_system(tcp_receive_system::<Bincode, Lz4>())
        .add_system(receive_system::<Bincode, Lz4>())
        .flush()
        .build()
}

/// Initializes the systems needed for TCP network communication and entity client sync with server.
fn initialize_client_systems() -> Schedule {
    Schedule::builder()
        .add_system(track_modifications_system())
        .add_system(tcp_sent_system::<Bincode, Lz4>())
        .add_system(make_modification_system())
        .flush()
        .build()
}

/// Basic example of reading received entity synchronization data.
pub fn receive_system<S: SerializationStrategy + 'static, C: CompressionStrategy + 'static>(
) -> Box<dyn Schedulable> {
    SystemBuilder::new("read_received_system")
        .write_resource::<ReceiveBufferResource>()
        .write_resource::<TrackResource>()
        .read_resource::<Packer<S, C>>()
        .with_query(<(legion::prelude::Write<Position>, Read<UidComponent>)>::query())
        .build(|command_buffer, mut world, resources, query| {
            /// 1) Filter and query modified components and retrieve the packets for those.
            let filter = query.clone().filter(modified(&resources.1));
            let modified_packets: Vec<ReceivedPacket> = resources.0.drain_modified();

            for (mut pos, identifier) in query.iter_mut(&mut world) {
                for packet in modified_packets.iter() {
                    if identifier.uid() == packet.identifier() {
                        if let legion_sync::Event::ComponentModified(data) = packet.event() {
                            Apply::apply_to(&mut *pos, &data, Bincode);
                            println!(".. Modified entity {:?}", packet.identifier());
                            break;
                        }
                    }
                }
            }

            /// 2) Filter and query removed components and retrieve the packets for those.
            let filter = query.clone().filter(removed(&resources.1));
            let removed_packets: Vec<ReceivedPacket> = resources.0.drain_removed();

            for (pos, identifier) in filter.iter_mut(&mut world) {
                for packet in removed_packets.iter() {
                    println!("X Removed entity {:?}", packet.identifier());
                }
            }

            /// 3) Filter and query inserted components and retrieve the packets for those.
            let inserted_packets: Vec<ReceivedPacket> = resources.0.drain_inserted();

            for packet in inserted_packets.iter() {
                if let legion_sync::Event::EntityInserted(data) = packet.event() {
                    let position = resources
                        .2
                        .serialization()
                        .deserialize::<Position>(data[0].data())
                        .unwrap();

                    command_buffer.insert(
                        (),
                        vec![(position, UidComponent::new(packet.identifier().clone()))],
                    );
                }

                println!("-> Inserted entity {:?}", packet.identifier());
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
