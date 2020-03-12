use crate::{
    components::UidComponent,
    event::Event,
    resources::{ReceiveBufferResource, RegisteredComponentsResource, TrackResource},
    ReceivedPacket,
};
use legion::prelude::{IntoQuery, Read, Schedulable, SystemBuilder};
use net_sync::uid::Uid;

/// This automatically handles received inserted events.
/// It writes the created entities to the command buffer of this system.
pub fn insert_received_entities_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("insert_received_entities_system")
        .write_resource::<ReceiveBufferResource>()
        .read_resource::<RegisteredComponentsResource>()
        .read_resource::<TrackResource>()
        .with_query(<Read<UidComponent>>::query())
        .build(|command_buffer, world, resource, query| {
            let inserted_packets: Vec<ReceivedPacket> = resource.0.drain_inserted();

            for packet in inserted_packets.iter() {
                if let Event::EntityInserted(_entity_id, records) = packet.event() {
                    for id in query.iter(&world) {
                        if id.id() == _entity_id.id() {
                            return;
                        }
                    }

                    let entity_builder = command_buffer.start_entity().build();

                    for component in records {
                        let registered_components = resource.1.by_uid();
                        let registered_component = registered_components
                            .get(&Uid(component.register_id()))
                            .unwrap();

                        registered_component.deserialize_single(
                            world,
                            command_buffer,
                            entity_builder.clone(),
                            &component.data(),
                        );
                    }
                }
            }
        })
}
