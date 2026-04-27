use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use sidereal_audio::AudioCueDefinition;
use sidereal_audio::{AudioProfileDefinition, AudioRegistry};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Resource, Clone, Default)]
pub(crate) struct AudioCatalogState {
    pub version: Option<String>,
    registry: Option<AudioRegistry>,
    profiles_by_id: HashMap<String, AudioProfileDefinition>,
    asset_ids_by_profile_id: HashMap<String, HashSet<String>>,
}

impl AudioCatalogState {
    pub fn apply_registry(&mut self, version: String, registry: AudioRegistry) {
        self.version = Some(version);
        self.asset_ids_by_profile_id.clear();
        self.profiles_by_id.clear();
        for profile in &registry.profiles {
            self.asset_ids_by_profile_id.insert(
                profile.profile_id.clone(),
                referenced_asset_ids_for_profile(profile),
            );
            self.profiles_by_id
                .insert(profile.profile_id.clone(), profile.clone());
        }
        self.registry = Some(registry);
    }

    pub fn registry(&self) -> Option<&AudioRegistry> {
        self.registry.as_ref()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn cue(&self, profile_id: &str, cue_id: &str) -> Option<&AudioCueDefinition> {
        self.profiles_by_id.get(profile_id)?.cues.get(cue_id)
    }

    pub fn profile_asset_ids(&self, profile_id: &str) -> Option<&HashSet<String>> {
        self.asset_ids_by_profile_id.get(profile_id)
    }
}

fn referenced_asset_ids_for_profile(profile: &AudioProfileDefinition) -> HashSet<String> {
    let mut asset_ids = HashSet::new();
    for cue in profile.cues.values() {
        if let Some(asset_id) = cue.playback.clip_asset_id.as_ref()
            && !asset_id.trim().is_empty()
        {
            asset_ids.insert(asset_id.clone());
        }
        for variant in &cue.playback.variants {
            if !variant.clip_asset_id.trim().is_empty() {
                asset_ids.insert(variant.clip_asset_id.clone());
            }
        }
    }
    asset_ids
}
