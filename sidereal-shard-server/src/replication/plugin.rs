use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::renet2::RenetClient;
use tracing::{info, warn, error, debug};
use serde::Serialize;
use bevy_rapier2d::prelude::Velocity;

use crate::config::{ShardConfig, ShardState};
use super::client::{ShardConnectionState, HandshakeTracker, EntityChangeTracker};
use sidereal_core::ecs::components::spatial::SpatialTracked;
use sidereal_core::ecs::plugins::replication::{
    common::{ClientStreamEvent, ReplicationClientStatus},
};
use sidereal_core::ecs::plugins::replication::client::ReplicationClient;

/// Plugin for the shard server's use of the replication client
pub struct ReplicationPlugin;

impl Plugin for ReplicationPlugin {
    fn build(&self, app: &mut App) {
        info!("Building shard replication plugin");
        
        // Add the core replication client plugin for connection management
        app.add_plugins(sidereal_core::ReplicationClientPlugin);
        
        // Initialize our shard-specific tracking resources
        app.init_resource::<HandshakeTracker>();
        app.init_resource::<EntityChangeTracker>();
        app.init_resource::<ShardConnectionState>();
        
        // Add systems that integrate with the core library's replication
        app.add_systems(Update, (
            // Monitor connection state
            monitor_connection_state,
            // Entity updates only when in Ready state
            send_entity_updates.run_if(in_state(ShardState::Ready)),
        ));
        
        info!("Shard replication plugin ready - connection will be managed by core library");
    }
}

/// Monitors replication connection state and updates shard state
fn monitor_connection_state(
    replication_client: Res<ReplicationClient>,
    mut next_state: ResMut<NextState<ShardState>>,
    mut connection_state: ResMut<ShardConnectionState>,
) {
    // When the core library's ReplicationClient is connected or authenticated
    if replication_client.status == ReplicationClientStatus::Connected || 
       replication_client.status == ReplicationClientStatus::Authenticated {
        // Update our local state
        if connection_state.status != ReplicationClientStatus::Authenticated {
            info!("Connection to replication server established");
            connection_state.status = ReplicationClientStatus::Authenticated;
            // Transition the shard to Ready state
            next_state.set(ShardState::Ready);
            info!("Shard state changed to Ready");
        }
    } else if connection_state.status == ReplicationClientStatus::Authenticated {
        // We were authenticated but now disconnected
        connection_state.status = replication_client.status;
        warn!("Connection to replication server lost (status: {:?})", replication_client.status);
    }
}

/// Send entity updates to the replication server
fn send_entity_updates(
    time: Res<Time>,
    query: Query<(Entity, &Transform, Option<&Velocity>), With<SpatialTracked>>,
    mut client_stream: EventWriter<ClientStreamEvent>,
    mut replication_client: ResMut<ReplicationClient>,
) {
    // Check if it's time to send updates
    if !replication_client.should_send_entity_updates(&time, 0.1) {
        return;
    }
    
    // Collect entities that need updates
    let mut updates = Vec::new();
    
    for (entity, transform, velocity) in query.iter() {
        // Get velocity as Vec2
        let vel = velocity.map_or(bevy::math::Vec2::ZERO, |v| bevy::math::Vec2::new(v.linvel.x, v.linvel.y));
        
        // Using the core library's entity tracking
        if replication_client.entity_needs_update(entity, transform, vel) {
            // Create entity update data
            let position = bevy::math::Vec2::new(transform.translation.x, transform.translation.y);
            
            // Add to updates
            updates.push((entity, position, vel));
        }
    }
    
    // Send updates if we have any
    if !updates.is_empty() {
        info!("Sending {} entity updates to replication server", updates.len());
        
        // Create a batch update event
        let update_event = EntityUpdateBatch {
            timestamp: time.elapsed_secs_f64(),
            entities: updates.iter().map(|(entity, pos, vel)| {
                EntityUpdate {
                    entity: *entity,
                    position: *pos,
                    velocity: *vel,
                }
            }).collect(),
        };
        
        // Send the update through the client stream
        client_stream.send(ClientStreamEvent {
            event_type: "entity_updates".to_string(),
            data: serde_json::to_string(&update_event).unwrap_or_default(),
        });
        
        // Update the entity update time in the core client
        replication_client.update_entity_update_time(&time);
    }
}

// Entity update data structures
#[derive(Serialize)]
struct EntityUpdateBatch {
    timestamp: f64,
    entities: Vec<EntityUpdate>,
}

#[derive(Serialize)]
struct EntityUpdate {
    entity: Entity,
    position: bevy::math::Vec2,
    velocity: bevy::math::Vec2,
}