use crate::ecs::components::id::Id;
use bevy::prelude::*;
use bevy::reflect::{PartialReflect, Reflect};
use bevy_reflect::serde::{ReflectDeserializer, ReflectSerializer};
use serde::de::DeserializeSeed;
use serde_json::{Value, from_str, to_string};
use std::collections::HashMap;

pub fn serialize_entity(entity: EntityRef, world: &World) -> String {
    let registry = world.resource::<AppTypeRegistry>().read();
    let mut components = HashMap::new();

    registry
        .iter()
        .filter_map(|reg| {
            reg.data::<ReflectComponent>()
                .and_then(|rc| rc.reflect(entity).map(|c| (reg, c)))
        })
        .for_each(|(registration, component)| {
            let type_name = registration.type_info().type_path();
            match serde_json::to_value(ReflectSerializer::new(
                component.as_partial_reflect(),
                &registry,
            )) {
                Ok(mut value) => {
                    if let Value::Object(ref mut map) = value {
                        if let Some(inner) = map.remove(type_name) {
                            value = inner;
                        }
                    }
                    components.insert(type_name.to_string(), value);
                }
                Err(e) => println!("Failed to serialize {}: {}", type_name, e),
            }
        });

    to_string(&components).unwrap_or_else(|e| {
        println!("Failed to serialize components: {}", e);
        String::new()
    })
}

pub fn deserialize_entity(
    serialized: &str,
    world: &mut World,
    entity: Entity,
) -> Result<(), String> {
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();

    let components: HashMap<String, Value> = match from_str(serialized) {
        Ok(c) => c,
        Err(e) => return Err(format!("Failed to parse serialized entity: {}", e)),
    };

    for (type_name, value) in components {
        // Find the registration for this type
        let registration = match registry.get_with_type_path(&type_name) {
            Some(reg) => reg,
            None => {
                println!(
                    "Warning: No registration found for component type: {}",
                    type_name
                );
                continue;
            }
        };

        // Get the ReflectComponent data for this registration
        let reflect_component = match registration.data::<ReflectComponent>() {
            Some(rc) => rc,
            None => {
                println!("Warning: Type {} is not a component", type_name);
                continue;
            }
        };

        // Create a Value wrapper with the type name
        let mut wrapper = serde_json::Map::new();
        wrapper.insert(type_name.clone(), value);
        let wrapped_value = Value::Object(wrapper);

        // Deserialize the component
        let deserializer = ReflectDeserializer::new(&registry);
        match deserializer.deserialize(&mut serde_json::Deserializer::from_str(
            &wrapped_value.to_string(),
        )) {
            Ok(component) => {
                let mut entity_ref = world.entity_mut(entity);
                reflect_component.apply_or_insert(&mut entity_ref, component.as_ref(), &registry);
            }
            Err(e) => println!("Failed to deserialize component {}: {}", type_name, e),
        }
    }

    Ok(())
}

pub fn update_entity(serialized: &str, world: &mut World) -> Result<Entity, String> {
    // Parse the serialized data first
    let components: HashMap<String, Value> = match from_str(serialized) {
        Ok(c) => c,
        Err(e) => return Err(format!("Failed to parse serialized entity: {}", e)),
    };

    // Check if there's an Id component
    if let Some(id_value) = components.get("sidereal::ecs::components::id::Id") {
        // Try to deserialize just the Id to find existing entity
        if let Ok(id_str) = serde_json::from_value::<String>(id_value.clone()) {
            // Query for entity with matching Id
            let entity = world
                .query_filtered::<Entity, With<Id>>()
                .iter(world)
                .find(|&e| {
                    if let Some(id) = world.get::<Id>(e) {
                        id.to_string() == id_str
                    } else {
                        false
                    }
                });

            // Use existing entity or create new one
            let entity = entity.unwrap_or_else(|| world.spawn_empty().id());
            deserialize_entity(serialized, world, entity)?;
            return Ok(entity);
        }
    }

    // No Id found or couldn't parse it - just create a new entity
    let entity = world.spawn_empty().id();
    deserialize_entity(serialized, world, entity)?;
    Ok(entity)
}

#[allow(dead_code)]
trait AsPartialReflect {
    fn as_partial_reflect(&self) -> &dyn PartialReflect;
}

impl AsPartialReflect for dyn Reflect {
    #[inline]
    fn as_partial_reflect(&self) -> &dyn PartialReflect {
        unsafe { std::mem::transmute(self) }
    }
}
