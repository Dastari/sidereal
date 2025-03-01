use bevy::prelude::*;
use tracing::{info, warn};
use uuid::Uuid;
use std::collections::HashMap;
use bevy_rapier2d::prelude::Velocity;

use sidereal_core::ecs::components::*;

/// Plugin for managing shadow entities
pub struct ShadowEntityPlugin;

impl Plugin for ShadowEntityPlugin {
    fn build(&self, app: &mut App) {
        info!("Building shadow entity plugin");
        
        // Register components and resources
        app.init_resource::<ShadowEntityRegistry>()
           .register_type::<ShadowEntity>()
           .register_type::<VisualOnly>();
           
        // Add systems
        app.add_systems(Update, (
            update_shadow_entities,
            prune_outdated_shadows,
        ));
    }
}

/// Shadow entity component for entities mirrored from neighboring shards
#[derive(Component, Reflect)]
pub struct ShadowEntity {
    pub source_cluster_id: Uuid,
    pub source_shard_id: Uuid,
    pub original_entity: Entity,
    pub last_updated: f64,
}

/// Marker component indicating entity is visual-only (no physics)
#[derive(Component, Reflect)]
pub struct VisualOnly;

/// Resource for tracking shadow entities
#[derive(Resource)]
pub struct ShadowEntityRegistry {
    // Maps original entity ID to local shadow entity
    entity_map: HashMap<Entity, ShadowEntityInfo>,
}

#[derive(Clone)]
pub struct ShadowEntityInfo {
    pub local_entity: Entity,
    pub source_shard_id: Uuid,
    pub last_updated: f64,
}

impl Default for ShadowEntityRegistry {
    fn default() -> Self {
        Self {
            entity_map: HashMap::new(),
        }
    }
}

impl ShadowEntityRegistry {
    pub fn register(&mut self, original_id: Entity, local_entity: Entity, source_shard_id: Uuid, timestamp: f64) {
        self.entity_map.insert(original_id, ShadowEntityInfo {
            local_entity,
            source_shard_id,
            last_updated: timestamp,
        });
    }
    
    pub fn get(&self, original_id: &Entity) -> Option<&ShadowEntityInfo> {
        self.entity_map.get(original_id)
    }
    
    pub fn update_timestamp(&mut self, original_id: &Entity, timestamp: f64) {
        if let Some(info) = self.entity_map.get_mut(original_id) {
            info.last_updated = timestamp;
        }
    }
    
    pub fn get_all(&self) -> impl Iterator<Item = (&Entity, &ShadowEntityInfo)> {
        self.entity_map.iter()
    }
    
    pub fn get_outdated(&self, cutoff_time: f64) -> Vec<Entity> {
        self.entity_map
            .iter()
            .filter(|(_, info)| info.last_updated < cutoff_time)
            .map(|(original_id, info)| info.local_entity)
            .collect()
    }
    
    pub fn remove(&mut self, original_id: &Entity) -> Option<ShadowEntityInfo> {
        self.entity_map.remove(original_id)
    }
}

/// System to update shadow entities (visual positioning)
fn update_shadow_entities(
    mut query: Query<
        (&mut Transform, &Velocity, &ShadowEntity),
        With<VisualOnly>
    >,
    time: Res<Time>,
) {
    // Basic prediction algorithm - move shadow entities based on their last known velocity
    // In a production system, this would be interpolation between known states
    
    let dt = time.delta_secs();
    
    for (mut transform, velocity, _shadow) in query.iter_mut() {
        // Simple position prediction based on velocity
        let movement = velocity.linvel * dt;
        transform.translation.x += movement.x;
        transform.translation.y += movement.y;
    }
}

/// Remove shadow entities that haven't been updated in the timeout period
fn prune_outdated_shadows(
    mut commands: Commands,
    mut registry: ResMut<ShadowEntityRegistry>,
    time: Res<Time>,
) {
    // Timeout in seconds
    const SHADOW_TIMEOUT: f64 = 5.0;
    
    let current_time = time.elapsed_secs_f64();
    
    // Find all shadow entities that haven't been updated within the timeout period
    let outdated = registry.get_outdated(current_time - SHADOW_TIMEOUT);
    
    // Remove each outdated entity
    for entity in outdated {
        info!("Removing outdated shadow entity: {:?}", entity);
        commands.entity(entity).despawn();
        registry.remove(&entity);
    }
} 