use crate::components::UidComponent;
use crate::event::{LegionEvent, LegionEventHandler};
use crate::filters::TrackResourceFilter;
use crate::resources::TrackResource;
use crate::universe::network::WorldMappingResource;
use crate::{
    resources::{EventResource, PostBoxResource, RegisteredComponentsResource},
    systems::SystemBuilderExt,
    ClientMessage,
};
use legion::prelude::{Entity, Schedulable, SystemBuilder};
use log::debug;
use net_sync::transport::PostBox;
use net_sync::uid::UidAllocator;
use net_sync::{ComponentData, ServerMessage};
use std::any::TypeId;

/// This system picks up all the changes since the last tick.
///
/// The modifications are retrieved from [EventListenerResource](LINK) and written to [TransportResource](LINK).
pub fn track_modifications_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("track_modifications_system")
        .read_registered_components()
        .write_resource::<PostBoxResource>()
        .read_resource::<EventResource>()
        .read_resource::<RegisteredComponentsResource>()
        .write_resource::<UidAllocator<Entity>>()
        .write_resource::<TrackResource>()
        .build(|_, world, resources, _| {
            let transport: &mut PostBox<ServerMessage, ClientMessage> = &mut resources.0;
            let event: &EventResource = &resources.1;
            let components: &RegisteredComponentsResource = &mut resources.2;
            let uid_allocator: &mut UidAllocator<Entity> = &mut resources.3;
            let track_resource: &mut TrackResource = &mut resources.4;

            let mut event_handler = LegionEventHandler::new();
            let events = event_handler.handle(&event.legion_events_rx, world, components);

            for legion_event in events {
                debug!("{:?}", legion_event);

                match legion_event {
                    LegionEvent::EntityInserted(entity, _component_count) => {
                        let identifier = uid_allocator.get(&entity);

                        // If the identifier is present in any, it means that we should skip the event,
                        // because it was caused by the merge world operation of the library.
                        if track_resource.remove_if_any_contains(identifier as usize) {
                            continue;
                        }

                        let mut serialized_components: Vec<ComponentData> = Vec::new();

                        for component in components.slice_with_uid().iter() {
                            // do not sent uid components, as the server will append it's onw.
                            if component.1.ty() == TypeId::of::<UidComponent>() {
                                continue;
                            }

                            if let Some(data) = component
                                .1
                                .serialize_if_exists_in_subworld(world, entity)
                                .unwrap()
                            {
                                let record = ComponentData::new(component.0, data);
                                serialized_components.push(record);
                            }
                        }

                        transport.send_immediate(net_sync::ClientMessage::EntityInserted(
                            identifier,
                            serialized_components,
                        ));
                    }
                    LegionEvent::EntityRemoved(entity) => {
                        let identifier = uid_allocator.get(&entity);

                        // If the identifier is present in track recourse, it means that we should skip the event,
                        // because it was caused by remote changes from the server.
                        if track_resource.remove_if_any_contains(identifier as usize) {
                            uid_allocator.deallocate(entity);
                            continue;
                        }

                        transport
                            .send_immediate(net_sync::ClientMessage::EntityRemoved(identifier));
                    }
                    LegionEvent::ComponentAdded(entity, _component_count) => {
                        let identifier = uid_allocator.get(&entity);

                        // If the identifier is present in any, it means that we should skip the event,
                        // because it was caused by the merge world operation of the library.
                        if track_resource.remove_if_any_contains(identifier as usize) {
                            continue;
                        }

                        transport.send_immediate(net_sync::ClientMessage::ComponentAdd(
                            identifier,
                            ComponentData::new(0, vec![]),
                        ));
                    }
                    LegionEvent::ComponentRemoved(entity, _component_count) => {
                        let identifier = uid_allocator.get(&entity);

                        // If the identifier is present in any, it means that we should skip the event,
                        // because it was caused by the merge world operation of the library.
                        if track_resource.remove_if_any_contains(identifier as usize) {
                            continue;
                        }

                        transport
                            .send_immediate(net_sync::ClientMessage::ComponentRemoved(identifier));
                    }
                }
            }

            for modified in event.changed_components() {
                debug!("Modified {:?}", modified);

                let uid = components
                    .get_uid(&modified.type_id)
                    .expect("Component type not registered");

                transport.send(net_sync::ClientMessage::ComponentModified(
                    modified.identifier,
                    ComponentData::new(*uid, modified.modified_fields),
                ));
            }
        })
}
