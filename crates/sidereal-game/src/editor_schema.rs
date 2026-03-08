use bevy::prelude::*;
use bevy::reflect::{
    EnumInfo, NamedField, OpaqueInfo, StructInfo, TupleInfo, TupleStructInfo, TypeInfo,
    TypeRegistry,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum ComponentEditorValueKind {
    Bool,
    SignedInteger,
    UnsignedInteger,
    Float,
    String,
    Vec2,
    Vec3,
    Vec4,
    ColorRgb,
    ColorRgba,
    Enum,
    Sequence,
    Struct,
    Tuple,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize, Default)]
pub struct ComponentEditorFieldSchema {
    pub field_path: String,
    pub field_name: String,
    pub display_name: String,
    pub value_kind: ComponentEditorValueKind,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub unit: Option<String>,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize, Default)]
pub struct ComponentEditorSchema {
    pub root_value_kind: ComponentEditorValueKind,
    pub fields: Vec<ComponentEditorFieldSchema>,
}

pub fn default_component_editor_schema(type_path: &str) -> ComponentEditorSchema {
    ComponentEditorSchema {
        root_value_kind: infer_value_kind(type_path, None),
        fields: Vec::new(),
    }
}

pub fn infer_component_editor_schema(
    type_registry: &TypeRegistry,
    type_path: &str,
) -> ComponentEditorSchema {
    let mut visited = HashSet::<String>::new();
    let mut schema = default_component_editor_schema(type_path);
    infer_type_path_into_schema(
        type_registry,
        type_path,
        None,
        None,
        &mut schema.fields,
        &mut visited,
    );
    if !schema.fields.is_empty() {
        schema.root_value_kind = ComponentEditorValueKind::Struct;
    }
    schema
}

fn infer_type_path_into_schema(
    type_registry: &TypeRegistry,
    type_path: &str,
    path_prefix: Option<&str>,
    name_hint: Option<&str>,
    fields: &mut Vec<ComponentEditorFieldSchema>,
    visited: &mut HashSet<String>,
) {
    if !visited.insert(type_path.to_string()) {
        return;
    }

    let Some(registration) = type_registry.get_with_type_path(type_path) else {
        visited.remove(type_path);
        return;
    };
    let type_info = registration.type_info();

    match type_info {
        TypeInfo::Struct(info) => {
            infer_struct_info(type_registry, info, path_prefix, fields, visited)
        }
        TypeInfo::TupleStruct(info) => {
            infer_tuple_struct_info(type_registry, info, path_prefix, name_hint, fields, visited)
        }
        TypeInfo::Tuple(info) => {
            infer_tuple_info(type_registry, info, path_prefix, name_hint, fields, visited)
        }
        TypeInfo::Enum(info) => infer_enum_info(info, path_prefix, name_hint, fields),
        TypeInfo::List(_) | TypeInfo::Array(_) | TypeInfo::Map(_) | TypeInfo::Set(_) => {
            let field_name =
                name_hint.unwrap_or(type_path.rsplit("::").next().unwrap_or(type_path));
            fields.push(ComponentEditorFieldSchema {
                field_path: path_prefix.unwrap_or(field_name).to_string(),
                field_name: field_name.to_string(),
                display_name: display_name(field_name),
                value_kind: ComponentEditorValueKind::Sequence,
                min: None,
                max: None,
                step: None,
                unit: None,
                options: Vec::new(),
            });
        }
        TypeInfo::Opaque(info) => infer_opaque_info(info, path_prefix, name_hint, fields),
    }

    visited.remove(type_path);
}

fn infer_struct_info(
    type_registry: &TypeRegistry,
    info: &StructInfo,
    path_prefix: Option<&str>,
    fields: &mut Vec<ComponentEditorFieldSchema>,
    visited: &mut HashSet<String>,
) {
    for field in info.iter() {
        push_named_field(type_registry, field, path_prefix, fields, visited);
    }
}

fn infer_tuple_struct_info(
    type_registry: &TypeRegistry,
    info: &TupleStructInfo,
    path_prefix: Option<&str>,
    name_hint: Option<&str>,
    fields: &mut Vec<ComponentEditorFieldSchema>,
    visited: &mut HashSet<String>,
) {
    if info.field_len() == 1 {
        let Some(field) = info.field_at(0) else {
            return;
        };
        let field_name = name_hint.unwrap_or("value");
        push_leaf_or_nested(
            type_registry,
            field.type_path(),
            path_prefix,
            field_name,
            fields,
            visited,
        );
        return;
    }

    for index in 0..info.field_len() {
        let Some(field) = info.field_at(index) else {
            continue;
        };
        let field_name = format!("item_{index}");
        let next_path = join_path(path_prefix, &field_name);
        push_leaf_or_nested(
            type_registry,
            field.type_path(),
            Some(next_path.as_str()),
            &field_name,
            fields,
            visited,
        );
    }
}

fn infer_tuple_info(
    type_registry: &TypeRegistry,
    info: &TupleInfo,
    path_prefix: Option<&str>,
    name_hint: Option<&str>,
    fields: &mut Vec<ComponentEditorFieldSchema>,
    visited: &mut HashSet<String>,
) {
    if info.field_len() == 1 {
        let Some(field) = info.field_at(0) else {
            return;
        };
        let field_name = name_hint.unwrap_or("value");
        push_leaf_or_nested(
            type_registry,
            field.type_path(),
            path_prefix,
            field_name,
            fields,
            visited,
        );
        return;
    }

    for index in 0..info.field_len() {
        let Some(field) = info.field_at(index) else {
            continue;
        };
        let field_name = format!("item_{index}");
        let next_path = join_path(path_prefix, &field_name);
        push_leaf_or_nested(
            type_registry,
            field.type_path(),
            Some(next_path.as_str()),
            &field_name,
            fields,
            visited,
        );
    }
}

fn infer_enum_info(
    info: &EnumInfo,
    path_prefix: Option<&str>,
    name_hint: Option<&str>,
    fields: &mut Vec<ComponentEditorFieldSchema>,
) {
    let field_name = name_hint.unwrap_or_else(|| info.type_path_table().short_path());
    let field_path = path_prefix.unwrap_or(field_name).to_string();
    let mut options = Vec::new();
    for variant in info.iter() {
        options.push(variant.name().to_string());
    }
    fields.push(ComponentEditorFieldSchema {
        field_path,
        field_name: field_name.to_string(),
        display_name: display_name(field_name),
        value_kind: ComponentEditorValueKind::Enum,
        min: None,
        max: None,
        step: None,
        unit: None,
        options,
    });
}

fn infer_opaque_info(
    info: &OpaqueInfo,
    path_prefix: Option<&str>,
    name_hint: Option<&str>,
    fields: &mut Vec<ComponentEditorFieldSchema>,
) {
    let field_name = name_hint.unwrap_or_else(|| info.type_path_table().short_path());
    let type_path = info.type_path_table().path();
    fields.push(field_schema(
        path_prefix.unwrap_or(field_name),
        field_name,
        infer_value_kind(type_path, Some(field_name)),
    ));
}

fn push_named_field(
    type_registry: &TypeRegistry,
    field: &NamedField,
    path_prefix: Option<&str>,
    fields: &mut Vec<ComponentEditorFieldSchema>,
    visited: &mut HashSet<String>,
) {
    let field_name = field.name();
    let next_path = join_path(path_prefix, field_name);
    push_leaf_or_nested(
        type_registry,
        field.type_path(),
        Some(next_path.as_str()),
        field_name,
        fields,
        visited,
    );
}

fn push_leaf_or_nested(
    type_registry: &TypeRegistry,
    type_path: &str,
    path_prefix: Option<&str>,
    field_name: &str,
    fields: &mut Vec<ComponentEditorFieldSchema>,
    visited: &mut HashSet<String>,
) {
    if is_leaf_like(type_path, field_name) {
        fields.push(field_schema(
            path_prefix.unwrap_or(field_name),
            field_name,
            infer_value_kind(type_path, Some(field_name)),
        ));
        return;
    }

    let before_len = fields.len();
    infer_type_path_into_schema(
        type_registry,
        type_path,
        path_prefix,
        Some(field_name),
        fields,
        visited,
    );
    if fields.len() == before_len {
        fields.push(field_schema(
            path_prefix.unwrap_or(field_name),
            field_name,
            infer_value_kind(type_path, Some(field_name)),
        ));
    }
}

fn field_schema(
    field_path: &str,
    field_name: &str,
    value_kind: ComponentEditorValueKind,
) -> ComponentEditorFieldSchema {
    let (min, max, step, unit) = numeric_hints(value_kind, field_name);
    ComponentEditorFieldSchema {
        field_path: field_path.to_string(),
        field_name: field_name.to_string(),
        display_name: display_name(field_name),
        value_kind,
        min,
        max,
        step,
        unit,
        options: Vec::new(),
    }
}

fn numeric_hints(
    value_kind: ComponentEditorValueKind,
    field_name: &str,
) -> (Option<f64>, Option<f64>, Option<f64>, Option<String>) {
    match value_kind {
        ComponentEditorValueKind::Bool
        | ComponentEditorValueKind::String
        | ComponentEditorValueKind::Vec2
        | ComponentEditorValueKind::Vec3
        | ComponentEditorValueKind::Vec4
        | ComponentEditorValueKind::ColorRgb
        | ComponentEditorValueKind::ColorRgba
        | ComponentEditorValueKind::Enum
        | ComponentEditorValueKind::Sequence
        | ComponentEditorValueKind::Struct
        | ComponentEditorValueKind::Tuple
        | ComponentEditorValueKind::Unknown => (None, None, None, None),
        ComponentEditorValueKind::SignedInteger => (None, None, Some(1.0), unit_hint(field_name)),
        ComponentEditorValueKind::UnsignedInteger => {
            (Some(0.0), None, Some(1.0), unit_hint(field_name))
        }
        ComponentEditorValueKind::Float => {
            if field_name.ends_with("_alpha")
                || field_name.ends_with("_opacity")
                || field_name.ends_with("_strength")
                || field_name.ends_with("_coverage")
                || field_name.ends_with("_falloff")
                || field_name.ends_with("_gain")
                || field_name.ends_with("_threshold")
                || field_name.ends_with("_level")
                || field_name.ends_with("_size")
                || field_name.ends_with("_power")
                || field_name.ends_with("_scale")
                || field_name.ends_with("_density")
                || field_name.ends_with("_speed")
                || field_name.ends_with("_rate")
                || field_name.ends_with("_intensity")
                || field_name.ends_with("_factor")
                || field_name.ends_with("_wrap")
                || field_name.ends_with("_boost")
            {
                return (Some(0.0), None, Some(0.01), unit_hint(field_name));
            }
            (None, None, Some(0.01), unit_hint(field_name))
        }
    }
}

fn unit_hint(field_name: &str) -> Option<String> {
    if field_name.ends_with("_m") {
        Some("m".to_string())
    } else if field_name.ends_with("_kg") {
        Some("kg".to_string())
    } else if field_name.ends_with("_mps") {
        Some("m/s".to_string())
    } else if field_name.ends_with("_mps2") {
        Some("m/s^2".to_string())
    } else if field_name.ends_with("_rad") {
        Some("rad".to_string())
    } else {
        None
    }
}

fn is_leaf_like(type_path: &str, field_name: &str) -> bool {
    matches!(
        infer_value_kind(type_path, Some(field_name)),
        ComponentEditorValueKind::Bool
            | ComponentEditorValueKind::SignedInteger
            | ComponentEditorValueKind::UnsignedInteger
            | ComponentEditorValueKind::Float
            | ComponentEditorValueKind::String
            | ComponentEditorValueKind::Vec2
            | ComponentEditorValueKind::Vec3
            | ComponentEditorValueKind::Vec4
            | ComponentEditorValueKind::ColorRgb
            | ComponentEditorValueKind::ColorRgba
    )
}

fn infer_value_kind(type_path: &str, field_name: Option<&str>) -> ComponentEditorValueKind {
    let lower = type_path.to_ascii_lowercase();
    let field_name = field_name.unwrap_or_default();
    if lower == "bool" {
        return ComponentEditorValueKind::Bool;
    }
    if matches!(
        lower.as_str(),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
    ) {
        return ComponentEditorValueKind::UnsignedInteger;
    }
    if matches!(
        lower.as_str(),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
    ) {
        return ComponentEditorValueKind::SignedInteger;
    }
    if matches!(lower.as_str(), "f32" | "f64") {
        return ComponentEditorValueKind::Float;
    }
    if lower == "alloc::string::string"
        || lower == "std::string::string"
        || lower == "bevy::utils::cowarc<'static, str>"
    {
        return ComponentEditorValueKind::String;
    }
    if lower.ends_with("::vec2") || lower == "glam::vec2" {
        return ComponentEditorValueKind::Vec2;
    }
    if lower.ends_with("::vec3") || lower == "glam::vec3" {
        if field_name.ends_with("_rgb") {
            return ComponentEditorValueKind::ColorRgb;
        }
        return ComponentEditorValueKind::Vec3;
    }
    if lower.ends_with("::vec4") || lower == "glam::vec4" {
        if field_name.ends_with("_rgba") {
            return ComponentEditorValueKind::ColorRgba;
        }
        return ComponentEditorValueKind::Vec4;
    }
    ComponentEditorValueKind::Unknown
}

fn join_path(prefix: Option<&str>, field_name: &str) -> String {
    match prefix {
        Some(prefix) if !prefix.is_empty() => format!("{prefix}.{field_name}"),
        _ => field_name.to_string(),
    }
}

fn display_name(field_name: &str) -> String {
    field_name
        .split('_')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
