use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Reflect, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TacticalKindIconBinding {
    pub kind: String,
    pub asset_id: String,
}

#[sidereal_component_macros::sidereal_component(
    kind = "tactical_presentation_defaults",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Eq, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct TacticalPresentationDefaults {
    #[serde(default)]
    pub default_map_icon_asset_id: Option<String>,
    #[serde(default)]
    pub icon_bindings_by_kind: Vec<TacticalKindIconBinding>,
}

impl TacticalPresentationDefaults {
    pub fn map_icon_asset_id_for_kind(&self, kind: Option<&str>) -> Option<&str> {
        if let Some(kind) = kind
            && let Some(binding) = self
                .icon_bindings_by_kind
                .iter()
                .find(|binding| binding.kind == kind)
        {
            return Some(binding.asset_id.as_str());
        }
        self.default_map_icon_asset_id.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::TacticalPresentationDefaults;

    #[test]
    fn tactical_presentation_defaults_deserializes_defaults() {
        let defaults = serde_json::from_str::<TacticalPresentationDefaults>(
            r#"{
                "default_map_icon_asset_id":"map_icon_ship_svg",
                "icon_bindings_by_kind":[
                    {"kind":"planet","asset_id":"map_icon_planet_svg"}
                ]
            }"#,
        )
        .expect("defaults");
        assert_eq!(
            defaults.map_icon_asset_id_for_kind(Some("planet")),
            Some("map_icon_planet_svg")
        );
        assert_eq!(
            defaults.map_icon_asset_id_for_kind(Some("unknown")),
            Some("map_icon_ship_svg")
        );
    }
}
