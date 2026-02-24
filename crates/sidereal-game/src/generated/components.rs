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
    app.insert_resource(GeneratedComponentRegistry {
        entries: generated_component_registry(),
    });
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

    entries.sort_unstable_by(|a, b| {
        a.component_kind
            .cmp(b.component_kind)
            .then_with(|| a.type_path.cmp(b.type_path))
    });
    entries
}
