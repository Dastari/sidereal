//! Replication adoption, control sync, and prediction runtime state.

use avian2d::prelude::{LinearVelocity, Position, Rotation};
use bevy::ecs::query::Has;
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use lightyear::frame_interpolation::FrameInterpolate;
use lightyear::prediction::correction::CorrectionPolicy;
use lightyear::prediction::prelude::{PredictionManager, RollbackMode};
use lightyear::prelude::PreSpawned;
use lightyear::prelude::client::Client;
use sidereal_game::{
    ActionQueue, BallisticProjectile, CollisionOutlineM, ControlledEntityGuid, EntityGuid,
    Hardpoint, MountedOn, PlayerTag, SimulationMotionWriter, SizeM, SpriteShaderAssetId,
    VisualAssetId, WorldPosition, WorldRotation, resolve_world_position,
    resolve_world_rotation_rad,
};
use sidereal_runtime_sync::{
    RuntimeEntityHierarchy, parse_guid_from_entity_id, register_runtime_entity,
};
use std::collections::{HashMap, HashSet};

use super::app_state::{ClientAppState, ClientSession, LocalPlayerViewState, SessionReadyState};
use super::components::{
    ControlledEntity, PendingInitialVisualReady, RemoteEntity, RemoteVisibleEntity,
    ReplicatedAdoptionHandled, StreamedSpriteShaderAssetId, StreamedVisualAssetId,
    StreamedVisualAttached, StreamedVisualAttachmentKind, WorldEntity,
};
use super::resources::{
    BootstrapWatchdogState, DeferredPredictedAdoptionState, LocalSimulationDebugMode,
    PredictionBootstrapTuning, PredictionCorrectionTuning, PredictionRollbackStateTuning,
    RemoteEntityRegistry,
};

#[allow(clippy::type_complexity)]
pub(crate) fn mark_new_ballistic_projectiles_prespawned(
    mut commands: Commands<'_, '_>,
    projectiles: Query<
        '_,
        '_,
        Entity,
        (
            With<BallisticProjectile>,
            Added<BallisticProjectile>,
            Without<PreSpawned>,
        ),
    >,
) {
    for entity in &projectiles {
        commands.entity(entity).insert(PreSpawned::default());
    }
}

pub(crate) fn ensure_replicated_entity_spatial_components(
    mut commands: Commands<'_, '_>,
    missing_transform: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ WorldPosition>,
            Option<&'_ WorldRotation>,
        ),
        (With<lightyear::prelude::Replicated>, Without<Transform>),
    >,
    missing_visibility: Query<
        '_,
        '_,
        Entity,
        (With<lightyear::prelude::Replicated>, Without<Visibility>),
    >,
) {
    for (entity, position, rotation, world_position, world_rotation) in &missing_transform {
        let mut transform = Transform::default();
        if let (Some(planar_position), Some(heading)) = (
            resolve_world_position(position, world_position),
            resolve_world_rotation_rad(rotation, world_rotation),
        ) && planar_position.is_finite()
            && heading.is_finite()
        {
            transform.translation.x = planar_position.x;
            transform.translation.y = planar_position.y;
            transform.translation.z = 0.0;
            transform.rotation = Quat::from_rotation_z(heading);
        }
        let global_transform = GlobalTransform::from(transform);
        commands
            .entity(entity)
            .insert((transform, global_transform, Visibility::default()));
    }
    for entity in &missing_visibility {
        commands.entity(entity).insert(Visibility::default());
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn ensure_hierarchy_parent_spatial_components(
    mut commands: Commands<'_, '_>,
    children_with_parent: Query<'_, '_, &'_ ChildOf>,
    parent_components: Query<
        '_,
        '_,
        (
            Has<Transform>,
            Has<GlobalTransform>,
            Has<Visibility>,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ WorldPosition>,
            Option<&'_ WorldRotation>,
        ),
    >,
) {
    let mut visited_parents = HashSet::<Entity>::new();
    for child_of in &children_with_parent {
        let entity = child_of.parent();
        if !visited_parents.insert(entity) {
            continue;
        }
        let Ok((
            has_transform,
            has_global_transform,
            has_visibility,
            position,
            rotation,
            world_position,
            world_rotation,
        )) = parent_components.get(entity)
        else {
            continue;
        };
        if has_transform && has_global_transform && has_visibility {
            continue;
        }
        let mut transform = Transform::default();
        if let (Some(planar_position), Some(heading)) = (
            resolve_world_position(position, world_position),
            resolve_world_rotation_rad(rotation, world_rotation),
        ) && planar_position.is_finite()
            && heading.is_finite()
        {
            transform.translation.x = planar_position.x;
            transform.translation.y = planar_position.y;
            transform.translation.z = 0.0;
            transform.rotation = Quat::from_rotation_z(heading);
        }
        let mut entity_commands = commands.entity(entity);
        if !has_transform {
            entity_commands.insert(transform);
        }
        if !has_global_transform {
            entity_commands.insert(GlobalTransform::from(transform));
        }
        if !has_visibility {
            entity_commands.insert(Visibility::default());
        }
    }
}

pub(crate) fn ensure_parent_spatial_components_on_children_added(
    trigger: On<Add, Children>,
    mut commands: Commands<'_, '_>,
    parent_components: Query<
        '_,
        '_,
        (
            Has<Transform>,
            Has<GlobalTransform>,
            Has<Visibility>,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ WorldPosition>,
            Option<&'_ WorldRotation>,
        ),
    >,
) {
    let entity = trigger.entity;
    let Ok((
        has_transform,
        has_global_transform,
        has_visibility,
        position,
        rotation,
        world_position,
        world_rotation,
    )) = parent_components.get(entity)
    else {
        return;
    };
    if has_transform && has_global_transform && has_visibility {
        return;
    }
    let mut transform = Transform::default();
    if let (Some(planar_position), Some(heading)) = (
        resolve_world_position(position, world_position),
        resolve_world_rotation_rad(rotation, world_rotation),
    ) && planar_position.is_finite()
        && heading.is_finite()
    {
        transform.translation.x = planar_position.x;
        transform.translation.y = planar_position.y;
        transform.translation.z = 0.0;
        transform.rotation = Quat::from_rotation_z(heading);
    }
    let mut entity_commands = commands.entity(entity);
    if !has_transform {
        entity_commands.insert(transform);
    }
    if !has_global_transform {
        entity_commands.insert(GlobalTransform::from(transform));
    }
    if !has_visibility {
        entity_commands.insert(Visibility::default());
    }
}

/// Defensive guard against malformed replicated hierarchy links.
///
/// Server should not replicate cyclic Bevy hierarchy links, but if bad data slips
/// through (for example from migration/script bugs), transform propagation can stack-overflow.
/// This system breaks invalid `ChildOf` links before `TransformSystems::Propagate`.
pub(crate) fn sanitize_invalid_childof_hierarchy_links(
    mut commands: Commands<'_, '_>,
    child_of_query: Query<'_, '_, (Entity, &'_ ChildOf)>,
) {
    if child_of_query.is_empty() {
        return;
    }

    let mut parent_by_child = HashMap::<Entity, Entity>::new();
    for (child, child_of) in &child_of_query {
        parent_by_child.insert(child, child_of.parent());
    }

    const MAX_DEPTH: usize = 256;
    for (child, parent) in parent_by_child.clone() {
        if child == parent {
            bevy::log::warn!(
                "detected self-parent hierarchy link; removing ChildOf child={:?} parent={:?}",
                child,
                parent
            );
            commands.entity(child).remove::<ChildOf>();
            continue;
        }
        let mut seen = HashSet::<Entity>::new();
        let mut cursor = parent;
        let mut cycle = false;
        for _ in 0..MAX_DEPTH {
            if !seen.insert(cursor) {
                cycle = true;
                break;
            }
            let Some(next) = parent_by_child.get(&cursor).copied() else {
                break;
            };
            if next == child {
                cycle = true;
                break;
            }
            cursor = next;
        }
        if cycle {
            bevy::log::warn!(
                "detected cyclic replicated hierarchy; removing ChildOf for child={:?}",
                child
            );
            commands.entity(child).remove::<ChildOf>();
        }
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
    next_state.set(ClientAppState::AssetLoading);
}

pub(crate) fn transition_asset_loading_to_in_world(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    asset_bootstrap_state: Res<'_, super::auth_net::AssetBootstrapRequestState>,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
) {
    if !app_state
        .as_ref()
        .is_some_and(|state| **state == ClientAppState::AssetLoading)
    {
        return;
    }
    if !asset_bootstrap_state.completed || asset_bootstrap_state.failed {
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
    // This is a defensive bootstrap guard for the native client runtime, not the desired
    // steady-state architecture. If prediction markers are still missing after this runs,
    // the real bug is elsewhere in the Lightyear spawn/target lifecycle.
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
    world_spatial_query: Query<'_, '_, (Option<&'_ WorldPosition>, Option<&'_ WorldRotation>)>,
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
        let (world_position, world_rotation) =
            world_spatial_query.get(entity).unwrap_or((None, None));
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
        let is_static_world_spatial =
            (world_position.is_some() || world_rotation.is_some()) && linear_velocity.is_none();
        if is_spatial_root
            && ((position.is_none() && world_position.is_none())
                || (rotation.is_none() && world_rotation.is_none())
                || (!is_static_world_spatial && linear_velocity.is_none()))
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
            && let Some(&existing_entity) =
                entity_registry.by_entity_id.get(runtime_entity_id.as_str())
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
            entity_registry
                .by_entity_id
                .remove(runtime_entity_id.as_str());
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
            PendingInitialVisualReady,
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
            Visibility::Hidden,
        ));
        if position.is_some() && rotation.is_some() {
            entity_commands.insert(FrameInterpolate::<Transform>::default());
        } else {
            entity_commands.remove::<FrameInterpolate<Transform>>();
        }

        if player_tag.is_none() {
            if let Some(visual_asset_id) = visual_asset_id {
                entity_commands.insert(StreamedVisualAssetId(visual_asset_id.0.clone()));
            } else {
                entity_commands.remove::<(
                    StreamedVisualAssetId,
                    StreamedVisualAttached,
                    StreamedVisualAttachmentKind,
                )>();
            }
        } else {
            entity_commands.remove::<(
                StreamedVisualAssetId,
                StreamedVisualAttached,
                StreamedVisualAttachmentKind,
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
    request_state: Res<'_, super::resources::ClientControlRequestState>,
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
        let authoritative_changed = player_view_state.controlled_entity_id.as_deref()
            != Some(authoritative_controlled_id.as_str());
        if player_view_state.controlled_entity_id.as_deref()
            != Some(authoritative_controlled_id.as_str())
        {
            player_view_state.controlled_entity_id = Some(authoritative_controlled_id.clone());
        }
        if request_state.pending_request_seq.is_none()
            && (player_view_state.desired_controlled_entity_id.is_none() || authoritative_changed)
        {
            // During bootstrap, the persisted authoritative control target may arrive after the
            // client has already defaulted its desired target to the player anchor. If we do not
            // realign desired control here, the client immediately asks the server to switch back
            // to self and appears to "forget" the last controlled ship on spawn.
            player_view_state.desired_controlled_entity_id = Some(authoritative_controlled_id);
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn sanitize_conflicting_prediction_interpolation_markers_system(
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    conflicting_entities: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ EntityGuid>,
            Has<ControlledEntity>,
            Has<SimulationMotionWriter>,
        ),
        (
            With<lightyear::prelude::Predicted>,
            With<lightyear::prelude::Interpolated>,
        ),
    >,
) {
    let target_guid = player_view_state
        .controlled_entity_id
        .as_deref()
        .and_then(parse_guid_from_entity_id)
        .or_else(|| {
            session
                .player_entity_id
                .as_deref()
                .and_then(parse_guid_from_entity_id)
        });

    for (entity, entity_guid, is_controlled, is_motion_writer) in &conflicting_entities {
        let keep_predicted = is_controlled
            || is_motion_writer
            || target_guid
                .zip(entity_guid)
                .is_some_and(|(target_guid, entity_guid)| entity_guid.0 == target_guid);

        if keep_predicted {
            // Sidereal's dynamic handoff can legitimately promote an already-visible confirmed or
            // interpolated replica into the owner-predicted lane. Lightyear inserts the new
            // marker from the respawned spawn action, but it does not automatically clear the
            // old observer marker from the reused local entity. That leaves a single runtime copy
            // acting as both Predicted and Interpolated, which produces exactly the kind of
            // mixed rotation/position correction the audit was trying to eliminate.
            commands
                .entity(entity)
                .remove::<lightyear::prelude::Interpolated>();
            bevy::log::warn!(
                "sanitized conflicting prediction markers on local control entity {:?}: kept Predicted and removed Interpolated",
                entity
            );
        } else {
            // For non-controlled entities, observer interpolation is the authoritative visual lane.
            // If a previous owner-predicted entity falls back to observer mode, keep Interpolated
            // and remove the stale Predicted marker so local-only writers cannot linger.
            commands
                .entity(entity)
                .remove::<lightyear::prelude::Predicted>();
            bevy::log::warn!(
                "sanitized conflicting prediction markers on observer entity {:?}: kept Interpolated and removed Predicted",
                entity
            );
        }
    }
}

pub(crate) fn sync_controlled_entity_tags_system(
    mut commands: Commands<'_, '_>,
    time: Res<'_, Time>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    mut adoption_state: ResMut<'_, DeferredPredictedAdoptionState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    controlled_query: Query<'_, '_, (Entity, &'_ ControlledEntity)>,
    writer_query: Query<'_, '_, Entity, With<SimulationMotionWriter>>,
    guid_candidates: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Has<PlayerTag>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
        ),
    >,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let local_player_wire_id = local_player_entity_id.clone();
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
    let local_player_guid = parse_guid_from_entity_id(local_player_entity_id);
    let is_player_anchor_target = target_guid
        .zip(local_player_guid)
        .is_some_and(|(target, player)| target == player);
    let target_entity = target_guid.and_then(|guid| {
        let mut best_predicted: Option<Entity> = None;
        let mut best_interpolated: Option<Entity> = None;
        let mut best_confirmed: Option<Entity> = None;
        let mut best_player_anchor: Option<Entity> = None;
        for (entity, entity_guid, is_player_anchor, is_predicted, is_interpolated) in
            &guid_candidates
        {
            if entity_guid.0 != guid {
                continue;
            }
            if is_player_anchor {
                best_player_anchor = match best_player_anchor {
                    Some(current) if current.to_bits() <= entity.to_bits() => Some(current),
                    _ => Some(entity),
                };
            }
            if is_predicted {
                best_predicted = match best_predicted {
                    Some(current) if current.to_bits() <= entity.to_bits() => Some(current),
                    _ => Some(entity),
                };
                continue;
            }
            if is_interpolated {
                best_interpolated = match best_interpolated {
                    Some(current) if current.to_bits() <= entity.to_bits() => Some(current),
                    _ => Some(entity),
                };
                continue;
            }
            best_confirmed = match best_confirmed {
                Some(current) if current.to_bits() <= entity.to_bits() => Some(current),
                _ => Some(entity),
            };
        }

        if let Some(predicted) = best_predicted {
            adoption_state.missing_predicted_control_entity_id = None;
            return Some(predicted);
        }

        if is_player_anchor_target {
            // Free-roam is routed through the persisted player anchor. That entity is a
            // Sidereal-specific control lane and can temporarily remain confirmed-only while
            // the camera and UI stay usable. This is intentionally different from a stock
            // "always predict the controlled pawn" example.
            adoption_state.missing_predicted_control_entity_id = None;
            return best_player_anchor.or(best_confirmed).or(best_interpolated);
        }

        let missing_id = target_entity_id.clone().unwrap_or_else(|| guid.to_string());
        adoption_state.missing_predicted_control_entity_id = Some(missing_id.clone());
        let now_s = time.elapsed_secs_f64();
        if now_s - adoption_state.last_missing_predicted_warn_at_s >= 1.0 {
            bevy::log::warn!(
                "controlled runtime target {} has no Predicted clone yet; refusing to bind local control to confirmed/interpolated fallback",
                missing_id
            );
            adoption_state.last_missing_predicted_warn_at_s = now_s;
        }
        None
    });

    for (entity, controlled) in &controlled_query {
        if Some(entity) != target_entity {
            commands.entity(entity).remove::<ControlledEntity>();
        } else if controlled.player_entity_id != local_player_wire_id {
            commands.entity(entity).insert(ControlledEntity {
                entity_id: controlled.entity_id.clone(),
                player_entity_id: local_player_wire_id.clone(),
            });
        }
    }
    for entity in &writer_query {
        if Some(entity) != target_entity {
            commands.entity(entity).remove::<SimulationMotionWriter>();
        }
    }

    if let Some(entity) = target_entity {
        commands.entity(entity).insert((
            ControlledEntity {
                entity_id: target_entity_id.clone().unwrap_or_default(),
                player_entity_id: local_player_wire_id,
            },
            SimulationMotionWriter,
        ));
    }
}
