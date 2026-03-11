//! Runtime bootstrap bridge.
//!
//! This module is binary-only Bevy integration that receives bootstrap UDP
//! messages and forwards entity-binding commands into the replication world.

use bevy::log::{error, info, warn};
use bevy::prelude::{Commands, Resource};
use sidereal_replication::bootstrap::{
    BootstrapProcessor, ControlHandleResult, PostgresBootstrapStore,
};
use std::net::UdpSocket;
use std::sync::{Mutex, mpsc};
use std::thread;
use uuid::Uuid;

/// Channel for bootstrap thread to request entity binding in the Bevy world.
#[derive(Resource)]
pub struct BootstrapEntityReceiver(pub Mutex<mpsc::Receiver<BootstrapEntityCommand>>);

#[derive(Debug, Clone)]
pub struct BootstrapEntityCommand {
    pub payload: BootstrapEntityCommandPayload,
}

#[derive(Debug, Clone)]
pub enum BootstrapEntityCommandPayload {
    BootstrapPlayer {
        player_entity_id: String,
    },
    AdminSpawnEntity {
        actor_account_id: Uuid,
        actor_player_entity_id: String,
        request_id: Uuid,
        player_entity_id: String,
        bundle_id: String,
        requested_entity_id: String,
        overrides: serde_json::Map<String, serde_json::Value>,
    },
}

pub fn start_replication_control_listener(mut commands: Commands<'_, '_>) {
    let bind_addr = std::env::var("REPLICATION_CONTROL_UDP_BIND")
        .unwrap_or_else(|_| "127.0.0.1:9004".to_string());
    let database_url = std::env::var("REPLICATION_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());

    let socket = match UdpSocket::bind(&bind_addr) {
        Ok(socket) => socket,
        Err(err) => {
            error!("failed to bind replication control UDP listener on {bind_addr}: {err}");
            return;
        }
    };
    let store = match PostgresBootstrapStore::connect(&database_url) {
        Ok(store) => store,
        Err(err) => {
            error!("failed to connect replication bootstrap store: {err}");
            return;
        }
    };
    let mut processor = match BootstrapProcessor::new(store) {
        Ok(processor) => processor,
        Err(err) => {
            error!("failed to initialize replication bootstrap processor: {err}");
            return;
        }
    };

    let (tx, rx) = mpsc::channel::<BootstrapEntityCommand>();
    commands.insert_resource(BootstrapEntityReceiver(Mutex::new(rx)));

    info!("replication control UDP listening on {}", bind_addr);
    thread::spawn(move || {
        loop {
            let mut buf = vec![0_u8; 8192];
            let (size, from) = match socket.recv_from(&mut buf) {
                Ok(v) => v,
                Err(err) => {
                    warn!("replication control recv error: {err}");
                    continue;
                }
            };
            let payload = &buf[..size];
            match processor.handle_payload(payload) {
                Ok(ControlHandleResult::Bootstrap(result)) => {
                    info!(
                        "replication bootstrap processed from {from}: account_id={}, player_entity_id={}, applied={}",
                        result.account_id, result.player_entity_id, result.applied
                    );
                    let _ = tx.send(BootstrapEntityCommand {
                        payload: BootstrapEntityCommandPayload::BootstrapPlayer {
                            player_entity_id: result.player_entity_id,
                        },
                    });
                }
                Ok(ControlHandleResult::AdminSpawn(result)) => {
                    info!(
                        "replication admin spawn command accepted from {from}: request_id={} actor_account_id={} target_player_entity_id={} bundle_id={} requested_entity_id={}",
                        result.request_id,
                        result.actor_account_id,
                        result.player_entity_id,
                        result.bundle_id,
                        result.requested_entity_id
                    );
                    let _ = tx.send(BootstrapEntityCommand {
                        payload: BootstrapEntityCommandPayload::AdminSpawnEntity {
                            actor_account_id: result.actor_account_id,
                            actor_player_entity_id: result.actor_player_entity_id,
                            request_id: result.request_id,
                            player_entity_id: result.player_entity_id,
                            bundle_id: result.bundle_id,
                            requested_entity_id: result.requested_entity_id,
                            overrides: result.overrides,
                        },
                    });
                }
                Err(err) => {
                    warn!("replication control message rejected from {from}: {err}");
                }
            }
        }
    });
}

pub fn drain_bootstrap_entity_commands(
    receiver: &BootstrapEntityReceiver,
) -> Vec<BootstrapEntityCommand> {
    let Ok(rx) = receiver.0.lock() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    while let Ok(cmd) = rx.try_recv() {
        out.push(cmd);
    }
    out
}
