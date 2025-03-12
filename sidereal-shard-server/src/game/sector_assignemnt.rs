// Add this system to your client plugin:

use crate::ecs::systems::sectors::SectorCoord;
use std::collections::HashSet;

#[derive(Resource)]
pub struct ShardSectorAssignments {
    pub assigned_sectors: HashSet<SectorCoord>,
}

impl Default for ShardSectorAssignments {
    fn default() -> Self {
        Self {
            assigned_sectors: HashSet::new(),
        }
    }
}

// Add to your client plugin implementation:
// app.init_resource::<ShardSectorAssignments>();
// app.add_systems(Update, handle_sector_assignments);

pub fn handle_sector_assignments(
    mut client: ResMut<RenetClient>,
    mut assignments: ResMut<ShardSectorAssignments>,
    mut network_events: EventReader<NetworkMessageEvent>,
) {
    for event in network_events.read() {
        match &event.message {
            NetworkMessage::AssignSectors { sectors } => {
                info!("Received {} sector assignments", sectors.len());
                
                // Add new sectors to our assignment
                for sector in sectors {
                    assignments.assigned_sectors.insert(*sector);
                }
                
                // Confirm assignment to the server
                let confirm_message = bincode::encode_to_vec(
                    &NetworkMessage::SectorAssignmentConfirm { 
                        sectors: sectors.clone() 
                    },
                    bincode::config::standard()
                ).unwrap();
                
                client.send_message(DefaultChannel::ReliableOrdered, confirm_message);
            },
            NetworkMessage::RevokeSectors { sectors } => {
                info!("Server revoked {} sector assignments", sectors.len());
                
                // Remove sectors from our assignment
                for sector in sectors {
                    assignments.assigned_sectors.remove(sector);
                }
            },
            _ => {} // Ignore other messages
        }
    }
}

// System to periodically report load to the server
pub fn report_shard_load(
    mut client: ResMut<RenetClient>,
    time: Res<Time>,
    // Add resources that indicate load here
) {
    // Report load every few seconds
    // This is a simplified example - in practice, you'd measure actual load
    // based on entity count, physics calculations, etc.
    let load_factor = 0.5; // 50% load (example)
    
    let load_message = bincode::encode_to_vec(
        &NetworkMessage::SectorLoadReport { load_factor },
        bincode::config::standard()
    ).unwrap();
    
    client.send_message(DefaultChannel::ReliableOrdered, load_message);
}