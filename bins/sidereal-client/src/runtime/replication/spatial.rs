// Replication adoption, control sync, and prediction runtime state.

use avian2d::prelude::{LinearVelocity, Position, Rotation};
use bevy::ecs::query::Has;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use lightyear::frame_interpolation::FrameInterpolate;
use lightyear::prediction::correction::CorrectionPolicy;
use lightyear::prediction::prelude::{PredictionManager, RollbackMode};
use lightyear::prelude::LocalTimeline;
use lightyear::prelude::PreSpawned;
use lightyear::prelude::client::Client;
use sidereal_game::{
    ActionQueue, BallisticProjectile, CollisionOutlineM, EntityGuid, Hardpoint, MountedOn,
    PlayerTag, SimulationMotionWriter, SizeM, SpriteShaderAssetId, VisualAssetId, WorldPosition,
    WorldRotation, resolve_world_position, resolve_world_rotation_rad,
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
    BootstrapWatchdogState, ControlBootstrapPhase, ControlBootstrapState,
    DeferredPredictedAdoptionState, PredictionBootstrapTuning, PredictionCorrectionTuning,
    PredictionRollbackStateTuning, RemoteEntityRegistry,
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
