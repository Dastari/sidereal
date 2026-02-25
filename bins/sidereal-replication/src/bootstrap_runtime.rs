use bevy::log::info;
use bevy::prelude::{Commands, Resource};
use sidereal_game::corvette_random_spawn_position;
use sidereal_replication::bootstrap::{BootstrapProcessor, PostgresBootstrapStore};
use std::net::UdpSocket;
use std::sync::{Mutex, mpsc};
use std::thread;

/// Channel for bootstrap thread to request ship spawning in the Bevy world.
#[derive(Resource)]
pub struct BootstrapShipReceiver(pub Mutex<mpsc::Receiver<BootstrapShipCommand>>);

#[derive(Debug, Clone)]
pub struct BootstrapShipCommand {
    pub account_id: uuid::Uuid,
    pub player_entity_id: String,
}

pub fn start_replication_control_listener(mut commands: Commands<'_, '_>) {
    let bind_addr = std::env::var("REPLICATION_CONTROL_UDP_BIND")
        .unwrap_or_else(|_| "127.0.0.1:9004".to_string());
    let database_url = std::env::var("REPLICATION_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());

    let socket = match UdpSocket::bind(&bind_addr) {
        Ok(socket) => socket,
        Err(err) => {
            eprintln!("failed to bind replication control UDP listener on {bind_addr}: {err}");
            return;
        }
    };
    let store = match PostgresBootstrapStore::connect(&database_url) {
        Ok(store) => store,
        Err(err) => {
            eprintln!("failed to connect replication bootstrap store: {err}");
            return;
        }
    };
    let mut processor = match BootstrapProcessor::new(store) {
        Ok(processor) => processor,
        Err(err) => {
            eprintln!("failed to initialize replication bootstrap processor: {err}");
            return;
        }
    };

    let (tx, rx) = mpsc::channel::<BootstrapShipCommand>();
    commands.insert_resource(BootstrapShipReceiver(Mutex::new(rx)));

    info!("replication control UDP listening on {}", bind_addr);
    thread::spawn(move || {
        loop {
            let mut buf = vec![0_u8; 8192];
            let (size, from) = match socket.recv_from(&mut buf) {
                Ok(v) => v,
                Err(err) => {
                    eprintln!("replication control recv error: {err}");
                    continue;
                }
            };
            let payload = &buf[..size];
            match processor.handle_payload(payload) {
                Ok(result) => {
                    println!(
                        "replication bootstrap processed from {from}: account_id={}, player_entity_id={}, applied={}",
                        result.account_id, result.player_entity_id, result.applied
                    );
                    let _ = tx.send(BootstrapShipCommand {
                        account_id: result.account_id,
                        player_entity_id: result.player_entity_id,
                    });
                }
                Err(err) => {
                    eprintln!("replication control message rejected from {from}: {err}");
                }
            }
        }
    });
}

pub fn drain_bootstrap_ship_commands(
    receiver: &BootstrapShipReceiver,
) -> Vec<BootstrapShipCommand> {
    let Ok(rx) = receiver.0.lock() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    while let Ok(cmd) = rx.try_recv() {
        out.push(cmd);
    }
    out
}

pub fn starter_spawn_position(account_id: uuid::Uuid) -> bevy::prelude::Vec3 {
    corvette_random_spawn_position(account_id)
}
