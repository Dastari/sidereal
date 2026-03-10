//! Asset cache and runtime HTTP asset fetch systems.

use super::app_state::ClientSession;
use super::components::{StreamedSpriteShaderAssetId, StreamedVisualAssetId};
use super::resources::AssetRootPath;
use super::resources::{AssetCacheAdapter, GatewayHttpAdapter};
use super::shaders;
use bevy::asset::RenderAssetUsages;
use bevy::ecs::lifecycle::RemovedComponentEntity;
use bevy::ecs::message::MessageCursor;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task, futures_lite::future};
use bevy_svg::prelude::Svg;
use lightyear::prelude::MessageReceiver;
use lightyear::prelude::client::{Client, Connected};
use sidereal_asset_runtime::{AssetCacheIndex, AssetCacheIndexRecord, sha256_hex};
use sidereal_game::{
    FullscreenLayer, RuntimePostProcessStack, RuntimeRenderLayerDefinition, SizeM,
    SpriteShaderAssetId,
};
use sidereal_net::ServerAssetCatalogVersionMessage;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Default)]
pub(crate) struct LocalAssetRecord {
    pub relative_cache_path: String,
    pub _content_type: String,
    pub _byte_len: u64,
    pub _chunk_count: u32,
    pub _asset_version: u64,
    pub _sha256_hex: String,
    pub ready: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeAssetCatalogRecord {
    pub _asset_guid: String,
    pub shader_family: Option<String>,
    pub dependencies: Vec<String>,
    pub url: String,
    pub relative_cache_path: String,
    pub content_type: String,
    pub _byte_len: u64,
    pub sha256_hex: String,
}

#[derive(Debug, Resource, Default, Clone)]
pub(crate) struct LocalAssetManager {
    pub records_by_asset_id: HashMap<String, LocalAssetRecord>,
    pub catalog_by_asset_id: HashMap<String, RuntimeAssetCatalogRecord>,
    pub catalog_version: Option<String>,
    pub cache_index: AssetCacheIndex,
    pub cache_index_loaded: bool,
    pub bootstrap_manifest_seen: bool,
    pub bootstrap_phase_complete: bool,
    pub bootstrap_total_bytes: u64,
    pub bootstrap_ready_bytes: u64,
    pub reload_generation: u64,
}

impl LocalAssetManager {
    pub fn bootstrap_complete(&self) -> bool {
        self.bootstrap_phase_complete
    }

    pub fn bootstrap_progress(&self) -> f32 {
        if self.bootstrap_total_bytes == 0 {
            return if self.bootstrap_manifest_seen {
                1.0
            } else {
                0.0
            };
        }
        (self.bootstrap_ready_bytes as f32 / self.bootstrap_total_bytes as f32).clamp(0.0, 1.0)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn cached_relative_path(&self, asset_id: &str) -> Option<&str> {
        self.records_by_asset_id
            .get(asset_id)
            .filter(|record| record.ready)
            .map(|record| record.relative_cache_path.as_str())
    }
}

#[derive(Debug, Resource, Default, Clone)]
pub(crate) struct AssetCatalogHotReloadState {
    pub pending_catalog_version: Option<String>,
    pub forced_asset_ids: HashSet<String>,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct RuntimeAssetNetIndicatorState {
    pub blinking_phase_s: f32,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct RuntimeAssetHttpFetchState {
    pending: Option<Task<Result<RuntimeAssetFetchResult, String>>>,
    in_flight_asset_ids: HashSet<String>,
    pending_parent_asset_ids: HashMap<String, String>,
    pub last_request_at_s: f64,
}

impl RuntimeAssetHttpFetchState {
    pub fn has_in_flight_fetch(&self) -> bool {
        !self.in_flight_asset_ids.is_empty()
    }

    pub fn in_flight_asset_ids_len(&self) -> usize {
        self.in_flight_asset_ids.len()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeAssetFetchResult {
    pub asset_id: String,
    pub relative_cache_path: String,
    pub content_type: String,
    pub byte_len: u64,
    pub asset_version: u64,
    pub sha256_hex: String,
    pub payload: Vec<u8>,
    pub cache_index: AssetCacheIndex,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct RuntimeAssetDependencyState {
    pub candidate_asset_ids: HashSet<String>,
    pub catalog_reload_generation: u64,
    pub forced_asset_ids_signature: u64,
    pub dependency_graph_rebuilds: u64,
    pub dependency_scan_runs: u64,
    fullscreen_layer_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    runtime_render_layer_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    runtime_post_process_stack_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    sprite_shader_asset_id_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    streamed_sprite_shader_asset_id_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    streamed_visual_asset_id_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
}

fn expand_catalog_dependencies(
    seed_asset_ids: HashSet<String>,
    asset_manager: &LocalAssetManager,
) -> HashSet<String> {
    let mut expanded = seed_asset_ids;
    let mut stack = expanded.iter().cloned().collect::<Vec<_>>();
    while let Some(asset_id) = stack.pop() {
        let Some(entry) = asset_manager.catalog_by_asset_id.get(&asset_id) else {
            continue;
        };
        for dependency in &entry.dependencies {
            if expanded.insert(dependency.clone()) {
                stack.push(dependency.clone());
            }
        }
    }
    expanded
}

pub(super) fn sync_runtime_asset_dependency_state_system(world: &mut World) {
    let mut dependency_state = world
        .remove_resource::<RuntimeAssetDependencyState>()
        .unwrap_or_default();
    dependency_state.dependency_scan_runs = dependency_state.dependency_scan_runs.saturating_add(1);
    let catalog_reload_generation = world
        .get_resource::<LocalAssetManager>()
        .map(|asset_manager| asset_manager.reload_generation)
        .unwrap_or_default();
    let forced_asset_ids_signature = world
        .get_resource::<AssetCatalogHotReloadState>()
        .map(|hot_reload| hash_asset_ids(&hot_reload.forced_asset_ids))
        .unwrap_or_default();
    let dependency_inputs_changed = catalog_reload_generation
        != dependency_state.catalog_reload_generation
        || forced_asset_ids_signature != dependency_state.forced_asset_ids_signature
        || has_any_component_changes::<FullscreenLayer>(world)
        || has_any_component_changes::<RuntimeRenderLayerDefinition>(world)
        || has_any_component_changes::<RuntimePostProcessStack>(world)
        || has_any_component_changes::<SpriteShaderAssetId>(world)
        || has_any_component_changes::<StreamedSpriteShaderAssetId>(world)
        || has_any_component_changes::<StreamedVisualAssetId>(world)
        || has_any_removed_components::<FullscreenLayer>(
            world,
            &mut dependency_state.fullscreen_layer_removal_cursor,
        )
        || has_any_removed_components::<RuntimeRenderLayerDefinition>(
            world,
            &mut dependency_state.runtime_render_layer_removal_cursor,
        )
        || has_any_removed_components::<RuntimePostProcessStack>(
            world,
            &mut dependency_state.runtime_post_process_stack_removal_cursor,
        )
        || has_any_removed_components::<SpriteShaderAssetId>(
            world,
            &mut dependency_state.sprite_shader_asset_id_removal_cursor,
        )
        || has_any_removed_components::<StreamedSpriteShaderAssetId>(
            world,
            &mut dependency_state.streamed_sprite_shader_asset_id_removal_cursor,
        )
        || has_any_removed_components::<StreamedVisualAssetId>(
            world,
            &mut dependency_state.streamed_visual_asset_id_removal_cursor,
        );
    if !dependency_inputs_changed {
        world.insert_resource(dependency_state);
        return;
    }

    let candidate_asset_ids = {
        let asset_manager = world
            .get_resource::<LocalAssetManager>()
            .expect("local asset manager should be initialized")
            .clone();
        let hot_reload = world
            .get_resource::<AssetCatalogHotReloadState>()
            .expect("asset hot reload state should be initialized")
            .clone();
        collect_runtime_asset_dependency_candidates(world, &asset_manager, &hot_reload)
    };
    dependency_state.candidate_asset_ids = candidate_asset_ids;
    dependency_state.catalog_reload_generation = catalog_reload_generation;
    dependency_state.forced_asset_ids_signature = forced_asset_ids_signature;
    dependency_state.dependency_graph_rebuilds =
        dependency_state.dependency_graph_rebuilds.saturating_add(1);
    world.insert_resource(dependency_state);
}

pub(super) fn receive_asset_catalog_version_messages(
    asset_manager: Res<'_, LocalAssetManager>,
    mut hot_reload: ResMut<'_, AssetCatalogHotReloadState>,
    mut receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<ServerAssetCatalogVersionMessage>,
        (With<Client>, With<Connected>),
    >,
) {
    for mut receiver in &mut receivers {
        for message in receiver.receive() {
            if asset_manager.catalog_version.as_deref() == Some(message.catalog_version.as_str()) {
                continue;
            }
            hot_reload.pending_catalog_version = Some(message.catalog_version);
        }
    }
}

pub(super) fn cached_asset_bytes(
    asset_id: &str,
    asset_manager: &LocalAssetManager,
    asset_root: &str,
    cache_adapter: AssetCacheAdapter,
) -> Option<Vec<u8>> {
    let catalog = asset_manager.catalog_by_asset_id.get(asset_id)?;
    let relative_cache_path = asset_manager
        .records_by_asset_id
        .get(asset_id)
        .map(|record| record.relative_cache_path.as_str())
        .unwrap_or(catalog.relative_cache_path.as_str());
    (cache_adapter.read_valid_asset_sync)(asset_root, relative_cache_path, &catalog.sha256_hex)
}

pub(super) fn cached_shader_source(
    asset_id: &str,
    asset_manager: &LocalAssetManager,
    asset_root: &str,
    cache_adapter: AssetCacheAdapter,
) -> Option<String> {
    let bytes = cached_asset_bytes(asset_id, asset_manager, asset_root, cache_adapter)?;
    String::from_utf8(bytes).ok()
}

pub(super) fn cached_image_handle(
    asset_id: &str,
    asset_manager: &LocalAssetManager,
    asset_root: &str,
    cache_adapter: AssetCacheAdapter,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    let catalog = asset_manager.catalog_by_asset_id.get(asset_id)?;
    let bytes = cached_asset_bytes(asset_id, asset_manager, asset_root, cache_adapter)?;
    let image = Image::from_buffer(
        &bytes,
        ImageType::MimeType(&catalog.content_type),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::default(),
        RenderAssetUsages::default(),
    )
    .ok()?;
    Some(images.add(image))
}

pub(super) fn cached_svg_handle(
    asset_id: &str,
    asset_manager: &LocalAssetManager,
    asset_root: &str,
    cache_adapter: AssetCacheAdapter,
    svg_assets: &mut Assets<Svg>,
    meshes: &mut Assets<Mesh>,
) -> Option<Handle<Svg>> {
    let catalog = asset_manager.catalog_by_asset_id.get(asset_id)?;
    let bytes = cached_asset_bytes(asset_id, asset_manager, asset_root, cache_adapter)?;
    let mut svg = Svg::from_bytes(
        &bytes,
        &catalog.relative_cache_path,
        None::<&std::path::Path>,
    )
    .ok()?;
    svg.name = catalog.relative_cache_path.clone();
    svg.mesh = meshes.add(svg.tessellate());
    Some(svg_assets.add(svg))
}

pub(super) fn resolved_world_sprite_size(
    texture_size_px: Option<UVec2>,
    size_m: Option<&SizeM>,
) -> Option<Vec2> {
    let bounds = size_m.map(|size| Vec2::new(size.width.max(0.1), size.length.max(0.1)));
    match (texture_size_px, bounds) {
        (Some(px), Some(bounds)) if px.x > 0 && px.y > 0 => {
            let px_size = Vec2::new(px.x as f32, px.y as f32);
            let scale = (bounds.x / px_size.x).min(bounds.y / px_size.y);
            Some(px_size * scale)
        }
        (None, Some(bounds)) => Some(bounds),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn queue_missing_catalog_assets_system(
    time: Res<'_, Time>,
    mut fetch_state: ResMut<'_, RuntimeAssetHttpFetchState>,
    asset_manager: Res<'_, LocalAssetManager>,
    dependency_state: Res<'_, RuntimeAssetDependencyState>,
    asset_root: Res<'_, AssetRootPath>,
    gateway_http: Res<'_, GatewayHttpAdapter>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
    session: Res<'_, ClientSession>,
) {
    let Some(access_token) = session.access_token.as_ref() else {
        return;
    };
    if fetch_state.pending.is_some() {
        return;
    }
    let now = time.elapsed_secs_f64();
    if now - fetch_state.last_request_at_s < 0.05 {
        return;
    }
    let Some(next_asset_id) = next_runtime_asset_fetch_candidate(
        &dependency_state.candidate_asset_ids,
        &asset_manager,
        &asset_root.0,
        *cache_adapter,
        &fetch_state.in_flight_asset_ids,
    ) else {
        return;
    };
    let Some(catalog) = asset_manager
        .catalog_by_asset_id
        .get(&next_asset_id)
        .cloned()
    else {
        return;
    };
    let unresolved_dependency = catalog
        .dependencies
        .iter()
        .find(|dependency| {
            !asset_present_in_cache_or_source(
                dependency,
                &asset_manager,
                &asset_root.0,
                *cache_adapter,
            ) && !fetch_state.in_flight_asset_ids.contains(*dependency)
        })
        .cloned();
    if let Some(dependency_asset_id) = unresolved_dependency {
        fetch_state
            .pending_parent_asset_ids
            .insert(dependency_asset_id.clone(), next_asset_id.clone());
        fetch_state.last_request_at_s = now;
        fetch_state
            .in_flight_asset_ids
            .insert(dependency_asset_id.clone());
        let Some(catalog) = asset_manager
            .catalog_by_asset_id
            .get(&dependency_asset_id)
            .cloned()
        else {
            fetch_state.in_flight_asset_ids.remove(&dependency_asset_id);
            fetch_state
                .pending_parent_asset_ids
                .remove(&dependency_asset_id);
            return;
        };
        info!(
            "runtime asset download queued: asset_id={} dependency_for={} relative_cache_path={}",
            dependency_asset_id, next_asset_id, catalog.relative_cache_path
        );
        let url = if catalog.url.starts_with("http://") || catalog.url.starts_with("https://") {
            catalog.url.clone()
        } else {
            format!("{}{}", session.gateway_url, catalog.url)
        };
        let access_token = access_token.clone();
        let gateway_http = *gateway_http;
        let cache_adapter = *cache_adapter;
        let asset_root = asset_root.0.clone();
        let mut cache_index = asset_manager.cache_index.clone();
        fetch_state.pending = Some(IoTaskPool::get().spawn(async move {
            let payload = (gateway_http.fetch_asset_bytes)(url, access_token.clone()).await?;
            let payload_sha = sha256_hex(&payload);
            if payload_sha != catalog.sha256_hex {
                return Err(format!(
                    "runtime asset checksum mismatch asset_id={} expected={} got={}",
                    dependency_asset_id, catalog.sha256_hex, payload_sha
                ));
            }
            cache_index.by_asset_id.insert(
                dependency_asset_id.clone(),
                AssetCacheIndexRecord {
                    asset_version: sidereal_asset_runtime::asset_version_from_sha256_hex(
                        &payload_sha,
                    ),
                    sha256_hex: payload_sha.clone(),
                },
            );
            (cache_adapter.write_asset)(
                asset_root.clone(),
                catalog.relative_cache_path.clone(),
                payload.clone(),
            )
            .await?;
            (cache_adapter.save_index)(asset_root.clone(), cache_index.clone()).await?;
            Ok(RuntimeAssetFetchResult {
                asset_id: dependency_asset_id,
                relative_cache_path: catalog.relative_cache_path,
                content_type: catalog.content_type,
                byte_len: payload.len() as u64,
                asset_version: sidereal_asset_runtime::asset_version_from_sha256_hex(&payload_sha),
                sha256_hex: payload_sha,
                payload,
                cache_index,
            })
        }));
        return;
    }
    let url = if catalog.url.starts_with("http://") || catalog.url.starts_with("https://") {
        catalog.url.clone()
    } else {
        format!("{}{}", session.gateway_url, catalog.url)
    };
    fetch_state
        .in_flight_asset_ids
        .insert(next_asset_id.clone());
    fetch_state.last_request_at_s = now;
    info!(
        "runtime asset download queued: asset_id={} relative_cache_path={}",
        next_asset_id, catalog.relative_cache_path
    );

    let access_token = access_token.clone();
    let gateway_http = *gateway_http;
    let cache_adapter = *cache_adapter;
    let asset_root = asset_root.0.clone();
    let mut cache_index = asset_manager.cache_index.clone();
    fetch_state.pending = Some(IoTaskPool::get().spawn(async move {
        let payload = (gateway_http.fetch_asset_bytes)(url, access_token.clone()).await?;
        let payload_sha = sha256_hex(&payload);
        if payload_sha != catalog.sha256_hex {
            return Err(format!(
                "runtime asset checksum mismatch asset_id={} expected={} got={}",
                next_asset_id, catalog.sha256_hex, payload_sha
            ));
        }
        cache_index.by_asset_id.insert(
            next_asset_id.clone(),
            AssetCacheIndexRecord {
                asset_version: sidereal_asset_runtime::asset_version_from_sha256_hex(&payload_sha),
                sha256_hex: payload_sha.clone(),
            },
        );
        (cache_adapter.write_asset)(
            asset_root.clone(),
            catalog.relative_cache_path.clone(),
            payload.clone(),
        )
        .await?;
        (cache_adapter.save_index)(asset_root.clone(), cache_index.clone()).await?;
        Ok(RuntimeAssetFetchResult {
            asset_id: next_asset_id,
            relative_cache_path: catalog.relative_cache_path,
            content_type: catalog.content_type,
            byte_len: payload.len() as u64,
            asset_version: sidereal_asset_runtime::asset_version_from_sha256_hex(&payload_sha),
            sha256_hex: payload_sha,
            payload,
            cache_index,
        })
    }));
}

#[allow(clippy::too_many_arguments)]
pub(super) fn poll_runtime_asset_http_fetches_system(
    mut fetch_state: ResMut<'_, RuntimeAssetHttpFetchState>,
    mut asset_manager: ResMut<'_, LocalAssetManager>,
    mut hot_reload: ResMut<'_, AssetCatalogHotReloadState>,
    mut session: ResMut<'_, ClientSession>,
    asset_root: Res<'_, AssetRootPath>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
    shader_assignments: Res<'_, shaders::RuntimeShaderAssignments>,
    mut shaders_assets: ResMut<'_, Assets<bevy::shader::Shader>>,
) {
    let Some(task) = fetch_state.pending.as_mut() else {
        return;
    };
    let Some(result) = bevy::tasks::block_on(future::poll_once(task)) else {
        return;
    };
    fetch_state.pending = None;

    match result {
        Ok(result) => {
            let payload_sha = sha256_hex(&result.payload);
            if payload_sha != result.sha256_hex {
                warn!(
                    "runtime asset download failed: runtime asset checksum mismatch asset_id={} expected={} got={}",
                    result.asset_id, result.sha256_hex, payload_sha
                );
                session.status = format!(
                    "Asset download failed: runtime asset checksum mismatch asset_id={} expected={} got={}",
                    result.asset_id, result.sha256_hex, payload_sha
                );
                session.ui_dirty = true;
            } else {
                asset_manager.cache_index = result.cache_index.clone();
                asset_manager.records_by_asset_id.insert(
                    result.asset_id.clone(),
                    LocalAssetRecord {
                        relative_cache_path: result.relative_cache_path.clone(),
                        _content_type: result.content_type.clone(),
                        _byte_len: result.byte_len,
                        _chunk_count: 1,
                        _asset_version: result.asset_version,
                        _sha256_hex: result.sha256_hex.clone(),
                        ready: true,
                    },
                );
                info!(
                    "runtime asset cache write complete: asset_id={} relative_cache_path={} bytes={}",
                    result.asset_id, result.relative_cache_path, result.byte_len
                );
                hot_reload.forced_asset_ids.remove(&result.asset_id);
                session.status = format!("Asset downloaded: {}", result.asset_id);
                session.ui_dirty = true;
                if shaders::shader_materials_enabled()
                    && result.relative_cache_path.ends_with(".wgsl")
                {
                    shaders::reload_streamed_shaders(
                        &mut shaders_assets,
                        &asset_root.0,
                        &asset_manager,
                        *cache_adapter,
                        &shader_assignments,
                    );
                }
            }
            fetch_state.in_flight_asset_ids.remove(&result.asset_id);
            fetch_state
                .pending_parent_asset_ids
                .remove(&result.asset_id);
        }
        Err(err) => {
            warn!("runtime asset download failed: {}", err);
            session.status = format!("Asset download failed: {}", err);
            session.ui_dirty = true;
            let maybe_id = fetch_state.in_flight_asset_ids.iter().next().cloned();
            if let Some(asset_id) = maybe_id {
                fetch_state.in_flight_asset_ids.remove(&asset_id);
                fetch_state.pending_parent_asset_ids.remove(&asset_id);
            }
        }
    }
}

fn asset_present_in_cache_or_source(
    asset_id: &str,
    asset_manager: &LocalAssetManager,
    asset_root: &str,
    cache_adapter: AssetCacheAdapter,
) -> bool {
    if asset_manager
        .records_by_asset_id
        .get(asset_id)
        .is_some_and(|record| record.ready)
    {
        return true;
    }
    let Some(catalog) = asset_manager.catalog_by_asset_id.get(asset_id) else {
        return false;
    };
    let Some(relative_cache_path) = asset_manager
        .records_by_asset_id
        .get(asset_id)
        .map(|record| record.relative_cache_path.as_str())
        .or(Some(catalog.relative_cache_path.as_str()))
    else {
        return false;
    };
    (cache_adapter.read_valid_asset_sync)(asset_root, relative_cache_path, &catalog.sha256_hex)
        .is_some()
}

fn collect_runtime_asset_dependency_candidates(
    world: &mut World,
    asset_manager: &LocalAssetManager,
    hot_reload: &AssetCatalogHotReloadState,
) -> HashSet<String> {
    let mut candidate_asset_ids = hot_reload
        .forced_asset_ids
        .iter()
        .filter(|asset_id| asset_manager.catalog_by_asset_id.contains_key(*asset_id))
        .cloned()
        .collect::<HashSet<_>>();

    let mut fullscreen_layers = world.query::<&FullscreenLayer>();
    for layer in fullscreen_layers.iter(world) {
        insert_asset_id(
            &mut candidate_asset_ids,
            Some(layer.shader_asset_id.as_str()),
        );
    }

    let mut runtime_render_layers = world.query::<&RuntimeRenderLayerDefinition>();
    for layer in runtime_render_layers.iter(world) {
        insert_asset_id(
            &mut candidate_asset_ids,
            Some(layer.shader_asset_id.as_str()),
        );
        insert_asset_id(&mut candidate_asset_ids, layer.params_asset_id.as_deref());
        for binding in &layer.texture_bindings {
            insert_asset_id(&mut candidate_asset_ids, Some(binding.asset_id.as_str()));
        }
    }

    let mut runtime_post_process_stacks = world.query::<&RuntimePostProcessStack>();
    for stack in runtime_post_process_stacks.iter(world) {
        for pass in &stack.passes {
            insert_asset_id(
                &mut candidate_asset_ids,
                Some(pass.shader_asset_id.as_str()),
            );
            insert_asset_id(&mut candidate_asset_ids, pass.params_asset_id.as_deref());
            for binding in &pass.texture_bindings {
                insert_asset_id(&mut candidate_asset_ids, Some(binding.asset_id.as_str()));
            }
        }
    }

    let mut sprite_shader_asset_ids = world.query::<&SpriteShaderAssetId>();
    for sprite_shader_asset_id in sprite_shader_asset_ids.iter(world) {
        insert_asset_id(
            &mut candidate_asset_ids,
            sprite_shader_asset_id.0.as_deref(),
        );
    }

    let mut streamed_sprite_shader_asset_ids = world.query::<&StreamedSpriteShaderAssetId>();
    for streamed in streamed_sprite_shader_asset_ids.iter(world) {
        insert_asset_id(&mut candidate_asset_ids, Some(streamed.0.as_str()));
    }

    let mut streamed_visual_asset_ids = world.query::<&StreamedVisualAssetId>();
    for visual in streamed_visual_asset_ids.iter(world) {
        insert_asset_id(&mut candidate_asset_ids, Some(visual.0.as_str()));
    }

    expand_catalog_dependencies(candidate_asset_ids, asset_manager)
}

fn insert_asset_id(asset_ids: &mut HashSet<String>, maybe_asset_id: Option<&str>) {
    if let Some(asset_id) = maybe_asset_id
        && !asset_id.trim().is_empty()
    {
        asset_ids.insert(asset_id.to_string());
    }
}

fn next_runtime_asset_fetch_candidate(
    candidate_asset_ids: &HashSet<String>,
    asset_manager: &LocalAssetManager,
    asset_root: &str,
    cache_adapter: AssetCacheAdapter,
    in_flight_asset_ids: &HashSet<String>,
) -> Option<String> {
    let next_asset_id = candidate_asset_ids
        .iter()
        .filter(|asset_id| asset_manager.catalog_by_asset_id.contains_key(*asset_id))
        .filter(|asset_id| !in_flight_asset_ids.contains(*asset_id))
        .find(|asset_id| {
            !asset_present_in_cache_or_source(asset_id, asset_manager, asset_root, cache_adapter)
        })?
        .clone();
    let catalog = asset_manager.catalog_by_asset_id.get(&next_asset_id)?;
    catalog
        .dependencies
        .iter()
        .find(|dependency| {
            !asset_present_in_cache_or_source(dependency, asset_manager, asset_root, cache_adapter)
                && !in_flight_asset_ids.contains(*dependency)
        })
        .cloned()
        .or(Some(next_asset_id))
}

fn has_any_component_changes<T: Component>(world: &mut World) -> bool {
    let mut query = world.query_filtered::<Entity, Or<(Added<T>, Changed<T>)>>();
    query.iter(world).next().is_some()
}

fn has_any_removed_components<T: Component>(
    world: &mut World,
    cursor: &mut Option<MessageCursor<RemovedComponentEntity>>,
) -> bool {
    let Some(component_id) = world.component_id::<T>() else {
        return false;
    };
    let Some(events) = world.removed_components().get(component_id) else {
        return false;
    };
    let reader = cursor.get_or_insert_with(Default::default);
    reader.read(events).next().is_some()
}

fn hash_asset_ids(asset_ids: &HashSet<String>) -> u64 {
    let mut sorted = asset_ids.iter().collect::<Vec<_>>();
    sorted.sort_unstable();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for asset_id in sorted {
        asset_id.hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::{
        AssetCatalogHotReloadState, LocalAssetManager, RuntimeAssetCatalogRecord,
        RuntimeAssetDependencyState, next_runtime_asset_fetch_candidate,
        sync_runtime_asset_dependency_state_system,
    };
    use crate::native::components::StreamedVisualAssetId;
    use crate::native::resources::AssetCacheAdapter;
    use bevy::prelude::*;
    use sidereal_asset_runtime::AssetCacheIndex;
    use sidereal_game::RuntimeRenderLayerDefinition;
    use std::collections::{HashMap, HashSet};

    fn test_asset_manager(entries: &[(&str, &[&str])]) -> LocalAssetManager {
        let mut catalog_by_asset_id = HashMap::new();
        for (asset_id, dependencies) in entries {
            catalog_by_asset_id.insert(
                (*asset_id).to_string(),
                RuntimeAssetCatalogRecord {
                    _asset_guid: format!("{asset_id}-guid"),
                    shader_family: None,
                    dependencies: dependencies
                        .iter()
                        .map(|value| (*value).to_string())
                        .collect(),
                    url: format!("/assets/{asset_id}"),
                    relative_cache_path: format!("cache/{asset_id}"),
                    content_type: "text/plain".to_string(),
                    _byte_len: 1,
                    sha256_hex: format!("sha-{asset_id}"),
                },
            );
        }
        LocalAssetManager {
            catalog_by_asset_id,
            reload_generation: 1,
            ..default()
        }
    }

    #[test]
    fn dependency_state_tracks_new_and_removed_asset_references() {
        let mut app = App::new();
        app.insert_resource(test_asset_manager(&[("planet_shader", &[])]));
        app.insert_resource(AssetCatalogHotReloadState::default());
        app.insert_resource(RuntimeAssetDependencyState::default());
        app.add_systems(Update, sync_runtime_asset_dependency_state_system);

        let entity = app
            .world_mut()
            .spawn(RuntimeRenderLayerDefinition {
                shader_asset_id: "planet_shader".to_string(),
                ..default()
            })
            .id();

        app.update();
        assert!(
            app.world()
                .resource::<RuntimeAssetDependencyState>()
                .candidate_asset_ids
                .contains("planet_shader")
        );

        app.world_mut()
            .entity_mut(entity)
            .remove::<RuntimeRenderLayerDefinition>();
        app.update();
        assert!(
            !app.world()
                .resource::<RuntimeAssetDependencyState>()
                .candidate_asset_ids
                .contains("planet_shader")
        );
    }

    #[test]
    fn dependency_state_expands_catalog_closure() {
        let mut app = App::new();
        app.insert_resource(test_asset_manager(&[
            ("visual_root", &["visual_dep"]),
            ("visual_dep", &[]),
        ]));
        app.insert_resource(AssetCatalogHotReloadState::default());
        app.insert_resource(RuntimeAssetDependencyState::default());
        app.add_systems(Update, sync_runtime_asset_dependency_state_system);

        app.world_mut()
            .spawn(StreamedVisualAssetId("visual_root".to_string()));

        app.update();

        let dependency_state = app.world().resource::<RuntimeAssetDependencyState>();
        assert!(dependency_state.candidate_asset_ids.contains("visual_root"));
        assert!(dependency_state.candidate_asset_ids.contains("visual_dep"));
    }

    #[test]
    fn fetch_candidate_prefers_unresolved_dependency_before_parent() {
        let asset_manager = test_asset_manager(&[("root", &["dep"]), ("dep", &[])]);
        let candidate_asset_ids = HashSet::from(["root".to_string(), "dep".to_string()]);
        let selected = next_runtime_asset_fetch_candidate(
            &candidate_asset_ids,
            &asset_manager,
            "data",
            AssetCacheAdapter {
                prepare_root: |_| Box::pin(async { Ok(()) }),
                load_index: |_| Box::pin(async { Ok(AssetCacheIndex::default()) }),
                save_index: |_, _| Box::pin(async { Ok(()) }),
                read_valid_asset: |_, _, _| Box::pin(async { Ok(None) }),
                write_asset: |_, _, _| Box::pin(async { Ok(()) }),
                read_valid_asset_sync: |_, _, _| None,
            },
            &HashSet::new(),
        );
        assert_eq!(selected.as_deref(), Some("dep"));
    }
}
