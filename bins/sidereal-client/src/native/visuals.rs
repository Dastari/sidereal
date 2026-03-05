//! Fullscreen/backdrop and streamed visual lifecycle systems.

use avian2d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::camera::visibility::RenderLayers;
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;
use bevy::state::state_scoped::DespawnOnExit;
use lightyear::interpolation::interpolation_history::ConfirmedHistory;
use lightyear::prelude::MessageReceiver;
use lightyear::prelude::input::native::ActionState;
use sidereal_game::{
    AfterburnerCapability, AfterburnerState, AmmoCount, BallisticWeapon, ControlledEntityGuid,
    Engine, EntityAction, EntityGuid, FlightComputer, FullscreenLayer, Hardpoint, MountedOn,
    ParentGuid, PlayerTag, SPACE_BACKGROUND_LAYER_KIND, STARFIELD_LAYER_KIND, SizeM,
    SpaceBackgroundFullscreenLayerBundle, SpaceBackgroundShaderSettings,
    StarfieldFullscreenLayerBundle, StarfieldShaderSettings, ThrusterPlumeShaderSettings,
};
use sidereal_net::PlayerInput;
use sidereal_net::ServerWeaponFiredMessage;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, TAU};

use super::app_state::ClientAppState;
use super::assets;
use super::assets::LocalAssetManager;
use super::backdrop::{
    SpaceBackgroundMaterial, StarfieldMaterial, StreamedSpriteShaderMaterial, ThrusterPlumeMaterial,
};
use super::components::{
    ControlledEntity, DebugBlueBackdrop, FallbackFullscreenLayer, FullscreenLayerRenderable,
    SpaceBackdropFallback, SpaceBackgroundBackdrop, StarfieldBackdrop, StreamedSpriteShaderAssetId,
    StreamedVisualAssetId, StreamedVisualAttached, StreamedVisualChild,
    SuppressedPredictedDuplicateVisual, ThrusterPlumeAttached, ThrusterPlumeChild,
    WeaponImpactSpark, WeaponTracerBolt, WeaponTracerCooldowns, WeaponTracerPool, WorldEntity,
};
use super::platform::{self, BACKDROP_RENDER_LAYER, STREAMED_SPRITE_PIXEL_SHADER_PATH};
use super::resources::{AssetRootPath, BootstrapWatchdogState};
use super::shaders;

const WEAPON_TRACER_POOL_SIZE: usize = 96;
const WEAPON_TRACER_SPEED_MPS: f32 = 1800.0;
const WEAPON_TRACER_LIFETIME_S: f32 = 0.2;
const WEAPON_TRACER_WIDTH_M: f32 = 0.35;
const WEAPON_TRACER_LENGTH_M: f32 = 9.0;
const WEAPON_TRACER_ROTATION_OFFSET_RAD: f32 = -FRAC_PI_2;
const WEAPON_TRACER_WIGGLE_BASE_FREQ_HZ: f32 = 18.0;
const WEAPON_TRACER_WIGGLE_MAX_AMP_MPS: f32 = 120.0;
const WEAPON_IMPACT_SPARK_TTL_S: f32 = 0.12;
const WEAPON_TRACER_MIN_TTL_S: f32 = 0.01;

#[allow(clippy::type_complexity)]
pub(super) fn ensure_fullscreen_layer_fallback_system(
    mut commands: Commands<'_, '_>,
    layers: Query<
        '_,
        '_,
        (
            Entity,
            Option<&FallbackFullscreenLayer>,
            Option<&FullscreenLayerRenderable>,
        ),
        With<FullscreenLayer>,
    >,
    asset_manager: Res<'_, LocalAssetManager>,
    watchdog: Res<'_, BootstrapWatchdogState>,
) {
    let mut fallback_entities = Vec::new();
    let mut has_authoritative_renderable_layer = false;
    for (entity, fallback_marker, renderable) in &layers {
        if fallback_marker.is_some() {
            fallback_entities.push(entity);
        } else if renderable.is_some() {
            has_authoritative_renderable_layer = true;
        }
    }
    if has_authoritative_renderable_layer {
        for entity in fallback_entities {
            if let Ok(mut entity_commands) = commands.get_entity(entity) {
                entity_commands.try_despawn();
            }
        }
        return;
    }
    if !layers.is_empty()
        || (!asset_manager.bootstrap_complete() && !watchdog.replication_state_seen)
    {
        return;
    }
    commands.spawn((
        SpaceBackgroundFullscreenLayerBundle::default(),
        FallbackFullscreenLayer,
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));
    commands.spawn((
        StarfieldFullscreenLayerBundle::default(),
        FallbackFullscreenLayer,
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));
    info!("client spawned fallback fullscreen layers (authoritative layers missing)");
}

pub(super) fn sync_fullscreen_layer_renderables_system(
    mut commands: Commands<'_, '_>,
    layers: Query<'_, '_, (Entity, &FullscreenLayer, Option<&FullscreenLayerRenderable>)>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut starfield_materials: ResMut<'_, Assets<StarfieldMaterial>>,
    mut space_background_materials: ResMut<'_, Assets<SpaceBackgroundMaterial>>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
) {
    for (entity, layer, rendered) in &layers {
        let Ok(mut entity_commands) = commands.get_entity(entity) else {
            continue;
        };
        let has_streamed_shader = shaders::fullscreen_layer_shader_ready(
            &asset_root.0,
            &asset_manager,
            &layer.shader_asset_id,
        );
        let is_supported_kind = layer.layer_kind == STARFIELD_LAYER_KIND
            || layer.layer_kind == SPACE_BACKGROUND_LAYER_KIND;
        let needs_rebuild = rendered.is_none_or(|existing| {
            existing.layer_kind != layer.layer_kind || existing.layer_order != layer.layer_order
        });

        if !is_supported_kind || !has_streamed_shader {
            if !is_supported_kind {
                warn!(
                    "unsupported fullscreen layer kind={} shader_asset_id={}",
                    layer.layer_kind, layer.shader_asset_id
                );
            } else {
                warn!(
                    "fullscreen layer waiting for shader readiness layer_kind={} shader_asset_id={}",
                    layer.layer_kind, layer.shader_asset_id
                );
            }
            if rendered.is_some() {
                entity_commands
                    .remove::<FullscreenLayerRenderable>()
                    .remove::<StarfieldBackdrop>()
                    .remove::<SpaceBackgroundBackdrop>()
                    .remove::<Mesh2d>()
                    .remove::<MeshMaterial2d<StarfieldMaterial>>()
                    .remove::<MeshMaterial2d<SpaceBackgroundMaterial>>();
            }
            continue;
        }

        if needs_rebuild {
            let mesh = meshes.add(Rectangle::new(1.0, 1.0));
            entity_commands
                .try_insert((
                    Mesh2d(mesh),
                    Transform::from_xyz(0.0, 0.0, layer.layer_order as f32),
                    RenderLayers::layer(BACKDROP_RENDER_LAYER),
                    FullscreenLayerRenderable {
                        layer_kind: layer.layer_kind.clone(),
                        layer_order: layer.layer_order,
                    },
                ))
                .remove::<FallbackFullscreenLayer>()
                .remove::<StarfieldBackdrop>()
                .remove::<SpaceBackgroundBackdrop>()
                .remove::<MeshMaterial2d<StarfieldMaterial>>()
                .remove::<MeshMaterial2d<SpaceBackgroundMaterial>>();

            if layer.layer_kind == STARFIELD_LAYER_KIND {
                let material = starfield_materials.add(StarfieldMaterial::default());
                entity_commands.try_insert((
                    StarfieldBackdrop,
                    MeshMaterial2d(material),
                    StarfieldShaderSettings::default(),
                ));
            } else {
                let material = space_background_materials.add(SpaceBackgroundMaterial::default());
                entity_commands.try_insert((
                    SpaceBackgroundBackdrop,
                    MeshMaterial2d(material),
                    SpaceBackgroundShaderSettings::default(),
                ));
            }
            info!(
                "fullscreen layer renderable ready layer_kind={} order={} shader_asset_id={}",
                layer.layer_kind, layer.layer_order, layer.shader_asset_id
            );
        } else {
            entity_commands.try_insert(Transform::from_xyz(0.0, 0.0, layer.layer_order as f32));
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn sync_backdrop_fullscreen_system(
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
    mut backdrop_query: Query<
        '_,
        '_,
        &mut Transform,
        (
            Or<(
                With<StarfieldBackdrop>,
                With<SpaceBackgroundBackdrop>,
                With<DebugBlueBackdrop>,
                With<SpaceBackdropFallback>,
            )>,
        ),
    >,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(viewport_size) = platform::safe_viewport_size(window) else {
        return;
    };
    let width = viewport_size.x;
    let height = viewport_size.y;
    for mut transform in &mut backdrop_query {
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
        transform.scale = Vec3::new(width, height, 1.0);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn suppress_duplicate_predicted_interpolated_visuals_system(
    mut commands: Commands<'_, '_>,
    world_entities: Query<
        '_,
        '_,
        (
            Entity,
            Option<&EntityGuid>,
            Has<ControlledEntityGuid>,
            Has<PlayerTag>,
            Has<ControlledEntity>,
            Has<lightyear::prelude::Interpolated>,
            Has<lightyear::prelude::Predicted>,
            Option<&'_ ConfirmedHistory<avian2d::prelude::Position>>,
            Option<&'_ ConfirmedHistory<avian2d::prelude::Rotation>>,
            Has<SuppressedPredictedDuplicateVisual>,
        ),
        With<WorldEntity>,
    >,
) {
    let mut best_entity_by_guid = HashMap::<uuid::Uuid, (Entity, i32, bool)>::new();
    for (
        entity,
        guid,
        has_controlled_entity_guid,
        has_player_tag,
        is_controlled,
        is_interpolated,
        is_predicted,
        position_history,
        rotation_history,
        is_suppressed,
    ) in &world_entities
    {
        let Some(guid) = guid else { continue };
        let interpolated_ready = !is_interpolated
            || (position_history.and_then(|h| h.end()).is_some()
                && rotation_history.and_then(|h| h.end()).is_some());
        let score = if has_controlled_entity_guid || has_player_tag {
            -100
        } else if is_controlled {
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
        match best_entity_by_guid.get_mut(&guid.0) {
            Some((winner, winner_score, winner_is_suppressed)) => {
                let winner_entity = *winner;
                let should_replace = score > *winner_score
                    || (score == *winner_score
                        && is_suppressed != *winner_is_suppressed
                        && *winner_is_suppressed)
                    || (score == *winner_score
                        && is_suppressed == *winner_is_suppressed
                        && entity.to_bits() < winner_entity.to_bits());
                if should_replace {
                    *winner = entity;
                    *winner_score = score;
                    *winner_is_suppressed = is_suppressed;
                }
            }
            None => {
                best_entity_by_guid.insert(guid.0, (entity, score, is_suppressed));
            }
        }
    }

    for (
        entity,
        guid,
        has_controlled_entity_guid,
        has_player_tag,
        _is_controlled,
        _is_interpolated,
        _is_predicted,
        _position_history,
        _rotation_history,
        is_suppressed,
    ) in &world_entities
    {
        let should_suppress = if has_controlled_entity_guid || has_player_tag {
            true
        } else {
            guid.and_then(|guid| best_entity_by_guid.get(&guid.0).copied())
                .is_some_and(|(winner, _, _)| winner != entity)
        };
        if should_suppress {
            if let Ok(mut entity_commands) = commands.get_entity(entity) {
                if !is_suppressed {
                    entity_commands.insert(SuppressedPredictedDuplicateVisual);
                }
                entity_commands.insert(Visibility::Hidden);
            }
        } else if is_suppressed && let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands
                .remove::<SuppressedPredictedDuplicateVisual>()
                .insert(Visibility::Visible);
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn cleanup_streamed_visual_children_system(
    mut commands: Commands<'_, '_>,
    parents: Query<
        '_,
        '_,
        (
            Entity,
            &'_ Children,
            Option<&'_ StreamedVisualAssetId>,
            Has<StreamedVisualAttached>,
            Has<SuppressedPredictedDuplicateVisual>,
            Option<&'_ PlayerTag>,
            Has<ControlledEntityGuid>,
        ),
        With<WorldEntity>,
    >,
    visual_children: Query<'_, '_, (), With<StreamedVisualChild>>,
) {
    for (
        parent_entity,
        children,
        visual_asset_id,
        has_visual_attached,
        is_suppressed,
        player_tag,
        has_controlled_entity_guid,
    ) in &parents
    {
        let should_clear_visual = visual_asset_id.is_none()
            || is_suppressed
            || player_tag.is_some()
            || has_controlled_entity_guid;
        if !should_clear_visual {
            continue;
        }
        let mut removed_any_child = false;
        for child in children.iter() {
            if visual_children.get(child).is_ok() {
                if let Ok(mut entity_commands) = commands.get_entity(child) {
                    entity_commands.try_despawn();
                }
                removed_any_child = true;
            }
        }
        if (has_visual_attached || removed_any_child)
            && let Ok(mut parent_commands) = commands.get_entity(parent_entity)
        {
            parent_commands.remove::<StreamedVisualAttached>();
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn attach_streamed_visual_assets_system(
    mut commands: Commands<'_, '_>,
    asset_server: Res<'_, AssetServer>,
    images: Res<'_, Assets<Image>>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut sprite_shader_materials: ResMut<'_, Assets<StreamedSpriteShaderMaterial>>,
    candidates: Query<
        '_,
        '_,
        (
            Entity,
            &StreamedVisualAssetId,
            Option<&SizeM>,
            Option<&StreamedSpriteShaderAssetId>,
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
    for (entity, asset_id, size_m, sprite_shader) in &candidates {
        let Some(path) = assets::streamed_visual_asset_path(&asset_id.0, &asset_manager) else {
            continue;
        };
        let Ok(mut entity_commands) = commands.get_entity(entity) else {
            continue;
        };
        entity_commands.try_insert((
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
        ));
        let image_handle = asset_server.load(path.clone());
        let rooted_path = std::path::PathBuf::from(&asset_root.0).join(&path);
        let texture_size_px = images
            .get(&image_handle)
            .map(|image| image.size())
            .or_else(|| assets::read_png_dimensions(&rooted_path));
        let custom_size = assets::resolved_world_sprite_size(texture_size_px, size_m);
        if let Some(sprite_shader) = sprite_shader
            && let Some(shader_path) =
                assets::streamed_sprite_shader_path(&sprite_shader.0, &asset_manager)
        {
            if shader_path != STREAMED_SPRITE_PIXEL_SHADER_PATH {
                warn!(
                    "unsupported streamed sprite shader path={} (expected {}); falling back to plain sprite",
                    shader_path, STREAMED_SPRITE_PIXEL_SHADER_PATH
                );
            } else if std::path::PathBuf::from(&asset_root.0)
                .join(STREAMED_SPRITE_PIXEL_SHADER_PATH)
                .is_file()
            {
                let quad_mesh = meshes.add(Rectangle::new(1.0, 1.0));
                let material = sprite_shader_materials.add(StreamedSpriteShaderMaterial {
                    image: image_handle.clone(),
                });
                let sprite_size = custom_size.unwrap_or(Vec2::splat(16.0));
                entity_commands.with_children(|child| {
                    child.spawn((
                        StreamedVisualChild,
                        Mesh2d(quad_mesh),
                        MeshMaterial2d(material),
                        Transform::from_xyz(0.0, 0.0, 0.2).with_scale(Vec3::new(
                            sprite_size.x,
                            sprite_size.y,
                            1.0,
                        )),
                    ));
                });
                entity_commands.try_insert(StreamedVisualAttached);
                continue;
            }
        }
        entity_commands.with_children(|child| {
            child.spawn((
                StreamedVisualChild,
                Sprite {
                    image: image_handle,
                    custom_size,
                    ..Default::default()
                },
                Transform::from_xyz(0.0, 0.0, 0.2),
            ));
        });
        entity_commands.try_insert(StreamedVisualAttached);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn attach_thruster_plume_visuals_system(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut plume_materials: ResMut<'_, Assets<ThrusterPlumeMaterial>>,
    engines: Query<
        '_,
        '_,
        Entity,
        (
            With<WorldEntity>,
            With<Engine>,
            Without<ThrusterPlumeAttached>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
) {
    for entity in &engines {
        let Ok(mut entity_commands) = commands.get_entity(entity) else {
            continue;
        };
        let plume_mesh = meshes.add(Rectangle::new(1.0, 1.0));
        let plume_material = plume_materials.add(ThrusterPlumeMaterial::default());
        entity_commands.with_children(|child| {
            child.spawn((
                ThrusterPlumeChild,
                Mesh2d(plume_mesh),
                MeshMaterial2d(plume_material),
                Transform::from_xyz(0.0, -0.35, 0.1).with_scale(Vec3::new(1.2, 0.02, 1.0)),
                Visibility::Visible,
            ));
        });
        entity_commands.insert(ThrusterPlumeAttached);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn update_thruster_plume_visuals_system(
    time: Res<'_, Time>,
    mut plume_materials: ResMut<'_, Assets<ThrusterPlumeMaterial>>,
    mut plume_children: Query<
        '_,
        '_,
        (
            &'_ MeshMaterial2d<ThrusterPlumeMaterial>,
            &'_ mut Transform,
            &'_ mut Visibility,
        ),
        With<ThrusterPlumeChild>,
    >,
    engines: Query<
        '_,
        '_,
        (
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
            &'_ FlightComputer,
            Option<&'_ AfterburnerState>,
        ),
    >,
) {
    let mut hull_state = HashMap::<uuid::Uuid, (f32, bool)>::new();
    for (guid, computer, afterburner_state) in &hulls {
        let thrust_alpha = computer.throttle.max(0.0).clamp(0.0, 1.0);
        let afterburner_active = afterburner_state.is_some_and(|state| state.active);
        hull_state.insert(guid.0, (thrust_alpha, afterburner_active));
    }

    for (children, mounted_on, afterburner_capability, plume_settings) in &engines {
        let Some((thrust_alpha, afterburner_active)) =
            hull_state.get(&mounted_on.parent_entity_id).copied()
        else {
            continue;
        };
        let settings = plume_settings.cloned().unwrap_or_default();
        if !settings.enabled {
            for child in children.iter() {
                if let Ok((_, _, mut visibility)) = plume_children.get_mut(child) {
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
            let Ok((material_handle, mut transform, mut visibility)) =
                plume_children.get_mut(child)
            else {
                continue;
            };
            if let Some(material) = plume_materials.get_mut(&material_handle.0) {
                material.params.shape_params = Vec4::new(
                    settings.falloff.max(0.05),
                    settings.edge_softness.max(0.1),
                    settings.noise_strength.max(0.0),
                    thrust_alpha.clamp(0.0, 1.0),
                );
                material.params.state_params = Vec4::new(
                    afterburner_alpha,
                    time.elapsed_secs(),
                    plume_alpha,
                    settings.flicker_hz.max(0.0),
                );
                material.params.base_color = settings.base_color_rgb.extend(1.0);
                material.params.hot_color = settings.hot_color_rgb.extend(1.0);
                material.params.afterburner_color = settings.afterburner_color_rgb.extend(1.0);
            }
            transform.translation = Vec3::new(0.0, -(plume_length * 0.5 + 0.35), 0.1);
            transform.scale = Vec3::new(plume_width, plume_length, 1.0);
            *visibility = if plume_alpha > 0.001 {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

pub(super) fn ensure_weapon_tracer_pool_system(
    mut commands: Commands<'_, '_>,
    mut pool: ResMut<'_, WeaponTracerPool>,
) {
    if !pool.bolts.is_empty() {
        return;
    }
    pool.bolts.reserve(WEAPON_TRACER_POOL_SIZE);
    for _ in 0..WEAPON_TRACER_POOL_SIZE {
        let bolt = commands
            .spawn((
                WeaponTracerBolt {
                    excluded_entity: None,
                    velocity: Vec2::ZERO,
                    ttl_s: 0.0,
                    lateral_normal: Vec2::ZERO,
                    wiggle_phase_rad: 0.0,
                    wiggle_freq_hz: WEAPON_TRACER_WIGGLE_BASE_FREQ_HZ,
                    wiggle_amp_mps: 0.0,
                },
                Sprite {
                    color: Color::srgba(1.0, 0.95, 0.6, 0.0),
                    custom_size: Some(Vec2::new(WEAPON_TRACER_WIDTH_M, WEAPON_TRACER_LENGTH_M)),
                    ..Default::default()
                },
                Transform::from_xyz(0.0, 0.0, 0.35),
                Visibility::Hidden,
                WorldEntity,
                DespawnOnExit(ClientAppState::InWorld),
            ))
            .id();
        pool.bolts.push(bolt);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn emit_weapon_tracer_visuals_system(
    time: Res<'_, Time>,
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
            &'_ mut Sprite,
            &'_ mut Visibility,
            &'_ mut WeaponTracerBolt,
        ),
    >,
) {
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
        linear_velocity,
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
            let ship_linear_velocity = linear_velocity.map(|v| v.0).unwrap_or(Vec2::ZERO);
            let omega = angular_velocity.map(|v| v.0).unwrap_or(0.0);
            let tangential_velocity = Vec2::new(
                -omega * muzzle_offset_world.y,
                omega * muzzle_offset_world.x,
            );
            let lateral_normal = Vec2::new(-direction.y, direction.x);
            let spin_wiggle_amp_mps =
                (omega.abs() * 18.0).clamp(0.0, WEAPON_TRACER_WIGGLE_MAX_AMP_MPS);
            let initial_velocity =
                direction * WEAPON_TRACER_SPEED_MPS + ship_linear_velocity + tangential_velocity;

            let bolt_entity = pool.bolts[pool.next_index % pool.bolts.len()];
            pool.next_index = (pool.next_index + 1) % pool.bolts.len();
            if let Ok((mut transform, mut sprite, mut visibility, mut bolt)) =
                bolts.get_mut(bolt_entity)
            {
                transform.translation = Vec3::new(origin.x, origin.y, 0.35);
                transform.rotation = Quat::from_rotation_z(
                    initial_velocity.to_angle() + WEAPON_TRACER_ROTATION_OFFSET_RAD,
                );
                bolt.excluded_entity = Some(ship_entity);
                bolt.velocity = initial_velocity;
                let range_ttl_s = (weapon.max_range_m.max(1.0) / WEAPON_TRACER_SPEED_MPS)
                    .clamp(WEAPON_TRACER_MIN_TTL_S, WEAPON_TRACER_LIFETIME_S);
                bolt.ttl_s = range_ttl_s;
                bolt.lateral_normal = lateral_normal;
                bolt.wiggle_phase_rad = 0.0;
                bolt.wiggle_freq_hz = WEAPON_TRACER_WIGGLE_BASE_FREQ_HZ + omega.abs() * 2.0;
                bolt.wiggle_amp_mps = spin_wiggle_amp_mps;
                sprite.color = Color::srgba(1.0, 0.95, 0.6, 0.95);
                *visibility = Visibility::Visible;
            }
            *cooldown = weapon.cooldown_seconds();
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn receive_remote_weapon_tracer_messages_system(
    mut pool: ResMut<'_, WeaponTracerPool>,
    mut receivers: Query<
        '_,
        '_,
        &'_ mut MessageReceiver<ServerWeaponFiredMessage>,
        (
            With<lightyear::prelude::client::Client>,
            With<lightyear::prelude::client::Connected>,
        ),
    >,
    controlled_roots: Query<'_, '_, &'_ EntityGuid, (With<ControlledEntity>, With<WorldEntity>)>,
    world_entity_guids: Query<'_, '_, (Entity, &'_ EntityGuid), With<WorldEntity>>,
    mut bolts: Query<
        '_,
        '_,
        (
            &'_ mut Transform,
            &'_ mut Sprite,
            &'_ mut Visibility,
            &'_ mut WeaponTracerBolt,
        ),
    >,
) {
    if pool.bolts.is_empty() {
        return;
    }
    let local_controlled_guids: std::collections::HashSet<uuid::Uuid> =
        controlled_roots.iter().map(|guid| guid.0).collect();
    let shooter_entity_by_guid: HashMap<uuid::Uuid, Entity> = world_entity_guids
        .iter()
        .map(|(entity, guid)| (guid.0, entity))
        .collect();

    for mut receiver in &mut receivers {
        for message in receiver.receive() {
            let Some(shooter_runtime_id) =
                sidereal_net::RuntimeEntityId::parse(message.shooter_entity_id.as_str())
            else {
                continue;
            };
            if local_controlled_guids.contains(&shooter_runtime_id.as_uuid()) {
                // Local player already renders immediate predicted tracers.
                continue;
            }

            let bolt_entity = pool.bolts[pool.next_index % pool.bolts.len()];
            pool.next_index = (pool.next_index + 1) % pool.bolts.len();
            if let Ok((mut transform, mut sprite, mut visibility, mut bolt)) =
                bolts.get_mut(bolt_entity)
            {
                let origin = Vec2::new(message.origin_xy[0], message.origin_xy[1]);
                let velocity = Vec2::new(message.velocity_xy[0], message.velocity_xy[1]);
                transform.translation = Vec3::new(origin.x, origin.y, 0.35);
                if velocity.length_squared() > f32::EPSILON {
                    transform.rotation = Quat::from_rotation_z(
                        velocity.to_angle() + WEAPON_TRACER_ROTATION_OFFSET_RAD,
                    );
                }
                bolt.excluded_entity = shooter_entity_by_guid
                    .get(&shooter_runtime_id.as_uuid())
                    .copied();
                bolt.velocity = velocity;
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
                sprite.color = Color::srgba(1.0, 0.95, 0.6, 0.95);
                *visibility = Visibility::Visible;
            }
        }
    }
}

pub(super) fn update_weapon_tracer_visuals_system(
    mut commands: Commands<'_, '_>,
    time: Res<'_, Time>,
    spatial_query: SpatialQuery<'_, '_>,
    mut bolts: Query<
        '_,
        '_,
        (
            &'_ mut Transform,
            &'_ mut Sprite,
            &'_ mut Visibility,
            &'_ mut WeaponTracerBolt,
        ),
    >,
) {
    let dt_s = time.delta_secs();
    for (mut transform, mut sprite, mut visibility, mut bolt) in &mut bolts {
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
                bolt.ttl_s = 0.0;
                *visibility = Visibility::Hidden;
                hit_this_frame = true;
                commands.spawn((
                    WeaponImpactSpark {
                        ttl_s: WEAPON_IMPACT_SPARK_TTL_S,
                        max_ttl_s: WEAPON_IMPACT_SPARK_TTL_S,
                    },
                    Sprite {
                        color: Color::srgba(1.0, 0.9, 0.55, 0.95),
                        custom_size: Some(Vec2::splat(2.8)),
                        ..Default::default()
                    },
                    Transform::from_xyz(impact_pos.x, impact_pos.y, 0.45),
                    WorldEntity,
                    DespawnOnExit(ClientAppState::InWorld),
                ));
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
        sprite.color = Color::srgba(1.0, 0.95, 0.6, alpha * 0.95);
        *visibility = if alpha > 0.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub(super) fn update_weapon_impact_sparks_system(
    mut commands: Commands<'_, '_>,
    time: Res<'_, Time>,
    mut sparks: Query<
        '_,
        '_,
        (
            Entity,
            &'_ mut WeaponImpactSpark,
            &'_ mut Sprite,
            &'_ mut Transform,
        ),
    >,
) {
    let dt_s = time.delta_secs();
    for (entity, mut spark, mut sprite, mut transform) in &mut sparks {
        spark.ttl_s = (spark.ttl_s - dt_s).max(0.0);
        if spark.ttl_s <= 0.0 {
            if let Ok(mut entity_commands) = commands.get_entity(entity) {
                entity_commands.try_despawn();
            }
            continue;
        }
        let t = (spark.ttl_s / spark.max_ttl_s).clamp(0.0, 1.0);
        sprite.color = Color::srgba(1.0, 0.9, 0.55, t * 0.95);
        let scale = 0.55 + (1.0 - t) * 1.4;
        transform.scale = Vec3::splat(scale);
    }
}

fn rotate_vec2(rotation: Quat, input: Vec2) -> Vec2 {
    (rotation * input.extend(0.0)).truncate()
}
