use crate::ecs::components::hull::Hull;
use crate::ecs::components::physics::{PhysicsBody, PhysicsState};
use crate::ecs::components::spatial::{ClusterCoords, Position, SectorCoords};
use crate::ecs::components::Name;
use bevy::prelude::*;
use bevy_reflect::serde::{ReflectDeserializer, ReflectSerializer};
use bevy_reflect::{GetTypeRegistration, PartialReflect, Reflect, TypeRegistration, TypeRegistry};
use serde::de::DeserializeSeed;
use serde::{Deserialize, Serialize};
use std::any::TypeId;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializedEntity {
    pub components: HashMap<String, serde_json::Value>,
}

pub struct EntitySerializationPlugin;

impl Plugin for EntitySerializationPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<AppTypeRegistry>() {
            app.init_resource::<AppTypeRegistry>();
        }
        let type_registry = app.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        for registration in registry.iter() {
            let type_name = registration.type_info().type_path();
            let type_id = registration.type_info().type_id();
            println!("Registered type: {} (TypeId: {:?})", type_name, type_id);
        }
        drop(registry);
    }
}

pub trait EntitySerializer {
    fn serialize_entity(&self, entity: Entity) -> Result<SerializedEntity, String>;
    fn deserialize_entity(&mut self, serialized: &SerializedEntity) -> Result<Entity, String>;
}

impl EntitySerializer for World {
    fn serialize_entity(&self, entity: Entity) -> Result<SerializedEntity, String> {
        let entity_ref = match self.get_entity(entity) {
            Ok(entity_ref) => entity_ref,
            Err(_) => return Err(format!("Entity {:?} not found", entity)),
        };
        let type_registry = self.resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let mut components = HashMap::new();
        for registration in registry.iter() {
            let type_name = registration.type_info().type_path().to_string();
            if let Some(component_id) = registration.data::<ReflectComponent>() {
                if let Some(component) = component_id.reflect(entity_ref) {
                    let partial_component = as_partial_reflect(component);
                    let serializer = ReflectSerializer::new(partial_component, &registry);
                    let value = serde_json::to_value(&serializer).map_err(|err| {
                        format!("Failed to serialize component {}: {}", type_name, err)
                    })?;
                    let value = if let serde_json::Value::Object(mut map) = value {
                        if map.contains_key(&type_name) && map.len() == 1 {
                            map.remove(&type_name).unwrap()
                        } else {
                            serde_json::Value::Object(map)
                        }
                    } else {
                        value
                    };
                    components.insert(type_name, value);
                }
            }
        }
        Ok(SerializedEntity { components })
    }

    fn deserialize_entity(&mut self, serialized: &SerializedEntity) -> Result<Entity, String> {
        let entity = self.spawn_empty().id();
        let type_registry = self.resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let mut entity_mut = self.entity_mut(entity);
        for (type_name, value) in &serialized.components {
            if let Some(registration) = find_registration_by_name(&registry, type_name) {
                if let Some(component_id) = registration.data::<ReflectComponent>() {
                    let deserializer = ReflectDeserializer::new(&registry);
                    let json_str = value.to_string();
                    let mut json_de = serde_json::Deserializer::from_str(&json_str);
                    let reflect_value = deserializer.deserialize(&mut json_de).map_err(|err| {
                        format!("Failed to deserialize component {}: {}", type_name, err)
                    })?;
                    component_id.apply(&mut entity_mut, reflect_value.as_ref());
                }
            } else {
                return Err(format!("Type {} not found in registry", type_name));
            }
        }
        Ok(entity)
    }
}

fn find_registration_by_name<'a>(
    registry: &'a TypeRegistry,
    type_name: &str,
) -> Option<&'a TypeRegistration> {
    registry
        .iter()
        .find(|registration| registration.type_info().type_path() == type_name)
}

fn as_partial_reflect(value: &dyn Reflect) -> &dyn PartialReflect {
    unsafe { std::mem::transmute::<&dyn Reflect, &dyn PartialReflect>(value) }
}

pub trait EntitySerializationExt {
    fn register_serializable_component<T>(&mut self) -> &mut Self
    where
        T: Component + Reflect + GetTypeRegistration;
}

impl EntitySerializationExt for App {
    fn register_serializable_component<T>(&mut self) -> &mut Self
    where
        T: Component + Reflect + GetTypeRegistration,
    {
        let type_registry = self.world().resource::<AppTypeRegistry>().clone();
        {
            let mut registry = type_registry.write();
            registry.register::<T>();
        }
        self.insert_resource(type_registry);
        self
    }
}
