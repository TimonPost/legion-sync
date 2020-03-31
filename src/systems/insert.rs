use crate::{
    components::UidComponent,
    resources::{PostOfficeResource, RegisteredComponentsResource, TrackResource},
};
use legion::prelude::{Entity, Schedulable, SystemBuilder};
use net_sync::{transport::PostOffice, uid::UidAllocator, ClientMessage};

/// This automatically handles received inserted events.
/// It writes the created entities to the command buffer of this system.
pub fn insert_received_entities_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("insert_received_entities_system")
        .read_resource::<RegisteredComponentsResource>()
        .write_resource::<TrackResource>()
        .write_resource::<PostOfficeResource>()
        .write_resource::<UidAllocator<Entity>>()
        .build(|command_buffer, _, resource, _| {
            let registered: &RegisteredComponentsResource = &resource.0;
            let track: &mut TrackResource = &mut resource.1;
            let postoffice: &mut PostOffice = &mut resource.2;
            let allocator: &mut UidAllocator<Entity> = &mut resource.3;

            let clients = postoffice.clients_mut();
            for (_id, client) in clients.with_inbox() {
                let postbox = client.postbox_mut();
                let inserted_packets: Vec<ClientMessage> = postbox.drain_inbox(|e| match e {
                    ClientMessage::EntityInserted(_, _) => true,
                    _ => false,
                });

                for event in inserted_packets.iter() {
                    if let ClientMessage::EntityInserted(client_id, records) = event {
                        let entity = command_buffer.start_entity().build();

                        let server_id = allocator.allocate(entity, Some(*client_id));

                        command_buffer.add_component(entity, UidComponent::new(server_id));

                        for component in records {
                            let registered_components = registered.by_uid();
                            let registered_component = registered_components
                                .get(&component.component_id())
                                .unwrap();

                            registered_component.deserialize(
                                command_buffer,
                                entity.clone(),
                                component.data(),
                            );
                        }
                    }
                }
            }
        })
}
