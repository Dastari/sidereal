// F3 debug overlay: toggle, snapshot collection, and snapshot-driven gizmo drawing.

use avian2d::prelude::{AngularVelocity, LinearVelocity, Position, Rotation};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use lightyear::interpolation::interpolation_history::ConfirmedHistory;
use lightyear::prelude::{ConfirmedTick, LocalTimeline};
use sidereal_core::SIM_TICK_HZ;
use sidereal_game::{
    BallisticWeapon, CollisionAabbM, CollisionOutlineM, DisplayName, Engine, EntityGuid,
    EntityLabels, Hardpoint, MountedOn, ParentGuid, PlanetBodyShaderSettings, PlayerTag, SizeM,
    WorldPosition, WorldRotation,
};
use sidereal_runtime_sync::parse_guid_from_entity_id;
use std::collections::HashMap;

use super::app_state::{ClientSession, LocalPlayerViewState};
use super::assets::{LocalAssetManager, RuntimeAssetDependencyState, RuntimeAssetHttpFetchState};
use super::backdrop::{
    AsteroidSpriteShaderMaterial, PlanetVisualMaterial, RuntimeEffectMaterial,
    StreamedSpriteShaderMaterial,
};
use super::components::{
    ControlledEntity, DebugVelocityArrowHeadLower, DebugVelocityArrowHeadUpper,
    DebugVelocityArrowShaft, RuntimeWorldVisualFamily, RuntimeWorldVisualPass, StreamedVisualChild,
    SuppressedPredictedDuplicateVisual, WeaponImpactSpark, WeaponImpactSparkPool, WeaponTracerBolt,
    WeaponTracerPool, WorldEntity,
};
use super::dev_console::{DevConsoleState, is_console_open};
use super::resources::{
    ControlBootstrapPhase, ControlBootstrapState, DebugCollisionShape, DebugControlledLane,
    DebugEntityLane, DebugOverlayEntity, DebugOverlaySnapshot, DebugOverlayState,
    DebugOverlayStats, DebugSeverity, DebugTextRow, DuplicateVisualResolutionState,
    HudPerfCounters, NativePredictionRecoveryState, PredictionCorrectionTuning,
    RenderLayerPerfCounters, RuntimeAssetPerfCounters, RuntimeStallDiagnostics,
};
use super::transforms::interpolated_presentation_ready;

const DEBUG_OVERLAY_Z_OFFSET: f32 = 6.0;
const REPLICATED_OVERLAY_Z_STEP: f32 = 0.0;
const INTERPOLATED_OVERLAY_Z_STEP: f32 = 0.18;
const PREDICTED_OVERLAY_Z_STEP: f32 = 0.36;
const CONFIRMED_OVERLAY_Z_STEP: f32 = 0.54;
const CONFIRMED_OVERLAY_POSITION_EPSILON_M: f32 = 0.05;
const CONFIRMED_OVERLAY_ROTATION_EPSILON_RAD: f32 = 0.01;
const VELOCITY_ARROW_SCALE: f32 = 0.5;
const HARDPOINT_CROSS_HALF_SIZE: f32 = 2.0;
const COMPONENT_MARKER_HALF_SIZE: f32 = 2.6;
const VELOCITY_ARROW_SHAFT_THICKNESS: f32 = 0.18;
const VELOCITY_ARROW_HEAD_LENGTH: f32 = 0.7;
const VELOCITY_ARROW_HEAD_THICKNESS: f32 = 0.12;
const VELOCITY_ARROW_HEAD_SPREAD_RAD: f32 = 0.7;
const DEBUG_STALL_GAP_THRESHOLD_MS: f64 = 100.0;

#[derive(SystemParam)]
pub(crate) struct DebugOverlayStatsInputs<'w, 's> {
    tracer_pool: Res<'w, WeaponTracerPool>,
    spark_pool: Res<'w, WeaponImpactSparkPool>,
    asset_manager: Res<'w, LocalAssetManager>,
    runtime_asset_dependency_state: Res<'w, RuntimeAssetDependencyState>,
    runtime_asset_fetch_state: Res<'w, RuntimeAssetHttpFetchState>,
    runtime_asset_perf: Res<'w, RuntimeAssetPerfCounters>,
    hud_perf: Res<'w, HudPerfCounters>,
    render_layer_perf: Res<'w, RenderLayerPerfCounters>,
    duplicate_resolution: Res<'w, DuplicateVisualResolutionState>,
    mesh_assets: Res<'w, Assets<Mesh>>,
    generic_sprite_materials: Res<'w, Assets<StreamedSpriteShaderMaterial>>,
    asteroid_materials: Res<'w, Assets<AsteroidSpriteShaderMaterial>>,
    planet_materials: Res<'w, Assets<PlanetVisualMaterial>>,
    effect_materials: Res<'w, Assets<RuntimeEffectMaterial>>,
    cameras: Query<'w, 's, &'static Camera>,
    visual_passes: Query<'w, 's, &'static RuntimeWorldVisualPass>,
    streamed_visual_children: Query<'w, 's, (), With<StreamedVisualChild>>,
    tracer_entities: Query<'w, 's, &'static Visibility, With<WeaponTracerBolt>>,
    spark_entities: Query<'w, 's, &'static Visibility, With<WeaponImpactSpark>>,
}

