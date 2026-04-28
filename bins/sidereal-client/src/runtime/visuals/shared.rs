// Fullscreen/backdrop and streamed visual lifecycle systems.

use avian2d::prelude::{Position, Rotation, SpatialQuery, SpatialQueryFilter};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::ecs::system::SystemParam;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::state::state_scoped::DespawnOnExit;
use bevy::{math::DVec2, prelude::*};
use lightyear::interpolation::interpolation_history::ConfirmedHistory;
use lightyear::prelude::Confirmed;
use lightyear::prelude::input::native::ActionState;
use sidereal_game::{
    AfterburnerCapability, AfterburnerState, AmmoCount, BallisticProjectile, BallisticWeapon,
    ControlledEntityGuid, EntityAction, EntityGuid, EntityLabels, FlightComputer, Hardpoint,
    MountedOn, ParentGuid, PlanetBodyShaderSettings, PlayerTag, ProceduralSprite,
    RuntimeRenderLayerDefinition, RuntimeWorldVisualPassDefinition, RuntimeWorldVisualStack, SizeM,
    ThrusterPlumeShaderSettings, WorldPosition, WorldRotation,
    generate_procedural_sprite_image_set, resolve_world_position, resolve_world_rotation_rad,
};
use sidereal_net::PlayerInput;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, TAU};

use super::app_state::ClientAppState;
use super::assets;
use super::assets::LocalAssetManager;
use super::backdrop::{
    AsteroidSpriteShaderMaterial, PlanetBodyUniforms, PlanetVisualMaterial, RuntimeEffectMaterial,
    RuntimeEffectUniforms, SharedWorldLightingUniforms, StarVisualMaterial,
    StreamedSpriteShaderMaterial,
};
use super::components::{
    BallisticProjectileVisualAttached, CanonicalPresentationEntity, ControlledEntity,
    PendingInitialVisualReady, PendingVisibilityFadeIn, PlanetBodyCamera,
    ResolvedRuntimeRenderLayer, RuntimeWorldVisualFamily, RuntimeWorldVisualPass,
    RuntimeWorldVisualPassKind, RuntimeWorldVisualPassSet,
    StreamedProceduralSpriteVisualFingerprint, StreamedSpriteShaderAssetId, StreamedVisualAssetId,
    StreamedVisualAttached, StreamedVisualAttachmentKind, StreamedVisualChild,
    SuppressedPredictedDuplicateVisual, WeaponImpactExplosion, WeaponImpactExplosionPool,
    WeaponImpactSpark, WeaponImpactSparkPool, WeaponTracerBolt, WeaponTracerCooldowns,
    WeaponTracerPool, WorldEntity,
};
use super::ecs_util::queue_despawn_if_exists;
use super::lighting::{CameraLocalLightSet, WorldLightingState};
use super::platform::PLANET_BODY_RENDER_LAYER;
use super::resources::AssetRootPath;
use super::resources::CameraMotionState;
use super::resources::DuplicateVisualResolutionState;
use super::resources::RuntimeSharedQuadMesh;
use super::shaders;
use super::transforms::interpolated_presentation_ready;
use crate::runtime::combat_messages::{
    RemoteEntityDestructionRuntimeMessage, RemoteWeaponFiredRuntimeMessage,
};

const WEAPON_TRACER_POOL_SIZE: usize = 96;

type WeaponImpactSparkQueryItem<'a> = (
    Entity,
    &'a mut WeaponImpactSpark,
    &'a mut Transform,
    &'a MeshMaterial2d<RuntimeEffectMaterial>,
    &'a mut Visibility,
);

type WeaponImpactExplosionQueryItem<'a> = (
    Entity,
    &'a mut WeaponImpactExplosion,
    &'a mut Transform,
    &'a MeshMaterial2d<RuntimeEffectMaterial>,
    &'a mut Visibility,
);

type WeaponTracerBoltQueryItem<'a> = (
    &'a mut Transform,
    &'a MeshMaterial2d<RuntimeEffectMaterial>,
    &'a mut Visibility,
    &'a mut WeaponTracerBolt,
);

type WeaponTracerBoltQueryFilter = (Without<WeaponImpactSpark>, Without<WeaponImpactExplosion>);
type WeaponImpactSparkQueryFilter = (Without<WeaponTracerBolt>, Without<WeaponImpactExplosion>);
type WeaponImpactExplosionQueryFilter = (Without<WeaponTracerBolt>, Without<WeaponImpactSpark>);

#[derive(SystemParam)]
pub(super) struct ThrusterPlumeAttachAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    quad_mesh: ResMut<'w, RuntimeSharedQuadMesh>,
    plume_materials: ResMut<'w, Assets<RuntimeEffectMaterial>>,
    world_lighting: Res<'w, WorldLightingState>,
    camera_local_lights: Res<'w, CameraLocalLightSet>,
}
const WEAPON_TRACER_SPEED_MPS: f32 = 650.0;
const WEAPON_TRACER_LIFETIME_S: f32 = 0.32;
const WEAPON_TRACER_WIDTH_M: f32 = 0.62;
const WEAPON_TRACER_LENGTH_M: f32 = 52.0;
const WEAPON_TRACER_ROTATION_OFFSET_RAD: f32 = -FRAC_PI_2;
const WEAPON_TRACER_WIGGLE_BASE_FREQ_HZ: f32 = 18.0;
const WEAPON_TRACER_WIGGLE_MAX_AMP_MPS: f32 = 120.0;
const WEAPON_IMPACT_SPARK_TTL_S: f32 = 0.12;
const WEAPON_IMPACT_SPARK_POOL_SIZE: usize = 48;
const WEAPON_IMPACT_EXPLOSION_TTL_S: f32 = 0.18;
const WEAPON_IMPACT_EXPLOSION_POOL_SIZE: usize = 32;
const WEAPON_TRACER_MIN_TTL_S: f32 = 0.01;
const PLANET_CLOUD_BACK_LAYER_Z_OFFSET: f32 = -0.2;
const PLANET_CLOUD_FRONT_LAYER_Z_OFFSET: f32 = 0.5;
const PLANET_BODY_LAYER_Z_OFFSET: f32 = 0.0;
const PLANET_RING_BACK_LAYER_Z_OFFSET: f32 = -0.45;
const PLANET_RING_FRONT_LAYER_Z_OFFSET: f32 = 0.65;
const PLANET_PROJECTED_CULL_STATIC_VIEWPORT_MARGIN: f32 = 0.25;
const PLANET_PROJECTED_CULL_ZOOM_OUT_VIEWPORT_MARGIN: f32 = 0.50;
const PLANET_PROJECTED_CULL_MIN_MARGIN_PX: f32 = 96.0;
const PLANET_PROJECTED_CULL_RETENTION_GRACE_S: f64 = 0.35;
const PLANET_PROJECTED_CULL_ZOOM_OUT_HOLD_S: f64 = 0.35;
const PLANET_PROJECTED_CULL_ZOOM_OUT_SCALE_THRESHOLD: f32 = 0.02;
const STREAMED_VISUAL_BASE_LAYER_Z: f32 = 0.2;
const PROJECTILE_VISUAL_WIDTH_M: f32 = 0.45;
const PROJECTILE_VISUAL_LENGTH_M: f32 = 2.8;
const PROJECTILE_VISUAL_Z: f32 = 0.38;
const DESTRUCTION_EXPLOSION_TTL_S: f32 = 0.65;
const DESTRUCTION_EXPLOSION_BASE_SCALE: f32 = 8.0;
const DESTRUCTION_EXPLOSION_GROWTH_SCALE: f32 = 18.0;
const DESTRUCTION_EXPLOSION_INTENSITY: f32 = 1.45;

enum StreamedVisualMaterialKind {
    Plain,
    GenericShader,
    AsteroidShader,
}

#[derive(Default)]
pub(super) struct StreamedVisualAssetCaches {
    last_reload_generation: u64,
    asteroid_sprite_cache: HashMap<(uuid::Uuid, u64), (Handle<Image>, Handle<Image>)>,
    flat_normal_image: Option<Handle<Image>>,
    streamed_image_cache: HashMap<String, Handle<Image>>,
}

fn shared_unit_quad_handle(
    quad_mesh: &mut RuntimeSharedQuadMesh,
    meshes: &mut Assets<Mesh>,
) -> Handle<Mesh> {
    if let Some(handle) = quad_mesh.unit_quad.clone() {
        return handle;
    }
    let handle = meshes.add(Rectangle::new(1.0, 1.0));
    quad_mesh.unit_quad = Some(handle.clone());
    quad_mesh.allocations = quad_mesh.allocations.saturating_add(1);
    handle
}

fn sync_planar_projectile_transform(transform: &mut Transform, position: DVec2, heading_rad: f64) {
    transform.translation.x = position.x as f32;
    transform.translation.y = position.y as f32;
    transform.rotation = Quat::from_rotation_z(heading_rad as f32);
}

fn shader_rotation_uniform(rotation_rad: f64) -> Vec4 {
    let rotation_rad = rotation_rad as f32;
    Vec4::new(rotation_rad.cos(), rotation_rad.sin(), 0.0, 0.0)
}
