use bevy::prelude::*;
use bevy_reflect::serde::{ReflectDeserializer, ReflectSerializer};
use bevy_reflect::{GetTypeRegistration, PartialReflect, Reflect, TypeRegistration, TypeRegistry};
use serde::de::DeserializeSeed;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use bincode::{Encode, Decode};
use avian2d::prelude::*;
use crate::ecs::components::*;
#[derive(Serialize, Deserialize, Debug, Clone, Encode, Decode)]
pub struct SerializedEntity {
    pub components: HashMap<String, String>,
}

pub struct EntitySerializationPlugin;

impl Plugin for EntitySerializationPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<AppTypeRegistry>() {
            app.init_resource::<AppTypeRegistry>();
        }

        app.register_serializable_component::<Object>()
        .register_serializable_component::<Id>()
        .register_serializable_component::<ColliderMarker>()
        .register_serializable_component::<ColliderAabb>()
        .register_serializable_component::<AccumulatedTranslation>()
        .register_serializable_component::<AngularVelocity>()
        .register_serializable_component::<ExternalAngularImpulse>()
        .register_serializable_component::<ExternalForce>()
        .register_serializable_component::<ExternalImpulse>()
        .register_serializable_component::<ExternalTorque>()
        .register_serializable_component::<LinearVelocity>()
        .register_serializable_component::<ColliderDensity>()
        .register_serializable_component::<ColliderMassProperties>()
        .register_serializable_component::<ComputedAngularInertia>()
        .register_serializable_component::<ComputedCenterOfMass>()
        .register_serializable_component::<ComputedMass>()
        .register_serializable_component::<RigidBody>()
        .register_serializable_component::<Transform>()
        .register_serializable_component::<Rotation>()
        .register_serializable_component::<Name>()
        .register_serializable_component::<InSector>()
        .register_serializable_component::<Parent>();

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
                    
                    // Convert the JSON value to a string
                    let json_string = serde_json::to_string(&value).map_err(|err| {
                        format!("Failed to convert JSON to string for component {}: {}", type_name, err)
                    })?;
                    
                    components.insert(type_name, json_string);
                }
            }
        }
        Ok(SerializedEntity { components })
    }

    fn deserialize_entity(&mut self, serialized: &SerializedEntity) -> Result<Entity, String> {
        let entity = self.spawn_empty().id();
        let type_registry = self.resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();

        for (type_name, json_string) in &serialized.components {
            // Parse the JSON string back to serde_json::Value
            let value: serde_json::Value = serde_json::from_str(json_string).map_err(|err| {
                format!("Failed to parse JSON string for component {}: {}", type_name, err)
            })?;
            
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
        // Just use Bevy's built-in register_type method
        self.register_type::<T>()
    }
}
