//! Asset cache and runtime HTTP asset fetch systems.

use super::app_state::ClientSession;
use super::components::{StreamedSpriteShaderAssetId, StreamedVisualAssetId};
use super::resources::AssetRootPath;
use super::resources::{AssetCacheAdapter, GatewayHttpAdapter, RuntimeAssetPerfCounters};
use super::shaders;
use bevy::asset::RenderAssetUsages;
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
use std::time::Instant;

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
    pending_fetches: Vec<RuntimeAssetFetchTask>,
    pending_persists: Vec<RuntimeAssetPersistTask>,
    save_index_task: Option<RuntimeAssetSaveIndexTask>,
    cache_index_dirty: bool,
    in_flight_asset_ids: HashSet<String>,
    pending_parent_asset_ids: HashMap<String, String>,
}

impl RuntimeAssetHttpFetchState {
    pub fn has_in_flight_fetch(&self) -> bool {
        !self.in_flight_asset_ids.is_empty()
            || !self.pending_persists.is_empty()
            || self.save_index_task.is_some()
    }

    pub fn in_flight_asset_ids_len(&self) -> usize {
        self.in_flight_asset_ids.len()
    }
}

#[derive(Debug)]
struct RuntimeAssetFetchTask {
    asset_id: String,
    queued_at: Instant,
    task: Task<Result<RuntimeAssetFetchResult, String>>,
}

#[derive(Debug)]
struct RuntimeAssetPersistTask {
    asset_id: String,
    queued_at: Instant,
    task: Task<Result<RuntimeAssetPersistResult, String>>,
}

#[derive(Debug)]
struct RuntimeAssetSaveIndexTask {
    queued_at: Instant,
    task: Task<Result<(), String>>,
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
}

#[derive(Debug, Clone)]
struct RuntimeAssetPersistResult {
    pub asset_id: String,
    pub relative_cache_path: String,
    pub content_type: String,
    pub byte_len: u64,
    pub asset_version: u64,
    pub sha256_hex: String,
}

const MAX_CONCURRENT_RUNTIME_ASSET_FETCHES: usize = 4;

#[derive(Debug, Resource, Default)]
pub(crate) struct RuntimeAssetDependencyState {
    pub candidate_asset_ids: HashSet<String>,
    pub catalog_reload_generation: u64,
    pub forced_asset_ids_signature: u64,
    pub dependency_graph_rebuilds: u64,
    pub dependency_scan_runs: u64,
}

#[derive(Debug, Resource)]
pub(crate) struct RuntimeAssetDependencyDirtyState {
    pub dirty: bool,
}

impl Default for RuntimeAssetDependencyDirtyState {
    fn default() -> Self {
        Self { dirty: true }
    }
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn mark_runtime_asset_dependency_state_dirty_system(
    mut dirty_state: ResMut<'_, RuntimeAssetDependencyDirtyState>,
    fullscreen_changed: Query<'_, '_, (), Or<(Added<FullscreenLayer>, Changed<FullscreenLayer>)>>,
    runtime_render_layer_changed: Query<
        '_,
        '_,
        (),
        Or<(
            Added<RuntimeRenderLayerDefinition>,
            Changed<RuntimeRenderLayerDefinition>,
        )>,
    >,
    runtime_post_process_changed: Query<
        '_,
        '_,
        (),
        Or<(
            Added<RuntimePostProcessStack>,
            Changed<RuntimePostProcessStack>,
        )>,
    >,
    sprite_shader_changed: Query<
        '_,
        '_,
        (),
        Or<(Added<SpriteShaderAssetId>, Changed<SpriteShaderAssetId>)>,
    >,
    streamed_sprite_shader_changed: Query<
        '_,
        '_,
        (),
        Or<(
            Added<StreamedSpriteShaderAssetId>,
            Changed<StreamedSpriteShaderAssetId>,
        )>,
    >,
    streamed_visual_changed: Query<
        '_,
        '_,
        (),
        Or<(Added<StreamedVisualAssetId>, Changed<StreamedVisualAssetId>)>,
    >,
    mut removed_fullscreen_layer: RemovedComponents<'_, '_, FullscreenLayer>,
    mut removed_runtime_render_layer: RemovedComponents<'_, '_, RuntimeRenderLayerDefinition>,
    mut removed_runtime_post_process: RemovedComponents<'_, '_, RuntimePostProcessStack>,
    mut removed_sprite_shader: RemovedComponents<'_, '_, SpriteShaderAssetId>,
    mut removed_streamed_sprite_shader: RemovedComponents<'_, '_, StreamedSpriteShaderAssetId>,
    mut removed_streamed_visual: RemovedComponents<'_, '_, StreamedVisualAssetId>,
) {
    let changed = fullscreen_changed.iter().next().is_some()
        || runtime_render_layer_changed.iter().next().is_some()
        || runtime_post_process_changed.iter().next().is_some()
        || sprite_shader_changed.iter().next().is_some()
        || streamed_sprite_shader_changed.iter().next().is_some()
        || streamed_visual_changed.iter().next().is_some()
        || removed_fullscreen_layer.read().next().is_some()
        || removed_runtime_render_layer.read().next().is_some()
        || removed_runtime_post_process.read().next().is_some()
        || removed_sprite_shader.read().next().is_some()
        || removed_streamed_sprite_shader.read().next().is_some()
        || removed_streamed_visual.read().next().is_some();
    if changed {
        dirty_state.dirty = true;
    }
}

pub(super) fn sync_runtime_asset_dependency_state_system(world: &mut World) {
    let mut dependency_state = world
        .remove_resource::<RuntimeAssetDependencyState>()
        .unwrap_or_default();
    let mut dirty_state = world
        .remove_resource::<RuntimeAssetDependencyDirtyState>()
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
    let dependency_inputs_changed = dirty_state.dirty
        || catalog_reload_generation != dependency_state.catalog_reload_generation
        || forced_asset_ids_signature != dependency_state.forced_asset_ids_signature;
    if !dependency_inputs_changed {
        world.insert_resource(dirty_state);
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
    dirty_state.dirty = false;
    world.insert_resource(dirty_state);
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
    _time: Res<'_, Time>,
    mut fetch_state: ResMut<'_, RuntimeAssetHttpFetchState>,
    mut perf: ResMut<'_, RuntimeAssetPerfCounters>,
    asset_manager: Res<'_, LocalAssetManager>,
    dependency_state: Res<'_, RuntimeAssetDependencyState>,
    asset_root: Res<'_, AssetRootPath>,
    gateway_http: Res<'_, GatewayHttpAdapter>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
    session: Res<'_, ClientSession>,
) {
    perf.queue_runs = perf.queue_runs.saturating_add(1);
    let Some(access_token) = session.access_token.as_ref() else {
        perf.pending_fetch_count = fetch_state.pending_fetches.len();
        perf.pending_persist_count = fetch_state.pending_persists.len();
        perf.cache_index_dirty = fetch_state.cache_index_dirty;
        return;
    };
    if fetch_state.pending_fetches.len() >= MAX_CONCURRENT_RUNTIME_ASSET_FETCHES {
        perf.pending_fetch_count = fetch_state.pending_fetches.len();
        perf.pending_persist_count = fetch_state.pending_persists.len();
        perf.cache_index_dirty = fetch_state.cache_index_dirty;
        return;
    }
    while fetch_state.pending_fetches.len() < MAX_CONCURRENT_RUNTIME_ASSET_FETCHES {
        let Some(next_asset_id) = next_runtime_asset_fetch_candidate(
            &dependency_state.candidate_asset_ids,
            &asset_manager,
            &asset_root.0,
            *cache_adapter,
            &fetch_state.in_flight_asset_ids,
        ) else {
            break;
        };
        let Some(catalog) = asset_manager
            .catalog_by_asset_id
            .get(&next_asset_id)
            .cloned()
        else {
            break;
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
        let (asset_id, parent_asset_id, catalog) =
            if let Some(dependency_asset_id) = unresolved_dependency {
                let Some(dependency_catalog) = asset_manager
                    .catalog_by_asset_id
                    .get(&dependency_asset_id)
                    .cloned()
                else {
                    break;
                };
                (dependency_asset_id, Some(next_asset_id), dependency_catalog)
            } else {
                (next_asset_id, None, catalog)
            };
        if let Some(parent_asset_id) = parent_asset_id.clone() {
            fetch_state
                .pending_parent_asset_ids
                .insert(asset_id.clone(), parent_asset_id.clone());
            info!(
                "runtime asset download queued: asset_id={} dependency_for={} relative_cache_path={}",
                asset_id, parent_asset_id, catalog.relative_cache_path
            );
        } else {
            info!(
                "runtime asset download queued: asset_id={} relative_cache_path={}",
                asset_id, catalog.relative_cache_path
            );
        }
        let url = if catalog.url.starts_with("http://") || catalog.url.starts_with("https://") {
            catalog.url.clone()
        } else {
            format!("{}{}", session.gateway_url, catalog.url)
        };
        fetch_state.in_flight_asset_ids.insert(asset_id.clone());
        perf.fetches_queued = perf.fetches_queued.saturating_add(1);
        let access_token = access_token.clone();
        let gateway_http = *gateway_http;
        fetch_state.pending_fetches.push(RuntimeAssetFetchTask {
            asset_id: asset_id.clone(),
            queued_at: Instant::now(),
            task: IoTaskPool::get().spawn(async move {
                let payload = (gateway_http.fetch_asset_bytes)(url, access_token).await?;
                let payload_sha = sha256_hex(&payload);
                if payload_sha != catalog.sha256_hex {
                    return Err(format!(
                        "runtime asset checksum mismatch asset_id={} expected={} got={}",
                        asset_id, catalog.sha256_hex, payload_sha
                    ));
                }
                Ok(RuntimeAssetFetchResult {
                    asset_id,
                    relative_cache_path: catalog.relative_cache_path,
                    content_type: catalog.content_type,
                    byte_len: payload.len() as u64,
                    asset_version: sidereal_asset_runtime::asset_version_from_sha256_hex(
                        &payload_sha,
                    ),
                    sha256_hex: payload_sha,
                    payload,
                })
            }),
        });
    }
    perf.pending_fetch_count = fetch_state.pending_fetches.len();
    perf.pending_persist_count = fetch_state.pending_persists.len();
    perf.cache_index_dirty = fetch_state.cache_index_dirty;
}

#[allow(clippy::too_many_arguments)]
pub(super) fn poll_runtime_asset_http_fetches_system(
    mut fetch_state: ResMut<'_, RuntimeAssetHttpFetchState>,
    mut perf: ResMut<'_, RuntimeAssetPerfCounters>,
    mut asset_manager: ResMut<'_, LocalAssetManager>,
    mut hot_reload: ResMut<'_, AssetCatalogHotReloadState>,
    mut session: ResMut<'_, ClientSession>,
    asset_root: Res<'_, AssetRootPath>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
    shader_assignments: Res<'_, shaders::RuntimeShaderAssignments>,
    mut shaders_assets: ResMut<'_, Assets<bevy::shader::Shader>>,
) {
    perf.fetch_poll_runs = perf.fetch_poll_runs.saturating_add(1);
    let poll_started_at = Instant::now();
    if fetch_state.pending_fetches.is_empty()
        && fetch_state.pending_persists.is_empty()
        && fetch_state.save_index_task.is_none()
    {
        perf.pending_fetch_count = 0;
        perf.pending_persist_count = 0;
        perf.cache_index_dirty = fetch_state.cache_index_dirty;
        perf.fetch_poll_last_ms = 0.0;
        return;
    }
    let mut completed_results = Vec::new();
    let mut task_index = 0usize;
    while task_index < fetch_state.pending_fetches.len() {
        let Some(result) = bevy::tasks::block_on(future::poll_once(
            &mut fetch_state.pending_fetches[task_index].task,
        )) else {
            task_index += 1;
            continue;
        };
        let completed = fetch_state.pending_fetches.swap_remove(task_index);
        completed_results.push((completed.asset_id, completed.queued_at, result));
    }

    for (queued_asset_id, queued_at, result) in completed_results {
        perf.fetches_completed = perf.fetches_completed.saturating_add(1);
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
                    let asset_root = asset_root.0.clone();
                    let cache_adapter = *cache_adapter;
                    fetch_state.pending_persists.push(RuntimeAssetPersistTask {
                        asset_id: result.asset_id.clone(),
                        queued_at: Instant::now(),
                        task: IoTaskPool::get().spawn(async move {
                            (cache_adapter.write_asset)(
                                asset_root,
                                result.relative_cache_path.clone(),
                                result.payload,
                            )
                            .await?;
                            Ok(RuntimeAssetPersistResult {
                                asset_id: result.asset_id,
                                relative_cache_path: result.relative_cache_path,
                                content_type: result.content_type,
                                byte_len: result.byte_len,
                                asset_version: result.asset_version,
                                sha256_hex: result.sha256_hex,
                            })
                        }),
                    });
                }
            }
            Err(err) => {
                warn!("runtime asset download failed: {}", err);
                session.status = format!("Asset download failed: {}", err);
                session.ui_dirty = true;
                fetch_state.in_flight_asset_ids.remove(&queued_asset_id);
                fetch_state
                    .pending_parent_asset_ids
                    .remove(&queued_asset_id);
            }
        }
        let _fetch_task_ms = queued_at.elapsed().as_secs_f64() * 1000.0;
    }

    let mut completed_persists = Vec::new();
    let mut persist_index = 0usize;
    while persist_index < fetch_state.pending_persists.len() {
        let Some(result) = bevy::tasks::block_on(future::poll_once(
            &mut fetch_state.pending_persists[persist_index].task,
        )) else {
            persist_index += 1;
            continue;
        };
        let completed = fetch_state.pending_persists.swap_remove(persist_index);
        completed_persists.push((completed.asset_id, completed.queued_at, result));
    }

    for (queued_asset_id, queued_at, result) in completed_persists {
        let persist_task_ms = queued_at.elapsed().as_secs_f64() * 1000.0;
        perf.persist_task_last_ms = persist_task_ms;
        perf.persist_task_max_ms = perf.persist_task_max_ms.max(persist_task_ms);
        match result {
            Ok(result) => {
                perf.persists_completed = perf.persists_completed.saturating_add(1);
                asset_manager.cache_index.by_asset_id.insert(
                    result.asset_id.clone(),
                    AssetCacheIndexRecord {
                        asset_version: result.asset_version,
                        sha256_hex: result.sha256_hex.clone(),
                    },
                );
                fetch_state.cache_index_dirty = true;
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
                fetch_state.in_flight_asset_ids.remove(&result.asset_id);
                fetch_state
                    .pending_parent_asset_ids
                    .remove(&result.asset_id);
            }
            Err(err) => {
                warn!("runtime asset cache write failed: {}", err);
                session.status = format!("Asset download failed: {}", err);
                session.ui_dirty = true;
                fetch_state.in_flight_asset_ids.remove(&queued_asset_id);
                fetch_state
                    .pending_parent_asset_ids
                    .remove(&queued_asset_id);
            }
        }
    }

    if let Some(save_index_task) = fetch_state.save_index_task.as_mut()
        && let Some(result) = bevy::tasks::block_on(future::poll_once(&mut save_index_task.task))
    {
        let save_index_ms = save_index_task.queued_at.elapsed().as_secs_f64() * 1000.0;
        perf.save_index_last_ms = save_index_ms;
        perf.save_index_max_ms = perf.save_index_max_ms.max(save_index_ms);
        perf.save_index_completions = perf.save_index_completions.saturating_add(1);
        fetch_state.save_index_task = None;
        if let Err(err) = result {
            warn!("runtime asset cache index save failed: {}", err);
            session.status = format!("Asset download failed: {}", err);
            session.ui_dirty = true;
            fetch_state.cache_index_dirty = true;
        }
    }

    if fetch_state.cache_index_dirty && fetch_state.save_index_task.is_none() {
        fetch_state.cache_index_dirty = false;
        let asset_root = asset_root.0.clone();
        let cache_index = asset_manager.cache_index.clone();
        let cache_adapter = *cache_adapter;
        perf.save_index_starts = perf.save_index_starts.saturating_add(1);
        fetch_state.save_index_task = Some(RuntimeAssetSaveIndexTask {
            queued_at: Instant::now(),
            task: IoTaskPool::get()
                .spawn(async move { (cache_adapter.save_index)(asset_root, cache_index).await }),
        });
    }
    perf.pending_fetch_count = fetch_state.pending_fetches.len();
    perf.pending_persist_count = fetch_state.pending_persists.len();
    perf.cache_index_dirty = fetch_state.cache_index_dirty;
    perf.fetch_poll_last_ms = poll_started_at.elapsed().as_secs_f64() * 1000.0;
    perf.fetch_poll_max_ms = perf.fetch_poll_max_ms.max(perf.fetch_poll_last_ms);
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
        RuntimeAssetDependencyDirtyState, RuntimeAssetDependencyState,
        mark_runtime_asset_dependency_state_dirty_system, next_runtime_asset_fetch_candidate,
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
        app.insert_resource(RuntimeAssetDependencyDirtyState::default());
        app.add_systems(
            Update,
            (
                mark_runtime_asset_dependency_state_dirty_system,
                sync_runtime_asset_dependency_state_system
                    .after(mark_runtime_asset_dependency_state_dirty_system),
            ),
        );

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
        app.insert_resource(RuntimeAssetDependencyDirtyState::default());
        app.add_systems(
            Update,
            (
                mark_runtime_asset_dependency_state_dirty_system,
                sync_runtime_asset_dependency_state_system
                    .after(mark_runtime_asset_dependency_state_dirty_system),
            ),
        );

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
