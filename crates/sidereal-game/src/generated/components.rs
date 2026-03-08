use crate::component_meta::VisibilityScope;
pub use crate::components::*;
use crate::editor_schema::{
    ComponentEditorFieldSchema, ComponentEditorSchema, ComponentEditorValueKind,
    default_component_editor_schema, infer_component_editor_schema,
};
use bevy::ecs::reflect::AppTypeRegistry;
use bevy::prelude::*;

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ComponentRegistryEntry {
    pub component_kind: &'static str,
    pub type_path: &'static str,
    pub replication_visibility: Vec<VisibilityScope>,
    pub editor_schema: ComponentEditorSchema,
}

#[derive(Debug, Resource, Clone, Reflect)]
#[reflect(Resource)]
pub struct GeneratedComponentRegistry {
    pub entries: Vec<ComponentRegistryEntry>,
}

pub fn register_generated_components(app: &mut App) {
    for registration in ::inventory::iter::<crate::component_meta::SiderealComponentRegistration> {
        (registration.register_reflect)(app);
    }

    app.register_type::<avian2d::prelude::Position>();
    app.register_type::<avian2d::prelude::Rotation>();
    app.register_type::<avian2d::prelude::LinearVelocity>();
    app.register_type::<avian2d::prelude::AngularVelocity>();
    app.register_type::<avian2d::prelude::RigidBody>();
    app.register_type::<avian2d::prelude::Mass>();
    app.register_type::<avian2d::prelude::AngularInertia>();
    app.register_type::<avian2d::prelude::LinearDamping>();
    app.register_type::<avian2d::prelude::AngularDamping>();
    app.register_type::<ComponentEditorValueKind>();
    app.register_type::<ComponentEditorFieldSchema>();
    app.register_type::<ComponentEditorSchema>();
    app.register_type::<ComponentRegistryEntry>();
    app.register_type::<GeneratedComponentRegistry>();

    let entries = {
        let app_type_registry = app.world().resource::<AppTypeRegistry>().clone();
        generated_component_registry_with_type_registry(&app_type_registry)
    };
    app.insert_resource(GeneratedComponentRegistry { entries });
}

/// Avian component entries appended to the macro-collected registry so that
/// third-party physics types persist/hydrate through the same generic path.
/// Uses `TypePath::type_path()` at runtime so paths stay correct across
/// Avian versions.
fn avian_registry_entries() -> Vec<ComponentRegistryEntry> {
    use avian2d::prelude as av;
    use bevy::reflect::TypePath;

    // Leak the strings so they have 'static lifetime matching the rest of
    // the registry entries. This runs once at startup.
    fn leak(s: &str) -> &'static str {
        Box::leak(s.to_string().into_boxed_str())
    }

    vec![
        ComponentRegistryEntry {
            component_kind: "avian_position",
            type_path: leak(av::Position::type_path()),
            replication_visibility: vec![VisibilityScope::Public],
            editor_schema: default_component_editor_schema(av::Position::type_path()),
        },
        ComponentRegistryEntry {
            component_kind: "avian_rotation",
            type_path: leak(av::Rotation::type_path()),
            replication_visibility: vec![VisibilityScope::Public],
            editor_schema: default_component_editor_schema(av::Rotation::type_path()),
        },
        ComponentRegistryEntry {
            component_kind: "avian_linear_velocity",
            type_path: leak(av::LinearVelocity::type_path()),
            replication_visibility: vec![VisibilityScope::Public],
            editor_schema: default_component_editor_schema(av::LinearVelocity::type_path()),
        },
        ComponentRegistryEntry {
            component_kind: "avian_angular_velocity",
            type_path: leak(av::AngularVelocity::type_path()),
            replication_visibility: vec![VisibilityScope::Public],
            editor_schema: default_component_editor_schema(av::AngularVelocity::type_path()),
        },
        ComponentRegistryEntry {
            component_kind: "avian_rigid_body",
            type_path: leak(av::RigidBody::type_path()),
            replication_visibility: vec![VisibilityScope::Public],
            editor_schema: default_component_editor_schema(av::RigidBody::type_path()),
        },
        ComponentRegistryEntry {
            component_kind: "avian_mass",
            type_path: leak(av::Mass::type_path()),
            replication_visibility: vec![VisibilityScope::Public],
            editor_schema: default_component_editor_schema(av::Mass::type_path()),
        },
        ComponentRegistryEntry {
            component_kind: "avian_angular_inertia",
            type_path: leak(av::AngularInertia::type_path()),
            replication_visibility: vec![VisibilityScope::Public],
            editor_schema: default_component_editor_schema(av::AngularInertia::type_path()),
        },
        ComponentRegistryEntry {
            component_kind: "avian_linear_damping",
            type_path: leak(av::LinearDamping::type_path()),
            replication_visibility: vec![VisibilityScope::Public],
            editor_schema: default_component_editor_schema(av::LinearDamping::type_path()),
        },
        ComponentRegistryEntry {
            component_kind: "avian_angular_damping",
            type_path: leak(av::AngularDamping::type_path()),
            replication_visibility: vec![VisibilityScope::Public],
            editor_schema: default_component_editor_schema(av::AngularDamping::type_path()),
        },
    ]
}

pub fn generated_component_registry() -> Vec<ComponentRegistryEntry> {
    let mut entries = ::inventory::iter::<crate::component_meta::SiderealComponentRegistration>
        .into_iter()
        .filter(|registration| registration.meta.persist)
        .map(|registration| ComponentRegistryEntry {
            component_kind: registration.meta.kind,
            type_path: (registration.type_path)(),
            replication_visibility: registration.meta.visibility.to_vec(),
            editor_schema: default_component_editor_schema((registration.type_path)()),
        })
        .collect::<Vec<_>>();

    entries.extend(avian_registry_entries());

    entries.sort_unstable_by(|a, b| {
        a.component_kind
            .cmp(b.component_kind)
            .then_with(|| a.type_path.cmp(b.type_path))
    });
    entries
}

pub fn generated_component_registry_with_type_registry(
    app_type_registry: &AppTypeRegistry,
) -> Vec<ComponentRegistryEntry> {
    let mut entries = generated_component_registry();
    let type_registry = app_type_registry.read();
    for entry in &mut entries {
        entry.editor_schema = infer_component_editor_schema(&type_registry, entry.type_path);
    }
    entries
}
