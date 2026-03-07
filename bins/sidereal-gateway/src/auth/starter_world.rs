use async_trait::async_trait;
use serde_json::json;
use sidereal_persistence::{GraphComponentRecord, GraphEntityRecord, GraphPersistence};
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
    let player_init_config = load_player_init_config(
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
        .get(&player_init_config.ship_bundle_id)
    else {
        return Err(AuthError::Internal(format!(
            "accounts/player_init.lua selected ship_bundle_id={} missing from bundles/bundle_registry.lua",
            player_init_config.ship_bundle_id
        )));
    };

    if selected_bundle.bundle_class != "ship" {
        return Err(AuthError::Internal(format!(
            "accounts/player_init.lua selected bundle_id={} bundle_class={} (expected ship)",
            selected_bundle.bundle_id, selected_bundle.bundle_class
        )));
    }

    info!(
        "gateway starter ship bundle selected {} (scripted graph records) for account_id={} player_entity_id={}",
        selected_bundle.bundle_id, account_id, player_entity_id
    );
    let mut graph_records = load_graph_records_for_bundle(
        &scripts_root,
        selected_bundle,
        ScriptContext {
            account_id,
            player_entity_id,
            email,
        },
    )?;
    let controlled_entity_id =
        resolve_controlled_entity_id(&graph_records, selected_bundle.bundle_id.as_str())?;
    graph_records.insert(
        0,
        build_player_graph_record(player_entity_id, email, account_id, &controlled_entity_id),
    );
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

fn component(
    component_id: String,
    component_kind: &str,
    properties: serde_json::Value,
) -> GraphComponentRecord {
    GraphComponentRecord {
        component_id,
        component_kind: component_kind.to_string(),
        properties,
    }
}

fn build_player_graph_record(
    player_entity_id: &str,
    email: &str,
    account_id: Uuid,
    controlled_entity_id: &str,
) -> GraphEntityRecord {
    let component_id = |kind: &str| format!("{player_entity_id}:{kind}");
    GraphEntityRecord {
        entity_id: player_entity_id.to_string(),
        labels: vec!["Entity".to_string(), "Player".to_string()],
        properties: json!({}),
        components: vec![
            component(component_id("display_name"), "display_name", json!(email)),
            component(component_id("player_tag"), "player_tag", json!({})),
            component(
                component_id("account_id"),
                "account_id",
                json!(account_id.to_string()),
            ),
            component(
                component_id("controlled_entity_guid"),
                "controlled_entity_guid",
                json!(controlled_entity_id),
            ),
            component(
                component_id("entity_labels"),
                "entity_labels",
                json!(["Player"]),
            ),
            component(
                component_id("action_capabilities"),
                "action_capabilities",
                json!({
                    "supported": [
                        "Forward",
                        "Backward",
                        "LongitudinalNeutral",
                        "Left",
                        "Right",
                        "LateralNeutral",
                        "Brake",
                        "AfterburnerOn",
                        "AfterburnerOff",
                    ]
                }),
            ),
            component(
                component_id("character_movement_controller"),
                "character_movement_controller",
                json!({
                    "speed_mps": 220.0,
                    "max_accel_mps2": 880.0,
                    "damping_per_s": 8.0,
                }),
            ),
            component(
                component_id("action_queue"),
                "action_queue",
                json!({ "pending": ["LongitudinalNeutral", "LateralNeutral"] }),
            ),
            component(
                component_id("tactical_map_ui_settings"),
                "tactical_map_ui_settings",
                json!({
                    "map_distance_m": 90.0,
                    "map_zoom_wheel_sensitivity": 0.12,
                    "overlay_takeover_alpha": 0.995,
                    "grid_major_color_rgb": [0.22, 0.34, 0.48],
                    "grid_minor_color_rgb": [0.22, 0.34, 0.48],
                    "grid_micro_color_rgb": [0.22, 0.34, 0.48],
                    "grid_major_alpha": 0.14,
                    "grid_minor_alpha": 0.126,
                    "grid_micro_alpha": 0.113,
                    "grid_major_glow_alpha": 0.02,
                    "grid_minor_glow_alpha": 0.018,
                    "grid_micro_glow_alpha": 0.016,
                    "background_color_rgb": [0.005, 0.008, 0.02],
                    "line_width_major_px": 1.4,
                    "line_width_minor_px": 0.95,
                    "line_width_micro_px": 0.75,
                    "glow_width_major_px": 2.0,
                    "glow_width_minor_px": 1.5,
                    "glow_width_micro_px": 1.2,
                    "fx_mode": 1,
                    "fx_opacity": 0.45,
                    "fx_noise_amount": 0.12,
                    "fx_scanline_density": 360.0,
                    "fx_scanline_speed": 0.65,
                    "fx_crt_distortion": 0.02,
                    "fx_vignette_strength": 0.24,
                    "fx_green_tint_mix": 0.0,
                }),
            ),
            component(
                component_id("avian_position"),
                "avian_position",
                json!([0.0, 0.0]),
            ),
            component(
                component_id("avian_rotation"),
                "avian_rotation",
                json!({ "cos": 1.0, "sin": 0.0 }),
            ),
            component(
                component_id("avian_linear_velocity"),
                "avian_linear_velocity",
                json!([0.0, 0.0]),
            ),
            component(
                component_id("avian_rigid_body"),
                "avian_rigid_body",
                json!("Dynamic"),
            ),
            component(component_id("avian_mass"), "avian_mass", json!(1.0)),
            component(
                component_id("avian_angular_inertia"),
                "avian_angular_inertia",
                json!(1.0),
            ),
            component(
                component_id("avian_linear_damping"),
                "avian_linear_damping",
                json!(0.0),
            ),
            component(
                component_id("avian_angular_damping"),
                "avian_angular_damping",
                json!(0.0),
            ),
        ],
    }
}

fn resolve_controlled_entity_id(
    records: &[GraphEntityRecord],
    bundle_id: &str,
) -> Result<String, AuthError> {
    if let Some(record) = records
        .iter()
        .find(|record| record.labels.iter().any(|label| label == "Ship"))
    {
        return Ok(record.entity_id.clone());
    }
    records
        .first()
        .map(|record| record.entity_id.clone())
        .ok_or_else(|| {
            AuthError::Internal(format!(
                "bundle {} returned no graph records; cannot resolve controlled_entity_guid",
                bundle_id
            ))
        })
}
