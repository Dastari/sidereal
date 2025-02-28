use bevy::prelude::*;
use tracing::info;

use crate::scene::SceneState;

/// Plugin for handling replication tasks
pub struct ReplicationPlugin;

impl Plugin for ReplicationPlugin {
    fn build(&self, app: &mut App) {
        info!("Building replication plugin");
        
        app.add_systems(Update, heartbeat_system.run_if(in_state(SceneState::Ready)));
    }
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