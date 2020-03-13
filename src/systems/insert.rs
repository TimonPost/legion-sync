use crate::{
    components::UidComponent,
    resources::{PostOfficeResource, RegisteredComponentsResource, TrackResource},
};
use legion::prelude::{Entity, IntoQuery, Read, Schedulable, SystemBuilder};
use net_sync::{
    transport::{PostOffice, ReceivedPacket},
    uid::{Uid, UidAllocator},
    Event,
};

/// This automatically handles received inserted events.
/// It writes the created entities to the command buffer of this system.
pub fn insert_received_entities_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("insert_received_entities_system")
        .read_resource::<RegisteredComponentsResource>()
        .read_resource::<TrackResource>()
        .write_resource::<PostOfficeResource>()
        .write_resource::<UidAllocator<Entity>>()
        .build(|command_buffer, world, resource, query| {
            let mut postoffice: &mut PostOffice = &mut resource.2;

            let mut clients = postoffice.clients_mut();
            for (id, client) in clients.with_inbox() {
                let mut postbox = client.postbox_mut();
                let inserted_packets: Vec<Event> = postbox.drain_inbox_inserted();

                for event in inserted_packets.iter() {
                    if let Event::EntityInserted(_entity_id, records) = event {
                        let entity = command_buffer.start_entity().build();

                        let server_id = resource.3.allocate(entity, None);
                        client.add_id_mapping(*_entity_id, server_id);

                        command_buffer.add_component(entity, UidComponent::new(server_id));

                        for component in records {
                            let registered_components = resource.0.by_uid();
                            let registered_component = registered_components
                                .get(&Uid(component.register_id()))
                                .unwrap();

                            registered_component.deserialize_single(
                                world,
                                command_buffer,
                                entity.clone(),
                                &component.data(),
                            );
                        }

                        // TODO: sent ack to client
                    }
                }
            }
        })
}
