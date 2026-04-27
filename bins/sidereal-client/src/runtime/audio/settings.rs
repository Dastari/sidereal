use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AudioBusSettings {
    pub volume_db: f32,
    pub muted: bool,
}

impl Default for AudioBusSettings {
    fn default() -> Self {
        Self {
            volume_db: 0.0,
            muted: false,
        }
    }
}

#[derive(Debug, Resource, Clone, Default)]
pub(crate) struct AudioSettings {
    pub initialized_catalog_version: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    pub master_volume_db: f32,
    #[cfg(not(target_arch = "wasm32"))]
    pub master_muted: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub master_low_pass_hz: Option<f64>,
    pub buses: HashMap<String, AudioBusSettings>,
}
