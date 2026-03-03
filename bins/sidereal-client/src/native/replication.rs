//! Replication adoption, control sync, and prediction runtime state.

use avian2d::prelude::{
    AngularDamping, AngularInertia, AngularVelocity, Collider, LinearDamping, LinearVelocity,
    LockedAxes, Mass, Position, RigidBody, Rotation,
};
use bevy::ecs::query::Has;
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use lightyear::prediction::correction::CorrectionPolicy;
use lightyear::prediction::prelude::PredictionManager;
use lightyear::prelude::client::Client;
use sidereal_game::{
    ActionQueue, CollisionAabbM, CollisionOutlineM, CollisionProfile, ControlledEntityGuid,
    EntityGuid, Hardpoint, MountedOn, PlayerTag, SizeM, SpriteShaderAssetId, TotalMassKg,
    VisualAssetId, angular_inertia_from_size, collider_from_collision_shape,
};
use sidereal_runtime_sync::{
    RuntimeEntityHierarchy, parse_guid_from_entity_id, register_runtime_entity,
};
use std::collections::{HashMap, HashSet};

use super::app_state::{ClientAppState, ClientSession, LocalPlayerViewState, SessionReadyState};
use super::components::{
    ControlledEntity, RemoteEntity, RemoteVisibleEntity, ReplicatedAdoptionHandled,
    StreamedSpriteShaderAssetId, StreamedVisualAssetId, StreamedVisualAttached, WorldEntity,
};
use super::resources::{
    BootstrapWatchdogState, DeferredPredictedAdoptionState, LocalSimulationDebugMode,
    PredictionBootstrapTuning, PredictionCorrectionTuning, RemoteEntityRegistry,
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

#[allow(clippy::type_complexity)]
pub(crate) fn ensure_ui_node_spatial_components(
    mut commands: Commands<'_, '_>,
    ui_nodes: Query<
        '_,
        '_,
        (
            Entity,
            Has<Transform>,
            Has<GlobalTransform>,
            Has<Visibility>,
        ),
        With<Node>,
    >,
) {
    for (entity, has_transform, has_global_transform, has_visibility) in &ui_nodes {
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

pub(crate) fn candidate_runtime_entity_score(
    is_root_entity: bool,
    is_local_controlled_entity: bool,
    predicted_mode: bool,
) -> i32 {
    if is_local_controlled_entity {
        if predicted_mode { 500 } else { 400 }
    } else if is_root_entity {
        if predicted_mode { 200 } else { 100 }
    } else {
        50
    }
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

pub(crate) fn resolve_authoritative_control_entity_id_with_snapshot(
    entity_registry: &RuntimeEntityHierarchy,
    runtime_entity_id_by_guid: &HashMap<String, String>,
    local_player_entity_id: &str,
    controlled_entity_guid: Option<&ControlledEntityGuid>,
) -> Option<String> {
    let control_guid = controlled_entity_guid.and_then(|v| v.0.as_deref())?;

    if parse_guid_from_entity_id(local_player_entity_id)
        .is_some_and(|player_guid| player_guid.to_string() == control_guid)
    {
        return runtime_entity_id_by_guid
            .get(control_guid)
            .cloned()
            .or_else(|| {
                runtime_entity_id_from_guid(entity_registry, local_player_entity_id, control_guid)
            })
            .or_else(|| Some(local_player_entity_id.to_string()));
    }

    runtime_entity_id_by_guid
        .get(control_guid)
        .cloned()
        .or_else(|| {
            runtime_entity_id_from_guid(entity_registry, local_player_entity_id, control_guid)
        })
}

pub(crate) fn existing_runtime_entity_score(
    is_world_entity: bool,
    is_controlled: bool,
    is_predicted: bool,
    is_interpolated: bool,
    is_remote: bool,
) -> i32 {
    if is_controlled {
        if is_predicted { 500 } else { 400 }
    } else if is_remote {
        if is_predicted {
            200
        } else if is_interpolated {
            100
        } else {
            90
        }
    } else if is_world_entity {
        80
    } else {
        0
    }
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
    mut managers: Query<'_, '_, &mut PredictionManager, (With<Client>, Added<PredictionManager>)>,
) {
    for mut manager in &mut managers {
        manager.rollback_policy.max_rollback_ticks = tuning.max_rollback_ticks;
        manager.correction_policy = if tuning.instant_correction {
            CorrectionPolicy::instant_correction()
        } else {
            CorrectionPolicy::default()
        };
        bevy::log::info!(
            "configured prediction manager (max_rollback_ticks={}, correction_mode={})",
            tuning.max_rollback_ticks,
            if tuning.instant_correction {
                "instant"
            } else {
                "smooth"
            }
        );
    }
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
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    mut entity_registry: ResMut<'_, RuntimeEntityHierarchy>,
    mut remote_registry: ResMut<'_, RemoteEntityRegistry>,
    collision_outlines: Query<'_, '_, &'_ CollisionOutlineM>,
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
            Option<&'_ TotalMassKg>,
            Option<&'_ CollisionAabbM>,
            Option<&'_ CollisionProfile>,
            Option<&'_ ControlledEntityGuid>,
            Option<&'_ VisualAssetId>,
            Option<&'_ SpriteShaderAssetId>,
        ),
        (
            With<lightyear::prelude::Replicated>,
            Without<ReplicatedAdoptionHandled>,
            Without<WorldEntity>,
            Without<DespawnOnExit<ClientAppState>>,
        ),
    >,
    controlled_query: Query<'_, '_, Entity, With<ControlledEntity>>,
    adopted_entity_state: Query<
        '_,
        '_,
        (
            Has<WorldEntity>,
            Has<ControlledEntity>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
            Has<RemoteEntity>,
        ),
    >,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let mut runtime_entity_id_by_guid = HashMap::<String, String>::new();
    for (_, guid, mounted_on, hardpoint, _player_tag, ..) in &replicated_entities {
        let Some(guid) = guid else {
            continue;
        };
        if mounted_on.is_some() || hardpoint.is_some() {
            continue;
        }
        let guid_key = guid.0.to_string();
        runtime_entity_id_by_guid
            .entry(guid_key.clone())
            .or_insert(guid_key);
    }

    let mut authoritative_controlled_entity_id = player_view_state.controlled_entity_id.clone();
    for (_, guid, mounted_on, hardpoint, player_tag, .., controlled_entity_guid, _, _) in
        &replicated_entities
    {
        let Some(guid) = guid else {
            continue;
        };
        if mounted_on.is_some() || hardpoint.is_some() || player_tag.is_none() {
            continue;
        }
        let runtime_entity_id = guid.0.to_string();
        if !ids_refer_to_same_guid(runtime_entity_id.as_str(), local_player_entity_id) {
            continue;
        }
        let controlled_id = resolve_authoritative_control_entity_id_with_snapshot(
            &entity_registry,
            &runtime_entity_id_by_guid,
            local_player_entity_id,
            controlled_entity_guid,
        );
        if let Some(controlled_id) = controlled_id {
            player_view_state.controlled_entity_id = Some(controlled_id);
            authoritative_controlled_entity_id = player_view_state.controlled_entity_id.clone();
        }
        break;
    }

    let mut seen_runtime_entity_ids = HashSet::<String>::new();

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
        total_mass_kg,
        collision_aabb,
        collision_profile,
        controlled_entity_guid,
        visual_asset_id,
        sprite_shader_asset_id,
    ) in &replicated_entities
    {
        let Some(guid) = guid else {
            continue;
        };
        watchdog.replication_state_seen = true;
        let runtime_entity_id = guid.0.to_string();
        if !seen_runtime_entity_ids.insert(runtime_entity_id.clone()) {
            commands
                .entity(entity)
                .insert((ReplicatedAdoptionHandled, Visibility::Hidden));
            continue;
        }
        let is_root_entity = mounted_on.is_none() && hardpoint.is_none() && player_tag.is_none();
        let is_local_player_entity =
            ids_refer_to_same_guid(runtime_entity_id.as_str(), local_player_entity_id);
        let is_local_controlled_entity = (is_root_entity || is_local_player_entity)
            && authoritative_controlled_entity_id.as_deref() == Some(runtime_entity_id.as_str());
        if is_local_player_entity
            && let Some(controlled_id) = resolve_authoritative_control_entity_id_with_snapshot(
                &entity_registry,
                &runtime_entity_id_by_guid,
                local_player_entity_id,
                controlled_entity_guid,
            )
        {
            player_view_state.controlled_entity_id = Some(controlled_id);
        }
        let predicted_mode = !local_mode.0;
        let candidate_score = candidate_runtime_entity_score(
            is_root_entity,
            is_local_controlled_entity,
            predicted_mode,
        );
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

        if let Some(&existing_entity) = entity_registry.by_entity_id.get(runtime_entity_id.as_str())
            && existing_entity != entity
        {
            if let Ok((is_world, is_controlled, is_predicted, is_interpolated, is_remote)) =
                adopted_entity_state.get(existing_entity)
            {
                let existing_score = existing_runtime_entity_score(
                    is_world,
                    is_controlled,
                    is_predicted,
                    is_interpolated,
                    is_remote,
                );
                if candidate_score <= existing_score {
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

                commands.entity(existing_entity).remove::<Name>();
                if is_world {
                    commands
                        .entity(existing_entity)
                        .insert(Visibility::Hidden)
                        .remove::<(
                            WorldEntity,
                            RemoteEntity,
                            RemoteVisibleEntity,
                            ControlledEntity,
                            StreamedVisualAssetId,
                            StreamedVisualAttached,
                            StreamedSpriteShaderAssetId,
                        )>();
                }
                if entity_registry.by_entity_id.get(runtime_entity_id.as_str())
                    == Some(&existing_entity)
                {
                    entity_registry
                        .by_entity_id
                        .remove(runtime_entity_id.as_str());
                }
                if remote_registry.by_entity_id.get(runtime_entity_id.as_str())
                    == Some(&existing_entity)
                {
                    remote_registry
                        .by_entity_id
                        .remove(runtime_entity_id.as_str());
                }
            } else {
                entity_registry
                    .by_entity_id
                    .remove(runtime_entity_id.as_str());
                if remote_registry.by_entity_id.get(runtime_entity_id.as_str())
                    == Some(&existing_entity)
                {
                    remote_registry
                        .by_entity_id
                        .remove(runtime_entity_id.as_str());
                }
            }
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

        register_runtime_entity(&mut entity_registry, runtime_entity_id.clone(), entity);
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
            let position = position.map(|p| p.0).unwrap_or(Vec2::ZERO);
            let rotation = rotation.copied().unwrap_or(Rotation::IDENTITY);
            let velocity = linear_velocity.map(|v| v.0).unwrap_or(Vec2::ZERO);
            let has_physics_data = size_m.is_some() && total_mass_kg.is_some_and(|m| m.0 > 0.0);
            let allow_collider = collision_profile
                .copied()
                .is_some_and(CollisionProfile::is_collidable);
            if has_physics_data {
                let size = size_m.copied().unwrap();
                let mass_kg = total_mass_kg.map(|m| m.0).unwrap();
                if allow_collider {
                    let collider = collider_from_collision_shape(
                        &size,
                        collision_aabb,
                        collision_outlines.get(entity).ok(),
                    );
                    entity_commands.insert((
                        RigidBody::Dynamic,
                        collider,
                        Mass(mass_kg),
                        angular_inertia_from_size(mass_kg, &size),
                        Position(position),
                        rotation,
                        LinearVelocity(velocity),
                        AngularVelocity::default(),
                        LinearDamping(0.0),
                        AngularDamping(0.0),
                    ));
                } else {
                    entity_commands.insert((
                        Position(position),
                        rotation,
                        LinearVelocity(velocity),
                        AngularVelocity::default(),
                    ));
                    entity_commands.remove::<(
                        RigidBody,
                        Collider,
                        Mass,
                        AngularInertia,
                        LockedAxes,
                        LinearDamping,
                        AngularDamping,
                    )>();
                }
            } else {
                entity_commands.insert((Position(position), rotation, LinearVelocity(velocity)));
            }
            if predicted_mode {
                entity_commands
                    .insert(lightyear::prelude::Predicted)
                    .remove::<lightyear::prelude::Interpolated>();
            }
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
            if predicted_mode {
                // Non-controlled roots are receive-only and must not depend on interpolation
                // marker transitions to receive authoritative motion updates.
                entity_commands.remove::<(
                    lightyear::prelude::Predicted,
                    lightyear::prelude::Interpolated,
                )>();
            }
            entity_commands.remove::<ActionQueue>();
            entity_commands.remove::<(
                RigidBody,
                Collider,
                Mass,
                AngularInertia,
                LockedAxes,
                LinearDamping,
                AngularDamping,
            )>();
        } else if predicted_mode {
            // Non-root entities (including player anchor entities) must not retain
            // stale prediction markers from previous control modes.
            entity_commands.remove::<(
                lightyear::prelude::Predicted,
                lightyear::prelude::Interpolated,
            )>();
            if !is_local_player_entity {
                entity_commands.remove::<ActionQueue>();
            }
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
    let target_entity = target_entity_id
        .as_ref()
        .and_then(|id| entity_registry.by_entity_id.get(id.as_str()).copied());

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

#[allow(clippy::type_complexity)]
pub(crate) fn converge_local_prediction_markers_system(
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    entities: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ EntityGuid>,
            Option<&'_ MountedOn>,
            Option<&'_ Hardpoint>,
            Option<&'_ PlayerTag>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
        ),
        With<WorldEntity>,
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
    let controlled_entity_id = player_view_state
        .controlled_entity_id
        .clone()
        .unwrap_or_else(|| local_player_runtime_id.clone());

    for (entity, guid, mounted_on, hardpoint, player_tag, is_predicted, is_interpolated) in
        &entities
    {
        let Some(guid) = guid else { continue };
        let guid_str = guid.0.to_string();
        let is_local_player =
            ids_refer_to_same_guid(guid_str.as_str(), local_player_runtime_id.as_str());
        let is_controlled_target =
            ids_refer_to_same_guid(guid_str.as_str(), controlled_entity_id.as_str());
        let is_root_entity = mounted_on.is_none() && hardpoint.is_none() && player_tag.is_none();

        if is_controlled_target {
            if !is_predicted || is_interpolated {
                commands
                    .entity(entity)
                    .insert(lightyear::prelude::Predicted)
                    .remove::<lightyear::prelude::Interpolated>();
            }
            continue;
        }

        if is_local_player {
            if is_predicted || is_interpolated {
                commands.entity(entity).remove::<(
                    lightyear::prelude::Predicted,
                    lightyear::prelude::Interpolated,
                )>();
            }
            continue;
        }

        if is_root_entity {
            if is_predicted || is_interpolated {
                commands.entity(entity).remove::<(
                    lightyear::prelude::Predicted,
                    lightyear::prelude::Interpolated,
                )>();
            }
        } else if is_predicted || is_interpolated {
            commands.entity(entity).remove::<(
                lightyear::prelude::Predicted,
                lightyear::prelude::Interpolated,
            )>();
        }
    }
}
