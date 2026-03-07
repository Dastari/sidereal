//! Asset cache and runtime HTTP asset fetch systems.

use super::app_state::ClientSession;
use super::components::{StreamedSpriteShaderAssetId, StreamedVisualAssetId};
use super::resources::AssetRootPath;
use super::shaders;
use bevy::log::warn;
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task, futures_lite::future};
use sidereal_asset_runtime::{
    AssetCacheIndex, AssetCacheIndexRecord, cache_index_path, save_cache_index, sha256_hex,
};
use sidereal_game::{
    FullscreenLayer, RuntimePostProcessStack, RuntimeRenderLayerDefinition, SizeM,
    SpriteShaderAssetId,
};
use std::collections::{HashMap, HashSet};

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
    pub url: String,
    pub relative_cache_path: String,
    pub content_type: String,
    pub _byte_len: u64,
    pub sha256_hex: String,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct LocalAssetManager {
    pub records_by_asset_id: HashMap<String, LocalAssetRecord>,
    pub catalog_by_asset_id: HashMap<String, RuntimeAssetCatalogRecord>,
    pub cache_index: AssetCacheIndex,
    pub cache_index_loaded: bool,
    pub bootstrap_manifest_seen: bool,
    pub bootstrap_phase_complete: bool,
    pub bootstrap_total_bytes: u64,
    pub bootstrap_ready_bytes: u64,
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

    pub fn cached_relative_path(&self, asset_id: &str) -> Option<&str> {
        self.records_by_asset_id
            .get(asset_id)
            .filter(|record| record.ready)
            .map(|record| record.relative_cache_path.as_str())
    }
}

#[derive(Debug, Resource, Default)]
pub(crate) struct RuntimeAssetNetIndicatorState {
    pub blinking_phase_s: f32,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct RuntimeAssetHttpFetchState {
    pending: Option<Task<Result<RuntimeAssetFetchResult, String>>>,
    in_flight_asset_ids: HashSet<String>,
    pub last_request_at_s: f64,
}

impl RuntimeAssetHttpFetchState {
    pub fn has_in_flight_fetch(&self) -> bool {
        !self.in_flight_asset_ids.is_empty()
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
}

pub(super) fn streamed_visual_asset_path(
    asset_id: &str,
    asset_manager: &LocalAssetManager,
) -> Option<String> {
    let relative = asset_manager.cached_relative_path(asset_id)?;
    if !(relative.ends_with(".png")
        || relative.ends_with(".jpg")
        || relative.ends_with(".jpeg")
        || relative.ends_with(".webp"))
    {
        return None;
    }
    Some(format!("data/cache_stream/{relative}"))
}

pub(super) fn streamed_svg_asset_path(
    asset_id: &str,
    asset_manager: &LocalAssetManager,
) -> Option<String> {
    let relative = asset_manager.cached_relative_path(asset_id)?;
    if !relative.ends_with(".svg") {
        return None;
    }
    Some(format!("data/cache_stream/{relative}"))
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

pub(super) fn read_png_dimensions(path: &std::path::Path) -> Option<UVec2> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 24 {
        return None;
    }
    const PNG_SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes[0..8] != PNG_SIG {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    Some(UVec2::new(width, height))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn queue_missing_catalog_assets_system(
    time: Res<'_, Time>,
    mut fetch_state: ResMut<'_, RuntimeAssetHttpFetchState>,
    asset_manager: Res<'_, LocalAssetManager>,
    asset_root: Res<'_, AssetRootPath>,
    session: Res<'_, ClientSession>,
    fullscreen_layers: Query<'_, '_, &'_ FullscreenLayer>,
    runtime_render_layers: Query<'_, '_, &'_ RuntimeRenderLayerDefinition>,
    runtime_post_process_stacks: Query<'_, '_, &'_ RuntimePostProcessStack>,
    sprite_shader_asset_ids: Query<'_, '_, &'_ SpriteShaderAssetId>,
    streamed_sprite_shader_asset_ids: Query<'_, '_, &'_ StreamedSpriteShaderAssetId>,
    streamed_visual_asset_ids: Query<'_, '_, &'_ StreamedVisualAssetId>,
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
    let mut candidate_asset_ids = std::collections::HashSet::<String>::new();
    for layer in &fullscreen_layers {
        if !layer.shader_asset_id.trim().is_empty() {
            candidate_asset_ids.insert(layer.shader_asset_id.clone());
        }
    }
    for layer in &runtime_render_layers {
        if !layer.shader_asset_id.trim().is_empty() {
            candidate_asset_ids.insert(layer.shader_asset_id.clone());
        }
        if let Some(params_asset_id) = layer.params_asset_id.as_ref()
            && !params_asset_id.trim().is_empty()
        {
            candidate_asset_ids.insert(params_asset_id.clone());
        }
        for binding in &layer.texture_bindings {
            if !binding.asset_id.trim().is_empty() {
                candidate_asset_ids.insert(binding.asset_id.clone());
            }
        }
    }
    for stack in &runtime_post_process_stacks {
        for pass in &stack.passes {
            if !pass.shader_asset_id.trim().is_empty() {
                candidate_asset_ids.insert(pass.shader_asset_id.clone());
            }
            if let Some(params_asset_id) = pass.params_asset_id.as_ref()
                && !params_asset_id.trim().is_empty()
            {
                candidate_asset_ids.insert(params_asset_id.clone());
            }
            for binding in &pass.texture_bindings {
                if !binding.asset_id.trim().is_empty() {
                    candidate_asset_ids.insert(binding.asset_id.clone());
                }
            }
        }
    }
    for sprite_shader_asset_id in &sprite_shader_asset_ids {
        if let Some(asset_id) = sprite_shader_asset_id.0.as_ref()
            && !asset_id.trim().is_empty()
        {
            candidate_asset_ids.insert(asset_id.clone());
        }
    }
    for streamed in &streamed_sprite_shader_asset_ids {
        if !streamed.0.trim().is_empty() {
            candidate_asset_ids.insert(streamed.0.clone());
        }
    }
    for visual in &streamed_visual_asset_ids {
        if !visual.0.trim().is_empty() {
            candidate_asset_ids.insert(visual.0.clone());
        }
    }
    let Some(next_asset_id) = candidate_asset_ids
        .into_iter()
        .filter(|asset_id| {
            !asset_present_in_cache_or_source(asset_id, &asset_manager, &asset_root.0)
        })
        .filter(|asset_id| !fetch_state.in_flight_asset_ids.contains(asset_id))
        .find(|asset_id| asset_manager.catalog_by_asset_id.contains_key(asset_id))
    else {
        return;
    };
    let Some(catalog) = asset_manager
        .catalog_by_asset_id
        .get(&next_asset_id)
        .cloned()
    else {
        return;
    };
    let url = if catalog.url.starts_with("http://") || catalog.url.starts_with("https://") {
        catalog.url.clone()
    } else {
        format!("{}{}", session.gateway_url, catalog.url)
    };
    fetch_state
        .in_flight_asset_ids
        .insert(next_asset_id.clone());
    fetch_state.last_request_at_s = now;

    let access_token = access_token.clone();
    fetch_state.pending = Some(IoTaskPool::get().spawn(async move {
        (|| -> Result<RuntimeAssetFetchResult, String> {
            let client = reqwest::blocking::Client::new();
            let response_bytes = client
                .get(url)
                .bearer_auth(access_token)
                .send()
                .map_err(|err| err.to_string())?
                .error_for_status()
                .map_err(|err| err.to_string())?
                .bytes()
                .map_err(|err| err.to_string())?;
            let payload = response_bytes.to_vec();
            let payload_sha = sha256_hex(&payload);
            if payload_sha != catalog.sha256_hex {
                return Err(format!(
                    "runtime asset checksum mismatch asset_id={} expected={} got={}",
                    next_asset_id, catalog.sha256_hex, payload_sha
                ));
            }
            Ok(RuntimeAssetFetchResult {
                asset_id: next_asset_id,
                relative_cache_path: catalog.relative_cache_path,
                content_type: catalog.content_type,
                byte_len: payload.len() as u64,
                asset_version: sidereal_asset_runtime::asset_version_from_sha256_hex(&payload_sha),
                sha256_hex: payload_sha,
                payload,
            })
        })()
    }));
}

pub(super) fn poll_runtime_asset_http_fetches_system(
    mut fetch_state: ResMut<'_, RuntimeAssetHttpFetchState>,
    mut asset_manager: ResMut<'_, LocalAssetManager>,
    mut session: ResMut<'_, ClientSession>,
    asset_root: Res<'_, AssetRootPath>,
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
            let target = std::path::PathBuf::from(&asset_root.0)
                .join("data/cache_stream")
                .join(&result.relative_cache_path);
            let write_result = (|| -> Result<(), String> {
                let payload_sha = sha256_hex(&result.payload);
                if payload_sha != result.sha256_hex {
                    return Err(format!(
                        "runtime asset checksum mismatch asset_id={} expected={} got={}",
                        result.asset_id, result.sha256_hex, payload_sha
                    ));
                }
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
                }
                std::fs::write(&target, &result.payload).map_err(|err| err.to_string())?;
                Ok(())
            })();
            match write_result {
                Ok(()) => {
                    asset_manager.cache_index.by_asset_id.insert(
                        result.asset_id.clone(),
                        AssetCacheIndexRecord {
                            asset_version: result.asset_version,
                            sha256_hex: result.sha256_hex.clone(),
                        },
                    );
                    let index_path = cache_index_path(&asset_root.0);
                    if let Err(err) = save_cache_index(&index_path, &asset_manager.cache_index) {
                        warn!("failed saving cache index: {}", err);
                    }
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
                    session.status = format!("Asset downloaded: {}", result.asset_id);
                    session.ui_dirty = true;
                    if shaders::shader_materials_enabled()
                        && result.relative_cache_path.ends_with(".wgsl")
                    {
                        shaders::reload_streamed_shaders(
                            &mut shaders_assets,
                            &asset_root.0,
                            &asset_manager,
                        );
                    }
                }
                Err(err) => {
                    warn!("runtime asset download failed: {}", err);
                    session.status = format!("Asset download failed: {}", err);
                    session.ui_dirty = true;
                }
            }
            fetch_state.in_flight_asset_ids.remove(&result.asset_id);
        }
        Err(err) => {
            warn!("runtime asset download failed: {}", err);
            session.status = format!("Asset download failed: {}", err);
            session.ui_dirty = true;
            let maybe_id = fetch_state.in_flight_asset_ids.iter().next().cloned();
            if let Some(asset_id) = maybe_id {
                fetch_state.in_flight_asset_ids.remove(&asset_id);
            }
        }
    }
}

fn asset_present_in_cache_or_source(
    asset_id: &str,
    asset_manager: &LocalAssetManager,
    asset_root: &str,
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
    let rooted_stream_path = std::path::PathBuf::from(asset_root)
        .join("data/cache_stream")
        .join(relative_cache_path);
    if !rooted_stream_path.is_file() {
        return false;
    }
    let Ok(bytes) = std::fs::read(rooted_stream_path) else {
        return false;
    };
    sha256_hex(&bytes) == catalog.sha256_hex
}
