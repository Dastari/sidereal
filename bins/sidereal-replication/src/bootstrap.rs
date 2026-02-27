use postgres::{Client, NoTls};
use serde::Deserialize;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

const BOOTSTRAP_KIND: &str = "bootstrap_player";
const AUTH_CHARACTERS_TABLE: &str = "auth_characters";

#[derive(Debug, Deserialize)]
pub struct BootstrapWireMessage {
    pub kind: String,
    pub account_id: String,
    pub player_entity_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapCommand {
    pub account_id: Uuid,
    pub player_entity_id: String,
}

impl TryFrom<BootstrapWireMessage> for BootstrapCommand {
    type Error = BootstrapError;

    fn try_from(value: BootstrapWireMessage) -> Result<Self, Self::Error> {
        if value.kind != BOOTSTRAP_KIND {
            return Err(BootstrapError::Validation(format!(
                "unknown bootstrap kind: {}",
                value.kind
            )));
        }
        let account_id = Uuid::parse_str(&value.account_id)
            .map_err(|_| BootstrapError::Validation("invalid account_id uuid".to_string()))?;
        if !value.player_entity_id.starts_with("player:")
            || value.player_entity_id.trim().len() <= "player:".len()
        {
            return Err(BootstrapError::Validation(
                "player_entity_id must be a non-empty player:<id> value".to_string(),
            ));
        }

        Ok(Self {
            account_id,
            player_entity_id: value.player_entity_id,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapHandleResult {
    pub account_id: Uuid,
    pub player_entity_id: String,
    pub applied: bool,
}

pub trait BootstrapStore {
    fn ensure_schema(&mut self) -> Result<(), BootstrapError>;
    fn apply_bootstrap_if_absent(
        &mut self,
        command: &BootstrapCommand,
    ) -> Result<bool, BootstrapError>;
}

pub struct BootstrapProcessor<S: BootstrapStore> {
    store: S,
}

impl<S: BootstrapStore> BootstrapProcessor<S> {
    pub fn new(mut store: S) -> Result<Self, BootstrapError> {
        store.ensure_schema()?;
        Ok(Self { store })
    }

    pub fn handle_payload(
        &mut self,
        payload: &[u8],
    ) -> Result<BootstrapHandleResult, BootstrapError> {
        let message: BootstrapWireMessage = serde_json::from_slice(payload)
            .map_err(|err| BootstrapError::Serialization(err.to_string()))?;
        let command = BootstrapCommand::try_from(message)?;
        let applied = self.store.apply_bootstrap_if_absent(&command)?;
        Ok(BootstrapHandleResult {
            account_id: command.account_id,
            player_entity_id: command.player_entity_id,
            applied,
        })
    }
}

pub struct PostgresBootstrapStore {
    client: Client,
}

impl PostgresBootstrapStore {
    pub fn connect(database_url: &str) -> Result<Self, BootstrapError> {
        let client = Client::connect(database_url, NoTls)
            .map_err(|err| BootstrapError::Storage(format!("postgres connect failed: {err}")))?;
        Ok(Self { client })
    }
}

impl BootstrapStore for PostgresBootstrapStore {
    fn ensure_schema(&mut self) -> Result<(), BootstrapError> {
        self.client
            .batch_execute(
                "
                CREATE TABLE IF NOT EXISTS replication_player_bootstrap (
                    player_entity_id TEXT PRIMARY KEY,
                    account_id UUID NOT NULL,
                    applied_at_epoch_s BIGINT NOT NULL
                );
                CREATE UNIQUE INDEX IF NOT EXISTS replication_player_bootstrap_player_entity_idx
                    ON replication_player_bootstrap (player_entity_id);

                CREATE TABLE IF NOT EXISTS replication_bootstrap_events (
                    event_id BIGSERIAL PRIMARY KEY,
                    account_id UUID NOT NULL,
                    player_entity_id TEXT NOT NULL,
                    applied BOOLEAN NOT NULL,
                    received_at_epoch_s BIGINT NOT NULL
                );
                ",
            )
            .map_err(|err| BootstrapError::Storage(format!("schema ensure failed: {err}")))
    }

    fn apply_bootstrap_if_absent(
        &mut self,
        command: &BootstrapCommand,
    ) -> Result<bool, BootstrapError> {
        let now = now_epoch_s() as i64;
        let mut tx = self
            .client
            .transaction()
            .map_err(|err| BootstrapError::Storage(format!("transaction begin failed: {err}")))?;

        let inserted = tx
            .query_opt(
                &format!(
                    "
                    SELECT 1 FROM {AUTH_CHARACTERS_TABLE}
                    WHERE account_id = $1 AND player_entity_id = $2
                    "
                ),
                &[&command.account_id, &command.player_entity_id],
            )
            .map_err(|err| {
                BootstrapError::Storage(format!("bootstrap ownership lookup failed: {err}"))
            })?
            .is_some();

        if !inserted {
            tx.rollback().map_err(|err| {
                BootstrapError::Storage(format!(
                    "transaction rollback failed after ownership mismatch: {err}"
                ))
            })?;
            return Err(BootstrapError::Validation(
                "bootstrap rejected: account does not own requested player_entity_id".to_string(),
            ));
        }

        let inserted = tx
            .query_opt(
                "
                INSERT INTO replication_player_bootstrap (account_id, player_entity_id, applied_at_epoch_s)
                VALUES ($1, $2, $3)
                ON CONFLICT (player_entity_id) DO NOTHING
                RETURNING player_entity_id
                ",
                &[&command.account_id, &command.player_entity_id, &now],
            )
            .map_err(|err| BootstrapError::Storage(format!("bootstrap upsert failed: {err}")))?
            .is_some();

        tx.execute(
            "
            INSERT INTO replication_bootstrap_events (account_id, player_entity_id, applied, received_at_epoch_s)
            VALUES ($1, $2, $3, $4)
            ",
            &[&command.account_id, &command.player_entity_id, &inserted, &now],
        )
        .map_err(|err| BootstrapError::Storage(format!("event insert failed: {err}")))?;

        tx.commit()
            .map_err(|err| BootstrapError::Storage(format!("transaction commit failed: {err}")))?;
        Ok(inserted)
    }
}

#[derive(Default)]
pub struct InMemoryBootstrapStore {
    applied_player_entities: HashSet<String>,
    events: Vec<BootstrapHandleResult>,
}

impl InMemoryBootstrapStore {
    pub fn events(&self) -> &[BootstrapHandleResult] {
        &self.events
    }
}

impl BootstrapStore for InMemoryBootstrapStore {
    fn ensure_schema(&mut self) -> Result<(), BootstrapError> {
        Ok(())
    }

    fn apply_bootstrap_if_absent(
        &mut self,
        command: &BootstrapCommand,
    ) -> Result<bool, BootstrapError> {
        let applied = self
            .applied_player_entities
            .insert(command.player_entity_id.clone());
        self.events.push(BootstrapHandleResult {
            account_id: command.account_id,
            player_entity_id: command.player_entity_id.clone(),
            applied,
        });
        Ok(applied)
    }
}

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Serialization(String),
    #[error("{0}")]
    Storage(String),
}

fn now_epoch_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs()
}
