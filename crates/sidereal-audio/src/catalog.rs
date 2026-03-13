use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AudioRegistry {
    pub schema_version: u32,
    #[serde(default)]
    pub buses: Vec<AudioBusDefinition>,
    #[serde(default)]
    pub sends: Vec<AudioSendDefinition>,
    #[serde(default)]
    pub environments: Vec<AudioEnvironmentDefinition>,
    #[serde(default)]
    pub concurrency_groups: Vec<AudioConcurrencyGroup>,
    #[serde(default)]
    pub profiles: Vec<AudioProfileDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioBusDefinition {
    pub bus_id: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub default_volume_db: Option<f32>,
    #[serde(default)]
    pub muted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioSendDefinition {
    pub send_id: String,
    #[serde(default)]
    pub effects: Vec<AudioEffectDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioEnvironmentDefinition {
    pub environment_id: String,
    #[serde(default)]
    pub bus_overrides: BTreeMap<String, AudioBusEnvironmentOverride>,
    #[serde(default)]
    pub send_level_db: BTreeMap<String, f32>,
    #[serde(default)]
    pub bus_effect_overrides: BTreeMap<String, Vec<AudioEffectDefinition>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AudioBusEnvironmentOverride {
    #[serde(default)]
    pub volume_db: Option<f32>,
    #[serde(default)]
    pub muted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioConcurrencyGroup {
    pub group_id: String,
    pub max_instances: u32,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioProfileDefinition {
    pub profile_id: String,
    pub kind: String,
    #[serde(default)]
    pub cues: BTreeMap<String, AudioCueDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioCueDefinition {
    pub playback: AudioPlaybackDefinition,
    pub route: AudioRouteDefinition,
    pub spatial: AudioSpatialDefinition,
    #[serde(default)]
    pub concurrency: Option<AudioCueConcurrency>,
    #[serde(default)]
    pub ducking: Option<AudioCueDucking>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioPlaybackDefinition {
    pub kind: String,
    #[serde(default)]
    pub clip_asset_id: Option<String>,
    #[serde(default)]
    pub variants: Vec<AudioWeightedClipRef>,
    #[serde(default)]
    pub intro_start_s: Option<f32>,
    #[serde(default)]
    pub loop_start_s: Option<f32>,
    #[serde(default)]
    pub loop_end_s: Option<f32>,
    #[serde(default)]
    pub outro_start_s: Option<f32>,
    #[serde(default)]
    pub clip_end_s: Option<f32>,
    #[serde(default)]
    pub loop_region: Option<AudioLoopRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioWeightedClipRef {
    pub clip_asset_id: String,
    #[serde(default = "default_weight")]
    pub weight: f32,
}

fn default_weight() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioLoopRegion {
    pub start_s: f32,
    pub end_s: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioRouteDefinition {
    pub bus: String,
    #[serde(default)]
    pub sends: Vec<AudioSendLevelDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioSendLevelDefinition {
    pub send_id: String,
    pub level_db: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioSpatialDefinition {
    pub mode: String,
    #[serde(default)]
    pub min_distance_m: Option<f32>,
    #[serde(default)]
    pub max_distance_m: Option<f32>,
    #[serde(default)]
    pub rolloff: Option<String>,
    #[serde(default)]
    pub pan_strength: Option<f32>,
    #[serde(default)]
    pub distance_lowpass: Option<AudioDistanceLowpassDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioDistanceLowpassDefinition {
    pub enabled: bool,
    #[serde(default)]
    pub near_hz: Option<f32>,
    #[serde(default)]
    pub far_hz: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioCueConcurrency {
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub steal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioCueDucking {
    #[serde(default)]
    pub music_db: Option<f32>,
    #[serde(default)]
    pub sfx_db: Option<f32>,
    #[serde(default)]
    pub tween_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioEffectDefinition {
    pub kind: String,
    #[serde(flatten)]
    pub params: BTreeMap<String, JsonValue>,
}
