use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[sidereal_component_macros::sidereal_component(
    kind = "discovered_static_landmarks",
    persist = true,
    replicate = false,
    visibility = [OwnerOnly]
)]
#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DiscoveredStaticLandmarks {
    pub landmark_entity_ids: Vec<uuid::Uuid>,
}

impl DiscoveredStaticLandmarks {
    pub fn contains(&self, entity_id: uuid::Uuid) -> bool {
        self.landmark_entity_ids.contains(&entity_id)
    }

    pub fn insert(&mut self, entity_id: uuid::Uuid) -> bool {
        if self.contains(entity_id) {
            return false;
        }
        self.landmark_entity_ids.push(entity_id);
        true
    }
}
