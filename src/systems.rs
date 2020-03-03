//! A number of systems that can be used to synchronize and trace components.

use legion::prelude::{Schedulable, SystemBuilder};

use crate::resources::{
    EventResource, ReceiveBufferResource, RegisteredComponentsResource, SentBufferResource,
};
use crate::{Event, ReceivedPacket};
use net_sync::uid::Uid;

pub mod tcp;

/// This system picks up all the changes since the last tick.
///
/// The modifications are retrieved from [EventListenerResource](LINK) and written to [TransportResource](LINK).
pub fn track_modifications_system() -> Box<dyn Schedulable> {
    let mut builder = SystemBuilder::new("track_modifications_system");

    for component in RegisteredComponentsResource::new().slice_with_uid().iter() {
        builder = component.1.add_to_system(builder);
    }

    builder
        .write_resource::<SentBufferResource>()
        .read_resource::<EventResource>()
        .read_resource::<RegisteredComponentsResource>()
        .build(|_, world, resources, _| {
            resources
                .1
                .gather_events(&mut resources.0, &resources.2, world);
        })
}

/// This automatically handles received inserted events.
/// It writes the created entities to the command buffer of this system.
pub fn insert_received_entities_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("insert_received_entities_system")
        .write_resource::<ReceiveBufferResource>()
        .read_resource::<RegisteredComponentsResource>()
        .build(|command_buffer, mut world, resource, _| {
            let inserted_packets: Vec<ReceivedPacket> = resource.0.drain_inserted();

            for packet in inserted_packets.iter() {
                if let Event::EntityInserted(_entity_id, records) = packet.event() {
                    let mut entity_builder = command_buffer.start_entity().build();

                    //                    debug!(
                    //                        "Inserted Event, create new entity {:?} with {:?} components.",
                    //                        _entity_id,
                    //                        records.len()
                    //                    );

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

                        //                        debug!(
                        //                            "Added component {:?} to entity {:?}",
                        //                            registered_component.type_name(),
                        //                            _entity_id
                        //                        );
                    }
                }
            }
        })
}
