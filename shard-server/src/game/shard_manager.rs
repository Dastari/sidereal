use crate::net::renet2_client::{ClientState, Renet2ClientConfig, Renet2ClientListener};
use bevy::prelude::*;
use sidereal::net::config::SHARD_CHANNEL_RELIABLE;
use sidereal::net::messages::{ReplicationToShardMessage, ShardToReplicationMessage};
pub struct ShardManagerPlugin;

impl Plugin for ShardManagerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(ClientState::Connected),
            send_shard_identification,
        );
    }
}

fn send_shard_identification(
    mut listener: ResMut<Renet2ClientListener>,
    config: Res<Renet2ClientConfig>,
    mut sent: Local<bool>,
) {
    let client = &mut listener.client;
    if !client.is_connected() {
        *sent = false;
        return;
    }

    if !*sent {
        info!(shard_id = %config.shard_id, "Sending shard identification to replication server");
        let message = ShardToReplicationMessage::IdentifyShard {
            shard_id: config.shard_id,
        };
        match bincode::serde::encode_to_vec(&message, bincode::config::standard()) {
            Ok(bytes) => {
                client.send_message(SHARD_CHANNEL_RELIABLE, bytes);
                *sent = true;
                info!("Shard identification sent.");
            }
            Err(e) => error!("Failed to serialize shard identification: {:?}", e),
        }
    }
}

fn send_load_stats(
    mut listener: ResMut<Renet2ClientListener>,

    time: Res<Time>,
    mut last_update: Local<f64>,
) {
    let Renet2ClientListener { client, transport } = listener.as_mut();
    if !client.is_connected() {
        return;
    }

    let current_time = time.elapsed().as_secs_f64();
    if current_time - *last_update < 10.0 {
        return;
    }
    *last_update = current_time;

    // Placeholder counts - replace with actual queries
    let entity_count = 100; // TODO: Replace with query.iter().count() or similar
    let player_count = 5; // TODO: Replace with query for players

    let message = ShardToReplicationMessage::ShardLoadUpdate {
        entity_count,
        player_count,
    };

    match bincode::serde::encode_to_vec(&message, bincode::config::standard()) {
        Ok(bytes) => {
            client.send_message(SHARD_CHANNEL_RELIABLE, bytes);
            debug!(
                "Sent load update (entities={}, players={})",
                entity_count, player_count
            );
        }
        Err(e) => error!("Failed to serialize load update: {:?}", e),
    }
}
