use bevy::prelude::*;
use bevy_reflect::serde::{ReflectDeserializer, ReflectSerializer};
use bevy_reflect::{GetTypeRegistration, PartialReflect, Reflect, TypeRegistration, TypeRegistry};
use serde::de::DeserializeSeed;
use serde::{Deserialize, Serialize};
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
        println!("Created empty entity: {:?}", entity);
        let type_registry = self.resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();

        for (type_name, value) in &serialized.components {
            println!("Deserializing component: {}", type_name);
            if let Some(registration) = find_registration_by_name(&registry, type_name) {
                if let Some(component_id) = registration.data::<ReflectComponent>() {
                    let wrapped_value = serde_json::json!({
                        type_name: value
                    });

                    let deserializer = ReflectDeserializer::new(&registry);
                    let json_str = wrapped_value.to_string();
                    let mut json_de = serde_json::Deserializer::from_str(&json_str);
                    let reflect_value = deserializer.deserialize(&mut json_de).map_err(|err| {
                        format!("Failed to deserialize component {}: {}", type_name, err)
                    })?;

                    // Create a mutable reference that can access the world for apply_or_insert
                    let mut entity_world_mut = self.entity_mut(entity);

                    // Use apply_or_insert instead of apply so it works for new components
                    component_id.apply_or_insert(
                        &mut entity_world_mut,
                        reflect_value.as_ref(),
                        &registry,
                    );
                }
            } else {
                return Err(format!("Type {} not found in registry", type_name));
            }
            println!("Successfully deserialized component: {}", type_name);
        }

        println!("All components deserialized");
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
        // Just use Bevy's built-in register_type method
        self.register_type::<T>()
    }
}
