use bevy::prelude::*;
use tracing::info;

use crate::scene::SceneState;
use sidereal_core::ecs::components::*;

/// Plugin for handling replication tasks
pub struct ReplicationPlugin;

impl Plugin for ReplicationPlugin {
    fn build(&self, app: &mut App) {
        info!("Building replication plugin");
        
        // Register replication events
        app.add_event::<ReplicationEvent>();
        
        // Add systems
        app.add_systems(Update, (
            heartbeat_system,
            handle_replication_events,
        ).run_if(in_state(SceneState::Ready)));
    }
}

/// Events for entity replication
#[derive(Event)]
pub enum ReplicationEvent {
    EntityUpdated {
        entity: Entity,
        cluster_id: uuid::Uuid,
    },
    EntityCreated {
        entity: Entity,
        cluster_id: uuid::Uuid,
    },
    EntityDeleted {
        entity: Entity,
        cluster_id: uuid::Uuid,
    },
}

/// Simple system to log heartbeat messages for the replication server
fn heartbeat_system() {
    static mut LAST_HEARTBEAT: Option<std::time::Instant> = None;
    
    let now = std::time::Instant::now();
    
    unsafe {
        if let Some(last) = LAST_HEARTBEAT {
            if now.duration_since(last).as_secs() >= 10 {
                info!("Replication server heartbeat");
                LAST_HEARTBEAT = Some(now);
            }
        } else {
            info!("Replication server started");
            LAST_HEARTBEAT = Some(now);
        }
    }
}

/// Handle replication events
fn handle_replication_events(
    mut events: EventReader<ReplicationEvent>,
) {
    for event in events.read() {
        match event {
            ReplicationEvent::EntityUpdated { entity, cluster_id } => {
                info!("Entity {:?} updated in cluster {}", entity, cluster_id);
                // In a real implementation, this would send the update to the appropriate shard
            },
            ReplicationEvent::EntityCreated { entity, cluster_id } => {
                info!("Entity {:?} created in cluster {}", entity, cluster_id);
                // In a real implementation, this would notify the shard of the new entity
            },
            ReplicationEvent::EntityDeleted { entity, cluster_id } => {
                info!("Entity {:?} deleted in cluster {}", entity, cluster_id);
                // In a real implementation, this would notify the shard to delete the entity
            },
        }
    }
} 