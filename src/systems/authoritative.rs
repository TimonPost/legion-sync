use crate::resources::{PostOfficeResource, TrackResource};
use crate::systems::SystemBuilderExt;
use legion::prelude::{Entity, Schedulable, SystemBuilder};
use net_sync::state::WorldState;
use net_sync::transport::{ClientId, PostOffice};
use net_sync::uid::{Uid, UidAllocator};
use net_sync::{ClientMessage, ComponentData, ComponentId, ServerMessage};
use std::ops::DerefMut;

pub struct AuthoritativeResource {
    // client id / entity id
    entity_remove_callback: fn(ClientId, Uid) -> bool,
    // client id, entity id, components
    entity_insert_callback: fn(ClientId, Uid, &Vec<ComponentData>) -> bool,
    // client id, entity id, modification data
    component_modify_callback: fn(ClientId, Uid, &ComponentData) -> bool,
    // client id, entity id, component data
    component_add_callback: fn(ClientId, Uid, &ComponentData) -> bool,
    // client id, entity id, component id
    component_remove_callback: fn(ClientId, Uid, ComponentId) -> bool,
}

impl AuthoritativeResource {
    pub fn new() -> AuthoritativeResource {
        AuthoritativeResource {
            entity_remove_callback: |_, _| true,
            entity_insert_callback: |_, _, _| true,
            component_modify_callback: |_, _, _| true,
            component_add_callback: |_, _, _| true,
            component_remove_callback: |_, _, _| true,
        }
    }

    pub fn add_entity_remove_callback(&mut self, callback: fn(ClientId, Uid) -> bool) {
        self.entity_remove_callback = callback;
    }

    pub fn add_entity_insert_callback(
        &mut self,
        callback: fn(ClientId, Uid, &Vec<ComponentData>) -> bool,
    ) {
        self.entity_insert_callback = callback;
    }

    pub fn add_component_modify_callback(
        &mut self,
        callback: fn(ClientId, Uid, &ComponentData) -> bool,
    ) {
        self.component_modify_callback = callback;
    }

    pub fn add_component_add_callback(
        &mut self,
        callback: fn(ClientId, Uid, &ComponentData) -> bool,
    ) {
        self.component_add_callback = callback;
    }

    pub fn add_component_remove_callback(
        &mut self,
        callback: fn(ClientId, Uid, ComponentId) -> bool,
    ) {
        self.component_remove_callback = callback;
    }
}

pub fn authoritative_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("authoritative_system")
        .write_registered_components()
        .write_resource::<PostOfficeResource>()
        .read_resource::<AuthoritativeResource>()
        .write_resource::<WorldState>()
        .write_resource::<UidAllocator<Entity>>()
        .write_resource::<TrackResource>()
        .build(|command_buffer, mut world, resource, query| {
            let mut postoffice: &mut PostOffice = &mut resource.0;
            let mut authoritative: &AuthoritativeResource = &resource.1;
            let mut world_state: &mut WorldState = &mut resource.2;
            let mut allocator: &mut UidAllocator<Entity> = &mut resource.3;
            let mut track: &mut TrackResource = &mut resource.4;

            for (client_id, mut client) in postoffice.clients_mut().with_inbox().into_iter() {
                if client.postbox().empty_inbox() {
                    continue;
                }

                let mut to_remove = Vec::new();
                let mut to_acknowledge = Vec::new();

                for (i, event) in client.postbox_mut().enumerate_inbox_mut() {
                    let is_authorized = match event.deref_mut() {
                        crate::ClientMessage::EntityInserted(
                            ref mut client_entity_id,
                            components_data,
                        ) => {
                            let accepted = (authoritative.entity_insert_callback)(
                                *client_id,
                                *client_entity_id,
                                components_data,
                            );

                            let server_entity_id = *allocator.reserved(*client_entity_id).expect("Server id should be reserved by transport system on packet receive.");

                            if accepted {
                                to_acknowledge.push(ServerMessage::EntityInsertAck(*client_entity_id, server_entity_id));
                                *client_entity_id = server_entity_id;
                                event.set_acknowledged(true);
                            }

                            accepted
                            // inserted world state will be updated in clone_merge.
                        }
                        crate::ClientMessage::EntityRemoved(entity_id) => {
                            if (authoritative.entity_remove_callback)(*client_id, *entity_id) {
                                world_state.remove_entity(*entity_id);
                                true
                            } else {
                                false
                            }
                        }
                        crate::ClientMessage::ComponentModified(
                            entity_id,
                            component_data,
                        ) => {
                            (authoritative.component_modify_callback)(
                                *client_id,
                                *entity_id,
                                &component_data,
                            )
                            // changes in world state will be updated in clone_merge.
                        }
                        crate::ClientMessage::ComponentRemoved(entity_id) => {
                            if (authoritative.component_remove_callback)(
                                *client_id, *entity_id, 0,
                            ) {
                                // TODO: real component id.
                                world_state.remove_component(*entity_id, 0);
                                true
                            } else {
                                false
                            }
                        }
                        crate::ClientMessage::ComponentAdd(entity_id, component_data) => {
                            if (authoritative.component_add_callback)(
                                *client_id,
                                *entity_id,
                                &component_data,
                            ) {
                                world_state.add_component(*entity_id, component_data.clone());
                                true
                            } else {
                                false
                            }
                        }
                    };

                    if !is_authorized {
                        to_remove.push(i);
                    }
                }

                for remove in to_remove {
                    client.postbox_mut().remove_from_inbox(remove);
                }

                for message in to_acknowledge {
                    match message {
                        ServerMessage::EntityInsertAck(client_entity_id, server_entity_id) => {
                            client.postbox_mut().send(message);
                            client.add_id_mapping(client_entity_id, server_entity_id);
                            track.inserted.remove(client_entity_id as usize);
                            track.insert(server_entity_id as usize);
                        },
                        _ => {}
                    }
                }
            }
        })
}
