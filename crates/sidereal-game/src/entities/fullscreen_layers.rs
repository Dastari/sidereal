use bevy::prelude::*;
use uuid::Uuid;

use crate::{
    DisplayName, EntityGuid, FullscreenLayer, SpaceBackgroundShaderSettings,
    StarfieldShaderSettings,
};

use super::ship::corvette::{
    default_space_background_shader_asset_id, default_starfield_shader_asset_id,
};

pub const SPACE_BACKGROUND_LAYER_KIND: &str = "space_background";
pub const STARFIELD_LAYER_KIND: &str = "starfield";

pub const SPACE_BACKGROUND_LAYER_ORDER: i32 = -200;
pub const STARFIELD_LAYER_ORDER: i32 = -190;

pub const SPACE_BACKGROUND_BACKDROP_ENTITY_GUID: Uuid =
    Uuid::from_u128(0x0012_ebad_0000_0000_0000_0000_0000_0002);
pub const STARFIELD_BACKDROP_ENTITY_GUID: Uuid =
    Uuid::from_u128(0x0012_ebad_0000_0000_0000_0000_0000_0001);

#[derive(Bundle, Debug, Clone)]
pub struct SpaceBackgroundFullscreenLayerBundle {
    pub fullscreen_layer: FullscreenLayer,
    pub display_name: DisplayName,
    pub shader_settings: SpaceBackgroundShaderSettings,
    pub entity_guid: EntityGuid,
}

impl Default for SpaceBackgroundFullscreenLayerBundle {
    fn default() -> Self {
        Self {
            fullscreen_layer: FullscreenLayer {
                layer_kind: SPACE_BACKGROUND_LAYER_KIND.to_string(),
                shader_asset_id: default_space_background_shader_asset_id().to_string(),
                layer_order: SPACE_BACKGROUND_LAYER_ORDER,
            },
            display_name: DisplayName("SpaceBackground".into()),
            shader_settings: SpaceBackgroundShaderSettings::default(),
            entity_guid: EntityGuid(SPACE_BACKGROUND_BACKDROP_ENTITY_GUID),
        }
    }
}

#[derive(Bundle, Debug, Clone)]
pub struct StarfieldFullscreenLayerBundle {
    pub fullscreen_layer: FullscreenLayer,
    pub display_name: DisplayName,
    pub shader_settings: StarfieldShaderSettings,
    pub entity_guid: EntityGuid,
}

impl Default for StarfieldFullscreenLayerBundle {
    fn default() -> Self {
        Self {
            fullscreen_layer: FullscreenLayer {
                layer_kind: STARFIELD_LAYER_KIND.to_string(),
                shader_asset_id: default_starfield_shader_asset_id().to_string(),
                layer_order: STARFIELD_LAYER_ORDER,
            },
            display_name: DisplayName("StarField".into()),
            shader_settings: StarfieldShaderSettings::default(),
            entity_guid: EntityGuid(STARFIELD_BACKDROP_ENTITY_GUID),
        }
    }
}
