use crate::net::renet2_client::{ClientState, Renet2ClientConfig, Renet2ClientListener};
use bevy::prelude::*;
use sidereal::net::config::SHARD_CHANNEL_RELIABLE;
use sidereal::net::messages::ShardToReplicationMessage;
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
