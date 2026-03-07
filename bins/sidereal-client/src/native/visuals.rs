//! Fullscreen/backdrop and streamed visual lifecycle systems.

use avian2d::prelude::{Position, SpatialQuery, SpatialQueryFilter};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::state::state_scoped::DespawnOnExit;
use lightyear::interpolation::interpolation_history::ConfirmedHistory;
use lightyear::prelude::MessageReceiver;
use lightyear::prelude::input::native::ActionState;
use sidereal_game::{
    AfterburnerCapability, AfterburnerState, AmmoCount, BallisticWeapon, ControlledEntityGuid,
    Engine, EntityAction, EntityGuid, FlightComputer, Hardpoint, MountedOn, ParentGuid,
    PlanetBodyShaderSettings, PlayerTag, ProceduralSprite, RuntimeRenderLayerDefinition,
    RuntimeWorldVisualPassDefinition, RuntimeWorldVisualStack, SizeM, ThrusterPlumeShaderSettings,
    WorldPosition, generate_procedural_sprite_image_set, resolve_world_position,
};
use sidereal_net::PlayerInput;
use sidereal_net::ServerWeaponFiredMessage;
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
    ControlledEntity, PendingInitialVisualReady, PendingVisibilityFadeIn,
    ResolvedRuntimeRenderLayer, RuntimeWorldVisualFamily, RuntimeWorldVisualPass,
    RuntimeWorldVisualPassKind, RuntimeWorldVisualPassSet, StreamedSpriteShaderAssetId,
    StreamedVisualAssetId, StreamedVisualAttached, StreamedVisualChild,
    SuppressedPredictedDuplicateVisual, WeaponImpactSpark, WeaponTracerBolt, WeaponTracerCooldowns,
    WeaponTracerPool, WorldEntity,
};
use super::ecs_util::{queue_despawn_if_exists, queue_despawn_if_exists_force};
use super::lighting::{CameraLocalLightSet, WorldLightingState};
use super::platform::PLANET_BODY_RENDER_LAYER;
use super::resources::AssetRootPath;
use super::resources::CameraMotionState;
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
const ASTEROID_TEXTURE_ASSET_ID: &str = "asteroid_texture_red_png";
const PLANET_BODY_PARALLAX_FACTOR: f32 = 0.18;
const PLANET_CLOUD_BACK_LAYER_Z_OFFSET: f32 = -0.2;
const PLANET_CLOUD_FRONT_LAYER_Z_OFFSET: f32 = 0.5;
const PLANET_BODY_LAYER_Z_OFFSET: f32 = 0.0;
const PLANET_RING_BACK_LAYER_Z_OFFSET: f32 = -0.45;
const PLANET_RING_FRONT_LAYER_Z_OFFSET: f32 = 0.65;
const STREAMED_VISUAL_BASE_LAYER_Z: f32 = 0.2;

enum StreamedVisualMaterialKind {
    Plain,
    GenericShader,
    AsteroidShader,
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
            Has<PlanetBodyShaderSettings>,
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
        has_planet_shader,
        has_visual_attached,
        is_suppressed,
        player_tag,
        has_controlled_entity_guid,
    ) in &parents
    {
        let should_clear_visual = visual_asset_id.is_none()
            || has_planet_shader
            || is_suppressed
            || player_tag.is_some()
            || has_controlled_entity_guid;
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
            parent_commands.remove::<StreamedVisualAttached>();
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn attach_streamed_visual_assets_system(
    mut commands: Commands<'_, '_>,
    asset_server: Res<'_, AssetServer>,
    mut images: ResMut<'_, Assets<Image>>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut sprite_shader_materials: ResMut<'_, Assets<StreamedSpriteShaderMaterial>>,
    mut asteroid_shader_materials: ResMut<'_, Assets<AsteroidSpriteShaderMaterial>>,
    world_lighting: Res<'_, WorldLightingState>,
    camera_local_lights: Res<'_, CameraLocalLightSet>,
    mut asteroid_sprite_cache: Local<
        '_,
        HashMap<(uuid::Uuid, u64), (Handle<Image>, Handle<Image>)>,
    >,
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
        entity_commands.try_insert((
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
        ));

        let world_sprite_kind =
            sprite_shader.and_then(|shader| shaders::world_sprite_shader_kind(&shader.0));
        let is_asteroid_shader = matches!(
            world_sprite_kind,
            Some(shaders::RuntimeWorldSpriteShaderKind::Asteroid)
        );
        let generated_asteroid_image = if is_asteroid_shader
            && asset_id.0 == ASTEROID_TEXTURE_ASSET_ID
            && let Some(procedural_sprite) = procedural_sprite
            && procedural_sprite.generator_id == "asteroid_rocky_v1"
        {
            let guid = entity_guid
                .map(|guid| guid.0)
                .unwrap_or_else(uuid::Uuid::nil);
            let fingerprint = procedural_sprite_fingerprint(procedural_sprite);
            Some(
                asteroid_sprite_cache
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
        } else {
            let Some(path) = assets::streamed_visual_asset_path(&asset_id.0, &asset_manager) else {
                continue;
            };
            asset_server.load(path)
        };

        let texture_size_px = generated_asteroid_image
            .as_ref()
            .and_then(|handle| images.get(handle))
            .map(|image| image.size())
            .or_else(|| {
                let path = assets::streamed_visual_asset_path(&asset_id.0, &asset_manager)?;
                let rooted_path = std::path::PathBuf::from(&asset_root.0).join(path);
                images
                    .get(&image_handle)
                    .map(|image| image.size())
                    .or_else(|| assets::read_png_dimensions(&rooted_path))
            });
        let custom_size = assets::resolved_world_sprite_size(texture_size_px, size_m);

        let has_streamed_sprite_shader_path = sprite_shader.is_some_and(|shader| {
            shaders::world_sprite_shader_ready(&asset_root.0, &asset_manager, &shader.0)
        });
        let material_kind = resolve_streamed_visual_material_kind(
            use_shader_materials,
            world_sprite_kind,
            has_streamed_sprite_shader_path,
        );
        match material_kind {
            StreamedVisualMaterialKind::AsteroidShader => {
                let quad_mesh = meshes.add(Rectangle::new(1.0, 1.0));
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
                        Mesh2d(quad_mesh),
                        MeshMaterial2d(material),
                        Transform::from_xyz(x, y, z).with_scale(Vec3::new(
                            sprite_size.x,
                            sprite_size.y,
                            1.0,
                        )),
                    ));
                });
                entity_commands.try_insert(StreamedVisualAttached);
                continue;
            }
            StreamedVisualMaterialKind::GenericShader => {
                let quad_mesh = meshes.add(Rectangle::new(1.0, 1.0));
                let material = sprite_shader_materials.add(StreamedSpriteShaderMaterial {
                    image: image_handle.clone(),
                });
                let sprite_size = custom_size.unwrap_or(Vec2::splat(16.0));
                let (x, y, z) = streamed_visual_layer_transform(resolved_render_layer, Vec2::ZERO);
                entity_commands.with_children(|child| {
                    child.spawn((
                        StreamedVisualChild,
                        Mesh2d(quad_mesh),
                        MeshMaterial2d(material),
                        Transform::from_xyz(x, y, z).with_scale(Vec3::new(
                            sprite_size.x,
                            sprite_size.y,
                            1.0,
                        )),
                    ));
                });
                entity_commands.try_insert(StreamedVisualAttached);
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
        entity_commands.try_insert(StreamedVisualAttached);
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
            streamed_visual_layer_transform(Some(layer), camera_motion.world_position_xy);
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

#[allow(clippy::type_complexity)]
pub(super) fn attach_planet_visual_stack_system(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut planet_materials: ResMut<'_, Assets<PlanetVisualMaterial>>,
    time: Res<'_, Time>,
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
        let time_s = time.elapsed_secs();
        let world_position = resolve_world_position(position, world_position).unwrap_or(Vec2::ZERO);
        let diameter_m = size_m
            .map(|v| v.length.max(v.width).max(1.0))
            .unwrap_or(256.0);
        let layer_base_z = planet_layer_base_z(resolved_render_layer);
        let mut next_pass_set = pass_set.copied().unwrap_or_default();
        let Some(body_pass) =
            find_world_visual_pass(visual_stack, RuntimeWorldVisualPassKind::PlanetBody)
        else {
            continue;
        };
        if !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetBody)
            && shaders::world_polygon_shader_kind(&body_pass.shader_asset_id)
                == Some(shaders::RuntimeWorldPolygonShaderKind::PlanetVisual)
        {
            let mesh = meshes.add(Rectangle::new(1.0, 1.0));
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
                    Mesh2d(mesh),
                    MeshMaterial2d(material),
                    RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                    Transform::from_xyz(
                        0.0,
                        0.0,
                        layer_base_z + PLANET_BODY_LAYER_Z_OFFSET + depth_bias_z,
                    )
                    .with_scale(Vec3::new(
                        diameter_m * scale_multiplier,
                        diameter_m * scale_multiplier,
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
            && shaders::world_polygon_shader_kind(&back_pass.shader_asset_id)
                == Some(shaders::RuntimeWorldPolygonShaderKind::PlanetVisual)
            && shaders::world_polygon_shader_kind(&front_pass.shader_asset_id)
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
                        Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
                        MeshMaterial2d(back_material),
                        RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                        Transform::from_xyz(
                            0.0,
                            0.0,
                            layer_base_z + PLANET_CLOUD_BACK_LAYER_Z_OFFSET + back_depth,
                        )
                        .with_scale(Vec3::new(
                            diameter_m * back_scale,
                            diameter_m * back_scale,
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
                        Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
                        MeshMaterial2d(front_material),
                        RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                        Transform::from_xyz(
                            0.0,
                            0.0,
                            layer_base_z + PLANET_CLOUD_FRONT_LAYER_Z_OFFSET + front_depth,
                        )
                        .with_scale(Vec3::new(
                            diameter_m * front_scale,
                            diameter_m * front_scale,
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
            && shaders::world_polygon_shader_kind(&back_pass.shader_asset_id)
                == Some(shaders::RuntimeWorldPolygonShaderKind::PlanetVisual)
            && shaders::world_polygon_shader_kind(&front_pass.shader_asset_id)
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
                        Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
                        MeshMaterial2d(back_material),
                        RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                        Transform::from_xyz(
                            0.0,
                            0.0,
                            layer_base_z + PLANET_RING_BACK_LAYER_Z_OFFSET + back_depth,
                        )
                        .with_scale(Vec3::new(
                            diameter_m * back_scale,
                            diameter_m * back_scale,
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
                        Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
                        MeshMaterial2d(front_material),
                        RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                        Transform::from_xyz(
                            0.0,
                            0.0,
                            layer_base_z + PLANET_RING_FRONT_LAYER_Z_OFFSET + front_depth,
                        )
                        .with_scale(Vec3::new(
                            diameter_m * front_scale,
                            diameter_m * front_scale,
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
    mut commands: Commands<'_, '_>,
    mut planets: Query<
        '_,
        '_,
        (
            Entity,
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
    for (entity, settings, mut visibility, pending_initial_visual_ready) in &mut planets {
        if !settings.enabled {
            continue;
        }
        if *visibility != Visibility::Visible {
            *visibility = Visibility::Visible;
        }
        if pending_initial_visual_ready.is_some()
            && let Ok(mut entity_commands) = commands.get_entity(entity)
        {
            entity_commands.remove::<PendingInitialVisualReady>();
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
        ),
        (),
    >,
) {
    let time_s = time.elapsed_secs();
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
        let layer_parallax = resolved_render_layer
            .map(|layer| runtime_layer_parallax_factor(&layer.definition))
            .unwrap_or(PLANET_BODY_PARALLAX_FACTOR);
        let layer_base_z = planet_layer_base_z(resolved_render_layer);
        let parallax_offset = -camera_motion.world_position_xy * (1.0 - layer_parallax);
        for child in children.iter() {
            if let Ok((pass, planet_material, mut transform)) = planet_visuals.get_mut(child) {
                if pass.family != RuntimeWorldVisualFamily::Planet {
                    continue;
                }
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
                    transform.scale = Vec3::new(
                        diameter_m * scale_multiplier,
                        diameter_m * scale_multiplier,
                        1.0,
                    );
                }
                transform.translation.x = parallax_offset.x;
                transform.translation.y = parallax_offset.y;
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

fn planet_layer_base_z(resolved_render_layer: Option<&ResolvedRuntimeRenderLayer>) -> f32 {
    resolved_render_layer
        .map(|layer| runtime_layer_z_bias(&layer.definition))
        .unwrap_or(-60.0)
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
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut plume_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    world_lighting: Res<'_, WorldLightingState>,
    camera_local_lights: Res<'_, CameraLocalLightSet>,
    visual_children: Query<'_, '_, &'_ RuntimeWorldVisualPass>,
    engines: Query<
        '_,
        '_,
        (Entity, &'_ Children, Option<&'_ RuntimeWorldVisualPassSet>),
        (
            With<WorldEntity>,
            With<Engine>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
) {
    if !shader_materials_enabled() {
        return;
    }
    for (entity, children, pass_set) in &engines {
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
        let plume_mesh = meshes.add(Rectangle::new(1.0, 1.0));
        let plume_material = plume_materials.add(RuntimeEffectMaterial {
            lighting: SharedWorldLightingUniforms::from_state_for_world_position(
                &world_lighting,
                Vec2::ZERO,
                &camera_local_lights,
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
                Transform::from_xyz(0.0, -0.35, 0.1).with_scale(Vec3::new(1.2, 0.02, 1.0)),
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

#[allow(clippy::type_complexity)]
pub(super) fn update_asteroid_shader_lighting_system(
    world_lighting: Res<'_, WorldLightingState>,
    camera_local_lights: Res<'_, CameraLocalLightSet>,
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
            shader_asset_id.and_then(|shader| shaders::world_sprite_shader_kind(&shader.0)),
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
    mut effect_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut pool: ResMut<'_, WeaponTracerPool>,
) {
    if !pool.bolts.is_empty() {
        return;
    }
    pool.bolts.reserve(WEAPON_TRACER_POOL_SIZE);
    for _ in 0..WEAPON_TRACER_POOL_SIZE {
        let mesh = meshes.add(Rectangle::new(1.0, 1.0));
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
                Mesh2d(mesh),
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

    for mut receiver in &mut receivers {
        for message in receiver.receive() {
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
                    transform.rotation = Quat::from_rotation_z(
                        velocity.to_angle() + WEAPON_TRACER_ROTATION_OFFSET_RAD,
                    );
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
}

pub(super) fn update_weapon_tracer_visuals_system(
    mut commands: Commands<'_, '_>,
    time: Res<'_, Time>,
    spatial_query: SpatialQuery<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut effect_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
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
                commands.spawn((
                    WeaponImpactSpark {
                        ttl_s: WEAPON_IMPACT_SPARK_TTL_S,
                        max_ttl_s: WEAPON_IMPACT_SPARK_TTL_S,
                    },
                    Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
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
                    Transform::from_xyz(impact_pos.x, impact_pos.y, 0.45),
                    Visibility::Visible,
                    WorldEntity,
                    DespawnOnExit(ClientAppState::InWorld),
                ));
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
                commands.spawn((
                    WeaponImpactSpark {
                        ttl_s: WEAPON_IMPACT_SPARK_TTL_S,
                        max_ttl_s: WEAPON_IMPACT_SPARK_TTL_S,
                    },
                    Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
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
                    Transform::from_xyz(impact_pos.x, impact_pos.y, 0.45),
                    Visibility::Visible,
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
    mut commands: Commands<'_, '_>,
    time: Res<'_, Time>,
    mut spark_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut sparks: Query<
        '_,
        '_,
        (
            Entity,
            &'_ mut WeaponImpactSpark,
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
        ),
    >,
) {
    let dt_s = time.delta_secs();
    for (entity, mut spark, mut transform, material_handle, mut visibility) in &mut sparks {
        spark.ttl_s = (spark.ttl_s - dt_s).max(0.0);
        if spark.ttl_s <= 0.0 {
            queue_despawn_if_exists_force(&mut commands, entity);
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

fn rotate_vec2(rotation: Quat, input: Vec2) -> Vec2 {
    (rotation * input.extend(0.0)).truncate()
}
