use bevy::{prelude::*, time::common_conditions::on_timer};
use bevy_renet::renet::*;
use sidereal_core::ecs::plugins::sectors::SectorCoord;
use sidereal_core::ecs::systems::network::{NetworkMessage, NetworkMessageEvent};
use std::collections::HashSet;
use std::time::Duration;

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

pub fn handle_sector_assignments(
    mut client: ResMut<RenetClient>,
    mut assignments: ResMut<ShardSectorAssignments>,
    mut network_events: EventReader<NetworkMessageEvent>,
) {
    for event in network_events.read() {
        match &event.message {
            NetworkMessage::AssignSectors { sectors } => {
                info!("Received {} sector assignments", sectors.len());
                for sector in sectors {
                    assignments.assigned_sectors.insert(*sector);
                }
                let confirm_message = bincode::encode_to_vec(
                    &NetworkMessage::SectorAssignmentConfirm {
                        sectors: sectors.clone(),
                    },
                    bincode::config::standard(),
                )
                .unwrap();
                client.send_message(DefaultChannel::ReliableOrdered, confirm_message);
            }
            NetworkMessage::RevokeSectors { sectors } => {
                info!("Server revoked {} sector assignments", sectors.len());
                for sector in sectors {
                    assignments.assigned_sectors.remove(sector);
                }
            }
            _ => {}
        }
    }
}

pub fn report_shard_load(mut client: ResMut<RenetClient>, _time: Res<Time>) {
    let load_factor = 0.5;
    let load_message = bincode::encode_to_vec(
        &NetworkMessage::SectorLoadReport { load_factor },
        bincode::config::standard(),
    )
    .unwrap();
    client.send_message(DefaultChannel::ReliableOrdered, load_message);
}

pub struct SectorAssignmentPlugin;

impl Plugin for SectorAssignmentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShardSectorAssignments>();
        app.add_systems(Update, handle_sector_assignments);
        app.add_systems(
            Update,
            report_shard_load.run_if(on_timer(Duration::from_secs(30))),
        );
    }
}
