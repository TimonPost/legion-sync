// use crate::register::ComponentRegistration;
// use legion::{world::EntityAllocator, prelude::*, storage::{
//     ArchetypeDescription, ComponentMeta, ComponentResourceSet, ComponentTypeId, TagMeta,
//     TagStorage, TagTypeId,
// }};
// use net_sync::uid;
// use net_sync::uid::{Uid, UidAllocator};
// use serde::{
//     de::{self, DeserializeSeed, IgnoredAny, Visitor},
//     Deserialize, Deserializer, Serialize, Serializer,
// };
// use std::any::Any;
// use std::iter::FromIterator;
// use std::{any::TypeId, cell::RefCell, collections::HashMap, marker::PhantomData, ptr::NonNull};
// use type_uuid::TypeUuid;
//
// use crate::resources::{HashmapRegistry, RegisteredComponentsResource};
// use serde::de::Error;
//
// use std::fs::read_to_string;
//
// pub struct ComponentDeserializer<'de, T: Deserialize<'de>> {
//     ptr: *mut T,
//     _marker: PhantomData<&'de T>,
// }
//
// impl<'de, T: Deserialize<'de> + 'static> DeserializeSeed<'de> for ComponentDeserializer<'de, T> {
//     type Value = ();
//     fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         let value = <T as Deserialize<'de>>::deserialize(deserializer)?;
//         unsafe {
//             std::ptr::write(self.ptr, value);
//         }
//         Ok(())
//     }
// }
//
// pub struct ComponentSeqDeserializer<'a, T> {
//     pub (crate) get_next_storage_fn: &'a mut dyn FnMut() -> Option<(NonNull<u8>, usize)>,
//     pub (crate) _marker: PhantomData<T>,
// }
//
// impl<'de, 'a, T: for<'b> Deserialize<'b> + 'static> DeserializeSeed<'de>
//     for ComponentSeqDeserializer<'a, T>
// {
//     type Value = ();
//     fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         deserializer.deserialize_seq(self)
//     }
// }
//
// impl<'de, 'a, T: for<'b> Deserialize<'b> + 'static> Visitor<'de>
//     for ComponentSeqDeserializer<'a, T>
// {
//     type Value = ();
//
//     fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
//         formatter.write_str("sequence of objects")
//     }
//     fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
//     where
//         A: de::SeqAccess<'de>,
//     {
//         let size = seq.size_hint();
//         for _ in 0..size.unwrap_or(std::usize::MAX) {
//             match (self.get_next_storage_fn)() {
//                 Some((storage_ptr, storage_len)) => {
//                     let storage_ptr = storage_ptr.as_ptr() as *mut T;
//                     for idx in 0..storage_len {
//                         let element_ptr = unsafe { storage_ptr.add(idx) };
//
//                         if seq
//                             .next_element_seed(ComponentDeserializer {
//                                 ptr: element_ptr,
//                                 _marker: PhantomData,
//                             })?
//                             .is_none()
//                         {
//                             panic!(
//                                 "expected {} elements in chunk but only {} found",
//                                 storage_len, idx
//                             );
//                         }
//                     }
//                 }
//                 None => {
//                     if seq.next_element::<IgnoredAny>()?.is_some() {
//                         panic!("unexpected element when there was no storage space available");
//                     } else {
//                         // No more elements and no more storage - that's what we want!
//                         break;
//                     }
//                 }
//                 _ => {}
//             }
//         }
//
//         Ok(())
//     }
// }
//
// #[derive(Clone)]
// pub struct TagRegistration {
//     pub uuid: type_uuid::Bytes,
//     pub ty: TypeId,
//     tag_serialize_fn: fn(&TagStorage, &mut dyn FnMut(&dyn erased_serde::Serialize)),
//     tag_deserialize_fn: fn(
//         deserializer: &mut dyn erased_serde::Deserializer,
//         &mut TagStorage,
//     ) -> Result<(), erased_serde::Error>,
//     register_tag_fn: fn(&mut ArchetypeDescription),
// }
//
// impl TagRegistration {
//     pub fn of<
//         T: TypeUuid
//             + Serialize
//             + for<'de> Deserialize<'de>
//             + PartialEq
//             + Clone
//             + Send
//             + Sync
//             + 'static,
//     >() -> Self {
//         Self {
//             uuid: T::UUID,
//             ty: TypeId::of::<T>(),
//             tag_serialize_fn: |tag_storage, serialize_fn| {
//                 // it's safe because we know this is the correct type due to lookup
//                 let slice = unsafe { tag_storage.data_slice::<T>() };
//                 serialize_fn(&&*slice);
//             },
//             tag_deserialize_fn: |deserializer, tag_storage| {
//                 // TODO implement visitor to avoid allocation of Vec
//                 let tag_vec = <Vec<T> as Deserialize>::deserialize(deserializer)?;
//                 for tag in tag_vec {
//                     // Tag types should line up, making this safe
//                     unsafe {
//                         tag_storage.push(tag);
//                     }
//                 }
//                 Ok(())
//             },
//             register_tag_fn: |desc| {
//                 desc.register_tag::<T>();
//             },
//         }
//     }
// }
//
// #[derive(Serialize, Deserialize)]
// pub struct SerializedArchetypeDescription {
//     tag_types: Vec<type_uuid::Bytes>,
//     component_types: Vec<u32>,
// }
//
// pub struct SerializeImplementation {
//     pub tag_types: HashMap<TypeId, TagRegistration>,
//
//     pub registered_components: RegisteredComponentsResource,
//
//     pub entity_map: RefCell<HashMap<Entity, u32>>,
// }
//
// impl legion::serialize::ser::WorldSerializer for SerializeImplementation {
//     fn can_serialize_tag(&self, ty: &TagTypeId, _meta: &TagMeta) -> bool {
//         self.tag_types.get(&ty.type_id()).is_some()
//     }
//
//     fn can_serialize_component(&self, ty: &ComponentTypeId, _meta: &ComponentMeta) -> bool {
//         let registry = self.registered_components.by_type_id();
//         registry.get(&ty.type_id()).is_some()
//     }
//
//     fn serialize_archetype_description<S: Serializer>(
//         &self,
//         serializer: S,
//         archetype_desc: &ArchetypeDescription,
//     ) -> Result<S::Ok, S::Error> {
//         let tags_to_serialize = archetype_desc
//             .tags()
//             .iter()
//             .filter_map(|(ty, _)| self.tag_types.get(&ty.type_id()))
//             .map(|reg| reg.uuid)
//             .collect::<Vec<_>>();
//
//         let components_to_serialize = archetype_desc
//             .components()
//             .iter()
//             .filter_map(|(ty, _)| self.registered_components.get_uid(&ty.type_id()))
//             .map(|reg| *reg)
//             .collect::<Vec<Uid>>();
//
//         SerializedArchetypeDescription {
//             tag_types: tags_to_serialize,
//             component_types: components_to_serialize,
//         }
//             .serialize(serializer)
//     }
//
//     fn serialize_components<S: Serializer>(
//         &self,
//         serializer: S,
//         component_type: &ComponentTypeId,
//         _component_meta: &ComponentMeta,
//         components: &ComponentResourceSet,
//     ) -> Result<S::Ok, S::Error> {
//         let registry = self.registered_components.by_type_id();
//
//         if let Some(registration) = registry.get(&component_type.type_id()) {
//             let result = RefCell::new(None);
//             let serializer = RefCell::new(Some(serializer));
//             {
//                 let mut result_ref = result.borrow_mut();
//                 registration.component_serialize_fn(components, &mut |serialize| {
//                     result_ref.replace(erased_serde::serialize(
//                         serialize,
//                         serializer.borrow_mut().take().unwrap(),
//                     ));
//                 });
//             }
//             return result.borrow_mut().take().unwrap();
//         }
//
//         panic!(
//             "received unserializable type {:?}, this should be filtered by can_serialize",
//             component_type
//         );
//     }
//
//     fn serialize_tags<S: Serializer>(
//         &self,
//         serializer: S,
//         tag_type: &TagTypeId,
//         _tag_meta: &TagMeta,
//         tags: &TagStorage,
//     ) -> Result<S::Ok, S::Error> {
//         if let Some(registration) = self.tag_types.get(&tag_type.type_id()) {
//             let result = RefCell::new(None);
//             let serializer = RefCell::new(Some(serializer));
//             {
//                 let mut result_ref = result.borrow_mut();
//                 (registration.tag_serialize_fn)(tags, &mut |serialize| {
//                     result_ref.replace(erased_serde::serialize(
//                         serialize,
//                         serializer.borrow_mut().take().unwrap(),
//                     ));
//                 });
//             }
//             return result.borrow_mut().take().unwrap();
//         }
//         panic!(
//             "received unserializable type {:?}, this should be filtered by can_serialize",
//             tag_type
//         );
//     }
//
//     fn serialize_entities<S: Serializer>(
//         &self,
//         serializer: S,
//         entities: &[Entity],
//     ) -> Result<S::Ok, S::Error> {
//         let mut uid_map = self.entity_map.borrow_mut();
//         serializer.collect_seq(entities.iter().map(|e| {
//             if let Some(result) = uid_map.get(e) {
//                 return Some(*result);
//             }
//             None
//         })
//             .filter(|x| x.is_some())
//         )
//     }
// }
//
// pub struct DeserializeImplementation {
//    pub tag_types: HashMap<TypeId, TagRegistration>,
//    pub tag_types_by_uuid: HashMap<type_uuid::Bytes, TagRegistration>,
//
//    pub registered_components: RegisteredComponentsResource,
//    pub entity_map: RefCell<HashMap<uid::Uid, Entity>>,
// }
//
// impl legion::serialize::de::WorldDeserializer for DeserializeImplementation {
//     fn deserialize_archetype_description<'de, D: Deserializer<'de>>(
//         &self,
//         deserializer: D,
//     ) -> Result<ArchetypeDescription, <D as Deserializer<'de>>::Error> {
//         let serialized_desc =
//             <SerializedArchetypeDescription as Deserialize>::deserialize(deserializer)?;
//
//         let mut desc = ArchetypeDescription::default();
//
//         for tag in serialized_desc.tag_types {
//             if let Some(registration) = self.tag_types_by_uuid.get(&tag) {
//                 (registration.register_tag_fn)(&mut desc);
//             }
//         }
//
//         let registration = self.registered_components.by_uid();
//
//         for comp in serialized_desc.component_types {
//             if let Some(registration) = registration.get(&comp) {
//                 registration.register_component_fn(&mut desc);
//             }
//         }
//
//         Ok(desc)
//     }
//
//     fn deserialize_components<'de, D: Deserializer<'de>>(
//         &self,
//         deserializer: D,
//         component_type: &ComponentTypeId,
//         _component_meta: &ComponentMeta,
//         get_next_storage_fn: &mut dyn FnMut() -> Option<(NonNull<u8>, usize)>,
//     ) -> Result<(), <D as Deserializer<'de>>::Error> {
//         let registry = self.registered_components.by_type_id();
//         if let Some(reg) = registry.get(&component_type.type_id()) {
//             let mut erased = erased_serde::Deserializer::erase(deserializer);
//             reg.component_deserialize_fn(&mut erased, get_next_storage_fn)
//                 .map_err(<<D as serde::Deserializer<'de>>::Error as serde::de::Error>::custom)?;
//         } else {
//             <IgnoredAny>::deserialize(deserializer)?;
//         }
//
//         Ok(())
//     }
//
//     fn deserialize_tags<'de, D: Deserializer<'de>>(
//         &self,
//         deserializer: D,
//         tag_type: &TagTypeId,
//         _tag_meta: &TagMeta,
//         tags: &mut TagStorage,
//     ) -> Result<(), <D as Deserializer<'de>>::Error> {
//         if let Some(reg) = self.tag_types.get(&tag_type.0) {
//             let mut erased = erased_serde::Deserializer::erase(deserializer);
//             (reg.tag_deserialize_fn)(&mut erased, tags)
//                 .map_err(<<D as serde::Deserializer<'de>>::Error as serde::de::Error>::custom)?;
//         } else {
//             <IgnoredAny>::deserialize(deserializer)?;
//         }
//         Ok(())
//     }
//
//     fn deserialize_entities<'de, D: Deserializer<'de>>(
//         &self,
//         deserializer: D,
//         entity_allocator: &EntityAllocator,
//         entities: &mut Vec<Entity>,
//     ) -> Result<(), <D as Deserializer<'de>>::Error> {
//         let entity_uuids = <Vec<u32> as Deserialize>::deserialize(deserializer)?;
//         let mut entity_map = self.entity_map.borrow_mut();
//         for id in entity_uuids {
//             let entity = entity_allocator.create_entity();
//             entity_map.insert(id, entity);
//             entities.push(entity);
//         }
//         Ok(())
//     }
// }
