use async_trait::async_trait;
use sidereal_core::bootstrap_wire::{BOOTSTRAP_KIND, BootstrapCommand, BootstrapWireMessage};
use sidereal_persistence::GraphPersistence;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

use crate::auth::error::AuthError;

#[async_trait]
pub trait BootstrapDispatcher: Send + Sync {
    async fn dispatch(&self, command: &BootstrapCommand) -> Result<(), AuthError>;
}

#[derive(Debug)]
pub struct UdpBootstrapDispatcher {
    socket: UdpSocket,
    target: SocketAddr,
}

impl UdpBootstrapDispatcher {
    pub fn new(socket: UdpSocket, target: SocketAddr) -> Self {
        Self { socket, target }
    }

    pub async fn from_env() -> Result<Self, AuthError> {
        let target_raw = std::env::var("REPLICATION_CONTROL_UDP_ADDR").map_err(|_| {
            AuthError::Config(
                "REPLICATION_CONTROL_UDP_ADDR is required for bootstrap handoff".to_string(),
            )
        })?;
        let target: SocketAddr = target_raw
            .parse()
            .map_err(|_| AuthError::Config("invalid REPLICATION_CONTROL_UDP_ADDR".to_string()))?;

        let bind = std::env::var("GATEWAY_REPLICATION_CONTROL_UDP_BIND")
            .unwrap_or_else(|_| "0.0.0.0:0".to_string());
        let socket = UdpSocket::bind(&bind)
            .await
            .map_err(|err| AuthError::Config(format!("udp bind failed: {err}")))?;

        Ok(Self { socket, target })
    }
}

#[derive(Debug, Clone)]
pub struct DirectBootstrapDispatcher {
    pub database_url: String,
}

impl DirectBootstrapDispatcher {
    pub fn from_env() -> Self {
        let database_url = std::env::var("GATEWAY_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());
        Self { database_url }
    }
}

#[async_trait]
impl BootstrapDispatcher for UdpBootstrapDispatcher {
    async fn dispatch(&self, command: &BootstrapCommand) -> Result<(), AuthError> {
        let payload = BootstrapWireMessage {
            kind: BOOTSTRAP_KIND.to_string(),
            account_id: command.account_id.to_string(),
            player_entity_id: command.player_entity_id.clone(),
        };
        let bytes = serde_json::to_vec(&payload)
            .map_err(|err| AuthError::Internal(format!("bootstrap serialize failed: {err}")))?;
        self.socket
            .send_to(&bytes, self.target)
            .await
            .map_err(|err| AuthError::Internal(format!("bootstrap send failed: {err}")))?;
        Ok(())
    }
}

#[async_trait]
impl BootstrapDispatcher for DirectBootstrapDispatcher {
    async fn dispatch(&self, command: &BootstrapCommand) -> Result<(), AuthError> {
        let database_url = self.database_url.clone();
        let command = command.clone();
        tokio::task::spawn_blocking(move || {
            let mut persistence = GraphPersistence::connect(&database_url)
                .map_err(|err| AuthError::Internal(format!("persistence connect failed: {err}")))?;
            persistence.ensure_schema().map_err(|err| {
                AuthError::Internal(format!("persistence ensure schema failed: {err}"))
            })?;
            let records = persistence
                .load_graph_records()
                .map_err(|err| AuthError::Internal(format!("load graph records failed: {err}")))?;
            if !records
                .iter()
                .any(|record| record.entity_id == command.player_entity_id)
            {
                return Err(AuthError::Internal(format!(
                    "bootstrap rejected: player entity {} not found in graph persistence",
                    command.player_entity_id
                )));
            }
            Ok::<_, AuthError>(())
        })
        .await
        .map_err(|err| AuthError::Internal(format!("bootstrap dispatch task failed: {err}")))?
    }
}

#[derive(Debug, Default)]
pub struct NoopBootstrapDispatcher;

#[async_trait]
impl BootstrapDispatcher for NoopBootstrapDispatcher {
    async fn dispatch(&self, _command: &BootstrapCommand) -> Result<(), AuthError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct RecordingBootstrapDispatcher {
    commands: Mutex<Vec<BootstrapCommand>>,
}

impl RecordingBootstrapDispatcher {
    pub async fn commands(&self) -> Vec<BootstrapCommand> {
        self.commands.lock().await.clone()
    }
}

#[async_trait]
impl BootstrapDispatcher for RecordingBootstrapDispatcher {
    async fn dispatch(&self, command: &BootstrapCommand) -> Result<(), AuthError> {
        self.commands.lock().await.push(command.clone());
        Ok(())
    }
}
