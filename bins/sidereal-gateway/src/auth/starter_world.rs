use async_trait::async_trait;
use sidereal_persistence::GraphPersistence;
use std::collections::HashSet;
use std::env;
use tracing::info;
use uuid::Uuid;

use crate::auth::error::AuthError;
use crate::auth::starter_world_scripts::{
    ScriptContext, load_bundle_registry, load_graph_records_for_bundle, load_new_account_config,
    load_world_init_graph_records, scripts_root_dir,
};

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
    info!(
        "gateway starter world persistence begin account_id={} player_entity_id={}",
        account_id, player_entity_id
    );
    let scripts_root = scripts_root_dir();
    let bundle_registry = load_bundle_registry(&scripts_root)?;
    let new_account_config = load_new_account_config(
        &scripts_root,
        ScriptContext {
            account_id,
            player_entity_id,
            email,
        },
    )?;

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

    let existing_ids = records
        .iter()
        .map(|record| record.entity_id.clone())
        .collect::<HashSet<_>>();
    let world_init_records = load_world_init_graph_records(&scripts_root)?;
    let missing_world_records = world_init_records
        .into_iter()
        .filter(|record| !existing_ids.contains(&record.entity_id))
        .collect::<Vec<_>>();
    if !missing_world_records.is_empty() {
        info!(
            "gateway persisting {} missing world init records from scripted config",
            missing_world_records.len()
        );
        persistence
            .persist_graph_records(&missing_world_records, 0)
            .map_err(|err| {
                AuthError::Internal(format!("persist world init records failed: {err}"))
            })?;
    } else {
        info!("gateway world init records already present; no world init writes needed");
    }

    if records
        .iter()
        .any(|record| record.entity_id == player_entity_id)
    {
        return Err(AuthError::Internal(format!(
            "register invariant violation: player entity {player_entity_id} already exists in graph persistence"
        )));
    }
    let graph_records = match new_account_config.starter_bundle_id.as_str() {
        _ => {
            let Some(selected_bundle) = bundle_registry
                .bundles
                .get(&new_account_config.starter_bundle_id)
            else {
                return Err(AuthError::Internal(format!(
                    "accounts/on_new_account.lua selected starter_bundle_id={} missing from bundles/bundle_registry.lua",
                    new_account_config.starter_bundle_id
                )));
            };

            info!(
                "gateway starter bundle selected {} (scripted graph records) for account_id={} player_entity_id={}",
                selected_bundle.bundle_id, account_id, player_entity_id
            );
            load_graph_records_for_bundle(
                &scripts_root,
                selected_bundle,
                ScriptContext {
                    account_id,
                    player_entity_id,
                    email,
                },
            )?
        }
    };
    persistence
        .persist_graph_records(&graph_records, 0)
        .map_err(|err| AuthError::Internal(format!("persist starter world failed: {err}")))?;
    info!(
        "gateway starter world persistence complete account_id={} player_entity_id={} records={}",
        account_id,
        player_entity_id,
        graph_records.len()
    );
    Ok(())
}
