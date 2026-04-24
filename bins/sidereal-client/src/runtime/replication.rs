//! Replication adoption, control sync, and prediction runtime state.

use avian2d::prelude::{AngularVelocity, LinearVelocity, Position, Rotation};
use bevy::ecs::query::Has;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use lightyear::frame_interpolation::FrameInterpolate;
use lightyear::prediction::correction::CorrectionPolicy;
use lightyear::prediction::prelude::{PredictionManager, RollbackMode};
use lightyear::prelude::Confirmed;
use lightyear::prelude::LocalTimeline;
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
    BootstrapWatchdogState, ClientControlRequestState, ControlBootstrapPhase,
    ControlBootstrapState, DeferredPredictedAdoptionState, PredictionBootstrapTuning,
    PredictionCorrectionTuning, PredictionRollbackStateTuning, RemoteEntityRegistry,
};

type MissingReplicatedSpatialQueryItem<'a> = (
    Entity,
    Option<&'a Position>,
    Option<&'a Rotation>,
    Option<&'a WorldPosition>,
    Option<&'a WorldRotation>,
);

type ParentSpatialQueryItem<'a> = (
    Has<Transform>,
    Has<GlobalTransform>,
    Has<Visibility>,
    Option<&'a Position>,
    Option<&'a Rotation>,
    Option<&'a WorldPosition>,
    Option<&'a WorldRotation>,
);

type ControlledTagGuidCandidate<'a> = (
    Entity,
    &'a EntityGuid,
    Has<PlayerTag>,
    Has<lightyear::prelude::Predicted>,
    Has<lightyear::prelude::Interpolated>,
);

type LocalPlayerAuthorityCandidate<'a> = (
    &'a EntityGuid,
    Option<&'a ControlledEntityGuid>,
    Has<lightyear::prelude::Predicted>,
);

#[derive(SystemParam)]
pub(crate) struct ControlledEntityTagInputs<'w, 's> {
    session: Res<'w, ClientSession>,
    player_view_state: Res<'w, LocalPlayerViewState>,
    adoption_state: ResMut<'w, DeferredPredictedAdoptionState>,
    control_bootstrap_state: ResMut<'w, ControlBootstrapState>,
    entity_registry: Res<'w, RuntimeEntityHierarchy>,
    controlled_query: Query<'w, 's, (Entity, &'static ControlledEntity)>,
    writer_query: Query<'w, 's, Entity, With<SimulationMotionWriter>>,
    guid_candidates: Query<'w, 's, ControlledTagGuidCandidate<'static>>,
}

fn bootstrap_planar_heading(
    rotation: Option<&Rotation>,
    world_rotation: Option<&WorldRotation>,
) -> Option<f32> {
    world_rotation
        .map(|value| value.0)
        .filter(|value| value.is_finite())
        .or_else(|| {
            resolve_world_rotation_rad(rotation, world_rotation).filter(|value| value.is_finite())
        })
        .map(|value| value as f32)
        .or(Some(0.0))
}

fn intended_control_target_guid(
    session: &ClientSession,
    player_view_state: &LocalPlayerViewState,
    request_state: Option<&ClientControlRequestState>,
) -> Option<uuid::Uuid> {
    request_state
        .and_then(|state| state.pending_controlled_entity_id.as_deref())
        .or(player_view_state.desired_controlled_entity_id.as_deref())
        .or(player_view_state.controlled_entity_id.as_deref())
        .or(session.player_entity_id.as_deref())
        .and_then(parse_guid_from_entity_id)
}

#[allow(clippy::type_complexity)]
pub(crate) fn mark_new_ballistic_projectiles_prespawned(
    mut commands: Commands<'_, '_>,
    timeline: Res<'_, LocalTimeline>,
    projectiles: Query<
        '_,
        '_,
        (Entity, &'_ BallisticProjectile),
        (
            With<BallisticProjectile>,
            Added<BallisticProjectile>,
            Without<PreSpawned>,
        ),
    >,
) {
    for (entity, projectile) in &projectiles {
        commands.entity(entity).insert(PreSpawned::new(
            projectile.prespawn_hash_for_tick(timeline.tick().0),
        ));
    }
}

pub(crate) fn ensure_replicated_entity_spatial_components(
    mut commands: Commands<'_, '_>,
    missing_transform: Query<
        '_,
        '_,
        MissingReplicatedSpatialQueryItem<'_>,
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
            bootstrap_planar_heading(rotation, world_rotation),
        ) && planar_position.is_finite()
            && heading.is_finite()
        {
            transform.translation.x = planar_position.x as f32;
            transform.translation.y = planar_position.y as f32;
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
    parent_components: Query<'_, '_, ParentSpatialQueryItem<'_>>,
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
            bootstrap_planar_heading(rotation, world_rotation),
        ) && planar_position.is_finite()
            && heading.is_finite()
        {
            transform.translation.x = planar_position.x as f32;
            transform.translation.y = planar_position.y as f32;
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
    parent_components: Query<'_, '_, ParentSpatialQueryItem<'_>>,
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
        bootstrap_planar_heading(rotation, world_rotation),
    ) && planar_position.is_finite()
        && heading.is_finite()
    {
        transform.translation.x = planar_position.x as f32;
        transform.translation.y = planar_position.y as f32;
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

pub(crate) fn should_defer_spatial_root_adoption(
    is_spatial_root: bool,
    has_position: bool,
    has_rotation: bool,
    has_world_position: bool,
    has_world_rotation: bool,
) -> bool {
    is_spatial_root
        && ((!has_position && !has_world_position) || (!has_rotation && !has_world_rotation))
}

pub(crate) fn is_canonical_runtime_entity_lane(
    is_replicated: bool,
    is_predicted: bool,
    is_interpolated: bool,
) -> bool {
    is_replicated && !is_predicted && !is_interpolated
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

pub(crate) fn has_local_player_runtime_presence<'a, I>(
    entity_registry: &RuntimeEntityHierarchy,
    local_player_entity_id: &str,
    guid_candidates: I,
) -> bool
where
    I: IntoIterator<Item = &'a EntityGuid>,
{
    if entity_registry
        .by_entity_id
        .contains_key(local_player_entity_id)
    {
        return true;
    }

    let local_player_guid = parse_guid_from_entity_id(local_player_entity_id)
        .or_else(|| uuid::Uuid::parse_str(local_player_entity_id).ok());
    let Some(local_player_guid) = local_player_guid else {
        return false;
    };

    if entity_registry
        .by_entity_id
        .contains_key(local_player_guid.to_string().as_str())
    {
        return true;
    }

    guid_candidates
        .into_iter()
        .any(|entity_guid| entity_guid.0 == local_player_guid)
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
        .or_else(|| Some(control_guid.to_string()))
}

fn resolve_control_target_entity_id(
    entity_registry: &RuntimeEntityHierarchy,
    local_player_entity_id: &str,
    controlled_entity_id: Option<&str>,
) -> Option<String> {
    match controlled_entity_id {
        Some(id) if entity_registry.by_entity_id.contains_key(id) => Some(id.to_string()),
        Some(id) => runtime_entity_id_from_guid(entity_registry, local_player_entity_id, id)
            .or_else(|| Some(id.to_string())),
        None => runtime_entity_id_from_guid(
            entity_registry,
            local_player_entity_id,
            local_player_entity_id,
        )
        .or_else(|| Some(local_player_entity_id.to_string())),
    }
}

fn resolve_local_player_authoritative_control_entity_id<'a, I>(
    entity_registry: &RuntimeEntityHierarchy,
    local_player_entity_id: &str,
    candidates: I,
) -> Option<String>
where
    I: IntoIterator<Item = (&'a EntityGuid, Option<&'a ControlledEntityGuid>, bool)>,
{
    let mut fallback = None;

    for (entity_guid, controlled_entity_guid, is_predicted) in candidates {
        if !ids_refer_to_same_guid(local_player_entity_id, entity_guid.0.to_string().as_str()) {
            continue;
        }

        let resolved = resolve_authoritative_control_entity_id_from_registry(
            entity_registry,
            local_player_entity_id,
            controlled_entity_guid,
        );

        if is_predicted {
            if resolved.is_some() {
                return resolved;
            }
        } else if fallback.is_none() {
            fallback = resolved;
        }
    }

    fallback
}

pub(crate) fn transition_world_loading_to_in_world(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    session: Res<'_, ClientSession>,
    session_ready: Res<'_, SessionReadyState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    entity_guids: Query<'_, '_, &'_ EntityGuid>,
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
    let has_local_player_entity = has_local_player_runtime_presence(
        &entity_registry,
        local_player_entity_id,
        entity_guids.iter(),
    );
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

/// Lightyear still has an upstream TODO for the case where `Interpolated` is added to an
/// already-existing replicated entity. Sidereal hits that path during dynamic control handoff and
/// visibility-driven role churn, where an entity can already have raw Avian motion components
/// before the interpolated lane is fully bootstrapped.
///
/// Seed the missing `Confirmed<T>` mirrors from the current raw values so the entity can enter the
/// observer interpolation path immediately instead of waiting for a later delta to populate the
/// confirmed lane.
#[allow(clippy::type_complexity)]
pub(crate) fn bootstrap_missing_confirmed_components_for_interpolated_entities(
    mut commands: Commands<'_, '_>,
    entities: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ LinearVelocity>,
            Option<&'_ AngularVelocity>,
            Option<&'_ Confirmed<Position>>,
            Option<&'_ Confirmed<Rotation>>,
            Option<&'_ Confirmed<LinearVelocity>>,
            Option<&'_ Confirmed<AngularVelocity>>,
        ),
        With<lightyear::prelude::Interpolated>,
    >,
) {
    for (
        entity,
        position,
        rotation,
        linear_velocity,
        angular_velocity,
        confirmed_position,
        confirmed_rotation,
        confirmed_linear_velocity,
        confirmed_angular_velocity,
    ) in &entities
    {
        let mut entity_commands = commands.entity(entity);
        if confirmed_position.is_none()
            && let Some(position) = position
        {
            entity_commands.insert(Confirmed(*position));
        }
        if confirmed_rotation.is_none()
            && let Some(rotation) = rotation
        {
            entity_commands.insert(Confirmed(*rotation));
        }
        if confirmed_linear_velocity.is_none()
            && let Some(linear_velocity) = linear_velocity
        {
            entity_commands.insert(Confirmed(*linear_velocity));
        }
        if confirmed_angular_velocity.is_none()
            && let Some(angular_velocity) = angular_velocity
        {
            entity_commands.insert(Confirmed(*angular_velocity));
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(crate) fn adopt_native_lightyear_replicated_entities(
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
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
        let is_canonical_runtime_entity =
            is_canonical_runtime_entity_lane(is_replicated, _is_predicted, _is_interpolated);
        let is_local_player_entity =
            ids_refer_to_same_guid(runtime_entity_id.as_str(), local_player_entity_id);
        let is_local_controlled_entity = (is_root_entity || is_local_player_entity)
            && player_view_state.controlled_entity_id.as_deref()
                == Some(runtime_entity_id.as_str());
        let is_spatial_root = is_root_entity && size_m.is_some();
        if should_defer_spatial_root_adoption(
            is_spatial_root,
            position.is_some(),
            rotation.is_some(),
            world_position.is_some(),
            world_rotation.is_some(),
        ) {
            // Avoid adopting spatial roots at (0,0) until we at least have a usable pose.
            // Stationary remote observers may legitimately bootstrap without velocity.
            continue;
        }
        if should_defer_controlled_predicted_adoption(
            is_local_controlled_entity,
            position.is_some(),
            rotation.is_some(),
            linear_velocity.is_some(),
        ) {
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

        if is_canonical_runtime_entity
            && let Some(&existing_entity) =
                entity_registry.by_entity_id.get(runtime_entity_id.as_str())
            && existing_entity != entity
            && live_entities.get(existing_entity).is_err()
        {
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
        if is_canonical_runtime_entity {
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
            if is_canonical_runtime_entity {
                remote_registry
                    .by_entity_id
                    .insert(runtime_entity_id, entity);
            }
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

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{
        bootstrap_missing_confirmed_components_for_interpolated_entities,
        has_local_player_runtime_presence, is_canonical_runtime_entity_lane,
        resolve_control_target_entity_id, resolve_local_player_authoritative_control_entity_id,
        sanitize_conflicting_prediction_interpolation_markers_on_interpolated_added,
        sanitize_conflicting_prediction_interpolation_markers_on_predicted_added,
        should_defer_controlled_predicted_adoption, should_defer_spatial_root_adoption,
        sync_controlled_entity_tags_system,
    };
    use crate::runtime::app_state::{ClientSession, LocalPlayerViewState};
    use crate::runtime::components::ControlledEntity;
    use crate::runtime::resources::{
        ClientControlRequestState, ControlBootstrapPhase, ControlBootstrapState,
        DeferredPredictedAdoptionState,
    };
    use avian2d::prelude::{AngularVelocity, LinearVelocity, Position, Rotation};
    use bevy::app::Update;
    use bevy::prelude::{App, Entity, Time, Vec2};
    use lightyear::prelude::Confirmed;
    use sidereal_game::{ControlledEntityGuid, EntityGuid, SimulationMotionWriter};
    use sidereal_runtime_sync::RuntimeEntityHierarchy;
    use uuid::Uuid;

    #[test]
    fn spatial_root_adoption_allows_stationary_pose_complete_remote_entities() {
        assert!(!should_defer_spatial_root_adoption(
            true, true, true, false, false
        ));
    }

    #[test]
    fn spatial_root_adoption_still_defers_when_pose_is_missing() {
        assert!(should_defer_spatial_root_adoption(
            true, false, true, false, false
        ));
        assert!(should_defer_spatial_root_adoption(
            true, true, false, false, false
        ));
    }

    #[test]
    fn controlled_predicted_adoption_still_requires_velocity() {
        assert!(should_defer_controlled_predicted_adoption(
            true, true, true, false
        ));
        assert!(!should_defer_controlled_predicted_adoption(
            false, true, true, false
        ));
    }

    #[test]
    fn only_confirmed_lane_is_canonical_runtime_entity() {
        assert!(is_canonical_runtime_entity_lane(true, false, false));
        assert!(!is_canonical_runtime_entity_lane(true, false, true));
        assert!(!is_canonical_runtime_entity_lane(true, true, false));
    }

    #[test]
    fn control_bootstrap_generation_prefers_authoritative_server_generation() {
        assert_eq!(super::control_bootstrap_generation(4, 9, false), 9);
        assert_eq!(super::control_bootstrap_generation(0, 3, true), 3);
    }

    #[test]
    fn control_bootstrap_generation_falls_back_to_local_increment_only_when_needed() {
        assert_eq!(super::control_bootstrap_generation(0, 0, true), 1);
        assert_eq!(super::control_bootstrap_generation(7, 0, true), 8);
        assert_eq!(super::control_bootstrap_generation(7, 0, false), 7);
    }

    #[test]
    fn local_player_control_resolution_prefers_predicted_player_anchor() {
        let mut registry = RuntimeEntityHierarchy::default();
        registry.by_entity_id.insert(
            "ce9e421c-8b62-458a-803e-51e9ad272908".to_string(),
            Entity::from_bits(1),
        );

        let player_guid = EntityGuid(
            Uuid::parse_str("1521601b-7e69-4700-853f-eb1eb3a41199").expect("valid player guid"),
        );
        let control_guid =
            ControlledEntityGuid(Some("ce9e421c-8b62-458a-803e-51e9ad272908".to_string()));
        let stale_confirmed_guid = ControlledEntityGuid(Some("stale-should-not-win".to_string()));

        let resolved = resolve_local_player_authoritative_control_entity_id(
            &registry,
            "1521601b-7e69-4700-853f-eb1eb3a41199",
            [
                (&player_guid, Some(&stale_confirmed_guid), false),
                (&player_guid, Some(&control_guid), true),
            ],
        );

        assert_eq!(
            resolved.as_deref(),
            Some("ce9e421c-8b62-458a-803e-51e9ad272908")
        );
    }

    #[test]
    fn local_player_control_resolution_falls_back_to_non_predicted_anchor() {
        let mut registry = RuntimeEntityHierarchy::default();
        registry.by_entity_id.insert(
            "ce9e421c-8b62-458a-803e-51e9ad272908".to_string(),
            Entity::from_bits(1),
        );

        let player_guid = EntityGuid(
            Uuid::parse_str("1521601b-7e69-4700-853f-eb1eb3a41199").expect("valid player guid"),
        );
        let control_guid =
            ControlledEntityGuid(Some("ce9e421c-8b62-458a-803e-51e9ad272908".to_string()));

        let resolved = resolve_local_player_authoritative_control_entity_id(
            &registry,
            "1521601b-7e69-4700-853f-eb1eb3a41199",
            [(&player_guid, Some(&control_guid), false)],
        );

        assert_eq!(
            resolved.as_deref(),
            Some("ce9e421c-8b62-458a-803e-51e9ad272908")
        );
    }

    #[test]
    fn local_player_control_resolution_uses_raw_guid_when_registry_is_not_ready() {
        let registry = RuntimeEntityHierarchy::default();
        let player_guid = EntityGuid(
            Uuid::parse_str("1521601b-7e69-4700-853f-eb1eb3a41199").expect("valid player guid"),
        );
        let control_guid =
            ControlledEntityGuid(Some("ce9e421c-8b62-458a-803e-51e9ad272908".to_string()));

        let resolved = resolve_local_player_authoritative_control_entity_id(
            &registry,
            "1521601b-7e69-4700-853f-eb1eb3a41199",
            [(&player_guid, Some(&control_guid), true)],
        );

        assert_eq!(
            resolved.as_deref(),
            Some("ce9e421c-8b62-458a-803e-51e9ad272908")
        );
    }

    #[test]
    fn controlled_tag_target_falls_back_to_raw_guid_when_registry_is_not_ready() {
        let registry = RuntimeEntityHierarchy::default();

        let resolved = resolve_control_target_entity_id(
            &registry,
            "1521601b-7e69-4700-853f-eb1eb3a41199",
            Some("ce9e421c-8b62-458a-803e-51e9ad272908"),
        );

        assert_eq!(
            resolved.as_deref(),
            Some("ce9e421c-8b62-458a-803e-51e9ad272908")
        );
    }

    #[test]
    fn world_loading_presence_accepts_guid_only_local_player_clone() {
        let registry = RuntimeEntityHierarchy::default();
        let player_guid =
            Uuid::parse_str("1521601b-7e69-4700-853f-eb1eb3a41199").expect("valid player guid");
        let player_entity_guid = EntityGuid(player_guid);

        let present = has_local_player_runtime_presence(
            &registry,
            "1521601b-7e69-4700-853f-eb1eb3a41199",
            [&player_entity_guid],
        );

        assert!(present);
    }

    #[test]
    fn world_loading_presence_rejects_missing_local_player_guid() {
        let registry = RuntimeEntityHierarchy::default();
        let other_guid =
            Uuid::parse_str("ce9e421c-8b62-458a-803e-51e9ad272908").expect("valid other guid");
        let other_entity_guid = EntityGuid(other_guid);

        let present = has_local_player_runtime_presence(
            &registry,
            "1521601b-7e69-4700-853f-eb1eb3a41199",
            [&other_entity_guid],
        );

        assert!(!present);
    }

    #[test]
    fn interpolated_entities_seed_missing_confirmed_motion_components() {
        let mut app = App::new();
        app.add_systems(
            Update,
            bootstrap_missing_confirmed_components_for_interpolated_entities,
        );

        let entity = app
            .world_mut()
            .spawn((
                lightyear::prelude::Interpolated,
                Position(Vec2::new(10.0, -4.0).into()),
                Rotation::radians(0.25),
                LinearVelocity(Vec2::new(1.5, -2.0).into()),
                AngularVelocity(0.75),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .get::<Confirmed<Position>>(entity)
                .map(|value| value.0),
            Some(Position(Vec2::new(10.0, -4.0).into()))
        );
        assert_eq!(
            app.world()
                .get::<Confirmed<Rotation>>(entity)
                .map(|value| value.0),
            Some(Rotation::radians(0.25))
        );
        assert_eq!(
            app.world()
                .get::<Confirmed<LinearVelocity>>(entity)
                .map(|value| value.0),
            Some(LinearVelocity(Vec2::new(1.5, -2.0).into()))
        );
        assert_eq!(
            app.world()
                .get::<Confirmed<AngularVelocity>>(entity)
                .map(|value| value.0),
            Some(AngularVelocity(0.75))
        );
    }

    #[test]
    fn interpolated_entities_do_not_overwrite_existing_confirmed_motion_components() {
        let mut app = App::new();
        app.add_systems(
            Update,
            bootstrap_missing_confirmed_components_for_interpolated_entities,
        );

        let entity = app
            .world_mut()
            .spawn((
                lightyear::prelude::Interpolated,
                Position(Vec2::new(10.0, -4.0).into()),
                Confirmed(Position(Vec2::new(99.0, 42.0).into())),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .get::<Confirmed<Position>>(entity)
                .map(|value| value.0),
            Some(Position(Vec2::new(99.0, 42.0).into()))
        );
    }

    #[test]
    fn conflicting_marker_transition_keeps_predicted_for_local_control_target() {
        let mut app = App::new();
        app.insert_resource(ClientSession {
            player_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("ce9e421c-8b62-458a-803e-51e9ad272908".to_string()),
            ..Default::default()
        });
        app.add_observer(sanitize_conflicting_prediction_interpolation_markers_on_predicted_added);
        app.add_observer(
            sanitize_conflicting_prediction_interpolation_markers_on_interpolated_added,
        );

        let entity = app
            .world_mut()
            .spawn((
                EntityGuid(Uuid::parse_str("ce9e421c-8b62-458a-803e-51e9ad272908").unwrap()),
                lightyear::prelude::Interpolated,
                lightyear::prelude::Predicted,
            ))
            .id();

        app.update();

        assert!(
            app.world()
                .get::<lightyear::prelude::Predicted>(entity)
                .is_some()
        );
        assert!(
            app.world()
                .get::<lightyear::prelude::Interpolated>(entity)
                .is_none()
        );
    }

    #[test]
    fn conflicting_marker_transition_keeps_predicted_for_pending_control_target() {
        let mut app = App::new();
        app.insert_resource(ClientSession {
            player_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_string()),
            desired_controlled_entity_id: Some("ce9e421c-8b62-458a-803e-51e9ad272908".to_string()),
            ..Default::default()
        });
        app.insert_resource(ClientControlRequestState {
            pending_controlled_entity_id: Some("ce9e421c-8b62-458a-803e-51e9ad272908".to_string()),
            pending_request_seq: Some(1),
            ..Default::default()
        });
        app.add_observer(sanitize_conflicting_prediction_interpolation_markers_on_predicted_added);
        app.add_observer(
            sanitize_conflicting_prediction_interpolation_markers_on_interpolated_added,
        );

        let entity = app
            .world_mut()
            .spawn((
                EntityGuid(Uuid::parse_str("ce9e421c-8b62-458a-803e-51e9ad272908").unwrap()),
                lightyear::prelude::Interpolated,
                lightyear::prelude::Predicted,
            ))
            .id();

        app.update();

        assert!(
            app.world()
                .get::<lightyear::prelude::Predicted>(entity)
                .is_some()
        );
        assert!(
            app.world()
                .get::<lightyear::prelude::Interpolated>(entity)
                .is_none()
        );
    }

    #[test]
    fn conflicting_marker_transition_drops_predicted_from_previous_target_during_pending_handoff() {
        let mut app = App::new();
        app.insert_resource(ClientSession {
            player_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_string()),
            desired_controlled_entity_id: Some("ce9e421c-8b62-458a-803e-51e9ad272908".to_string()),
            ..Default::default()
        });
        app.insert_resource(ClientControlRequestState {
            pending_controlled_entity_id: Some("ce9e421c-8b62-458a-803e-51e9ad272908".to_string()),
            pending_request_seq: Some(1),
            ..Default::default()
        });
        app.add_observer(sanitize_conflicting_prediction_interpolation_markers_on_predicted_added);
        app.add_observer(
            sanitize_conflicting_prediction_interpolation_markers_on_interpolated_added,
        );

        let entity = app
            .world_mut()
            .spawn((
                EntityGuid(Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").unwrap()),
                lightyear::prelude::Interpolated,
                lightyear::prelude::Predicted,
            ))
            .id();

        app.update();

        assert!(
            app.world()
                .get::<lightyear::prelude::Interpolated>(entity)
                .is_some()
        );
        assert!(
            app.world()
                .get::<lightyear::prelude::Predicted>(entity)
                .is_none()
        );
    }

    #[test]
    fn conflicting_marker_transition_keeps_interpolated_for_observer_entity() {
        let mut app = App::new();
        app.insert_resource(ClientSession {
            player_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("ce9e421c-8b62-458a-803e-51e9ad272908".to_string()),
            ..Default::default()
        });
        app.add_observer(sanitize_conflicting_prediction_interpolation_markers_on_predicted_added);
        app.add_observer(
            sanitize_conflicting_prediction_interpolation_markers_on_interpolated_added,
        );

        let entity = app
            .world_mut()
            .spawn((
                EntityGuid(Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").unwrap()),
                lightyear::prelude::Interpolated,
                lightyear::prelude::Predicted,
            ))
            .id();

        app.update();

        assert!(
            app.world()
                .get::<lightyear::prelude::Interpolated>(entity)
                .is_some()
        );
        assert!(
            app.world()
                .get::<lightyear::prelude::Predicted>(entity)
                .is_none()
        );
    }

    #[test]
    fn controlled_ship_without_predicted_clone_stays_pending_bootstrap() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(ClientSession {
            player_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("ce9e421c-8b62-458a-803e-51e9ad272908".to_string()),
            ..Default::default()
        });
        app.insert_resource(DeferredPredictedAdoptionState::default());
        app.insert_resource(ControlBootstrapState::default());
        app.insert_resource(RuntimeEntityHierarchy::default());
        app.add_systems(Update, sync_controlled_entity_tags_system);

        let entity = app
            .world_mut()
            .spawn((EntityGuid(
                Uuid::parse_str("ce9e421c-8b62-458a-803e-51e9ad272908").unwrap(),
            ),))
            .id();

        app.update();

        assert!(app.world().get::<ControlledEntity>(entity).is_none());
        assert!(app.world().get::<SimulationMotionWriter>(entity).is_none());
        assert_eq!(
            app.world().resource::<ControlBootstrapState>().phase,
            ControlBootstrapPhase::PendingPredicted {
                target_entity_id: "ce9e421c-8b62-458a-803e-51e9ad272908".to_string(),
                generation: 1,
            }
        );
    }

    #[test]
    fn controlled_ship_binds_only_when_predicted_clone_exists() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(ClientSession {
            player_entity_id: Some("1521601b-7e69-4700-853f-eb1eb3a41199".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("ce9e421c-8b62-458a-803e-51e9ad272908".to_string()),
            ..Default::default()
        });
        app.insert_resource(DeferredPredictedAdoptionState::default());
        app.insert_resource(ControlBootstrapState::default());
        app.insert_resource(RuntimeEntityHierarchy::default());
        app.add_systems(Update, sync_controlled_entity_tags_system);

        let entity = app
            .world_mut()
            .spawn((
                EntityGuid(Uuid::parse_str("ce9e421c-8b62-458a-803e-51e9ad272908").unwrap()),
                lightyear::prelude::Predicted,
            ))
            .id();

        app.update();

        assert!(app.world().get::<ControlledEntity>(entity).is_some());
        assert!(app.world().get::<SimulationMotionWriter>(entity).is_some());
        assert_eq!(
            app.world().resource::<ControlBootstrapState>().phase,
            ControlBootstrapPhase::ActivePredicted {
                target_entity_id: "ce9e421c-8b62-458a-803e-51e9ad272908".to_string(),
                generation: 1,
                entity,
            }
        );
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_local_player_view_state_system(
    session: Res<'_, ClientSession>,
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    request_state: Res<'_, super::resources::ClientControlRequestState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    player_query: Query<'_, '_, LocalPlayerAuthorityCandidate<'_>, With<PlayerTag>>,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    if let Some(authoritative_controlled_id) = resolve_local_player_authoritative_control_entity_id(
        &entity_registry,
        local_player_entity_id,
        player_query,
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
fn sanitize_conflicting_prediction_interpolation_markers_for_entity(
    commands: &mut Commands<'_, '_>,
    entity: Entity,
    target_guid: Option<uuid::Uuid>,
    conflicting_entities: &Query<
        '_,
        '_,
        (
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
    let Ok((entity_guid, is_controlled, is_motion_writer)) = conflicting_entities.get(entity)
    else {
        return;
    };
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

#[allow(clippy::type_complexity)]
pub(crate) fn sanitize_conflicting_prediction_interpolation_markers_on_predicted_added(
    trigger: On<Add, lightyear::prelude::Predicted>,
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    request_state: Option<Res<'_, ClientControlRequestState>>,
    conflicting_entities: Query<
        '_,
        '_,
        (
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
    sanitize_conflicting_prediction_interpolation_markers_for_entity(
        &mut commands,
        trigger.entity,
        intended_control_target_guid(&session, &player_view_state, request_state.as_deref()),
        &conflicting_entities,
    );
}

fn control_bootstrap_generation(
    current_generation: u64,
    authoritative_generation: u64,
    target_changed: bool,
) -> u64 {
    if authoritative_generation > 0 {
        authoritative_generation
    } else if target_changed {
        current_generation.saturating_add(1)
    } else {
        current_generation
    }
}

fn rewrite_control_bootstrap_phase_generation(
    phase: &ControlBootstrapPhase,
    generation: u64,
) -> ControlBootstrapPhase {
    match phase {
        ControlBootstrapPhase::Idle => ControlBootstrapPhase::Idle,
        ControlBootstrapPhase::PendingPredicted {
            target_entity_id, ..
        } => ControlBootstrapPhase::PendingPredicted {
            target_entity_id: target_entity_id.clone(),
            generation,
        },
        ControlBootstrapPhase::ActiveAnchor {
            target_entity_id, ..
        } => ControlBootstrapPhase::ActiveAnchor {
            target_entity_id: target_entity_id.clone(),
            generation,
        },
        ControlBootstrapPhase::ActivePredicted {
            target_entity_id,
            entity,
            ..
        } => ControlBootstrapPhase::ActivePredicted {
            target_entity_id: target_entity_id.clone(),
            generation,
            entity: *entity,
        },
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn sanitize_conflicting_prediction_interpolation_markers_on_interpolated_added(
    trigger: On<Add, lightyear::prelude::Interpolated>,
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    request_state: Option<Res<'_, ClientControlRequestState>>,
    conflicting_entities: Query<
        '_,
        '_,
        (
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
    sanitize_conflicting_prediction_interpolation_markers_for_entity(
        &mut commands,
        trigger.entity,
        intended_control_target_guid(&session, &player_view_state, request_state.as_deref()),
        &conflicting_entities,
    );
}

pub(crate) fn sync_controlled_entity_tags_system(
    mut commands: Commands<'_, '_>,
    time: Res<'_, Time>,
    mut inputs: ControlledEntityTagInputs<'_, '_>,
) {
    let Some(local_player_entity_id) = inputs.session.player_entity_id.as_ref() else {
        return;
    };
    let local_player_wire_id = local_player_entity_id.clone();
    let target_entity_id = resolve_control_target_entity_id(
        &inputs.entity_registry,
        local_player_entity_id,
        inputs.player_view_state.controlled_entity_id.as_deref(),
    );
    let target_changed = inputs
        .control_bootstrap_state
        .authoritative_target_entity_id
        != target_entity_id;
    let desired_generation = control_bootstrap_generation(
        inputs.control_bootstrap_state.generation,
        inputs.player_view_state.controlled_entity_generation,
        target_changed,
    );
    let target_guid = target_entity_id
        .as_ref()
        .and_then(|id| parse_guid_from_entity_id(id));
    if target_changed {
        inputs.control_bootstrap_state.generation = desired_generation;
        inputs
            .control_bootstrap_state
            .authoritative_target_entity_id = target_entity_id.clone();
        inputs.control_bootstrap_state.last_transition_at_s = time.elapsed_secs_f64();
        inputs.control_bootstrap_state.phase = match target_entity_id.as_ref() {
            Some(target_entity_id) => ControlBootstrapPhase::PendingPredicted {
                target_entity_id: target_entity_id.clone(),
                generation: desired_generation,
            },
            None => ControlBootstrapPhase::Idle,
        };
    } else if desired_generation != inputs.control_bootstrap_state.generation {
        inputs.control_bootstrap_state.generation = desired_generation;
        inputs.control_bootstrap_state.phase = rewrite_control_bootstrap_phase_generation(
            &inputs.control_bootstrap_state.phase,
            desired_generation,
        );
    }
    let local_player_guid = parse_guid_from_entity_id(local_player_entity_id);
    let is_player_anchor_target = target_guid
        .zip(local_player_guid)
        .is_some_and(|(target, player)| target == player);
    let target_entity = target_guid.and_then(|guid| {
        let mut best_predicted: Option<(Entity, String)> = None;
        let mut best_interpolated: Option<(Entity, String)> = None;
        let mut best_confirmed: Option<(Entity, String)> = None;
        let mut best_player_anchor: Option<(Entity, String)> = None;
        for (entity, entity_guid, is_player_anchor, is_predicted, is_interpolated) in
            &inputs.guid_candidates
        {
            if entity_guid.0 != guid {
                continue;
            }
            let runtime_entity_id = entity_guid.0.to_string();
            if is_player_anchor {
                best_player_anchor = match best_player_anchor {
                    Some((current, _)) if current.to_bits() <= entity.to_bits() => {
                        Some((current, runtime_entity_id.clone()))
                    }
                    _ => Some((entity, runtime_entity_id.clone())),
                };
            }
            if is_predicted {
                best_predicted = match best_predicted {
                    Some((current, _)) if current.to_bits() <= entity.to_bits() => {
                        Some((current, runtime_entity_id.clone()))
                    }
                    _ => Some((entity, runtime_entity_id.clone())),
                };
                continue;
            }
            if is_interpolated {
                best_interpolated = match best_interpolated {
                    Some((current, _)) if current.to_bits() <= entity.to_bits() => {
                        Some((current, runtime_entity_id.clone()))
                    }
                    _ => Some((entity, runtime_entity_id.clone())),
                };
                continue;
            }
            best_confirmed = match best_confirmed {
                Some((current, _)) if current.to_bits() <= entity.to_bits() => {
                    Some((current, runtime_entity_id))
                }
                _ => Some((entity, runtime_entity_id)),
            };
        }

        if let Some((predicted, predicted_entity_id)) = best_predicted {
            inputs.adoption_state.missing_predicted_control_entity_id = None;
            inputs.control_bootstrap_state.phase = ControlBootstrapPhase::ActivePredicted {
                target_entity_id: predicted_entity_id,
                generation: inputs.control_bootstrap_state.generation,
                entity: predicted,
            };
            inputs.control_bootstrap_state.last_transition_at_s = time.elapsed_secs_f64();
            return Some(predicted);
        }

        if is_player_anchor_target {
            // Free-roam is routed through the persisted player anchor. That entity is a
            // Sidereal-specific control lane and can temporarily remain confirmed-only while
            // the camera and UI stay usable. This is intentionally different from a stock
            // "always predict the controlled pawn" example.
            inputs.adoption_state.missing_predicted_control_entity_id = None;
            if let Some((anchor, anchor_entity_id)) =
                best_player_anchor.or(best_confirmed).or(best_interpolated)
            {
                inputs.control_bootstrap_state.phase = ControlBootstrapPhase::ActiveAnchor {
                    target_entity_id: anchor_entity_id,
                    generation: inputs.control_bootstrap_state.generation,
                };
                inputs.control_bootstrap_state.last_transition_at_s = time.elapsed_secs_f64();
                return Some(anchor);
            }
            return None;
        }

        let missing_id = target_entity_id.clone().unwrap_or_else(|| guid.to_string());
        inputs.adoption_state.missing_predicted_control_entity_id = Some(missing_id.clone());
        inputs.control_bootstrap_state.phase = ControlBootstrapPhase::PendingPredicted {
            target_entity_id: missing_id.clone(),
            generation: inputs.control_bootstrap_state.generation,
        };
        let now_s = time.elapsed_secs_f64();
        if now_s - inputs.adoption_state.last_missing_predicted_warn_at_s >= 1.0 {
            bevy::log::warn!(
                "controlled runtime target {} has no Predicted clone yet; refusing to bind local control to confirmed/interpolated fallback",
                missing_id
            );
            inputs.adoption_state.last_missing_predicted_warn_at_s = now_s;
        }
        None
    });
    if target_guid.is_none() {
        inputs.control_bootstrap_state.phase = ControlBootstrapPhase::Idle;
    }

    for (entity, controlled) in &inputs.controlled_query {
        if Some(entity) != target_entity {
            commands.entity(entity).remove::<ControlledEntity>();
        } else if controlled.player_entity_id != local_player_wire_id {
            commands.entity(entity).insert(ControlledEntity {
                entity_id: controlled.entity_id.clone(),
                player_entity_id: local_player_wire_id.clone(),
            });
        }
    }
    for entity in &inputs.writer_query {
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
