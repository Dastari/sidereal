#[cfg(feature = "persistence")]
use bevy::ecs::reflect::{AppTypeRegistry, ReflectCommandExt, ReflectComponent};
use bevy::prelude::*;
#[cfg(feature = "persistence")]
use bevy::reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
#[cfg(feature = "persistence")]
use serde::Serialize as _;
#[cfg(feature = "persistence")]
use serde::de::DeserializeSeed;
#[cfg(feature = "persistence")]
use sidereal_game::GeneratedComponentRegistry;
#[cfg(feature = "persistence")]
use sidereal_persistence::{GraphComponentRecord, encode_reflect_component};
use std::collections::HashMap;

#[derive(Debug, Resource, Default)]
pub struct RuntimeEntityHierarchy {
    pub by_entity_id: HashMap<String, Entity>,
    pub pending_children_by_parent_id: HashMap<String, Vec<Entity>>,
}

#[cfg(feature = "persistence")]
pub fn component_type_path_map(registry: &GeneratedComponentRegistry) -> HashMap<String, String> {
    registry
        .entries
        .iter()
        .map(|entry| {
            (
                entry.component_kind.to_string(),
                entry.type_path.to_string(),
            )
        })
        .collect::<HashMap<_, _>>()
}

#[cfg(feature = "persistence")]
pub fn decode_component_payload<'a>(
    component_kind: &str,
    properties: &'a serde_json::Value,
    type_paths: &HashMap<String, String>,
) -> Option<&'a serde_json::Value> {
    let expected_type_path = type_paths.get(component_kind)?;
    let sanitized_key = expected_type_path.replace("::", "__");
    properties
        .as_object()
        .and_then(|obj| {
            obj.get(&sanitized_key)
                .or_else(|| obj.get(expected_type_path))
        })
        .or(Some(properties))
}

#[cfg(feature = "persistence")]
pub fn component_record<'a>(
    components: &'a [GraphComponentRecord],
    kind: &str,
) -> Option<&'a GraphComponentRecord> {
    components
        .iter()
        .find(|component| component.component_kind == kind)
}

#[cfg(feature = "persistence")]
pub fn decode_graph_component_payload<'a>(
    component: &'a GraphComponentRecord,
    type_paths: &HashMap<String, String>,
) -> Option<&'a serde_json::Value> {
    decode_component_payload(&component.component_kind, &component.properties, type_paths)
}

#[cfg(feature = "persistence")]
pub fn wrap_component_payload(
    component_kind: &str,
    payload: serde_json::Value,
    type_paths: &HashMap<String, String>,
) -> serde_json::Value {
    if let Some(type_path) = type_paths.get(component_kind) {
        encode_reflect_component(type_path, payload)
    } else {
        payload
    }
}

#[cfg(feature = "persistence")]
pub fn format_component_id(entity_id: &str, component_kind: &str) -> String {
    format!("{entity_id}:{component_kind}")
}

pub fn parse_vec3_value(value: &serde_json::Value) -> Option<Vec3> {
    let arr = value.as_array()?;
    if arr.len() != 3 {
        return None;
    }
    Some(Vec3::new(
        arr[0].as_f64()? as f32,
        arr[1].as_f64()? as f32,
        arr[2].as_f64()? as f32,
    ))
}

/// Extracts a UUID from a canonical entity_id string (bare UUID only).
pub fn parse_guid_from_entity_id(entity_id: &str) -> Option<uuid::Uuid> {
    uuid::Uuid::parse_str(entity_id).ok()
}

pub fn value_as_vec3_recursive(value: &serde_json::Value) -> Option<Vec3> {
    if let Some(arr) = value.as_array()
        && arr.len() == 3
    {
        return Some(Vec3::new(
            arr[0].as_f64()? as f32,
            arr[1].as_f64()? as f32,
            arr[2].as_f64()? as f32,
        ));
    }
    let obj = value.as_object()?;
    for nested in obj.values() {
        if let Some(v) = value_as_vec3_recursive(nested) {
            return Some(v);
        }
    }
    None
}

pub fn value_as_f32_recursive(value: &serde_json::Value) -> Option<f32> {
    if let Some(v) = value.as_f64() {
        return Some(v as f32);
    }
    let obj = value.as_object()?;
    for nested in obj.values() {
        if let Some(v) = value_as_f32_recursive(nested) {
            return Some(v);
        }
    }
    None
}

#[cfg(feature = "persistence")]
pub fn insert_registered_components_from_graph_records(
    commands: &mut Commands<'_, '_>,
    entity: Entity,
    components: &[GraphComponentRecord],
    type_paths: &HashMap<String, String>,
    app_type_registry: &AppTypeRegistry,
) {
    insert_registered_components(
        commands,
        entity,
        components,
        type_paths,
        app_type_registry,
        |component| (&component.component_kind, &component.properties),
    );
}

#[cfg(feature = "persistence")]
fn insert_registered_components<T>(
    commands: &mut Commands<'_, '_>,
    entity: Entity,
    components: &[T],
    type_paths: &HashMap<String, String>,
    app_type_registry: &AppTypeRegistry,
    get_kind_and_properties: impl Fn(&T) -> (&str, &serde_json::Value),
) {
    let type_registry = app_type_registry.read();
    for component in components {
        let (component_kind, properties) = get_kind_and_properties(component);
        let Some(type_path) = type_paths.get(component_kind) else {
            continue;
        };
        let Some(type_registration) = type_registry.get_with_type_path(type_path) else {
            continue;
        };
        let Some(payload) = decode_component_payload(component_kind, properties, type_paths) else {
            continue;
        };
        let payload_str = payload.to_string();
        let typed = TypedReflectDeserializer::new(type_registration, &type_registry);
        let mut deserializer = serde_json::Deserializer::from_str(&payload_str);
        let Ok(reflect_component) = typed.deserialize(&mut deserializer) else {
            continue;
        };
        commands.entity(entity).insert_reflect(reflect_component);
    }
}

pub fn register_runtime_entity(
    hierarchy: &mut RuntimeEntityHierarchy,
    entity_id: String,
    entity: Entity,
) {
    hierarchy.by_entity_id.insert(entity_id, entity);
}

/// Serialize all registered components present on `entity_ref` into `GraphComponentRecord`s.
///
/// Uses the `GeneratedComponentRegistry` to discover which components to look for,
/// then extracts each via `ReflectComponent::reflect()` and serializes with
/// `TypedReflectSerializer` to produce round-trippable JSON.
#[cfg(feature = "persistence")]
pub fn serialize_entity_components_to_graph_records(
    entity_id: &str,
    entity_ref: EntityRef<'_>,
    registry: &GeneratedComponentRegistry,
    app_type_registry: &AppTypeRegistry,
) -> Vec<GraphComponentRecord> {
    let type_registry = app_type_registry.read();
    let mut records = Vec::new();

    for entry in &registry.entries {
        let Some(type_registration) = type_registry.get_with_type_path(entry.type_path) else {
            continue;
        };
        let Some(reflect_component) = type_registration.data::<ReflectComponent>() else {
            continue;
        };
        let Some(reflected) = reflect_component.reflect(entity_ref) else {
            continue;
        };

        let serializer = TypedReflectSerializer::new(reflected, &type_registry);
        let mut buf = Vec::new();
        let mut json_serializer = serde_json::Serializer::new(&mut buf);
        if serializer.serialize(&mut json_serializer).is_err() {
            continue;
        }
        let Ok(json_value) = serde_json::from_slice::<serde_json::Value>(&buf) else {
            continue;
        };

        records.push(GraphComponentRecord {
            component_id: format_component_id(entity_id, entry.component_kind),
            component_kind: entry.component_kind.to_string(),
            properties: json_value,
        });
    }

    records
}
