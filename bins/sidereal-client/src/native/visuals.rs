//! Fullscreen/backdrop and streamed visual lifecycle systems.

use bevy::camera::visibility::RenderLayers;
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;
use bevy::state::state_scoped::DespawnOnExit;
use sidereal_game::{
    EntityGuid, FullscreenLayer, PlayerTag, SPACE_BACKGROUND_LAYER_KIND, STARFIELD_LAYER_KIND,
    SizeM, SpaceBackgroundFullscreenLayerBundle, SpaceBackgroundShaderSettings,
    StarfieldFullscreenLayerBundle, StarfieldShaderSettings,
};
use std::collections::HashMap;

use super::app_state::ClientAppState;
use super::assets;
use super::assets::LocalAssetManager;
use super::backdrop::{SpaceBackgroundMaterial, StarfieldMaterial, StreamedSpriteShaderMaterial};
use super::components::{
    ControlledEntity, DebugBlueBackdrop, FallbackFullscreenLayer, FullscreenLayerRenderable,
    SpaceBackdropFallback, SpaceBackgroundBackdrop, StarfieldBackdrop, StreamedSpriteShaderAssetId,
    StreamedVisualAssetId, StreamedVisualAttached, StreamedVisualChild,
    SuppressedPredictedDuplicateVisual, WorldEntity,
};
use super::platform::{self, BACKDROP_RENDER_LAYER, STREAMED_SPRITE_PIXEL_SHADER_PATH};
use super::resources::{AssetRootPath, BootstrapWatchdogState};
use super::shaders;

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
                entity_commands.despawn();
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
            Has<ControlledEntity>,
            Has<lightyear::prelude::Predicted>,
            Has<SuppressedPredictedDuplicateVisual>,
        ),
        With<WorldEntity>,
    >,
) {
    let mut best_entity_by_guid = HashMap::<uuid::Uuid, (Entity, i32)>::new();
    for (entity, guid, is_controlled, is_predicted, _is_suppressed) in &world_entities {
        let Some(guid) = guid else { continue };
        let score = if is_controlled {
            3
        } else if is_predicted {
            2
        } else {
            1
        };
        match best_entity_by_guid.get_mut(&guid.0) {
            Some((winner, winner_score)) => {
                if score > *winner_score {
                    *winner = entity;
                    *winner_score = score;
                }
            }
            None => {
                best_entity_by_guid.insert(guid.0, (entity, score));
            }
        }
    }

    for (entity, guid, _is_controlled, _is_predicted, is_suppressed) in &world_entities {
        let should_suppress = guid
            .and_then(|guid| best_entity_by_guid.get(&guid.0).copied())
            .is_some_and(|(winner, _)| winner != entity);
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
    ) in &parents
    {
        let should_clear_visual =
            visual_asset_id.is_none() || is_suppressed || player_tag.is_some();
        if !should_clear_visual {
            continue;
        }
        let mut removed_any_child = false;
        for child in children.iter() {
            if visual_children.get(child).is_ok() {
                if let Ok(mut entity_commands) = commands.get_entity(child) {
                    entity_commands.despawn();
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
