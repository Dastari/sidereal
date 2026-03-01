use crate::component_meta::VisibilityScope;
use bevy::prelude::*;

pub use crate::components::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentRegistryEntry {
    pub component_kind: &'static str,
    pub type_path: &'static str,
    pub replication_visibility: &'static [VisibilityScope],
}

#[derive(Debug, Resource, Clone)]
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

    app.insert_resource(GeneratedComponentRegistry {
        entries: generated_component_registry(),
    });
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
            replication_visibility: &[VisibilityScope::Public],
        },
        ComponentRegistryEntry {
            component_kind: "avian_rotation",
            type_path: leak(av::Rotation::type_path()),
            replication_visibility: &[VisibilityScope::Public],
        },
        ComponentRegistryEntry {
            component_kind: "avian_linear_velocity",
            type_path: leak(av::LinearVelocity::type_path()),
            replication_visibility: &[VisibilityScope::Public],
        },
        ComponentRegistryEntry {
            component_kind: "avian_angular_velocity",
            type_path: leak(av::AngularVelocity::type_path()),
            replication_visibility: &[VisibilityScope::Public],
        },
        ComponentRegistryEntry {
            component_kind: "avian_rigid_body",
            type_path: leak(av::RigidBody::type_path()),
            replication_visibility: &[VisibilityScope::Public],
        },
        ComponentRegistryEntry {
            component_kind: "avian_mass",
            type_path: leak(av::Mass::type_path()),
            replication_visibility: &[VisibilityScope::Public],
        },
        ComponentRegistryEntry {
            component_kind: "avian_angular_inertia",
            type_path: leak(av::AngularInertia::type_path()),
            replication_visibility: &[VisibilityScope::Public],
        },
        ComponentRegistryEntry {
            component_kind: "avian_linear_damping",
            type_path: leak(av::LinearDamping::type_path()),
            replication_visibility: &[VisibilityScope::Public],
        },
        ComponentRegistryEntry {
            component_kind: "avian_angular_damping",
            type_path: leak(av::AngularDamping::type_path()),
            replication_visibility: &[VisibilityScope::Public],
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
            replication_visibility: registration.meta.visibility,
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
