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

#[derive(Debug, Resource, Clone)]
pub(crate) struct AudioSettings {
    pub initialized_catalog_version: Option<String>,
    pub master_volume_db: f32,
    pub master_muted: bool,
    pub master_low_pass_hz: Option<f64>,
    pub buses: HashMap<String, AudioBusSettings>,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            initialized_catalog_version: None,
            master_volume_db: 0.0,
            master_muted: false,
            master_low_pass_hz: None,
            buses: HashMap::new(),
        }
    }
}
