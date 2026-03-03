use async_trait::async_trait;
use sidereal_persistence::GraphPersistence;
use std::env;
use uuid::Uuid;

use crate::auth::error::AuthError;

#[async_trait]
pub trait StarterWorldPersister: Send + Sync {
    async fn persist_starter_world(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
        email: &str,
    ) -> Result<(), AuthError>;
}

pub struct GraphStarterWorldPersister;

#[async_trait]
impl StarterWorldPersister for GraphStarterWorldPersister {
    async fn persist_starter_world(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
        email: &str,
    ) -> Result<(), AuthError> {
        let player_entity_id = player_entity_id.to_string();
        let email = email.to_string();
        tokio::task::spawn_blocking(move || {
            persist_starter_world_for_new_account(account_id, &player_entity_id, &email)
        })
        .await
        .map_err(|err| {
            AuthError::Internal(format!("starter world persistence task failed: {err}"))
        })?
    }
}

pub struct NoopStarterWorldPersister;

#[async_trait]
impl StarterWorldPersister for NoopStarterWorldPersister {
    async fn persist_starter_world(
        &self,
        _account_id: Uuid,
        _player_entity_id: &str,
        _email: &str,
    ) -> Result<(), AuthError> {
        Ok(())
    }
}

pub fn persist_starter_world_for_new_account(
    account_id: Uuid,
    player_entity_id: &str,
    email: &str,
) -> Result<(), AuthError> {
    let database_url = env::var("GATEWAY_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());
    let mut persistence = GraphPersistence::connect(&database_url)
        .map_err(|err| AuthError::Internal(format!("persistence connect failed: {err}")))?;
    persistence
        .ensure_schema()
        .map_err(|err| AuthError::Internal(format!("persistence ensure schema failed: {err}")))?;
    let records = persistence
        .load_graph_records()
        .map_err(|err| AuthError::Internal(format!("load graph records failed: {err}")))?;
    if records
        .iter()
        .any(|record| record.entity_id == player_entity_id)
    {
        return Err(AuthError::Internal(format!(
            "register invariant violation: player entity {player_entity_id} already exists in graph persistence"
        )));
    }
    let position = sidereal_game::corvette_random_spawn_position(account_id);
    let graph_records = sidereal_runtime_sync::entity_templates::new_player_starter_graph_records(
        account_id,
        player_entity_id,
        email,
        position,
    );
    persistence
        .persist_graph_records(&graph_records, 0)
        .map_err(|err| AuthError::Internal(format!("persist starter world failed: {err}")))?;
    Ok(())
}
