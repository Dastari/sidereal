use async_trait::async_trait;
use serde_json::Value as JsonValue;
use sidereal_persistence::{GraphEntityRecord, GraphPersistence};
use std::env;
use tracing::info;
use uuid::Uuid;

use crate::auth::error::AuthError;
use crate::auth::starter_world_scripts::{
    ScriptContext, load_bundle_registry, load_graph_records_for_bundle, load_player_init_config,
    scripts_root_dir,
};

#[async_trait]
pub trait StarterWorldPersister: Send + Sync {
    async fn persist_starter_world(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
        email: &str,
    ) -> Result<(), AuthError>;

    async fn remove_character_world(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
    ) -> Result<(), AuthError>;

    async fn reset_character_world(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
        email: &str,
    ) -> Result<(), AuthError> {
        self.remove_character_world(account_id, player_entity_id)
            .await?;
        self.persist_starter_world(account_id, player_entity_id, email)
            .await
    }
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

    async fn remove_character_world(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
    ) -> Result<(), AuthError> {
        let player_entity_id = player_entity_id.to_string();
        tokio::task::spawn_blocking(move || {
            remove_character_world_records(account_id, &player_entity_id)
        })
        .await
        .map_err(|err| AuthError::Internal(format!("starter world removal task failed: {err}")))?
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

    async fn remove_character_world(
        &self,
        _account_id: Uuid,
        _player_entity_id: &str,
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
    let player_init_config = load_player_init_config(
        &scripts_root,
        ScriptContext {
            account_id,
            player_entity_id,
            email,
            controlled_entity_guid: None,
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

    if records
        .iter()
        .any(|record| record.entity_id == player_entity_id)
    {
        return Err(AuthError::Internal(format!(
            "register invariant violation: player entity {player_entity_id} already exists in graph persistence"
        )));
    }
    let Some(selected_bundle) = bundle_registry
        .bundles
        .get(&player_init_config.controlled_bundle_id)
    else {
        return Err(AuthError::Internal(format!(
            "accounts/player_init.lua selected controlled_bundle_id={} missing from bundles/bundle_registry.lua",
            player_init_config.controlled_bundle_id
        )));
    };

    if selected_bundle.bundle_class != "controllable" {
        return Err(AuthError::Internal(format!(
            "accounts/player_init.lua selected bundle_id={} bundle_class={} (expected controllable)",
            selected_bundle.bundle_id, selected_bundle.bundle_class
        )));
    }

    info!(
        "gateway starter controlled bundle selected {} (scripted graph records) for account_id={} player_entity_id={}",
        selected_bundle.bundle_id, account_id, player_entity_id
    );
    let controlled_graph_records = load_graph_records_for_bundle(
        &scripts_root,
        selected_bundle,
        ScriptContext {
            account_id,
            player_entity_id,
            email,
            controlled_entity_guid: None,
        },
    )?;
    let controlled_entity_id = resolve_controlled_entity_id(
        &controlled_graph_records,
        selected_bundle.bundle_id.as_str(),
    )?;

    let Some(player_bundle) = bundle_registry
        .bundles
        .get(&player_init_config.player_bundle_id)
    else {
        return Err(AuthError::Internal(format!(
            "accounts/player_init.lua selected player_bundle_id={} missing from bundles/bundle_registry.lua",
            player_init_config.player_bundle_id
        )));
    };
    if player_bundle.bundle_class != "player" {
        return Err(AuthError::Internal(format!(
            "accounts/player_init.lua selected player bundle_id={} bundle_class={} (expected player)",
            player_bundle.bundle_id, player_bundle.bundle_class
        )));
    }
    let mut graph_records = load_graph_records_for_bundle(
        &scripts_root,
        player_bundle,
        ScriptContext {
            account_id,
            player_entity_id,
            email,
            controlled_entity_guid: Some(&controlled_entity_id),
        },
    )?;
    graph_records.extend(controlled_graph_records);
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

pub fn remove_character_world_records(
    account_id: Uuid,
    player_entity_id: &str,
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
    let account_id = account_id.to_string();
    let mut entity_ids = records
        .iter()
        .filter(|record| character_world_record_matches(record, player_entity_id, &account_id))
        .map(|record| record.entity_id.clone())
        .collect::<Vec<_>>();
    entity_ids.sort();
    entity_ids.dedup();
    persistence
        .remove_graph_entities(&entity_ids)
        .map_err(|err| {
            AuthError::Internal(format!("remove character graph records failed: {err}"))
        })?;
    info!(
        "gateway character world records removed account_id={} player_entity_id={} records={}",
        account_id,
        player_entity_id,
        entity_ids.len()
    );
    Ok(())
}

fn character_world_record_matches(
    record: &GraphEntityRecord,
    player_entity_id: &str,
    account_id: &str,
) -> bool {
    record.entity_id == player_entity_id
        || record.components.iter().any(|component| {
            (component.component_kind == "owner_id"
                && component_value_equals(&component.properties, player_entity_id))
                || (component.component_kind == "account_id"
                    && component_value_equals(&component.properties, account_id))
        })
}

fn component_value_equals(value: &JsonValue, expected: &str) -> bool {
    match value {
        JsonValue::String(value) => value == expected,
        JsonValue::Object(object) => object
            .get("value")
            .and_then(JsonValue::as_str)
            .is_some_and(|value| value == expected),
        _ => false,
    }
}

fn resolve_controlled_entity_id(
    records: &[GraphEntityRecord],
    bundle_id: &str,
) -> Result<String, AuthError> {
    let mut marked_records = records
        .iter()
        .filter(|record| {
            record
                .components
                .iter()
                .any(|component| component.component_kind == "controlled_start_target")
        })
        .map(|record| record.entity_id.clone());

    let Some(controlled_entity_id) = marked_records.next() else {
        return Err(AuthError::Internal(format!(
            "bundle {} returned no controlled_start_target component; cannot resolve controlled_entity_guid",
            bundle_id
        )));
    };

    if marked_records.next().is_some() {
        return Err(AuthError::Internal(format!(
            "bundle {} returned multiple controlled_start_target components; starter control target is ambiguous",
            bundle_id
        )));
    }

    Ok(controlled_entity_id)
}

#[cfg(test)]
mod tests {
    use super::resolve_controlled_entity_id;
    use serde_json::json;
    use sidereal_persistence::{GraphComponentRecord, GraphEntityRecord};

    fn graph_record(entity_id: &str, component_kinds: &[&str]) -> GraphEntityRecord {
        GraphEntityRecord {
            entity_id: entity_id.to_string(),
            labels: vec!["Entity".to_string()],
            properties: json!({}),
            components: component_kinds
                .iter()
                .map(|component_kind| GraphComponentRecord {
                    component_id: format!("{entity_id}:{component_kind}"),
                    component_kind: (*component_kind).to_string(),
                    properties: json!({}),
                })
                .collect(),
        }
    }

    #[test]
    fn controlled_entity_resolves_from_marker_component_not_labels() {
        let records = vec![
            graph_record("module-a", &["display_name"]),
            graph_record("controlled-a", &["display_name", "controlled_start_target"]),
        ];

        let resolved = resolve_controlled_entity_id(&records, "test.bundle").expect("resolve");

        assert_eq!(resolved, "controlled-a");
    }

    #[test]
    fn controlled_entity_requires_exactly_one_marker_component() {
        let missing = resolve_controlled_entity_id(
            &[graph_record(
                "ship-looking-label-is-not-enough",
                &["display_name"],
            )],
            "test.bundle",
        )
        .expect_err("missing marker should fail");
        assert!(
            missing
                .to_string()
                .contains("no controlled_start_target component")
        );

        let duplicate = resolve_controlled_entity_id(
            &[
                graph_record("controlled-a", &["controlled_start_target"]),
                graph_record("controlled-b", &["controlled_start_target"]),
            ],
            "test.bundle",
        )
        .expect_err("duplicate marker should fail");
        assert!(
            duplicate
                .to_string()
                .contains("multiple controlled_start_target")
        );
    }
}
