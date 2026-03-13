//! Fullscreen/backdrop and streamed visual lifecycle systems.

use avian2d::prelude::{Position, SpatialQuery, SpatialQueryFilter};
use bevy::asset::{AssetId, RenderAssetUsages};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::state::state_scoped::DespawnOnExit;
use lightyear::interpolation::interpolation_history::ConfirmedHistory;
use lightyear::prelude::input::native::ActionState;
use sidereal_game::{
    AfterburnerCapability, AfterburnerState, AmmoCount, BallisticProjectile, BallisticWeapon,
    ControlledEntityGuid, EntityAction, EntityGuid, EntityLabels, FlightComputer, Hardpoint,
    MountedOn, ParentGuid, PlanetBodyShaderSettings, PlayerTag, ProceduralSprite,
    RuntimeRenderLayerDefinition, RuntimeWorldVisualPassDefinition, RuntimeWorldVisualStack, SizeM,
    ThrusterPlumeShaderSettings, WorldPosition, generate_procedural_sprite_image_set,
    resolve_world_position,
};
use sidereal_net::PlayerInput;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, TAU};

use super::app_state::ClientAppState;
use super::assets;
use super::assets::LocalAssetManager;
use super::backdrop::{
    AsteroidSpriteShaderMaterial, PlanetBodyUniforms, PlanetVisualMaterial, RuntimeEffectMaterial,
    RuntimeEffectUniforms, SharedWorldLightingUniforms, StreamedSpriteShaderMaterial,
};
use super::components::{
    BallisticProjectileVisualAttached, ControlledEntity, PendingInitialVisualReady,
    PendingVisibilityFadeIn, PlanetBodyCamera, ResolvedRuntimeRenderLayer,
    RuntimeWorldVisualFamily, RuntimeWorldVisualPass, RuntimeWorldVisualPassKind,
    RuntimeWorldVisualPassSet, StreamedSpriteShaderAssetId, StreamedVisualAssetId,
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
const WEAPON_TRACER_SPEED_MPS: f32 = 1800.0;
const WEAPON_TRACER_LIFETIME_S: f32 = 0.2;
const WEAPON_TRACER_WIDTH_M: f32 = 0.35;
const WEAPON_TRACER_LENGTH_M: f32 = 9.0;
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
const PLANET_PROJECTED_CULL_BUFFER_M: f32 = 120.0;
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

#[derive(Debug, Resource, Default)]
pub(crate) struct StreamedSpriteMaterialCache {
    pub reload_generation: u64,
    pub by_image: HashMap<AssetId<Image>, Handle<StreamedSpriteShaderMaterial>>,
}

#[derive(Default)]
pub(super) struct StreamedVisualAssetCaches {
    last_reload_generation: u64,
    asteroid_sprite_cache: HashMap<(uuid::Uuid, u64), (Handle<Image>, Handle<Image>)>,
    streamed_image_cache: HashMap<String, Handle<Image>>,
    generic_material_cache: StreamedSpriteMaterialCache,
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

fn sync_planar_projectile_transform(transform: &mut Transform, position: Vec2, heading_rad: f32) {
    transform.translation.x = position.x;
    transform.translation.y = position.y;
    transform.rotation = Quat::from_rotation_z(heading_rad);
}

fn shared_streamed_sprite_material_handle(
    cache: &mut StreamedSpriteMaterialCache,
    materials: &mut Assets<StreamedSpriteShaderMaterial>,
    image: &Handle<Image>,
) -> Handle<StreamedSpriteShaderMaterial> {
    let image_id = image.id();
    if let Some(handle) = cache.by_image.get(&image_id) {
        return handle.clone();
    }
    let handle = materials.add(StreamedSpriteShaderMaterial {
        image: image.clone(),
    });
    cache.by_image.insert(image_id, handle.clone());
    handle
}

fn activate_weapon_impact_spark(
    impact_pos: Vec2,
    pool: &mut WeaponImpactSparkPool,
    sparks: &mut Query<
        '_,
        '_,
        (
            &'_ mut WeaponImpactSpark,
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
        ),
        WeaponImpactSparkQueryFilter,
    >,
    effect_materials: &mut Assets<RuntimeEffectMaterial>,
) {
    if pool.sparks.is_empty() {
        return;
    }
    let spark_entity = pool.sparks[pool.next_index % pool.sparks.len()];
    pool.next_index = (pool.next_index + 1) % pool.sparks.len();
    let Ok((mut spark, mut transform, material_handle, mut visibility)) =
        sparks.get_mut(spark_entity)
    else {
        return;
    };
    spark.ttl_s = WEAPON_IMPACT_SPARK_TTL_S;
    spark.max_ttl_s = WEAPON_IMPACT_SPARK_TTL_S;
    transform.translation = Vec3::new(impact_pos.x, impact_pos.y, 0.45);
    transform.scale = Vec3::ONE;
    if let Some(material) = effect_materials.get_mut(&material_handle.0) {
        material.params = RuntimeEffectUniforms::impact_spark(
            0.0,
            1.0,
            1.0,
            0.95,
            Vec4::new(1.0, 0.9, 0.55, 1.0),
        );
    }
    *visibility = Visibility::Visible;
}

fn activate_weapon_impact_explosion(
    impact_pos: Vec2,
    pool: &mut WeaponImpactExplosionPool,
    explosions: &mut Query<
        '_,
        '_,
        (
            &'_ mut WeaponImpactExplosion,
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
        ),
        WeaponImpactExplosionQueryFilter,
    >,
    effect_materials: &mut Assets<RuntimeEffectMaterial>,
) {
    if pool.explosions.is_empty() {
        return;
    }
    let explosion_entity = pool.explosions[pool.next_index % pool.explosions.len()];
    pool.next_index = (pool.next_index + 1) % pool.explosions.len();
    let Ok((mut explosion, mut transform, material_handle, mut visibility)) =
        explosions.get_mut(explosion_entity)
    else {
        return;
    };
    explosion.ttl_s = WEAPON_IMPACT_EXPLOSION_TTL_S;
    explosion.max_ttl_s = WEAPON_IMPACT_EXPLOSION_TTL_S;
    explosion.base_scale = 1.2;
    explosion.growth_scale = 4.4;
    explosion.intensity_scale = 1.0;
    explosion.domain_scale = 1.12;
    explosion.screen_distortion_scale = 0.0;
    transform.translation = Vec3::new(impact_pos.x, impact_pos.y, 0.43);
    transform.scale = Vec3::splat(1.6);
    if let Some(material) = effect_materials.get_mut(&material_handle.0) {
        material.params = RuntimeEffectUniforms::explosion_burst(
            0.0,
            1.0,
            1.0,
            0.92,
            0.35,
            explosion.domain_scale,
            Vec4::new(1.0, 0.92, 0.68, 1.0),
            Vec4::new(1.0, 0.54, 0.16, 1.0),
            Vec4::new(0.24, 0.14, 0.08, 1.0),
        );
    }
    *visibility = Visibility::Visible;
}

fn activate_destruction_effect(
    profile_id: &str,
    impact_pos: Vec2,
    pool: &mut WeaponImpactExplosionPool,
    explosions: &mut Query<
        '_,
        '_,
        (
            &'_ mut WeaponImpactExplosion,
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
        ),
        WeaponImpactExplosionQueryFilter,
    >,
    effect_materials: &mut Assets<RuntimeEffectMaterial>,
) {
    if pool.explosions.is_empty() {
        return;
    }
    let explosion_entity = pool.explosions[pool.next_index % pool.explosions.len()];
    pool.next_index = (pool.next_index + 1) % pool.explosions.len();
    let Ok((mut explosion, mut transform, material_handle, mut visibility)) =
        explosions.get_mut(explosion_entity)
    else {
        return;
    };
    let (core_color, rim_color, smoke_color) = match profile_id {
        "explosion_burst" => (
            Vec4::new(1.0, 0.94, 0.76, 1.0),
            Vec4::new(1.0, 0.58, 0.18, 1.0),
            Vec4::new(0.22, 0.14, 0.10, 1.0),
        ),
        _ => (
            Vec4::new(1.0, 0.94, 0.76, 1.0),
            Vec4::new(1.0, 0.58, 0.18, 1.0),
            Vec4::new(0.22, 0.14, 0.10, 1.0),
        ),
    };
    explosion.ttl_s = DESTRUCTION_EXPLOSION_TTL_S;
    explosion.max_ttl_s = DESTRUCTION_EXPLOSION_TTL_S;
    explosion.base_scale = DESTRUCTION_EXPLOSION_BASE_SCALE;
    explosion.growth_scale = DESTRUCTION_EXPLOSION_GROWTH_SCALE;
    explosion.intensity_scale = DESTRUCTION_EXPLOSION_INTENSITY;
    explosion.domain_scale = 1.45;
    explosion.screen_distortion_scale = 1.0;
    transform.translation = Vec3::new(impact_pos.x, impact_pos.y, 0.52);
    transform.scale = Vec3::splat(DESTRUCTION_EXPLOSION_BASE_SCALE);
    if let Some(material) = effect_materials.get_mut(&material_handle.0) {
        material.params = RuntimeEffectUniforms::explosion_burst(
            0.0,
            DESTRUCTION_EXPLOSION_INTENSITY,
            1.25,
            1.0,
            0.55,
            explosion.domain_scale,
            core_color,
            rim_color,
            smoke_color,
        );
    }
    *visibility = Visibility::Visible;
}

fn has_engine_label(labels: &EntityLabels) -> bool {
    labels
        .0
        .iter()
        .any(|label| label.eq_ignore_ascii_case("engine"))
}

impl StreamedVisualMaterialKind {
    const fn attachment_kind(self) -> StreamedVisualAttachmentKind {
        match self {
            Self::Plain => StreamedVisualAttachmentKind::Plain,
            Self::GenericShader => StreamedVisualAttachmentKind::GenericShader,
            Self::AsteroidShader => StreamedVisualAttachmentKind::AsteroidShader,
        }
    }
}

fn pass_tag(
    family: RuntimeWorldVisualFamily,
    kind: RuntimeWorldVisualPassKind,
) -> RuntimeWorldVisualPass {
    RuntimeWorldVisualPass { family, kind }
}

fn runtime_world_visual_pass_kind(
    pass: &RuntimeWorldVisualPassDefinition,
) -> Option<RuntimeWorldVisualPassKind> {
    match (pass.visual_family.as_str(), pass.visual_kind.as_str()) {
        ("planet", "body") => Some(RuntimeWorldVisualPassKind::PlanetBody),
        ("planet", "cloud_back") => Some(RuntimeWorldVisualPassKind::PlanetCloudBack),
        ("planet", "cloud_front") => Some(RuntimeWorldVisualPassKind::PlanetCloudFront),
        ("planet", "ring_back") => Some(RuntimeWorldVisualPassKind::PlanetRingBack),
        ("planet", "ring_front") => Some(RuntimeWorldVisualPassKind::PlanetRingFront),
        ("thruster", "plume") => Some(RuntimeWorldVisualPassKind::ThrusterPlume),
        _ => None,
    }
}

fn find_world_visual_pass(
    stack: Option<&RuntimeWorldVisualStack>,
    kind: RuntimeWorldVisualPassKind,
) -> Option<&RuntimeWorldVisualPassDefinition> {
    let stack = stack?;
    stack.passes.iter().find(|pass| {
        pass.enabled && runtime_world_visual_pass_kind(pass).is_some_and(|value| value == kind)
    })
}

fn desired_world_visual_pass_set(
    stack: Option<&RuntimeWorldVisualStack>,
    family: RuntimeWorldVisualFamily,
) -> RuntimeWorldVisualPassSet {
    let mut set = RuntimeWorldVisualPassSet::default();
    let Some(stack) = stack else {
        return set;
    };
    for pass in &stack.passes {
        if !pass.enabled {
            continue;
        }
        let Some(kind) = runtime_world_visual_pass_kind(pass) else {
            continue;
        };
        let expected_family = match kind {
            RuntimeWorldVisualPassKind::PlanetBody
            | RuntimeWorldVisualPassKind::PlanetCloudBack
            | RuntimeWorldVisualPassKind::PlanetCloudFront
            | RuntimeWorldVisualPassKind::PlanetRingBack
            | RuntimeWorldVisualPassKind::PlanetRingFront => RuntimeWorldVisualFamily::Planet,
            RuntimeWorldVisualPassKind::ThrusterPlume => RuntimeWorldVisualFamily::Thruster,
        };
        if expected_family == family {
            set.insert(kind);
        }
    }
    set
}

fn visual_pass_scale_multiplier(
    pass: Option<&RuntimeWorldVisualPassDefinition>,
    fallback: f32,
) -> f32 {
    pass.and_then(|value| value.scale_multiplier)
        .unwrap_or(fallback)
}

fn visual_pass_depth_bias_z(pass: Option<&RuntimeWorldVisualPassDefinition>, fallback: f32) -> f32 {
    pass.and_then(|value| value.depth_bias_z)
        .unwrap_or(fallback)
}

fn shader_materials_enabled() -> bool {
    shaders::shader_materials_enabled()
}

fn procedural_sprite_fingerprint(sprite: &ProceduralSprite) -> u64 {
    let mut seed = 0x517cc1b727220a95u64;
    for byte in sprite.generator_id.as_bytes() {
        seed ^= u64::from(*byte);
        seed = seed.wrapping_mul(0x100000001b3);
    }
    seed ^= u64::from(sprite.resolution_px);
    seed ^= u64::from(sprite.crater_count) << 8;
    seed ^= u64::from(sprite.edge_noise.to_bits()) << 16;
    seed ^= u64::from(sprite.lobe_amplitude.to_bits()) << 32;
    for value in sprite
        .palette_dark_rgb
        .iter()
        .chain(sprite.palette_light_rgb.iter())
    {
        seed ^= u64::from(value.to_bits()).rotate_left(7);
        seed = seed.wrapping_mul(0x100000001b3);
    }
    seed
}

fn image_from_rgba(width: u32, height: u32, data: Vec<u8>) -> Image {
    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
}

fn ensure_visual_parent_spatial_components(entity_commands: &mut EntityCommands<'_>) {
    entity_commands.try_insert((
        Transform::default(),
        GlobalTransform::default(),
        Visibility::default(),
    ));
}

fn resolve_streamed_visual_material_kind(
    use_shader_materials: bool,
    world_sprite_kind: Option<shaders::RuntimeWorldSpriteShaderKind>,
    has_streamed_sprite_shader_path: bool,
) -> StreamedVisualMaterialKind {
    if !use_shader_materials {
        return StreamedVisualMaterialKind::Plain;
    }

    match world_sprite_kind {
        Some(shaders::RuntimeWorldSpriteShaderKind::Asteroid) => {
            StreamedVisualMaterialKind::AsteroidShader
        }
        Some(shaders::RuntimeWorldSpriteShaderKind::GenericSprite)
            if has_streamed_sprite_shader_path =>
        {
            StreamedVisualMaterialKind::GenericShader
        }
        _ => StreamedVisualMaterialKind::Plain,
    }
}

fn streamed_visual_needs_rebuild(
    attached_kind: Option<StreamedVisualAttachmentKind>,
    desired_kind: StreamedVisualMaterialKind,
) -> bool {
    attached_kind != Some(desired_kind.attachment_kind())
}

/// TODO: add symmetric fade-out on relevance loss via a short-lived visual ghost entity.
#[allow(clippy::type_complexity)]
pub(super) fn update_entity_visibility_fade_in_system(
    time: Res<'_, Time>,
    mut commands: Commands<'_, '_>,
    mut parents: Query<'_, '_, (Entity, &'_ Children, &'_ mut PendingVisibilityFadeIn)>,
    mut visual_children: Query<'_, '_, &'_ mut Sprite, With<StreamedVisualChild>>,
) {
    let dt_s = time.delta_secs().max(0.0);
    for (entity, children, mut fade) in &mut parents {
        fade.elapsed_s += dt_s;
        let alpha = if fade.duration_s <= 0.0 {
            1.0
        } else {
            (fade.elapsed_s / fade.duration_s).clamp(0.0, 1.0)
        };
        let mut touched_any = false;
        for child in children.iter() {
            if let Ok(mut sprite) = visual_children.get_mut(child) {
                touched_any = true;
                let mut srgba = sprite.color.to_srgba();
                srgba.alpha = alpha;
                sprite.color = Color::Srgba(srgba);
            }
        }
        if (alpha >= 0.999 || !touched_any)
            && let Ok(mut entity_commands) = commands.get_entity(entity)
        {
            entity_commands.remove::<PendingVisibilityFadeIn>();
        }
    }
}

pub(super) fn suppress_duplicate_predicted_interpolated_visuals_system(world: &mut World) {
    let mut state = world
        .remove_resource::<DuplicateVisualResolutionState>()
        .unwrap_or_default();

    collect_duplicate_visual_membership_changes(world, &mut state);
    collect_duplicate_visual_dirty_guid_changes(world, &mut state);

    let dirty_guids = if state.dirty_all {
        state.entities_by_guid.keys().copied().collect::<Vec<_>>()
    } else {
        state.dirty_guids.iter().copied().collect::<Vec<_>>()
    };

    for guid in dirty_guids {
        recompute_duplicate_visual_group(world, &mut state, guid);
    }

    state.dirty_guids.clear();
    state.dirty_all = false;
    state.duplicate_guid_groups = state
        .entities_by_guid
        .values()
        .filter(|entities| entities.len() > 1)
        .count();
    world.insert_resource(state);
}

fn collect_duplicate_visual_membership_changes(
    world: &mut World,
    state: &mut DuplicateVisualResolutionState,
) {
    if state.dirty_all {
        state.guid_by_entity.clear();
        state.entities_by_guid.clear();
        let mut query = world.query_filtered::<(Entity, &EntityGuid), With<WorldEntity>>();
        for (entity, guid) in query.iter(world) {
            state.guid_by_entity.insert(entity, guid.0);
            state
                .entities_by_guid
                .entry(guid.0)
                .or_default()
                .insert(entity);
            state.dirty_guids.insert(guid.0);
        }
        return;
    }

    let mut added_or_changed_guid = world.query_filtered::<(Entity, &EntityGuid), (
        With<WorldEntity>,
        Or<(Added<WorldEntity>, Added<EntityGuid>, Changed<EntityGuid>)>,
    )>();
    for (entity, guid) in added_or_changed_guid.iter(world) {
        let new_guid = guid.0;
        if let Some(previous_guid) = state.guid_by_entity.insert(entity, new_guid)
            && previous_guid != new_guid
        {
            if let Some(entities) = state.entities_by_guid.get_mut(&previous_guid) {
                entities.remove(&entity);
                if entities.is_empty() {
                    state.entities_by_guid.remove(&previous_guid);
                }
            }
            state.dirty_guids.insert(previous_guid);
        }
        state
            .entities_by_guid
            .entry(new_guid)
            .or_default()
            .insert(entity);
        state.dirty_guids.insert(new_guid);
    }

    let removed_entity_guid_entities = read_removed_duplicate_visual_entities::<EntityGuid>(
        world,
        &mut state.entity_guid_removal_cursor,
    );
    for entity in removed_entity_guid_entities {
        remove_duplicate_visual_membership_for_entity(state, entity);
    }
    let removed_world_entities = read_removed_duplicate_visual_entities::<WorldEntity>(
        world,
        &mut state.world_entity_removal_cursor,
    );
    for entity in removed_world_entities {
        remove_duplicate_visual_membership_for_entity(state, entity);
    }
}

fn remove_duplicate_visual_membership_for_entity(
    state: &mut DuplicateVisualResolutionState,
    entity: Entity,
) {
    if let Some(previous_guid) = state.guid_by_entity.remove(&entity) {
        if let Some(entities) = state.entities_by_guid.get_mut(&previous_guid) {
            entities.remove(&entity);
            if entities.is_empty() {
                state.entities_by_guid.remove(&previous_guid);
            }
        }
        state.dirty_guids.insert(previous_guid);
    }
}

fn collect_duplicate_visual_dirty_guid_changes(
    world: &mut World,
    state: &mut DuplicateVisualResolutionState,
) {
    mark_dirty_duplicate_visual_guids_for_changes::<ControlledEntityGuid>(world, state);
    mark_dirty_duplicate_visual_guids_for_changes::<PlayerTag>(world, state);
    mark_dirty_duplicate_visual_guids_for_changes::<ControlledEntity>(world, state);
    mark_dirty_duplicate_visual_guids_for_changes::<lightyear::prelude::Interpolated>(world, state);
    mark_dirty_duplicate_visual_guids_for_changes::<lightyear::prelude::Predicted>(world, state);
    mark_dirty_duplicate_visual_guids_for_additions::<ConfirmedHistory<avian2d::prelude::Position>>(
        world, state,
    );
    mark_dirty_duplicate_visual_guids_for_additions::<ConfirmedHistory<avian2d::prelude::Rotation>>(
        world, state,
    );

    for entity in read_removed_duplicate_visual_entities::<ControlledEntityGuid>(
        world,
        &mut state.controlled_entity_guid_removal_cursor,
    ) {
        mark_duplicate_visual_entity_guid_dirty(state, entity);
    }
    for entity in read_removed_duplicate_visual_entities::<PlayerTag>(
        world,
        &mut state.player_tag_removal_cursor,
    ) {
        mark_duplicate_visual_entity_guid_dirty(state, entity);
    }
    for entity in read_removed_duplicate_visual_entities::<ControlledEntity>(
        world,
        &mut state.controlled_entity_removal_cursor,
    ) {
        mark_duplicate_visual_entity_guid_dirty(state, entity);
    }
    for entity in read_removed_duplicate_visual_entities::<lightyear::prelude::Interpolated>(
        world,
        &mut state.interpolated_removal_cursor,
    ) {
        mark_duplicate_visual_entity_guid_dirty(state, entity);
    }
    for entity in read_removed_duplicate_visual_entities::<lightyear::prelude::Predicted>(
        world,
        &mut state.predicted_removal_cursor,
    ) {
        mark_duplicate_visual_entity_guid_dirty(state, entity);
    }
    for entity in read_removed_duplicate_visual_entities::<
        ConfirmedHistory<avian2d::prelude::Position>,
    >(world, &mut state.position_history_removal_cursor)
    {
        mark_duplicate_visual_entity_guid_dirty(state, entity);
    }
    for entity in read_removed_duplicate_visual_entities::<
        ConfirmedHistory<avian2d::prelude::Rotation>,
    >(world, &mut state.rotation_history_removal_cursor)
    {
        mark_duplicate_visual_entity_guid_dirty(state, entity);
    }
}

fn mark_dirty_duplicate_visual_guids_for_changes<T: Component>(
    world: &mut World,
    state: &mut DuplicateVisualResolutionState,
) {
    let mut query =
        world.query_filtered::<Entity, (With<WorldEntity>, Or<(Added<T>, Changed<T>)>)>();
    for entity in query.iter(world) {
        if let Some(guid) = state.guid_by_entity.get(&entity).copied() {
            state.dirty_guids.insert(guid);
        }
    }
}

fn mark_dirty_duplicate_visual_guids_for_additions<T: Component>(
    world: &mut World,
    state: &mut DuplicateVisualResolutionState,
) {
    let mut query = world.query_filtered::<Entity, (With<WorldEntity>, Added<T>)>();
    for entity in query.iter(world) {
        if let Some(guid) = state.guid_by_entity.get(&entity).copied() {
            state.dirty_guids.insert(guid);
        }
    }
}

fn mark_duplicate_visual_entity_guid_dirty(
    state: &mut DuplicateVisualResolutionState,
    entity: Entity,
) {
    if let Some(guid) = state.guid_by_entity.get(&entity).copied() {
        state.dirty_guids.insert(guid);
    }
}

fn read_removed_duplicate_visual_entities<T: Component>(
    world: &mut World,
    cursor: &mut Option<
        bevy::ecs::message::MessageCursor<bevy::ecs::lifecycle::RemovedComponentEntity>,
    >,
) -> Vec<Entity> {
    let Some(component_id) = world.component_id::<T>() else {
        return Vec::new();
    };
    let Some(events) = world.removed_components().get(component_id) else {
        return Vec::new();
    };
    let reader = cursor.get_or_insert_with(Default::default);
    reader
        .read(events)
        .map(|event| Entity::from(event.clone()))
        .collect()
}

fn recompute_duplicate_visual_group(
    world: &mut World,
    state: &mut DuplicateVisualResolutionState,
    guid: uuid::Uuid,
) {
    let member_entities = state
        .entities_by_guid
        .get(&guid)
        .cloned()
        .unwrap_or_default();
    let mut best_entity = None::<(Entity, i32)>;
    let mut live_entities = std::collections::HashSet::<Entity>::new();

    for entity in member_entities {
        let Some(entity_ref) = world.get_entity(entity).ok() else {
            continue;
        };
        if !entity_ref.contains::<WorldEntity>() {
            continue;
        }
        live_entities.insert(entity);
        let force_suppress =
            entity_ref.contains::<ControlledEntityGuid>() || entity_ref.contains::<PlayerTag>();
        if force_suppress {
            continue;
        }

        let is_controlled = entity_ref.contains::<ControlledEntity>();
        let is_interpolated = entity_ref.contains::<lightyear::prelude::Interpolated>();
        let is_predicted = entity_ref.contains::<lightyear::prelude::Predicted>();
        let interpolated_ready = entity_ref
            .contains::<ConfirmedHistory<avian2d::prelude::Position>>()
            && entity_ref.contains::<ConfirmedHistory<avian2d::prelude::Rotation>>();
        let score = if is_controlled {
            3
        } else if is_interpolated && interpolated_ready {
            2
        } else if is_predicted {
            1
        } else if is_interpolated {
            -1
        } else {
            0
        };
        match best_entity {
            Some((winner, winner_score))
                if score < winner_score
                    || (score == winner_score && entity.to_bits() >= winner.to_bits()) => {}
            _ => {
                best_entity = Some((entity, score));
            }
        }
    }

    if live_entities.is_empty() {
        state.entities_by_guid.remove(&guid);
        state.winner_by_guid.remove(&guid);
        return;
    }

    state.entities_by_guid.insert(guid, live_entities.clone());
    let previous_winner = state.winner_by_guid.get(&guid).copied();
    if let Some((winner, _)) = best_entity {
        if previous_winner != Some(winner) {
            state.winner_swap_count = state.winner_swap_count.saturating_add(1);
        }
        state.winner_by_guid.insert(guid, winner);
    } else {
        state.winner_by_guid.remove(&guid);
    }

    for entity in live_entities {
        let Some(entity_ref) = world.get_entity(entity).ok() else {
            continue;
        };
        let should_suppress = entity_ref.contains::<ControlledEntityGuid>()
            || entity_ref.contains::<PlayerTag>()
            || state
                .winner_by_guid
                .get(&guid)
                .is_some_and(|winner| *winner != entity);
        let is_suppressed = entity_ref.contains::<SuppressedPredictedDuplicateVisual>();
        let mut entity_mut = world.entity_mut(entity);
        if should_suppress {
            if !is_suppressed {
                entity_mut.insert(SuppressedPredictedDuplicateVisual);
            }
            entity_mut.insert(Visibility::Hidden);
        } else if is_suppressed {
            entity_mut.remove::<SuppressedPredictedDuplicateVisual>();
            entity_mut.insert(Visibility::Visible);
        }
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(super) fn cleanup_streamed_visual_children_system(
    mut commands: Commands<'_, '_>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    mut last_reload_generation: Local<'_, u64>,
    cache_adapter: Res<'_, super::resources::AssetCacheAdapter>,
    shader_assignments: Res<'_, shaders::RuntimeShaderAssignments>,
    parents: Query<
        '_,
        '_,
        (
            Entity,
            &'_ Children,
            Option<&'_ StreamedVisualAssetId>,
            Option<&'_ StreamedSpriteShaderAssetId>,
            Option<&'_ ProceduralSprite>,
            Has<PlanetBodyShaderSettings>,
            Has<StreamedVisualAttached>,
            Option<&'_ StreamedVisualAttachmentKind>,
            Has<SuppressedPredictedDuplicateVisual>,
            Option<&'_ PlayerTag>,
            Has<ControlledEntityGuid>,
        ),
        With<WorldEntity>,
    >,
    visual_children: Query<'_, '_, (), With<StreamedVisualChild>>,
) {
    let catalog_reloaded = *last_reload_generation != asset_manager.reload_generation;
    *last_reload_generation = asset_manager.reload_generation;
    for (
        parent_entity,
        children,
        visual_asset_id,
        sprite_shader_asset_id,
        procedural_sprite,
        has_planet_shader,
        has_visual_attached,
        attached_kind,
        is_suppressed,
        player_tag,
        has_controlled_entity_guid,
    ) in &parents
    {
        let world_sprite_kind = sprite_shader_asset_id
            .and_then(|shader| shaders::world_sprite_shader_kind(&shader_assignments, &shader.0));
        let has_streamed_sprite_shader_path = sprite_shader_asset_id.is_some_and(|shader| {
            shaders::world_sprite_shader_ready(
                &asset_root.0,
                &asset_manager,
                *cache_adapter,
                &shader.0,
            )
        });
        let desired_kind = if sprite_shader_asset_id.is_some()
            || procedural_sprite.is_some()
            || attached_kind.is_some()
        {
            Some(resolve_streamed_visual_material_kind(
                shader_materials_enabled(),
                world_sprite_kind,
                has_streamed_sprite_shader_path,
            ))
        } else {
            None
        };
        let should_clear_visual = visual_asset_id.is_none()
            || catalog_reloaded
            || has_planet_shader
            || is_suppressed
            || player_tag.is_some()
            || has_controlled_entity_guid
            || desired_kind.is_some_and(|desired| {
                streamed_visual_needs_rebuild(attached_kind.copied(), desired)
            });
        if !should_clear_visual {
            continue;
        }
        let mut removed_any_child = false;
        for child in children.iter() {
            if visual_children.get(child).is_ok() {
                queue_despawn_if_exists(&mut commands, child);
                removed_any_child = true;
            }
        }
        if (has_visual_attached || removed_any_child)
            && let Ok(mut parent_commands) = commands.get_entity(parent_entity)
        {
            parent_commands.remove::<(StreamedVisualAttached, StreamedVisualAttachmentKind)>();
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn attach_streamed_visual_assets_system(
    mut commands: Commands<'_, '_>,
    mut images: ResMut<'_, Assets<Image>>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    mut cached_assets: Local<'_, StreamedVisualAssetCaches>,
    cache_adapter: Res<'_, super::resources::AssetCacheAdapter>,
    shader_assignments: Res<'_, shaders::RuntimeShaderAssignments>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut quad_mesh: ResMut<'_, RuntimeSharedQuadMesh>,
    mut sprite_shader_materials: ResMut<'_, Assets<StreamedSpriteShaderMaterial>>,
    mut asteroid_shader_materials: ResMut<'_, Assets<AsteroidSpriteShaderMaterial>>,
    world_lighting: Res<'_, WorldLightingState>,
    camera_local_lights: Res<'_, CameraLocalLightSet>,
    candidates: Query<
        '_,
        '_,
        (
            Entity,
            &StreamedVisualAssetId,
            Option<&EntityGuid>,
            Option<&ProceduralSprite>,
            Option<&SizeM>,
            Option<&Position>,
            Option<&StreamedSpriteShaderAssetId>,
            Option<&ResolvedRuntimeRenderLayer>,
            Has<PlanetBodyShaderSettings>,
            Option<&PendingVisibilityFadeIn>,
        ),
        (
            With<WorldEntity>,
            Without<PlayerTag>,
            Without<ControlledEntityGuid>,
            Without<StreamedVisualAttached>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
) {
    if cached_assets.last_reload_generation != asset_manager.reload_generation {
        cached_assets.streamed_image_cache.clear();
        cached_assets.generic_material_cache.by_image.clear();
        cached_assets.generic_material_cache.reload_generation = asset_manager.reload_generation;
        cached_assets.last_reload_generation = asset_manager.reload_generation;
    }
    let use_shader_materials = shader_materials_enabled();
    for (
        entity,
        asset_id,
        entity_guid,
        procedural_sprite,
        size_m,
        position,
        sprite_shader,
        resolved_render_layer,
        has_planet_shader,
        pending_fade_in,
    ) in &candidates
    {
        if has_planet_shader {
            continue;
        }
        let Ok(mut entity_commands) = commands.get_entity(entity) else {
            continue;
        };
        ensure_visual_parent_spatial_components(&mut entity_commands);

        let world_sprite_kind = sprite_shader
            .and_then(|shader| shaders::world_sprite_shader_kind(&shader_assignments, &shader.0));
        let is_asteroid_shader = matches!(
            world_sprite_kind,
            Some(shaders::RuntimeWorldSpriteShaderKind::Asteroid)
        );
        let generated_asteroid_image = if is_asteroid_shader
            && let Some(procedural_sprite) = procedural_sprite
            && procedural_sprite.generator_id == "asteroid_rocky_v1"
        {
            let guid = entity_guid
                .map(|guid| guid.0)
                .unwrap_or_else(uuid::Uuid::nil);
            let fingerprint = procedural_sprite_fingerprint(procedural_sprite);
            Some(
                cached_assets
                    .asteroid_sprite_cache
                    .entry((guid, fingerprint))
                    .or_insert_with(|| {
                        let generated = generate_procedural_sprite_image_set(
                            &guid.to_string(),
                            procedural_sprite,
                        )
                        .expect("procedural asteroid sprite generation must succeed");
                        let albedo = images.add(image_from_rgba(
                            generated.width,
                            generated.height,
                            generated.albedo_rgba,
                        ));
                        let normal = images.add(image_from_rgba(
                            generated.width,
                            generated.height,
                            generated.normal_rgba,
                        ));
                        (albedo, normal)
                    })
                    .0
                    .clone(),
            )
        } else {
            None
        };

        let image_handle = if let Some(handle) = generated_asteroid_image.clone() {
            handle
        } else if let Some(handle) = cached_assets.streamed_image_cache.get(&asset_id.0) {
            handle.clone()
        } else {
            let Some(handle) = assets::cached_image_handle(
                &asset_id.0,
                &asset_manager,
                &asset_root.0,
                *cache_adapter,
                &mut images,
            ) else {
                continue;
            };
            cached_assets
                .streamed_image_cache
                .insert(asset_id.0.clone(), handle.clone());
            handle
        };

        let texture_size_px = generated_asteroid_image
            .as_ref()
            .and_then(|handle| images.get(handle))
            .map(|image| image.size())
            .or_else(|| images.get(&image_handle).map(|image| image.size()));
        let custom_size = assets::resolved_world_sprite_size(texture_size_px, size_m);

        let has_streamed_sprite_shader_path = sprite_shader.is_some_and(|shader| {
            shaders::world_sprite_shader_ready(
                &asset_root.0,
                &asset_manager,
                *cache_adapter,
                &shader.0,
            )
        });
        let material_kind = resolve_streamed_visual_material_kind(
            use_shader_materials,
            world_sprite_kind,
            has_streamed_sprite_shader_path,
        );
        match material_kind {
            StreamedVisualMaterialKind::AsteroidShader => {
                let shared_quad = shared_unit_quad_handle(&mut quad_mesh, &mut meshes);
                let material = asteroid_shader_materials.add(AsteroidSpriteShaderMaterial {
                    image: image_handle.clone(),
                    lighting: SharedWorldLightingUniforms::from_state_for_world_position(
                        &world_lighting,
                        position.map(|value| value.0).unwrap_or(Vec2::ZERO),
                        &camera_local_lights,
                    ),
                });
                let sprite_size = custom_size.unwrap_or(Vec2::splat(16.0));
                let (x, y, z) = streamed_visual_layer_transform(resolved_render_layer, Vec2::ZERO);
                entity_commands.with_children(|child| {
                    child.spawn((
                        StreamedVisualChild,
                        Mesh2d(shared_quad),
                        MeshMaterial2d(material),
                        Transform::from_xyz(x, y, z).with_scale(Vec3::new(
                            sprite_size.x,
                            sprite_size.y,
                            1.0,
                        )),
                    ));
                });
                entity_commands.try_insert((
                    StreamedVisualAttached,
                    StreamedVisualAttachmentKind::AsteroidShader,
                ));
                continue;
            }
            StreamedVisualMaterialKind::GenericShader => {
                let shared_quad = shared_unit_quad_handle(&mut quad_mesh, &mut meshes);
                let material = shared_streamed_sprite_material_handle(
                    &mut cached_assets.generic_material_cache,
                    &mut sprite_shader_materials,
                    &image_handle,
                );
                let sprite_size = custom_size.unwrap_or(Vec2::splat(16.0));
                let (x, y, z) = streamed_visual_layer_transform(resolved_render_layer, Vec2::ZERO);
                entity_commands.with_children(|child| {
                    child.spawn((
                        StreamedVisualChild,
                        Mesh2d(shared_quad),
                        MeshMaterial2d(material),
                        Transform::from_xyz(x, y, z).with_scale(Vec3::new(
                            sprite_size.x,
                            sprite_size.y,
                            1.0,
                        )),
                    ));
                });
                entity_commands.try_insert((
                    StreamedVisualAttached,
                    StreamedVisualAttachmentKind::GenericShader,
                ));
                continue;
            }
            StreamedVisualMaterialKind::Plain => {}
        }
        let (x, y, z) = streamed_visual_layer_transform(resolved_render_layer, Vec2::ZERO);
        entity_commands.with_children(|child| {
            child.spawn((
                StreamedVisualChild,
                Sprite {
                    image: image_handle,
                    color: if pending_fade_in.is_some() {
                        Color::srgba(1.0, 1.0, 1.0, 0.0)
                    } else {
                        Color::WHITE
                    },
                    custom_size,
                    ..Default::default()
                },
                Transform::from_xyz(x, y, z),
            ));
        });
        entity_commands.try_insert((StreamedVisualAttached, StreamedVisualAttachmentKind::Plain));
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{
        PROJECTILE_VISUAL_Z, StreamedVisualMaterialKind, WEAPON_IMPACT_SPARK_TTL_S,
        activate_destruction_effect, attach_ballistic_projectile_visuals_system,
        bootstrap_local_ballistic_projectile_visual_roots_system,
        ensure_planet_body_root_visibility_system, ensure_visual_parent_spatial_components,
        planet_camera_relative_translation, runtime_layer_screen_scale_factor,
        streamed_visual_needs_rebuild, suppress_duplicate_predicted_interpolated_visuals_system,
        sync_unadopted_ballistic_projectile_visual_roots_system,
        update_weapon_impact_sparks_system,
    };
    use crate::runtime::backdrop::RuntimeEffectMaterial;
    use crate::runtime::components::{
        BallisticProjectileVisualAttached, ControlledEntity, PendingInitialVisualReady,
        StreamedVisualAttachmentKind, SuppressedPredictedDuplicateVisual, WeaponImpactExplosion,
        WeaponImpactExplosionPool, WeaponImpactSpark, WorldEntity,
    };
    use crate::runtime::resources::DuplicateVisualResolutionState;
    use crate::runtime::transforms::{
        reveal_world_entities_when_initial_transform_ready,
        sync_interpolated_world_entity_transforms_without_history,
    };
    use avian2d::prelude::{Position, Rotation};
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;
    use bevy::sprite_render::MeshMaterial2d;
    use lightyear::prelude::Interpolated;
    use sidereal_game::{BallisticProjectile, DamageType, EntityGuid, PlanetBodyShaderSettings};

    #[test]
    fn streamed_visual_rebuilds_when_material_kind_changes() {
        assert!(streamed_visual_needs_rebuild(
            Some(StreamedVisualAttachmentKind::Plain),
            StreamedVisualMaterialKind::AsteroidShader,
        ));
        assert!(streamed_visual_needs_rebuild(
            Some(StreamedVisualAttachmentKind::GenericShader),
            StreamedVisualMaterialKind::Plain,
        ));
        assert!(!streamed_visual_needs_rebuild(
            Some(StreamedVisualAttachmentKind::AsteroidShader),
            StreamedVisualMaterialKind::AsteroidShader,
        ));
    }

    #[test]
    fn planet_root_visibility_waits_for_initial_visual_ready() {
        let mut app = App::new();
        app.add_systems(Update, ensure_planet_body_root_visibility_system);

        let entity = app
            .world_mut()
            .spawn((
                WorldEntity,
                PlanetBodyShaderSettings::default(),
                Visibility::Visible,
                PendingInitialVisualReady,
            ))
            .id();

        app.update();

        let entity_ref = app.world().entity(entity);
        assert_eq!(
            *entity_ref.get::<Visibility>().expect("visibility"),
            Visibility::Hidden
        );
        assert!(
            entity_ref.contains::<PendingInitialVisualReady>(),
            "planet root should stay pending until visuals are actually ready"
        );
    }

    #[test]
    fn visual_parent_spatial_components_are_backfilled() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();

        let mut commands = app.world_mut().commands();
        let mut entity_commands = commands.entity(entity);
        ensure_visual_parent_spatial_components(&mut entity_commands);

        app.update();

        let entity_ref = app.world().entity(entity);
        assert!(entity_ref.contains::<Transform>());
        assert!(entity_ref.contains::<GlobalTransform>());
        assert!(entity_ref.contains::<Visibility>());
    }

    #[test]
    fn planet_camera_relative_translation_tracks_projected_position() {
        let offset = planet_camera_relative_translation(
            None,
            Vec2::new(100.0, 50.0),
            Vec2::new(300.0, 90.0),
        );
        assert_eq!(offset, Vec2::new(-200.0, -40.0));

        let layer = crate::runtime::components::ResolvedRuntimeRenderLayer {
            layer_id: "midground_planets".to_string(),
            definition: sidereal_game::RuntimeRenderLayerDefinition {
                layer_id: "midground_planets".to_string(),
                phase: "world".to_string(),
                material_domain: "world_polygon".to_string(),
                shader_asset_id: "planet_visual_wgsl".to_string(),
                parallax_factor: Some(0.25),
                ..Default::default()
            },
        };
        let offset = planet_camera_relative_translation(
            Some(&layer),
            Vec2::new(100.0, 50.0),
            Vec2::new(300.0, 90.0),
        );
        assert_eq!(offset, Vec2::new(-50.0, -10.0));
    }

    #[test]
    fn runtime_layer_screen_scale_defaults_and_clamps() {
        let default_layer = sidereal_game::RuntimeRenderLayerDefinition::default();
        assert_eq!(runtime_layer_screen_scale_factor(&default_layer), 1.0);

        let mut authored = sidereal_game::RuntimeRenderLayerDefinition {
            screen_scale_factor: Some(1.5),
            ..Default::default()
        };
        assert_eq!(runtime_layer_screen_scale_factor(&authored), 1.5);

        authored.screen_scale_factor = Some(1000.0);
        assert_eq!(runtime_layer_screen_scale_factor(&authored), 64.0);
    }

    #[test]
    fn weapon_impact_spark_expiry_hides_instead_of_despawning() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<RuntimeEffectMaterial>();
        app.insert_resource(Time::<()>::default());
        app.add_systems(Update, update_weapon_impact_sparks_system);

        let material = {
            let mut materials = app
                .world_mut()
                .resource_mut::<Assets<RuntimeEffectMaterial>>();
            materials.add(RuntimeEffectMaterial::default())
        };

        let entity = app
            .world_mut()
            .spawn((
                WeaponImpactSpark {
                    ttl_s: 0.0,
                    max_ttl_s: WEAPON_IMPACT_SPARK_TTL_S,
                },
                Transform::default(),
                MeshMaterial2d(material),
                Visibility::Visible,
            ))
            .id();

        app.update();

        let entity_ref = app.world().entity(entity);
        assert!(entity_ref.contains::<WeaponImpactSpark>());
        assert_eq!(
            *entity_ref.get::<Visibility>().expect("visibility"),
            Visibility::Hidden
        );
    }

    #[test]
    fn destruction_effect_uses_existing_explosion_pool() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<RuntimeEffectMaterial>();
        app.insert_resource(WeaponImpactExplosionPool {
            explosions: Vec::new(),
            next_index: 0,
        });

        let material = {
            let mut materials = app
                .world_mut()
                .resource_mut::<Assets<RuntimeEffectMaterial>>();
            materials.add(RuntimeEffectMaterial::default())
        };

        let explosion = app
            .world_mut()
            .spawn((
                WeaponImpactExplosion {
                    ttl_s: 0.0,
                    max_ttl_s: 0.18,
                    base_scale: 1.2,
                    growth_scale: 4.4,
                    intensity_scale: 1.0,
                    domain_scale: 1.12,
                    screen_distortion_scale: 0.0,
                },
                Transform::default(),
                MeshMaterial2d(material),
                Visibility::Hidden,
            ))
            .id();
        app.world_mut()
            .resource_mut::<WeaponImpactExplosionPool>()
            .explosions
            .push(explosion);

        let _ = app.world_mut().run_system_once(
            |mut pool: ResMut<'_, WeaponImpactExplosionPool>,
             mut materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
             mut query: Query<
                '_,
                '_,
                (
                    &'_ mut WeaponImpactExplosion,
                    &'_ mut Transform,
                    &'_ MeshMaterial2d<RuntimeEffectMaterial>,
                    &'_ mut Visibility,
                ),
                super::WeaponImpactExplosionQueryFilter,
            >| {
                activate_destruction_effect(
                    "explosion_burst",
                    Vec2::new(12.0, -4.0),
                    &mut pool,
                    &mut query,
                    &mut materials,
                );
            },
        );

        let entity_ref = app.world().entity(explosion);
        let effect = entity_ref
            .get::<WeaponImpactExplosion>()
            .expect("explosion effect");
        let transform = entity_ref.get::<Transform>().expect("transform");
        assert_eq!(transform.translation.x, 12.0);
        assert_eq!(transform.translation.y, -4.0);
        assert!(
            effect.screen_distortion_scale > 0.0,
            "destruction effects should opt into screen-space distortion"
        );
        assert_eq!(
            *entity_ref.get::<Visibility>().expect("visibility"),
            Visibility::Visible
        );
    }

    #[test]
    fn local_ballistic_projectiles_get_immediate_visual_root_and_preserve_pose_on_attach() {
        let mut app = App::new();

        let entity = app
            .world_mut()
            .spawn((
                Position(Vec2::new(18.0, -7.5)),
                Rotation::from(Quat::from_rotation_z(0.35)),
                BallisticProjectile::new(
                    uuid::Uuid::new_v4(),
                    uuid::Uuid::new_v4(),
                    10.0,
                    DamageType::Ballistic,
                    0.25,
                    0.35,
                ),
            ))
            .id();

        let _ = app
            .world_mut()
            .run_system_once(bootstrap_local_ballistic_projectile_visual_roots_system);
        let _ = app
            .world_mut()
            .run_system_once(attach_ballistic_projectile_visuals_system);

        let entity_ref = app.world().entity(entity);
        let transform = entity_ref.get::<Transform>().expect("transform");
        assert_eq!(transform.translation.truncate(), Vec2::new(18.0, -7.5));
        assert!((transform.rotation.to_euler(EulerRot::XYZ).2 - 0.35).abs() < 0.001);
        assert!((transform.translation.z - PROJECTILE_VISUAL_Z).abs() < f32::EPSILON);
        assert!(
            entity_ref.contains::<BallisticProjectileVisualAttached>(),
            "local prespawned projectile should become renderable before replication adoption"
        );
        assert_eq!(
            *entity_ref.get::<Visibility>().expect("visibility"),
            Visibility::Visible
        );
    }

    #[test]
    fn unadopted_ballistic_projectile_transform_sync_preserves_visual_depth() {
        let mut app = App::new();

        let entity = app
            .world_mut()
            .spawn((
                Position(Vec2::new(4.0, 9.0)),
                Rotation::from(Quat::from_rotation_z(-0.6)),
                BallisticProjectile::new(
                    uuid::Uuid::new_v4(),
                    uuid::Uuid::new_v4(),
                    10.0,
                    DamageType::Ballistic,
                    0.25,
                    0.35,
                ),
                Transform::from_xyz(-100.0, 55.0, PROJECTILE_VISUAL_Z),
            ))
            .id();

        let _ = app
            .world_mut()
            .run_system_once(sync_unadopted_ballistic_projectile_visual_roots_system);

        let transform = app
            .world()
            .entity(entity)
            .get::<Transform>()
            .expect("transform");
        assert_eq!(transform.translation.truncate(), Vec2::new(4.0, 9.0));
        assert!((transform.rotation.to_euler(EulerRot::XYZ).2 + 0.6).abs() < 0.001);
        assert!(
            (transform.translation.z - PROJECTILE_VISUAL_Z).abs() < f32::EPSILON,
            "projectile root sync should not flatten the visual layer depth"
        );
    }

    #[test]
    fn observer_ballistic_projectile_uses_authoritative_spawn_pose_before_first_history_sample() {
        let mut app = App::new();

        let entity = app
            .world_mut()
            .spawn((
                Interpolated,
                WorldEntity,
                PendingInitialVisualReady,
                Visibility::Hidden,
                Transform::default(),
                Position(Vec2::new(64.0, -22.0)),
                Rotation::from(Quat::from_rotation_z(1.1)),
                BallisticProjectile::new(
                    uuid::Uuid::new_v4(),
                    uuid::Uuid::new_v4(),
                    10.0,
                    DamageType::Ballistic,
                    0.25,
                    0.35,
                ),
            ))
            .id();

        let _ = app
            .world_mut()
            .run_system_once(sync_interpolated_world_entity_transforms_without_history);
        let _ = app
            .world_mut()
            .run_system_once(reveal_world_entities_when_initial_transform_ready);
        let _ = app
            .world_mut()
            .run_system_once(attach_ballistic_projectile_visuals_system);

        let entity_ref = app.world().entity(entity);
        let transform = entity_ref.get::<Transform>().expect("transform");
        assert_eq!(transform.translation.truncate(), Vec2::new(64.0, -22.0));
        assert_ne!(
            transform.translation.truncate(),
            Vec2::ZERO,
            "observer projectile should not render at the origin before interpolation history exists"
        );
        assert!((transform.rotation.to_euler(EulerRot::XYZ).2 - 1.1).abs() < 0.001);
        assert!(
            entity_ref.contains::<BallisticProjectileVisualAttached>(),
            "observer projectile should attach the projectile tracer visual"
        );
        assert_eq!(
            *entity_ref.get::<Visibility>().expect("visibility"),
            Visibility::Visible
        );
        assert!(
            !entity_ref.contains::<PendingInitialVisualReady>(),
            "observer projectile should leave the pending-visual gate once the authoritative pose is available"
        );
    }

    #[test]
    fn duplicate_visual_winner_swaps_without_full_world_scan_state_reset() {
        let mut app = App::new();
        app.init_resource::<DuplicateVisualResolutionState>();
        app.add_systems(
            Update,
            suppress_duplicate_predicted_interpolated_visuals_system,
        );

        let guid = uuid::Uuid::new_v4();
        let controlled = app
            .world_mut()
            .spawn((
                WorldEntity,
                EntityGuid(guid),
                ControlledEntity {
                    entity_id: "controlled".to_string(),
                    player_entity_id: "player".to_string(),
                },
                Visibility::Visible,
            ))
            .id();
        let fallback = app
            .world_mut()
            .spawn((WorldEntity, EntityGuid(guid), Visibility::Visible))
            .id();

        app.update();
        assert!(
            !app.world()
                .entity(controlled)
                .contains::<SuppressedPredictedDuplicateVisual>()
        );
        assert!(
            app.world()
                .entity(fallback)
                .contains::<SuppressedPredictedDuplicateVisual>()
        );

        app.world_mut()
            .entity_mut(controlled)
            .remove::<ControlledEntity>();
        app.world_mut()
            .entity_mut(fallback)
            .insert(ControlledEntity {
                entity_id: "fallback".to_string(),
                player_entity_id: "player".to_string(),
            });

        app.update();

        assert!(
            app.world()
                .entity(controlled)
                .contains::<SuppressedPredictedDuplicateVisual>()
        );
        assert!(
            !app.world()
                .entity(fallback)
                .contains::<SuppressedPredictedDuplicateVisual>()
        );
        assert_eq!(
            app.world()
                .resource::<DuplicateVisualResolutionState>()
                .winner_by_guid
                .get(&guid),
            Some(&fallback)
        );
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn update_streamed_visual_layer_transforms_system(
    camera_motion: Res<'_, CameraMotionState>,
    parents: Query<'_, '_, &'_ ResolvedRuntimeRenderLayer>,
    mut children: Query<'_, '_, (&'_ ChildOf, &'_ mut Transform), With<StreamedVisualChild>>,
) {
    for (parent, mut transform) in &mut children {
        let Ok(layer) = parents.get(parent.parent()) else {
            continue;
        };
        let (x, y, z) =
            streamed_visual_layer_transform(Some(layer), camera_motion.parallax_position_xy);
        transform.translation.x = x;
        transform.translation.y = y;
        transform.translation.z = z;
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn cleanup_planet_body_visual_children_system(
    mut commands: Commands<'_, '_>,
    parents: Query<
        '_,
        '_,
        (
            Entity,
            &'_ Children,
            Option<&'_ PlanetBodyShaderSettings>,
            Option<&'_ RuntimeWorldVisualStack>,
            Option<&'_ RuntimeWorldVisualPassSet>,
            Has<SuppressedPredictedDuplicateVisual>,
        ),
        With<WorldEntity>,
    >,
    visual_children: Query<'_, '_, &'_ RuntimeWorldVisualPass>,
) {
    for (parent_entity, children, planet_settings, visual_stack, pass_set, is_suppressed) in
        &parents
    {
        let should_clear_all_visuals = planet_settings.is_none()
            || is_suppressed
            || !planet_settings.is_some_and(|v| v.enabled);
        let mut removed_any_child = false;
        let desired_pass_set =
            desired_world_visual_pass_set(visual_stack, RuntimeWorldVisualFamily::Planet);
        let mut next_pass_set = pass_set.copied().unwrap_or_default();
        for child in children.iter() {
            let Ok(pass) = visual_children.get(child) else {
                continue;
            };
            if pass.family != RuntimeWorldVisualFamily::Planet {
                continue;
            }
            let remove_child = should_clear_all_visuals || !desired_pass_set.contains(pass.kind);
            if remove_child {
                queue_despawn_if_exists(&mut commands, child);
                next_pass_set.remove(pass.kind);
                removed_any_child = true;
            }
        }
        if (pass_set.is_some() || removed_any_child)
            && let Ok(mut parent_commands) = commands.get_entity(parent_entity)
        {
            if should_clear_all_visuals || desired_pass_set.is_empty() {
                parent_commands.remove::<RuntimeWorldVisualPassSet>();
            } else {
                parent_commands.insert(desired_pass_set);
            }
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn attach_planet_visual_stack_system(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut quad_mesh: ResMut<'_, RuntimeSharedQuadMesh>,
    mut planet_materials: ResMut<'_, Assets<PlanetVisualMaterial>>,
    time: Res<'_, Time>,
    camera_motion: Res<'_, CameraMotionState>,
    shader_assignments: Res<'_, shaders::RuntimeShaderAssignments>,
    world_lighting: Res<'_, WorldLightingState>,
    camera_local_lights: Res<'_, CameraLocalLightSet>,
    mut candidates: Query<
        '_,
        '_,
        (
            Entity,
            &'_ PlanetBodyShaderSettings,
            Option<&'_ RuntimeWorldVisualStack>,
            Option<&'_ SizeM>,
            Option<&'_ Position>,
            Option<&'_ WorldPosition>,
            Option<&'_ ResolvedRuntimeRenderLayer>,
            &'_ mut Visibility,
            Option<&'_ RuntimeWorldVisualPassSet>,
        ),
        (
            With<WorldEntity>,
            Without<PlayerTag>,
            Without<ControlledEntityGuid>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
) {
    if !shader_materials_enabled() {
        return;
    }
    let camera_world_position_xy = camera_motion.parallax_position_xy;
    for (
        entity,
        settings,
        visual_stack,
        size_m,
        position,
        world_position,
        resolved_render_layer,
        mut visibility,
        pass_set,
    ) in &mut candidates
    {
        if !settings.enabled {
            continue;
        }
        let Ok(mut entity_commands) = commands.get_entity(entity) else {
            continue;
        };
        ensure_visual_parent_spatial_components(&mut entity_commands);
        let time_s = time.elapsed_secs();
        let world_position = resolve_world_position(position, world_position).unwrap_or(Vec2::ZERO);
        let diameter_m = size_m
            .map(|v| v.length.max(v.width).max(1.0))
            .unwrap_or(256.0);
        let layer_base_z = planet_layer_base_z(resolved_render_layer);
        let projected_center_world = planet_camera_relative_translation(
            resolved_render_layer,
            world_position,
            camera_world_position_xy,
        );
        let layer_screen_scale = resolved_render_layer
            .map(|layer| runtime_layer_screen_scale_factor(&layer.definition))
            .unwrap_or(1.0);
        let mut next_pass_set = pass_set.copied().unwrap_or_default();
        let Some(body_pass) =
            find_world_visual_pass(visual_stack, RuntimeWorldVisualPassKind::PlanetBody)
        else {
            continue;
        };
        if !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetBody)
            && shaders::world_polygon_shader_kind(&shader_assignments, &body_pass.shader_asset_id)
                == Some(shaders::RuntimeWorldPolygonShaderKind::PlanetVisual)
        {
            let mesh = shared_unit_quad_handle(&mut quad_mesh, &mut meshes);
            let material = planet_materials.add(PlanetVisualMaterial {
                params: PlanetBodyUniforms::from_settings(
                    settings,
                    time_s,
                    world_position,
                    &world_lighting,
                    &camera_local_lights,
                ),
            });
            let scale_multiplier = visual_pass_scale_multiplier(Some(body_pass), 1.0);
            let depth_bias_z = visual_pass_depth_bias_z(Some(body_pass), 0.0);
            entity_commands.with_children(|child| {
                child.spawn((
                    pass_tag(
                        RuntimeWorldVisualFamily::Planet,
                        RuntimeWorldVisualPassKind::PlanetBody,
                    ),
                    NoFrustumCulling,
                    Mesh2d(mesh),
                    MeshMaterial2d(material),
                    RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                    Transform::from_xyz(
                        projected_center_world.x,
                        projected_center_world.y,
                        layer_base_z + PLANET_BODY_LAYER_Z_OFFSET + depth_bias_z,
                    )
                    .with_scale(Vec3::new(
                        diameter_m * scale_multiplier * layer_screen_scale,
                        diameter_m * scale_multiplier * layer_screen_scale,
                        1.0,
                    )),
                ));
            });
            next_pass_set.insert(RuntimeWorldVisualPassKind::PlanetBody);
        }
        if let (Some(back_pass), Some(front_pass)) = (
            find_world_visual_pass(visual_stack, RuntimeWorldVisualPassKind::PlanetCloudBack),
            find_world_visual_pass(visual_stack, RuntimeWorldVisualPassKind::PlanetCloudFront),
        ) && (!next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetCloudBack)
            || !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetCloudFront))
            && shaders::world_polygon_shader_kind(&shader_assignments, &back_pass.shader_asset_id)
                == Some(shaders::RuntimeWorldPolygonShaderKind::PlanetVisual)
            && shaders::world_polygon_shader_kind(&shader_assignments, &front_pass.shader_asset_id)
                == Some(shaders::RuntimeWorldPolygonShaderKind::PlanetVisual)
        {
            let back_material = planet_materials.add(PlanetVisualMaterial {
                params: PlanetBodyUniforms::from_settings_with_pass(
                    settings,
                    time_s,
                    world_position,
                    Vec4::new(1.0, 0.0, 0.0, 0.0),
                    &world_lighting,
                    &camera_local_lights,
                ),
            });
            let front_material = planet_materials.add(PlanetVisualMaterial {
                params: PlanetBodyUniforms::from_settings_with_pass(
                    settings,
                    time_s,
                    world_position,
                    Vec4::new(2.0, 0.0, 0.0, 0.0),
                    &world_lighting,
                    &camera_local_lights,
                ),
            });
            let back_scale = visual_pass_scale_multiplier(Some(back_pass), 1.035);
            let front_scale = visual_pass_scale_multiplier(Some(front_pass), 1.035);
            let back_depth = visual_pass_depth_bias_z(Some(back_pass), -0.2);
            let front_depth = visual_pass_depth_bias_z(Some(front_pass), 0.5);
            entity_commands.with_children(|child| {
                if !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetCloudBack) {
                    child.spawn((
                        pass_tag(
                            RuntimeWorldVisualFamily::Planet,
                            RuntimeWorldVisualPassKind::PlanetCloudBack,
                        ),
                        NoFrustumCulling,
                        Mesh2d(shared_unit_quad_handle(&mut quad_mesh, &mut meshes)),
                        MeshMaterial2d(back_material),
                        RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                        Transform::from_xyz(
                            projected_center_world.x,
                            projected_center_world.y,
                            layer_base_z + PLANET_CLOUD_BACK_LAYER_Z_OFFSET + back_depth,
                        )
                        .with_scale(Vec3::new(
                            diameter_m * back_scale * layer_screen_scale,
                            diameter_m * back_scale * layer_screen_scale,
                            1.0,
                        )),
                    ));
                }
                if !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetCloudFront) {
                    child.spawn((
                        pass_tag(
                            RuntimeWorldVisualFamily::Planet,
                            RuntimeWorldVisualPassKind::PlanetCloudFront,
                        ),
                        NoFrustumCulling,
                        Mesh2d(shared_unit_quad_handle(&mut quad_mesh, &mut meshes)),
                        MeshMaterial2d(front_material),
                        RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                        Transform::from_xyz(
                            projected_center_world.x,
                            projected_center_world.y,
                            layer_base_z + PLANET_CLOUD_FRONT_LAYER_Z_OFFSET + front_depth,
                        )
                        .with_scale(Vec3::new(
                            diameter_m * front_scale * layer_screen_scale,
                            diameter_m * front_scale * layer_screen_scale,
                            1.0,
                        )),
                    ));
                }
            });
            next_pass_set.insert(RuntimeWorldVisualPassKind::PlanetCloudBack);
            next_pass_set.insert(RuntimeWorldVisualPassKind::PlanetCloudFront);
        }
        if let (Some(back_pass), Some(front_pass)) = (
            find_world_visual_pass(visual_stack, RuntimeWorldVisualPassKind::PlanetRingBack),
            find_world_visual_pass(visual_stack, RuntimeWorldVisualPassKind::PlanetRingFront),
        ) && (!next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetRingBack)
            || !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetRingFront))
            && shaders::world_polygon_shader_kind(&shader_assignments, &back_pass.shader_asset_id)
                == Some(shaders::RuntimeWorldPolygonShaderKind::PlanetVisual)
            && shaders::world_polygon_shader_kind(&shader_assignments, &front_pass.shader_asset_id)
                == Some(shaders::RuntimeWorldPolygonShaderKind::PlanetVisual)
        {
            let back_material = planet_materials.add(PlanetVisualMaterial {
                params: PlanetBodyUniforms::from_settings_with_pass(
                    settings,
                    time_s,
                    world_position,
                    Vec4::new(0.0, 1.0, 0.0, 0.0),
                    &world_lighting,
                    &camera_local_lights,
                ),
            });
            let front_material = planet_materials.add(PlanetVisualMaterial {
                params: PlanetBodyUniforms::from_settings_with_pass(
                    settings,
                    time_s,
                    world_position,
                    Vec4::new(0.0, 2.0, 0.0, 0.0),
                    &world_lighting,
                    &camera_local_lights,
                ),
            });
            let back_scale = visual_pass_scale_multiplier(Some(back_pass), 1.85);
            let front_scale = visual_pass_scale_multiplier(Some(front_pass), 1.85);
            let back_depth = visual_pass_depth_bias_z(Some(back_pass), -0.45);
            let front_depth = visual_pass_depth_bias_z(Some(front_pass), 0.65);
            entity_commands.with_children(|child| {
                if !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetRingBack) {
                    child.spawn((
                        pass_tag(
                            RuntimeWorldVisualFamily::Planet,
                            RuntimeWorldVisualPassKind::PlanetRingBack,
                        ),
                        NoFrustumCulling,
                        Mesh2d(shared_unit_quad_handle(&mut quad_mesh, &mut meshes)),
                        MeshMaterial2d(back_material),
                        RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                        Transform::from_xyz(
                            projected_center_world.x,
                            projected_center_world.y,
                            layer_base_z + PLANET_RING_BACK_LAYER_Z_OFFSET + back_depth,
                        )
                        .with_scale(Vec3::new(
                            diameter_m * back_scale * layer_screen_scale,
                            diameter_m * back_scale * layer_screen_scale,
                            1.0,
                        )),
                    ));
                }
                if !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetRingFront) {
                    child.spawn((
                        pass_tag(
                            RuntimeWorldVisualFamily::Planet,
                            RuntimeWorldVisualPassKind::PlanetRingFront,
                        ),
                        NoFrustumCulling,
                        Mesh2d(shared_unit_quad_handle(&mut quad_mesh, &mut meshes)),
                        MeshMaterial2d(front_material),
                        RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                        Transform::from_xyz(
                            projected_center_world.x,
                            projected_center_world.y,
                            layer_base_z + PLANET_RING_FRONT_LAYER_Z_OFFSET + front_depth,
                        )
                        .with_scale(Vec3::new(
                            diameter_m * front_scale * layer_screen_scale,
                            diameter_m * front_scale * layer_screen_scale,
                            1.0,
                        )),
                    ));
                }
            });
            next_pass_set.insert(RuntimeWorldVisualPassKind::PlanetRingBack);
            next_pass_set.insert(RuntimeWorldVisualPassKind::PlanetRingFront);
        }
        *visibility = Visibility::Visible;
        entity_commands.insert(next_pass_set);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn ensure_planet_body_root_visibility_system(
    mut planets: Query<
        '_,
        '_,
        (
            &'_ PlanetBodyShaderSettings,
            &'_ mut Visibility,
            Option<&'_ PendingInitialVisualReady>,
        ),
        (
            With<WorldEntity>,
            Without<SuppressedPredictedDuplicateVisual>,
            Without<PlayerTag>,
            Without<ControlledEntityGuid>,
        ),
    >,
) {
    for (settings, mut visibility, pending_initial_visual_ready) in &mut planets {
        if !settings.enabled {
            continue;
        }
        if pending_initial_visual_ready.is_some() {
            *visibility = Visibility::Hidden;
            continue;
        }
        if *visibility != Visibility::Visible {
            *visibility = Visibility::Visible;
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn update_planet_body_visuals_system(
    time: Res<'_, Time>,
    camera_motion: Res<'_, CameraMotionState>,
    world_lighting: Res<'_, WorldLightingState>,
    camera_local_lights: Res<'_, CameraLocalLightSet>,
    mut materials: ResMut<'_, Assets<PlanetVisualMaterial>>,
    planet_camera: Query<
        '_,
        '_,
        (&'_ Camera, &'_ Projection, &'_ GlobalTransform),
        With<PlanetBodyCamera>,
    >,
    planets: Query<
        '_,
        '_,
        (
            &'_ Children,
            &'_ PlanetBodyShaderSettings,
            Option<&'_ RuntimeWorldVisualStack>,
            Option<&'_ SizeM>,
            Option<&'_ Position>,
            Option<&'_ WorldPosition>,
            Option<&'_ ResolvedRuntimeRenderLayer>,
        ),
    >,
    mut planet_visuals: Query<
        '_,
        '_,
        (
            &'_ RuntimeWorldVisualPass,
            Option<&'_ MeshMaterial2d<PlanetVisualMaterial>>,
            &'_ mut Transform,
            &'_ mut Visibility,
        ),
        (),
    >,
) {
    let time_s = time.elapsed_secs();
    let camera_view = planet_camera.single().ok();
    let camera_world_position_xy = camera_motion.parallax_position_xy;
    for (
        children,
        settings,
        visual_stack,
        size_m,
        position,
        world_position,
        resolved_render_layer,
    ) in &planets
    {
        if !settings.enabled {
            continue;
        }
        let world_position = resolve_world_position(position, world_position).unwrap_or(Vec2::ZERO);
        let diameter_m = size_m
            .map(|v| v.length.max(v.width).max(1.0))
            .unwrap_or(256.0);
        let layer_base_z = planet_layer_base_z(resolved_render_layer);
        let projected_center_world = planet_camera_relative_translation(
            resolved_render_layer,
            world_position,
            camera_world_position_xy,
        );
        let layer_screen_scale = resolved_render_layer
            .map(|layer| runtime_layer_screen_scale_factor(&layer.definition))
            .unwrap_or(1.0);
        for child in children.iter() {
            if let Ok((pass, planet_material, mut transform, mut visibility)) =
                planet_visuals.get_mut(child)
            {
                if pass.family != RuntimeWorldVisualFamily::Planet {
                    continue;
                }
                let mut projected_radius_m = 0.0;
                if let Some(material_handle) = planet_material
                    && let Some(material) = materials.get_mut(&material_handle.0)
                {
                    let pass_definition = find_world_visual_pass(visual_stack, pass.kind);
                    let (pass_flags, base_z, base_scale) = match pass.kind {
                        RuntimeWorldVisualPassKind::PlanetBody => {
                            (Vec4::ZERO, layer_base_z + PLANET_BODY_LAYER_Z_OFFSET, 1.0)
                        }
                        RuntimeWorldVisualPassKind::PlanetCloudBack => (
                            Vec4::new(1.0, 0.0, 0.0, 0.0),
                            layer_base_z + PLANET_CLOUD_BACK_LAYER_Z_OFFSET,
                            1.035,
                        ),
                        RuntimeWorldVisualPassKind::PlanetCloudFront => (
                            Vec4::new(2.0, 0.0, 0.0, 0.0),
                            layer_base_z + PLANET_CLOUD_FRONT_LAYER_Z_OFFSET,
                            1.035,
                        ),
                        RuntimeWorldVisualPassKind::PlanetRingBack => (
                            Vec4::new(0.0, 1.0, 0.0, 0.0),
                            layer_base_z + PLANET_RING_BACK_LAYER_Z_OFFSET,
                            1.85,
                        ),
                        RuntimeWorldVisualPassKind::PlanetRingFront => (
                            Vec4::new(0.0, 2.0, 0.0, 0.0),
                            layer_base_z + PLANET_RING_FRONT_LAYER_Z_OFFSET,
                            1.85,
                        ),
                        _ => continue,
                    };
                    material.params = PlanetBodyUniforms::from_settings_with_pass(
                        settings,
                        time_s,
                        world_position,
                        pass_flags,
                        &world_lighting,
                        &camera_local_lights,
                    );
                    transform.translation.z =
                        base_z + visual_pass_depth_bias_z(pass_definition, 0.0);
                    let scale_multiplier =
                        visual_pass_scale_multiplier(pass_definition, base_scale);
                    let projected_diameter_m = diameter_m * scale_multiplier * layer_screen_scale;
                    transform.scale = Vec3::new(projected_diameter_m, projected_diameter_m, 1.0);
                    projected_radius_m = projected_diameter_m * 0.5;
                }
                transform.translation.x = projected_center_world.x;
                transform.translation.y = projected_center_world.y;
                let in_projected_view =
                    camera_view.is_none_or(|(camera, projection, camera_transform)| {
                        projected_planet_intersects_camera_view(
                            projected_center_world,
                            projected_radius_m,
                            PLANET_PROJECTED_CULL_BUFFER_M,
                            camera,
                            projection,
                            camera_transform,
                        )
                    });
                *visibility = if in_projected_view {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

fn runtime_layer_parallax_factor(definition: &RuntimeRenderLayerDefinition) -> f32 {
    definition.parallax_factor.unwrap_or(1.0).clamp(0.01, 4.0)
}

fn runtime_layer_z_bias(definition: &RuntimeRenderLayerDefinition) -> f32 {
    definition.depth_bias_z.unwrap_or(definition.order as f32)
}

fn runtime_layer_screen_scale_factor(definition: &RuntimeRenderLayerDefinition) -> f32 {
    definition
        .screen_scale_factor
        .unwrap_or(1.0)
        .clamp(0.01, 64.0)
}

fn planet_layer_base_z(resolved_render_layer: Option<&ResolvedRuntimeRenderLayer>) -> f32 {
    resolved_render_layer
        .map(|layer| runtime_layer_z_bias(&layer.definition))
        .unwrap_or(-60.0)
}

fn planet_camera_relative_translation(
    resolved_render_layer: Option<&ResolvedRuntimeRenderLayer>,
    planet_world_position: Vec2,
    camera_world_position_xy: Vec2,
) -> Vec2 {
    let parallax_factor = resolved_render_layer
        .map(|layer| runtime_layer_parallax_factor(&layer.definition))
        .unwrap_or(1.0);
    (planet_world_position - camera_world_position_xy) * parallax_factor
}

fn projected_planet_intersects_camera_view(
    projected_center_world: Vec2,
    projected_radius_m: f32,
    buffer_m: f32,
    camera: &Camera,
    projection: &Projection,
    camera_transform: &GlobalTransform,
) -> bool {
    let Some(viewport_size) = camera.logical_viewport_size() else {
        return false;
    };
    let Projection::Orthographic(orthographic) = projection else {
        return true;
    };
    let half_extents_world = viewport_size * orthographic.scale * 0.5;
    let radius_with_buffer = projected_radius_m.max(0.0) + buffer_m.max(0.0);
    let delta = projected_center_world - camera_transform.translation().truncate();
    delta.x >= -half_extents_world.x - radius_with_buffer
        && delta.x <= half_extents_world.x + radius_with_buffer
        && delta.y >= -half_extents_world.y - radius_with_buffer
        && delta.y <= half_extents_world.y + radius_with_buffer
}

fn streamed_visual_layer_transform(
    resolved_render_layer: Option<&ResolvedRuntimeRenderLayer>,
    camera_world_position_xy: Vec2,
) -> (f32, f32, f32) {
    let Some(layer) = resolved_render_layer else {
        return (0.0, 0.0, STREAMED_VISUAL_BASE_LAYER_Z);
    };
    let parallax_factor = runtime_layer_parallax_factor(&layer.definition);
    let parallax_offset = -camera_world_position_xy * (1.0 - parallax_factor);
    (
        parallax_offset.x,
        parallax_offset.y,
        STREAMED_VISUAL_BASE_LAYER_Z + runtime_layer_z_bias(&layer.definition),
    )
}

#[allow(clippy::type_complexity)]
pub(super) fn attach_thruster_plume_visuals_system(
    mut commands: Commands<'_, '_>,
    mut assets: ThrusterPlumeAttachAssets<'_>,
    visual_children: Query<'_, '_, &'_ RuntimeWorldVisualPass>,
    engines: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityLabels,
            &'_ Children,
            Option<&'_ RuntimeWorldVisualPassSet>,
        ),
        (
            With<WorldEntity>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
) {
    if !shader_materials_enabled() {
        return;
    }
    for (entity, labels, children, pass_set) in &engines {
        if !has_engine_label(labels) {
            continue;
        }
        let has_existing_plume_child = children.iter().any(|child| {
            visual_children.get(child).is_ok_and(|pass| {
                pass.family == RuntimeWorldVisualFamily::Thruster
                    && pass.kind == RuntimeWorldVisualPassKind::ThrusterPlume
            })
        });
        if has_existing_plume_child {
            continue;
        }
        let Ok(mut entity_commands) = commands.get_entity(entity) else {
            continue;
        };
        let plume_mesh = shared_unit_quad_handle(&mut assets.quad_mesh, &mut assets.meshes);
        let plume_material = assets.plume_materials.add(RuntimeEffectMaterial {
            lighting: SharedWorldLightingUniforms::from_state_for_world_position(
                &assets.world_lighting,
                Vec2::ZERO,
                &assets.camera_local_lights,
            ),
            ..RuntimeEffectMaterial::default()
        });
        entity_commands.with_children(|child| {
            child.spawn((
                pass_tag(
                    RuntimeWorldVisualFamily::Thruster,
                    RuntimeWorldVisualPassKind::ThrusterPlume,
                ),
                Mesh2d(plume_mesh),
                MeshMaterial2d(plume_material),
                Transform::from_xyz(0.0, -0.2, 0.1).with_scale(Vec3::new(1.0, 0.02, 1.0)),
                Visibility::Visible,
            ));
        });
        let mut next_pass_set = pass_set.copied().unwrap_or_default();
        next_pass_set.insert(RuntimeWorldVisualPassKind::ThrusterPlume);
        entity_commands.insert(next_pass_set);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn update_thruster_plume_visuals_system(
    time: Res<'_, Time>,
    world_lighting: Res<'_, WorldLightingState>,
    camera_local_lights: Res<'_, CameraLocalLightSet>,
    mut plume_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut plume_children: Query<
        '_,
        '_,
        (
            &'_ RuntimeWorldVisualPass,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Transform,
            &'_ GlobalTransform,
            &'_ mut Visibility,
        ),
        (),
    >,
    engines: Query<
        '_,
        '_,
        (
            &'_ EntityLabels,
            &'_ Children,
            &'_ MountedOn,
            Option<&'_ AfterburnerCapability>,
            Option<&'_ ThrusterPlumeShaderSettings>,
        ),
    >,
    hulls: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            Option<&'_ MountedOn>,
            &'_ FlightComputer,
            Option<&'_ AfterburnerState>,
        ),
    >,
) {
    let mut hull_state = HashMap::<uuid::Uuid, (f32, bool)>::new();
    for (guid, mounted_on, computer, afterburner_state) in &hulls {
        let thrust_alpha = computer.throttle.max(0.0).clamp(0.0, 1.0);
        let afterburner_active = afterburner_state.is_some_and(|state| state.active);
        let hull_guid = mounted_on
            .map(|mounted_on| mounted_on.parent_entity_id)
            .unwrap_or(guid.0);
        hull_state.insert(hull_guid, (thrust_alpha, afterburner_active));
    }

    for (labels, children, mounted_on, afterburner_capability, plume_settings) in &engines {
        if !has_engine_label(labels) {
            continue;
        }
        let Some((thrust_alpha, afterburner_active)) =
            hull_state.get(&mounted_on.parent_entity_id).copied()
        else {
            continue;
        };
        let settings = plume_settings.cloned().unwrap_or_default();
        if !settings.enabled {
            for child in children.iter() {
                if let Ok((_, _, _, _, mut visibility)) = plume_children.get_mut(child) {
                    *visibility = Visibility::Hidden;
                }
            }
            continue;
        }
        let live_afterburner =
            afterburner_active && afterburner_capability.is_some_and(|cap| cap.enabled);
        let thrust_alpha = if settings.debug_override_enabled {
            settings.debug_forced_thrust_alpha.clamp(0.0, 1.0)
        } else {
            thrust_alpha
        };
        let can_afterburn = if settings.debug_override_enabled {
            settings.debug_force_afterburner
        } else {
            live_afterburner
        };
        let base_length = settings.base_length_m.max(0.0);
        let max_length = settings.max_length_m.max(base_length);
        let reactive_length = (thrust_alpha * settings.reactive_length_scale).clamp(0.0, 1.0);
        let mut plume_length = base_length + (max_length - base_length) * reactive_length;
        if can_afterburn {
            plume_length *= settings.afterburner_length_scale.max(1.0);
        }
        plume_length = plume_length.max(0.02);

        let base_width = settings.base_width_m.max(0.02);
        let max_width = settings.max_width_m.max(base_width);
        let plume_width = base_width + (max_width - base_width) * reactive_length;

        let mut plume_alpha = settings.idle_core_alpha
            + (settings.max_alpha - settings.idle_core_alpha).max(0.0)
                * (thrust_alpha * settings.reactive_alpha_scale).clamp(0.0, 1.0);
        if can_afterburn {
            plume_alpha += settings.afterburner_alpha_boost.max(0.0);
        }
        plume_alpha = plume_alpha.clamp(0.0, 1.0);
        let afterburner_alpha = if can_afterburn { 1.0 } else { 0.0 };

        for child in children.iter() {
            let Ok((pass, material_handle, mut transform, global_transform, mut visibility)) =
                plume_children.get_mut(child)
            else {
                continue;
            };
            if pass.kind != RuntimeWorldVisualPassKind::ThrusterPlume {
                continue;
            }
            if let Some(material) = plume_materials.get_mut(&material_handle.0) {
                material.lighting = SharedWorldLightingUniforms::from_state_for_world_position(
                    &world_lighting,
                    global_transform.translation().truncate(),
                    &camera_local_lights,
                );
                material.params = RuntimeEffectUniforms::thruster_plume(
                    thrust_alpha.clamp(0.0, 1.0),
                    afterburner_alpha,
                    time.elapsed_secs(),
                    plume_alpha,
                    settings.falloff.max(0.05),
                    settings.edge_softness.max(0.1),
                    settings.noise_strength.max(0.0),
                    settings.flicker_hz.max(0.0),
                    settings.base_color_rgb.extend(1.0),
                    settings.hot_color_rgb.extend(1.0),
                    settings.afterburner_color_rgb.extend(1.0),
                );
            }
            transform.translation = Vec3::new(0.0, -(plume_length * 0.5 + plume_width * 0.18), 0.1);
            transform.scale = Vec3::new(plume_width, plume_length, 1.0);
            *visibility = if plume_alpha > 0.001 {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn update_asteroid_shader_lighting_system(
    world_lighting: Res<'_, WorldLightingState>,
    camera_local_lights: Res<'_, CameraLocalLightSet>,
    shader_assignments: Res<'_, shaders::RuntimeShaderAssignments>,
    mut asteroid_materials: ResMut<'_, Assets<AsteroidSpriteShaderMaterial>>,
    parents: Query<
        '_,
        '_,
        (
            &'_ Children,
            Option<&'_ Position>,
            Option<&'_ WorldPosition>,
            Option<&'_ StreamedSpriteShaderAssetId>,
        ),
        (
            With<WorldEntity>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
    children: Query<
        '_,
        '_,
        &'_ MeshMaterial2d<AsteroidSpriteShaderMaterial>,
        With<StreamedVisualChild>,
    >,
) {
    for (entity_children, position, world_position, shader_asset_id) in &parents {
        if !matches!(
            shader_asset_id.and_then(|shader| {
                shaders::world_sprite_shader_kind(&shader_assignments, &shader.0)
            }),
            Some(shaders::RuntimeWorldSpriteShaderKind::Asteroid)
        ) {
            continue;
        }
        let lighting = SharedWorldLightingUniforms::from_state_for_world_position(
            &world_lighting,
            resolve_world_position(position, world_position).unwrap_or(Vec2::ZERO),
            &camera_local_lights,
        );
        for child in entity_children.iter() {
            if let Ok(material_handle) = children.get(child)
                && let Some(material) = asteroid_materials.get_mut(&material_handle.0)
            {
                material.lighting = lighting.clone();
            }
        }
    }
}

pub(super) fn ensure_weapon_tracer_pool_system(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut quad_mesh: ResMut<'_, RuntimeSharedQuadMesh>,
    mut effect_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut pool: ResMut<'_, WeaponTracerPool>,
) {
    if !pool.bolts.is_empty() {
        return;
    }
    pool.bolts.reserve(WEAPON_TRACER_POOL_SIZE);
    let mesh = shared_unit_quad_handle(&mut quad_mesh, &mut meshes);
    for _ in 0..WEAPON_TRACER_POOL_SIZE {
        let material = effect_materials.add(RuntimeEffectMaterial {
            params: RuntimeEffectUniforms::beam_trail(
                0.0,
                0.0,
                0.65,
                0.35,
                0.12,
                Vec4::new(1.0, 0.96, 0.7, 1.0),
                Vec4::new(1.0, 0.72, 0.22, 1.0),
            ),
            ..RuntimeEffectMaterial::default()
        });
        let bolt = commands
            .spawn((
                WeaponTracerBolt {
                    excluded_entity: None,
                    velocity: Vec2::ZERO,
                    impact_xy: None,
                    ttl_s: 0.0,
                    lateral_normal: Vec2::ZERO,
                    wiggle_phase_rad: 0.0,
                    wiggle_freq_hz: WEAPON_TRACER_WIGGLE_BASE_FREQ_HZ,
                    wiggle_amp_mps: 0.0,
                },
                Mesh2d(mesh.clone()),
                MeshMaterial2d(material),
                Transform::from_xyz(0.0, 0.0, 0.35).with_scale(Vec3::new(
                    WEAPON_TRACER_WIDTH_M,
                    WEAPON_TRACER_LENGTH_M,
                    1.0,
                )),
                Visibility::Hidden,
                WorldEntity,
                DespawnOnExit(ClientAppState::InWorld),
            ))
            .id();
        pool.bolts.push(bolt);
    }
}

pub(super) fn ensure_weapon_impact_spark_pool_system(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut quad_mesh: ResMut<'_, RuntimeSharedQuadMesh>,
    mut effect_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut pool: ResMut<'_, WeaponImpactSparkPool>,
) {
    if !pool.sparks.is_empty() {
        return;
    }
    pool.sparks.reserve(WEAPON_IMPACT_SPARK_POOL_SIZE);
    let mesh = shared_unit_quad_handle(&mut quad_mesh, &mut meshes);
    for _ in 0..WEAPON_IMPACT_SPARK_POOL_SIZE {
        let spark = commands
            .spawn((
                WeaponImpactSpark {
                    ttl_s: 0.0,
                    max_ttl_s: WEAPON_IMPACT_SPARK_TTL_S,
                },
                Mesh2d(mesh.clone()),
                MeshMaterial2d(effect_materials.add(RuntimeEffectMaterial {
                    params: RuntimeEffectUniforms::impact_spark(
                        0.0,
                        1.0,
                        1.0,
                        0.95,
                        Vec4::new(1.0, 0.9, 0.55, 1.0),
                    ),
                    ..RuntimeEffectMaterial::default()
                })),
                Transform::from_xyz(0.0, 0.0, 0.45),
                Visibility::Hidden,
                WorldEntity,
                DespawnOnExit(ClientAppState::InWorld),
            ))
            .id();
        pool.sparks.push(spark);
    }
}

pub(super) fn ensure_weapon_impact_explosion_pool_system(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut quad_mesh: ResMut<'_, RuntimeSharedQuadMesh>,
    mut effect_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut pool: ResMut<'_, WeaponImpactExplosionPool>,
) {
    if !pool.explosions.is_empty() {
        return;
    }
    pool.explosions.reserve(WEAPON_IMPACT_EXPLOSION_POOL_SIZE);
    let mesh = shared_unit_quad_handle(&mut quad_mesh, &mut meshes);
    for _ in 0..WEAPON_IMPACT_EXPLOSION_POOL_SIZE {
        let explosion = commands
            .spawn((
                WeaponImpactExplosion {
                    ttl_s: 0.0,
                    max_ttl_s: WEAPON_IMPACT_EXPLOSION_TTL_S,
                    base_scale: 1.2,
                    growth_scale: 4.4,
                    intensity_scale: 1.0,
                    domain_scale: 1.12,
                    screen_distortion_scale: 0.0,
                },
                Mesh2d(mesh.clone()),
                MeshMaterial2d(effect_materials.add(RuntimeEffectMaterial {
                    params: RuntimeEffectUniforms::explosion_burst(
                        0.0,
                        1.0,
                        1.0,
                        0.92,
                        0.35,
                        1.12,
                        Vec4::new(1.0, 0.92, 0.68, 1.0),
                        Vec4::new(1.0, 0.54, 0.16, 1.0),
                        Vec4::new(0.24, 0.14, 0.08, 1.0),
                    ),
                    ..RuntimeEffectMaterial::default()
                })),
                Transform::from_xyz(0.0, 0.0, 0.43),
                Visibility::Hidden,
                WorldEntity,
                DespawnOnExit(ClientAppState::InWorld),
            ))
            .id();
        pool.explosions.push(explosion);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn bootstrap_local_ballistic_projectile_visual_roots_system(
    mut commands: Commands<'_, '_>,
    projectiles: Query<
        '_,
        '_,
        (Entity, &'_ Position, &'_ avian2d::prelude::Rotation),
        (
            With<BallisticProjectile>,
            Without<WorldEntity>,
            Without<Transform>,
        ),
    >,
) {
    for (entity, position, rotation) in &projectiles {
        let mut transform = Transform::default();
        sync_planar_projectile_transform(&mut transform, position.0, rotation.as_radians());
        let global_transform = GlobalTransform::from(transform);
        commands
            .entity(entity)
            .insert((transform, global_transform, Visibility::Visible));
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn sync_unadopted_ballistic_projectile_visual_roots_system(
    mut projectiles: Query<
        '_,
        '_,
        (
            &'_ Position,
            &'_ avian2d::prelude::Rotation,
            &'_ mut Transform,
        ),
        (With<BallisticProjectile>, Without<WorldEntity>),
    >,
) {
    for (position, rotation, mut transform) in &mut projectiles {
        sync_planar_projectile_transform(&mut transform, position.0, rotation.as_radians());
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn attach_ballistic_projectile_visuals_system(
    mut commands: Commands<'_, '_>,
    projectiles: Query<
        '_,
        '_,
        (
            Entity,
            Has<SuppressedPredictedDuplicateVisual>,
            Option<&'_ Transform>,
        ),
        (
            With<BallisticProjectile>,
            Without<BallisticProjectileVisualAttached>,
        ),
    >,
) {
    for (entity, is_suppressed, existing_transform) in &projectiles {
        let mut transform = existing_transform.copied().unwrap_or_default();
        transform.translation.z = PROJECTILE_VISUAL_Z;
        commands.entity(entity).insert((
            BallisticProjectileVisualAttached,
            Sprite {
                color: Color::srgb(1.0, 0.84, 0.3),
                custom_size: Some(Vec2::new(
                    PROJECTILE_VISUAL_WIDTH_M,
                    PROJECTILE_VISUAL_LENGTH_M,
                )),
                ..default()
            },
            transform,
            if is_suppressed {
                Visibility::Hidden
            } else {
                Visibility::Visible
            },
        ));
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_weapon_tracer_visuals_system(
    time: Res<'_, Time>,
    spatial_query: SpatialQuery<'_, '_>,
    connected_clients: Query<
        '_,
        '_,
        (),
        (
            With<lightyear::prelude::client::Client>,
            With<lightyear::prelude::client::Connected>,
        ),
    >,
    mut pool: ResMut<'_, WeaponTracerPool>,
    mut cooldowns: ResMut<'_, WeaponTracerCooldowns>,
    controlled_roots: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            &'_ avian2d::prelude::Position,
            &'_ avian2d::prelude::Rotation,
            Option<&'_ avian2d::prelude::LinearVelocity>,
            Option<&'_ avian2d::prelude::AngularVelocity>,
            Option<&'_ ActionState<PlayerInput>>,
        ),
        (With<ControlledEntity>, With<WorldEntity>),
    >,
    hardpoints: Query<'_, '_, (&'_ ParentGuid, &'_ Hardpoint)>,
    weapons: Query<
        '_,
        '_,
        (
            Entity,
            &'_ MountedOn,
            &'_ BallisticWeapon,
            Option<&'_ AmmoCount>,
        ),
        With<WorldEntity>,
    >,
    mut bolts: Query<
        '_,
        '_,
        (
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
            &'_ mut WeaponTracerBolt,
        ),
    >,
) {
    // Use authoritative server tracer messages for all online clients so
    // impact-stop behavior is identical across shooter/observers.
    if connected_clients.iter().next().is_some() {
        return;
    }

    if pool.bolts.is_empty() {
        return;
    }
    let dt_s = time.delta_secs();
    let mut hardpoint_by_mount = HashMap::<(uuid::Uuid, String), (Vec2, Quat)>::new();
    for (parent_guid, hardpoint) in &hardpoints {
        hardpoint_by_mount.insert(
            (parent_guid.0, hardpoint.hardpoint_id.clone()),
            (hardpoint.offset_m.truncate(), hardpoint.local_rotation),
        );
    }

    for cooldown in cooldowns.by_weapon_entity.values_mut() {
        *cooldown = (*cooldown - dt_s).max(0.0);
    }

    for (
        ship_entity,
        ship_guid,
        ship_position,
        ship_rotation,
        _linear_velocity,
        angular_velocity,
        action_state,
    ) in &controlled_roots
    {
        let firing =
            action_state.is_some_and(|state| state.0.actions.contains(&EntityAction::FirePrimary));
        if !firing {
            continue;
        }
        let ship_quat: Quat = (*ship_rotation).into();

        for (weapon_entity, mounted_on, weapon, ammo) in &weapons {
            if mounted_on.parent_entity_id != ship_guid.0 {
                continue;
            }
            if weapon.uses_projectile_entities() {
                continue;
            }
            if ammo.is_some_and(|value| value.current == 0) {
                continue;
            }
            let cooldown = cooldowns
                .by_weapon_entity
                .entry(weapon_entity)
                .or_insert(0.0);
            if *cooldown > 0.0 {
                continue;
            }

            let Some((hardpoint_offset, hardpoint_rotation)) = hardpoint_by_mount
                .get(&(mounted_on.parent_entity_id, mounted_on.hardpoint_id.clone()))
            else {
                continue;
            };
            let muzzle_quat = ship_quat * *hardpoint_rotation;
            let direction = (muzzle_quat * Vec3::Y).truncate();
            if direction.length_squared() <= f32::EPSILON {
                continue;
            }
            let direction = direction.normalize();
            let muzzle_offset_world = rotate_vec2(ship_quat, *hardpoint_offset);
            let origin = ship_position.0 + muzzle_offset_world;
            let omega = angular_velocity.map(|v| v.0).unwrap_or(0.0);
            let lateral_normal = Vec2::new(-direction.y, direction.x);
            let spin_wiggle_amp_mps =
                (omega.abs() * 18.0).clamp(0.0, WEAPON_TRACER_WIGGLE_MAX_AMP_MPS);
            let initial_velocity = direction * WEAPON_TRACER_SPEED_MPS;
            let impact_xy = Dir2::new(direction).ok().and_then(|ray_direction| {
                let filter = SpatialQueryFilter::from_excluded_entities([ship_entity]);
                spatial_query
                    .cast_ray(
                        origin,
                        ray_direction,
                        weapon.max_range_m.max(1.0),
                        true,
                        &filter,
                    )
                    .map(|hit| origin + ray_direction.as_vec2() * hit.distance)
            });

            let bolt_entity = pool.bolts[pool.next_index % pool.bolts.len()];
            pool.next_index = (pool.next_index + 1) % pool.bolts.len();
            if let Ok((mut transform, _material_handle, mut visibility, mut bolt)) =
                bolts.get_mut(bolt_entity)
            {
                transform.translation = Vec3::new(origin.x, origin.y, 0.35);
                transform.rotation = Quat::from_rotation_z(
                    initial_velocity.to_angle() + WEAPON_TRACER_ROTATION_OFFSET_RAD,
                );
                bolt.excluded_entity = Some(ship_entity);
                bolt.velocity = initial_velocity;
                bolt.impact_xy = impact_xy;
                let range_ttl_s = (weapon.max_range_m.max(1.0) / WEAPON_TRACER_SPEED_MPS)
                    .clamp(WEAPON_TRACER_MIN_TTL_S, WEAPON_TRACER_LIFETIME_S);
                bolt.ttl_s = range_ttl_s;
                bolt.lateral_normal = lateral_normal;
                bolt.wiggle_phase_rad = 0.0;
                bolt.wiggle_freq_hz = WEAPON_TRACER_WIGGLE_BASE_FREQ_HZ + omega.abs() * 2.0;
                bolt.wiggle_amp_mps = spin_wiggle_amp_mps;
                *visibility = Visibility::Visible;
            }
            *cooldown = weapon.cooldown_seconds();
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn receive_remote_weapon_tracer_messages_system(
    mut pool: ResMut<'_, WeaponTracerPool>,
    mut events: MessageReader<'_, '_, RemoteWeaponFiredRuntimeMessage>,
    controlled_roots: Query<'_, '_, &'_ EntityGuid, (With<ControlledEntity>, With<WorldEntity>)>,
    world_entity_guids: Query<'_, '_, (Entity, &'_ EntityGuid), With<WorldEntity>>,
    mut bolts: Query<
        '_,
        '_,
        (
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
            &'_ mut WeaponTracerBolt,
        ),
    >,
) {
    if pool.bolts.is_empty() {
        return;
    }
    let _local_controlled_guids: std::collections::HashSet<uuid::Uuid> =
        controlled_roots.iter().map(|guid| guid.0).collect();
    let shooter_entity_by_guid: HashMap<uuid::Uuid, Entity> = world_entity_guids
        .iter()
        .map(|(entity, guid)| (guid.0, entity))
        .collect();

    for event in events.read() {
        let message = &event.message;
        let Some(shooter_runtime_id) =
            sidereal_net::RuntimeEntityId::parse(message.shooter_entity_id.as_str())
        else {
            continue;
        };
        // Accept authoritative tracer messages even for locally controlled shooters.
        // This guarantees impact stop/impact VFX parity with server-side hitscan.

        let bolt_entity = pool.bolts[pool.next_index % pool.bolts.len()];
        pool.next_index = (pool.next_index + 1) % pool.bolts.len();
        if let Ok((mut transform, _material_handle, mut visibility, mut bolt)) =
            bolts.get_mut(bolt_entity)
        {
            let origin = Vec2::new(message.origin_xy[0], message.origin_xy[1]);
            let velocity = Vec2::new(message.velocity_xy[0], message.velocity_xy[1]);
            transform.translation = Vec3::new(origin.x, origin.y, 0.35);
            if velocity.length_squared() > f32::EPSILON {
                transform.rotation =
                    Quat::from_rotation_z(velocity.to_angle() + WEAPON_TRACER_ROTATION_OFFSET_RAD);
            }
            bolt.excluded_entity = shooter_entity_by_guid
                .get(&shooter_runtime_id.as_uuid())
                .copied();
            bolt.velocity = velocity;
            bolt.impact_xy = message
                .impact_xy
                .map(|impact_xy| Vec2::new(impact_xy[0], impact_xy[1]));
            bolt.ttl_s = message.ttl_s.max(0.01);
            let speed = velocity.length();
            let normal = if speed > f32::EPSILON {
                let direction = velocity / speed;
                Vec2::new(-direction.y, direction.x)
            } else {
                Vec2::ZERO
            };
            bolt.lateral_normal = normal;
            bolt.wiggle_phase_rad = 0.0;
            bolt.wiggle_freq_hz = WEAPON_TRACER_WIGGLE_BASE_FREQ_HZ;
            bolt.wiggle_amp_mps = 0.0;
            *visibility = Visibility::Visible;
        }
    }
}

pub(super) fn receive_remote_destruction_effect_messages_system(
    mut pool: ResMut<'_, WeaponImpactExplosionPool>,
    mut events: MessageReader<'_, '_, RemoteEntityDestructionRuntimeMessage>,
    mut effect_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut explosions: Query<
        '_,
        '_,
        (
            &'_ mut WeaponImpactExplosion,
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
        ),
        WeaponImpactExplosionQueryFilter,
    >,
) {
    if pool.explosions.is_empty() {
        return;
    }
    for event in events.read() {
        let message = &event.message;
        activate_destruction_effect(
            message.destruction_profile_id.as_str(),
            Vec2::new(message.origin_xy[0], message.origin_xy[1]),
            &mut pool,
            &mut explosions,
            &mut effect_materials,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update_weapon_tracer_visuals_system(
    time: Res<'_, Time>,
    spatial_query: SpatialQuery<'_, '_>,
    mut spark_pool: ResMut<'_, WeaponImpactSparkPool>,
    mut explosion_pool: ResMut<'_, WeaponImpactExplosionPool>,
    mut effect_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut bolts: Query<'_, '_, WeaponTracerBoltQueryItem<'_>, WeaponTracerBoltQueryFilter>,
    mut sparks: Query<
        '_,
        '_,
        (
            &'_ mut WeaponImpactSpark,
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
        ),
        WeaponImpactSparkQueryFilter,
    >,
    mut explosions: Query<
        '_,
        '_,
        (
            &'_ mut WeaponImpactExplosion,
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
        ),
        WeaponImpactExplosionQueryFilter,
    >,
) {
    let dt_s = time.delta_secs();
    for (mut transform, material_handle, mut visibility, mut bolt) in &mut bolts {
        if bolt.ttl_s <= 0.0 {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        }
        bolt.ttl_s = (bolt.ttl_s - dt_s).max(0.0);
        bolt.wiggle_phase_rad += TAU * bolt.wiggle_freq_hz * dt_s;
        let lateral_speed_mps = bolt.wiggle_phase_rad.sin() * bolt.wiggle_amp_mps;
        let frame_velocity = bolt.velocity + bolt.lateral_normal * lateral_speed_mps;
        let frame_step = frame_velocity * dt_s;
        let frame_distance = frame_step.length();
        let current_pos = transform.translation.truncate();
        let mut hit_this_frame = false;
        if let Some(impact_pos) = bolt.impact_xy {
            let to_impact = impact_pos - current_pos;
            let impact_distance = to_impact.length();
            if impact_distance <= frame_distance.max(0.001) {
                transform.translation.x = impact_pos.x;
                transform.translation.y = impact_pos.y;
                transform.translation.z = 0.35;
                bolt.ttl_s = bolt.ttl_s.min(0.03);
                bolt.velocity = Vec2::ZERO;
                bolt.wiggle_amp_mps = 0.0;
                bolt.impact_xy = None;
                *visibility = Visibility::Visible;
                hit_this_frame = true;
                activate_weapon_impact_spark(
                    impact_pos,
                    &mut spark_pool,
                    &mut sparks,
                    &mut effect_materials,
                );
                activate_weapon_impact_explosion(
                    impact_pos,
                    &mut explosion_pool,
                    &mut explosions,
                    &mut effect_materials,
                );
            }
        }
        if hit_this_frame {
            continue;
        }
        if frame_distance > f32::EPSILON
            && let Ok(ray_direction) = Dir2::new(frame_step / frame_distance)
        {
            let filter = if let Some(excluded) = bolt.excluded_entity {
                SpatialQueryFilter::from_excluded_entities([excluded])
            } else {
                SpatialQueryFilter::default()
            };
            if let Some(hit) =
                spatial_query.cast_ray(current_pos, ray_direction, frame_distance, true, &filter)
            {
                let impact_pos = current_pos + ray_direction.as_vec2() * hit.distance;
                transform.translation.x = impact_pos.x;
                transform.translation.y = impact_pos.y;
                transform.translation.z = 0.35;
                bolt.ttl_s = bolt.ttl_s.min(0.03);
                bolt.velocity = Vec2::ZERO;
                bolt.wiggle_amp_mps = 0.0;
                bolt.impact_xy = None;
                *visibility = Visibility::Visible;
                hit_this_frame = true;
                activate_weapon_impact_spark(
                    impact_pos,
                    &mut spark_pool,
                    &mut sparks,
                    &mut effect_materials,
                );
                activate_weapon_impact_explosion(
                    impact_pos,
                    &mut explosion_pool,
                    &mut explosions,
                    &mut effect_materials,
                );
            }
        }
        if hit_this_frame {
            continue;
        }
        transform.translation.x += frame_step.x;
        transform.translation.y += frame_step.y;
        if frame_velocity.length_squared() > f32::EPSILON {
            transform.rotation = Quat::from_rotation_z(
                frame_velocity.to_angle() + WEAPON_TRACER_ROTATION_OFFSET_RAD,
            );
        }
        transform.translation.z = 0.35;
        let alpha = (bolt.ttl_s / WEAPON_TRACER_LIFETIME_S).clamp(0.0, 1.0);
        if let Some(material) = effect_materials.get_mut(&material_handle.0) {
            material.params = RuntimeEffectUniforms::beam_trail(
                1.0 - alpha,
                alpha * 0.95,
                0.65,
                0.35,
                (bolt.wiggle_amp_mps / WEAPON_TRACER_WIGGLE_MAX_AMP_MPS).clamp(0.0, 1.0) * 0.2,
                Vec4::new(1.0, 0.96, 0.7, 1.0),
                Vec4::new(1.0, 0.72, 0.22, 1.0),
            );
        }
        *visibility = if alpha > 0.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub(super) fn update_weapon_impact_sparks_system(
    time: Res<'_, Time>,
    mut spark_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut sparks: Query<'_, '_, WeaponImpactSparkQueryItem<'_>, Without<WeaponTracerBolt>>,
) {
    let dt_s = time.delta_secs();
    for (_entity, mut spark, mut transform, material_handle, mut visibility) in &mut sparks {
        spark.ttl_s = (spark.ttl_s - dt_s).max(0.0);
        if spark.ttl_s <= 0.0 {
            *visibility = Visibility::Hidden;
            continue;
        }
        let t = (spark.ttl_s / spark.max_ttl_s).clamp(0.0, 1.0);
        let age_norm = 1.0 - t;
        if let Some(material) = spark_materials.get_mut(&material_handle.0) {
            material.params = RuntimeEffectUniforms::impact_spark(
                age_norm,
                1.0,
                1.0,
                t * 0.95,
                Vec4::new(1.0, 0.9, 0.55, 1.0),
            );
        }
        let scale = 1.0 + age_norm * 7.0;
        transform.scale = Vec3::splat(scale);
        *visibility = Visibility::Visible;
    }
}

pub(super) fn update_weapon_impact_explosions_system(
    time: Res<'_, Time>,
    mut explosion_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut explosions: Query<'_, '_, WeaponImpactExplosionQueryItem<'_>, Without<WeaponTracerBolt>>,
) {
    let dt_s = time.delta_secs();
    for (_entity, mut explosion, mut transform, material_handle, mut visibility) in &mut explosions
    {
        explosion.ttl_s = (explosion.ttl_s - dt_s).max(0.0);
        if explosion.ttl_s <= 0.0 {
            *visibility = Visibility::Hidden;
            continue;
        }
        let t = (explosion.ttl_s / explosion.max_ttl_s).clamp(0.0, 1.0);
        let age_norm = 1.0 - t;
        transform.scale = Vec3::splat(explosion.base_scale + age_norm * explosion.growth_scale);
        if let Some(material) = explosion_materials.get_mut(&material_handle.0) {
            material.params = RuntimeEffectUniforms::explosion_burst(
                age_norm,
                explosion.intensity_scale + (1.0 - age_norm) * 0.35,
                1.0 + age_norm * 0.5,
                t * 0.95,
                0.35 + age_norm * 0.2,
                explosion.domain_scale,
                Vec4::new(1.0, 0.94, 0.72, 1.0),
                Vec4::new(1.0, 0.5, 0.15, 1.0),
                Vec4::new(0.24, 0.14, 0.08, 1.0),
            );
        }
        *visibility = Visibility::Visible;
    }
}

fn rotate_vec2(rotation: Quat, input: Vec2) -> Vec2 {
    (rotation * input.extend(0.0)).truncate()
}
