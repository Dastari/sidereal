//! Replication adoption, control sync, and prediction runtime state.

use avian2d::prelude::{LinearVelocity, Position, Rotation};
use bevy::ecs::query::Has;
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use lightyear::prediction::correction::CorrectionPolicy;
use lightyear::prediction::prelude::{PredictionManager, RollbackMode};
use lightyear::prelude::client::Client;
use sidereal_game::{
    ActionQueue, CollisionOutlineM, ControlledEntityGuid, EntityGuid, Hardpoint, MountedOn,
    PlayerTag, SizeM, SpriteShaderAssetId,
    VisualAssetId,
};
use sidereal_runtime_sync::{
    RuntimeEntityHierarchy, parse_guid_from_entity_id, register_runtime_entity,
};
use std::collections::HashSet;

use super::app_state::{ClientAppState, ClientSession, LocalPlayerViewState, SessionReadyState};
use super::components::{
    ControlledEntity, RemoteEntity, RemoteVisibleEntity, ReplicatedAdoptionHandled,
    StreamedSpriteShaderAssetId, StreamedVisualAssetId, StreamedVisualAttached, WorldEntity,
};
use super::resources::{
    BootstrapWatchdogState, DeferredPredictedAdoptionState, LocalSimulationDebugMode,
    PredictionBootstrapTuning, PredictionCorrectionTuning, PredictionRollbackStateTuning,
    RemoteEntityRegistry,
};

pub(crate) fn ensure_replicated_entity_spatial_components(
    mut commands: Commands<'_, '_>,
    missing_transform: Query<
        '_,
        '_,
        Entity,
        (With<lightyear::prelude::Replicated>, Without<Transform>),
    >,
    missing_visibility: Query<
        '_,
        '_,
        Entity,
        (With<lightyear::prelude::Replicated>, Without<Visibility>),
    >,
) {
    for entity in &missing_transform {
        commands.entity(entity).insert((
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
        ));
    }
    for entity in &missing_visibility {
        commands.entity(entity).insert(Visibility::default());
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn ensure_hierarchy_parent_spatial_components(
    mut commands: Commands<'_, '_>,
    children_with_parent: Query<'_, '_, &'_ ChildOf>,
    parent_components: Query<'_, '_, (Has<Transform>, Has<GlobalTransform>, Has<Visibility>)>,
) {
    let mut visited_parents = HashSet::<Entity>::new();
    for child_of in &children_with_parent {
        let entity = child_of.parent();
        if !visited_parents.insert(entity) {
            continue;
        }
        let Ok((has_transform, has_global_transform, has_visibility)) =
            parent_components.get(entity)
        else {
            continue;
        };
        if has_transform && has_global_transform && has_visibility {
            continue;
        }
        let mut entity_commands = commands.entity(entity);
        if !has_transform {
            entity_commands.insert(Transform::default());
        }
        if !has_global_transform {
            entity_commands.insert(GlobalTransform::default());
        }
        if !has_visibility {
            entity_commands.insert(Visibility::default());
        }
    }
}

pub(crate) fn ensure_parent_spatial_components_on_children_added(
    trigger: On<Add, Children>,
    mut commands: Commands<'_, '_>,
    parent_components: Query<'_, '_, (Has<Transform>, Has<GlobalTransform>, Has<Visibility>)>,
) {
    let entity = trigger.entity;
    let Ok((has_transform, has_global_transform, has_visibility)) = parent_components.get(entity)
    else {
        return;
    };
    if has_transform && has_global_transform && has_visibility {
        return;
    }
    let mut entity_commands = commands.entity(entity);
    if !has_transform {
        entity_commands.insert(Transform::default());
    }
    if !has_global_transform {
        entity_commands.insert(GlobalTransform::default());
    }
    if !has_visibility {
        entity_commands.insert(Visibility::default());
    }
}

pub(crate) fn should_defer_controlled_predicted_adoption(
    is_local_controlled: bool,
    has_position: bool,
    has_rotation: bool,
    has_linear_velocity: bool,
) -> bool {
    is_local_controlled && (!has_position || !has_rotation || !has_linear_velocity)
}

pub(crate) fn runtime_entity_id_from_guid(
    entity_registry: &RuntimeEntityHierarchy,
    local_player_entity_id: &str,
    guid: &str,
) -> Option<String> {
    // Bare UUID is the canonical entity ID.
    if entity_registry.by_entity_id.contains_key(guid) {
        return Some(guid.to_string());
    }
    // Legacy prefixed lookup for backwards compatibility.
    for prefix in ["ship", "player", "module", "hardpoint"] {
        let candidate = format!("{prefix}:{guid}");
        if entity_registry.by_entity_id.contains_key(&candidate) {
            return Some(candidate);
        }
    }
    if parse_guid_from_entity_id(local_player_entity_id)
        .is_some_and(|player_guid| player_guid.to_string() == guid)
    {
        return Some(local_player_entity_id.to_string());
    }
    None
}

fn ids_refer_to_same_guid(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    parse_guid_from_entity_id(left)
        .zip(parse_guid_from_entity_id(right))
        .is_some_and(|(l, r)| l == r)
}

pub(crate) fn resolve_authoritative_control_entity_id_from_registry(
    entity_registry: &RuntimeEntityHierarchy,
    local_player_entity_id: &str,
    controlled_entity_guid: Option<&ControlledEntityGuid>,
) -> Option<String> {
    let control_guid = controlled_entity_guid.and_then(|v| v.0.as_deref())?;

    if parse_guid_from_entity_id(local_player_entity_id)
        .is_some_and(|player_guid| player_guid.to_string() == control_guid)
    {
        return runtime_entity_id_from_guid(entity_registry, local_player_entity_id, control_guid)
            .or_else(|| Some(local_player_entity_id.to_string()));
    }

    runtime_entity_id_from_guid(entity_registry, local_player_entity_id, control_guid)
}

pub(crate) fn transition_world_loading_to_in_world(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    session: Res<'_, ClientSession>,
    session_ready: Res<'_, SessionReadyState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
) {
    if !app_state
        .as_ref()
        .is_some_and(|state| **state == ClientAppState::WorldLoading)
    {
        return;
    }
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    if session_ready.ready_player_entity_id.as_deref() != Some(local_player_entity_id.as_str()) {
        return;
    }
    let has_local_player_entity = entity_registry
        .by_entity_id
        .contains_key(local_player_entity_id)
        || parse_guid_from_entity_id(local_player_entity_id).is_some_and(|guid| {
            entity_registry
                .by_entity_id
                .contains_key(guid.to_string().as_str())
        });
    if !has_local_player_entity {
        return;
    }
    next_state.set(ClientAppState::InWorld);
}

pub(crate) fn configure_prediction_manager_tuning(
    tuning: Res<'_, PredictionCorrectionTuning>,
    mut managers: Query<
        '_,
        '_,
        (Entity, &mut PredictionManager, Has<Client>),
        Added<PredictionManager>,
    >,
) {
    for (entity, mut manager, has_client_marker) in &mut managers {
        manager.rollback_policy.max_rollback_ticks = tuning.max_rollback_ticks;
        manager.rollback_policy.state = match tuning.rollback_state {
            PredictionRollbackStateTuning::Always => RollbackMode::Always,
            PredictionRollbackStateTuning::Check => RollbackMode::Check,
            PredictionRollbackStateTuning::Disabled => RollbackMode::Disabled,
        };
        manager.correction_policy = if tuning.instant_correction {
            CorrectionPolicy::instant_correction()
        } else {
            CorrectionPolicy::default()
        };
        bevy::log::info!(
            "configured prediction manager entity={} has_client_marker={} (rollback_state={:?}, max_rollback_ticks={}, correction_mode={})",
            entity,
            has_client_marker,
            manager.rollback_policy.state,
            tuning.max_rollback_ticks,
            if tuning.instant_correction {
                "instant"
            } else {
                "smooth"
            }
        );
    }
}

pub(crate) fn ensure_prediction_manager_present_system(
    mut commands: Commands<'_, '_>,
    clients: Query<'_, '_, Entity, With<Client>>,
    managers: Query<'_, '_, Entity, With<PredictionManager>>,
) {
    if managers.iter().next().is_some() {
        return;
    }
    let Ok(client_entity) = clients.single() else {
        return;
    };
    commands
        .entity(client_entity)
        .insert(PredictionManager::default());
    bevy::log::info!(
        "inserted missing prediction manager entity={} has_client=true",
        client_entity
    );
}

pub(crate) fn prune_runtime_entity_registry_system(
    mut entity_registry: ResMut<'_, RuntimeEntityHierarchy>,
    mut remote_registry: ResMut<'_, RemoteEntityRegistry>,
    live_entities: Query<'_, '_, ()>,
) {
    entity_registry
        .by_entity_id
        .retain(|_, entity| live_entities.get(*entity).is_ok());
    remote_registry
        .by_entity_id
        .retain(|_, entity| live_entities.get(*entity).is_ok());
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(crate) fn adopt_native_lightyear_replicated_entities(
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    local_mode: Res<'_, LocalSimulationDebugMode>,
    tuning: Res<'_, PredictionBootstrapTuning>,
    time: Res<'_, Time>,
    mut adoption_state: ResMut<'_, DeferredPredictedAdoptionState>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    mut entity_registry: ResMut<'_, RuntimeEntityHierarchy>,
    mut remote_registry: ResMut<'_, RemoteEntityRegistry>,
    live_entities: Query<'_, '_, ()>,
    _collision_outlines: Query<'_, '_, &'_ CollisionOutlineM>,
    replicated_entities: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ EntityGuid>,
            Option<&'_ MountedOn>,
            Option<&'_ Hardpoint>,
            Option<&'_ PlayerTag>,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ LinearVelocity>,
            Option<&'_ SizeM>,
            Option<&'_ VisualAssetId>,
            Option<&'_ SpriteShaderAssetId>,
            Has<lightyear::prelude::Replicated>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
        ),
        (
            With<EntityGuid>,
            Without<ReplicatedAdoptionHandled>,
            Without<WorldEntity>,
            Without<DespawnOnExit<ClientAppState>>,
        ),
    >,
    controlled_query: Query<'_, '_, Entity, With<ControlledEntity>>,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    for (
        entity,
        guid,
        mounted_on,
        hardpoint,
        player_tag,
        position,
        rotation,
        linear_velocity,
        size_m,
        visual_asset_id,
        sprite_shader_asset_id,
        is_replicated,
        _is_predicted,
        _is_interpolated,
    ) in &replicated_entities
    {
        let Some(guid) = guid else {
            continue;
        };
        watchdog.replication_state_seen = true;
        let runtime_entity_id = guid.0.to_string();
        let is_root_entity = mounted_on.is_none() && hardpoint.is_none() && player_tag.is_none();
        let is_local_player_entity =
            ids_refer_to_same_guid(runtime_entity_id.as_str(), local_player_entity_id);
        let is_local_controlled_entity = (is_root_entity || is_local_player_entity)
            && player_view_state.controlled_entity_id.as_deref()
                == Some(runtime_entity_id.as_str());
        let predicted_mode = !local_mode.0;
        let is_spatial_root = is_root_entity && size_m.is_some();
        if is_spatial_root
            && (position.is_none() || rotation.is_none() || linear_velocity.is_none())
        {
            // Avoid adopting partially replicated spatial roots at (0,0) until core
            // motion components arrive; this prevents transient wrong-world placement.
            continue;
        }
        if predicted_mode
            && should_defer_controlled_predicted_adoption(
                is_local_controlled_entity,
                position.is_some(),
                rotation.is_some(),
                linear_velocity.is_some(),
            )
        {
            let now_s = time.elapsed_secs_f64();
            let mut missing = Vec::new();
            if position.is_none() {
                missing.push("Position");
            }
            if rotation.is_none() {
                missing.push("Rotation");
            }
            if linear_velocity.is_none() {
                missing.push("LinearVelocity");
            }
            let missing_summary = missing.join(", ");
            if adoption_state.waiting_entity_id.as_deref() != Some(runtime_entity_id.as_str()) {
                adoption_state.waiting_entity_id = Some(runtime_entity_id.clone());
                adoption_state.wait_started_at_s = Some(now_s);
                adoption_state.last_warn_at_s = 0.0;
                adoption_state.dialog_shown = false;
            }
            adoption_state.last_missing_components = missing_summary.clone();
            if let Some(started_at_s) = adoption_state.wait_started_at_s {
                let wait_s = (now_s - started_at_s).max(0.0);
                if wait_s >= tuning.defer_warn_after_s
                    && now_s - adoption_state.last_warn_at_s >= tuning.defer_warn_interval_s
                {
                    bevy::log::warn!(
                        "deferring predicted controlled adoption for {} (wait {:.2}s, missing: {})",
                        runtime_entity_id,
                        wait_s,
                        missing_summary
                    );
                    adoption_state.last_warn_at_s = now_s;
                }
            }
            continue;
        }

        if is_replicated
            && let Some(&existing_entity) = entity_registry.by_entity_id.get(runtime_entity_id.as_str())
            && existing_entity != entity
        {
            if live_entities.get(existing_entity).is_ok() {
                commands
                    .entity(entity)
                    .insert((ReplicatedAdoptionHandled, Visibility::Hidden))
                    .remove::<(
                        ControlledEntity,
                        StreamedVisualAssetId,
                        StreamedVisualAttached,
                        StreamedSpriteShaderAssetId,
                    )>();
                continue;
            }

            // Stale map entry (entity was despawned/recycled); allow re-adoption.
            entity_registry.by_entity_id.remove(runtime_entity_id.as_str());
        }

        if adoption_state.waiting_entity_id.as_deref() == Some(runtime_entity_id.as_str()) {
            if let Some(started_at_s) = adoption_state.wait_started_at_s {
                let resolved_wait_s = (time.elapsed_secs_f64() - started_at_s).max(0.0);
                adoption_state.resolved_samples = adoption_state.resolved_samples.saturating_add(1);
                adoption_state.resolved_total_wait_s += resolved_wait_s;
                adoption_state.resolved_max_wait_s =
                    adoption_state.resolved_max_wait_s.max(resolved_wait_s);
                bevy::log::info!(
                    "predicted controlled adoption resolved for {} after {:.2}s (samples={}, max_wait_s={:.2})",
                    runtime_entity_id,
                    resolved_wait_s,
                    adoption_state.resolved_samples,
                    adoption_state.resolved_max_wait_s
                );
            }
            adoption_state.waiting_entity_id = None;
            adoption_state.wait_started_at_s = None;
            adoption_state.last_warn_at_s = 0.0;
            adoption_state.last_missing_components.clear();
            adoption_state.dialog_shown = false;
        }

        // Keep canonical runtime ID mapping pinned to the Confirmed entity (`Replicated`).
        // Predicted/Interpolated clones share EntityGuid and are resolved by GUID queries.
        if is_replicated {
            register_runtime_entity(&mut entity_registry, runtime_entity_id.clone(), entity);
        }
        let mut entity_commands = commands.entity(entity);
        entity_commands.insert((
            Name::new(runtime_entity_id.clone()),
            ReplicatedAdoptionHandled,
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
            Visibility::Visible,
        ));

        if player_tag.is_none() {
            if let Some(visual_asset_id) = visual_asset_id {
                entity_commands.insert(StreamedVisualAssetId(visual_asset_id.0.clone()));
            } else {
                entity_commands.remove::<(StreamedVisualAssetId, StreamedVisualAttached)>();
            }
        } else {
            entity_commands.remove::<(
                StreamedVisualAssetId,
                StreamedVisualAttached,
                StreamedSpriteShaderAssetId,
            )>();
        }
        if player_tag.is_none()
            && let Some(sprite_shader_asset_id) = sprite_shader_asset_id
            && let Some(shader_asset_id) = sprite_shader_asset_id.0.as_ref()
        {
            entity_commands.insert(StreamedSpriteShaderAssetId(shader_asset_id.clone()));
        } else {
            entity_commands.remove::<StreamedSpriteShaderAssetId>();
        }

        if is_local_controlled_entity {
            entity_commands.remove::<RemoteEntity>();
            entity_commands.insert(RemoteVisibleEntity {
                entity_id: runtime_entity_id.clone(),
            });
        } else if is_root_entity {
            entity_commands.insert((
                RemoteEntity,
                RemoteVisibleEntity {
                    entity_id: runtime_entity_id.clone(),
                },
            ));
            remote_registry
                .by_entity_id
                .insert(runtime_entity_id, entity);
            entity_commands.remove::<ActionQueue>();
        } else if !is_local_player_entity {
            entity_commands.remove::<ActionQueue>();
        }
    }

    let now_s = time.elapsed_secs_f64();
    if adoption_state.resolved_samples > 0
        && now_s - adoption_state.last_summary_at_s >= tuning.defer_summary_interval_s
    {
        let avg_wait_s =
            adoption_state.resolved_total_wait_s / adoption_state.resolved_samples as f64;
        bevy::log::info!(
            "predicted adoption delay summary samples={} avg_wait_s={:.2} max_wait_s={:.2}",
            adoption_state.resolved_samples,
            avg_wait_s,
            adoption_state.resolved_max_wait_s
        );
        adoption_state.last_summary_at_s = now_s;
    }

    let controlled_count = controlled_query.iter().count();
    if controlled_count > 1 {
        bevy::log::warn!(
            "multiple controlled entities detected under native replication; keeping latest control target"
        );
    }
    if controlled_count > 0 {
        adoption_state.waiting_entity_id = None;
        adoption_state.wait_started_at_s = None;
        adoption_state.last_warn_at_s = 0.0;
        adoption_state.last_missing_components.clear();
        adoption_state.dialog_shown = false;
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_local_player_view_state_system(
    session: Res<'_, ClientSession>,
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    player_query: Query<'_, '_, Option<&'_ ControlledEntityGuid>, With<PlayerTag>>,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let Some(local_player_runtime_id) = runtime_entity_id_from_guid(
        &entity_registry,
        local_player_entity_id,
        local_player_entity_id,
    ) else {
        return;
    };

    let Some(&local_player_entity) = entity_registry
        .by_entity_id
        .get(local_player_runtime_id.as_str())
    else {
        return;
    };
    let Ok(controlled) = player_query.get(local_player_entity) else {
        return;
    };

    if let Some(authoritative_controlled_id) = resolve_authoritative_control_entity_id_from_registry(
        &entity_registry,
        local_player_entity_id,
        controlled,
    ) {
        player_view_state.controlled_entity_id = Some(authoritative_controlled_id);
        if player_view_state.desired_controlled_entity_id.is_none() {
            player_view_state.desired_controlled_entity_id =
                player_view_state.controlled_entity_id.clone();
        }
    }
}

pub(crate) fn sync_controlled_entity_tags_system(
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    player_view_state: ResMut<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    controlled_query: Query<'_, '_, (Entity, &'_ ControlledEntity)>,
    guid_candidates: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
        ),
    >,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let local_player_runtime_id = runtime_entity_id_from_guid(
        &entity_registry,
        local_player_entity_id,
        local_player_entity_id,
    )
    .unwrap_or_else(|| local_player_entity_id.clone());

    let target_entity_id = match player_view_state.controlled_entity_id.as_ref() {
        Some(id) if entity_registry.by_entity_id.contains_key(id.as_str()) => Some(id.clone()),
        Some(id) => runtime_entity_id_from_guid(&entity_registry, local_player_entity_id, id),
        None => Some(local_player_runtime_id.clone()),
    };
    let target_guid = target_entity_id
        .as_ref()
        .and_then(|id| parse_guid_from_entity_id(id));
    let target_entity = target_guid
        .and_then(|guid| {
            let mut best: Option<(Entity, i32)> = None;
            for (entity, entity_guid, is_predicted, is_interpolated) in &guid_candidates {
                if entity_guid.0 != guid {
                    continue;
                }
                let score = if is_predicted {
                    3
                } else if is_interpolated {
                    2
                } else {
                    1
                };
                match best {
                    Some((winner, winner_score))
                        if score < winner_score
                            || (score == winner_score && winner.to_bits() <= entity.to_bits()) => {}
                    _ => best = Some((entity, score)),
                }
            }
            best.map(|(entity, _)| entity)
        })
        .or_else(|| {
            target_entity_id
                .as_ref()
                .and_then(|id| entity_registry.by_entity_id.get(id.as_str()).copied())
        });

    for (entity, controlled) in &controlled_query {
        if Some(entity) != target_entity {
            commands.entity(entity).remove::<ControlledEntity>();
        } else if controlled.player_entity_id != local_player_runtime_id {
            commands.entity(entity).insert(ControlledEntity {
                entity_id: controlled.entity_id.clone(),
                player_entity_id: local_player_runtime_id.clone(),
            });
        }
    }

    if let Some(entity) = target_entity {
        commands.entity(entity).insert(ControlledEntity {
            entity_id: target_entity_id.clone().unwrap_or_default(),
            player_entity_id: local_player_runtime_id,
        });
    }
}
