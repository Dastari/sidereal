//! Asset stream resources and client asset streaming systems.

use bevy::log::{info, warn};
use bevy::prelude::*;
use lightyear::prelude::client::{Client, Connected};
use lightyear::prelude::{MessageReceiver, MessageSender};
use sidereal_asset_runtime::{
    AssetCacheIndex, AssetCacheIndexRecord, cache_index_path, load_cache_index, save_cache_index,
    sha256_hex,
};
use sidereal_game::{
    FullscreenLayer, SizeM, SpriteShaderAssetId, default_space_background_shader_asset_id,
    default_starfield_shader_asset_id,
};
use sidereal_net::{
    AssetAckMessage, AssetChannel, AssetRequestMessage, AssetStreamChunkMessage,
    AssetStreamManifestMessage, RequestedAsset,
};
use std::collections::HashMap;

use super::app_state::ClientSession;
use super::components::StreamedSpriteShaderAssetId;
use super::resources::{AssetRootPath, BootstrapWatchdogState};
use super::shaders;

#[derive(Debug, Clone, Default)]
pub(crate) struct PendingAssetChunks {
    pub relative_cache_path: String,
    pub byte_len: u64,
    pub chunk_count: u32,
    pub chunks: Vec<Option<Vec<u8>>>,
    pub counts_toward_bootstrap: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LocalAssetRecord {
    pub relative_cache_path: String,
    pub _content_type: String,
    pub _byte_len: u64,
    pub _chunk_count: u32,
    pub asset_version: u64,
    pub sha256_hex: String,
    pub ready: bool,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct LocalAssetManager {
    pub records_by_asset_id: HashMap<String, LocalAssetRecord>,
    pub pending_assets: HashMap<String, PendingAssetChunks>,
    pub requested_asset_ids: std::collections::HashSet<String>,
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

    pub fn should_show_runtime_stream_indicator(&self) -> bool {
        self.bootstrap_complete() && !self.pending_assets.is_empty()
    }

    pub fn is_cache_fresh(&self, asset_id: &str, asset_version: u64, sha256_hex: &str) -> bool {
        self.cache_index
            .by_asset_id
            .get(asset_id)
            .is_some_and(|entry| {
                entry.asset_version == asset_version && entry.sha256_hex == sha256_hex
            })
    }
}

#[derive(Debug, Resource, Default)]
pub(crate) struct RuntimeAssetStreamIndicatorState {
    pub blinking_phase_s: f32,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct CriticalAssetRequestState {
    pub last_request_at_s: f64,
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

pub(super) fn streamed_sprite_shader_path(
    asset_id: &str,
    asset_manager: &LocalAssetManager,
) -> Option<String> {
    let relative = asset_manager.cached_relative_path(asset_id)?;
    if !relative.ends_with(".wgsl") {
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn receive_lightyear_asset_stream_messages(
    mut manifest_receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<AssetStreamManifestMessage>,
        (With<Client>, With<Connected>),
    >,
    mut chunk_receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<AssetStreamChunkMessage>,
        (With<Client>, With<Connected>),
    >,
    mut request_senders: Query<
        '_,
        '_,
        &mut MessageSender<AssetRequestMessage>,
        (With<Client>, With<Connected>),
    >,
    mut ack_senders: Query<
        '_,
        '_,
        &mut MessageSender<AssetAckMessage>,
        (With<Client>, With<Connected>),
    >,
    mut asset_manager: ResMut<'_, LocalAssetManager>,
    mut session: ResMut<'_, ClientSession>,
    asset_root: Res<'_, AssetRootPath>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
    _asset_server: Res<'_, AssetServer>,
    mut shaders_assets: ResMut<'_, Assets<bevy::shader::Shader>>,
) {
    for mut receiver in &mut manifest_receivers {
        for manifest in receiver.receive() {
            watchdog.asset_manifest_seen = true;
            info!(
                "client received asset manifest entries={}",
                manifest.assets.len()
            );
            let is_bootstrap_manifest = !asset_manager.bootstrap_phase_complete;
            if !asset_manager.bootstrap_manifest_seen {
                asset_manager.bootstrap_manifest_seen = true;
                asset_manager.bootstrap_total_bytes = 0;
                asset_manager.bootstrap_ready_bytes = 0;
            }
            if !asset_manager.cache_index_loaded {
                let index_path = cache_index_path(&asset_root.0);
                asset_manager.cache_index = load_cache_index(&index_path).unwrap_or_default();
                asset_manager.cache_index_loaded = true;
            }
            let mut requested_assets = Vec::<RequestedAsset>::new();
            for asset in &manifest.assets {
                let target = std::path::PathBuf::from(&asset_root.0)
                    .join("data/cache_stream")
                    .join(&asset.relative_cache_path);
                let has_cached_file = std::fs::metadata(&target)
                    .ok()
                    .is_some_and(|meta| meta.len() > 0);
                let already_cached = has_cached_file
                    && asset_manager.is_cache_fresh(
                        &asset.asset_id,
                        asset.asset_version,
                        &asset.sha256_hex,
                    );
                let mut record = LocalAssetRecord {
                    relative_cache_path: asset.relative_cache_path.clone(),
                    _content_type: asset.content_type.clone(),
                    _byte_len: asset.byte_len,
                    _chunk_count: asset.chunk_count,
                    asset_version: asset.asset_version,
                    sha256_hex: asset.sha256_hex.clone(),
                    ready: already_cached,
                };
                if already_cached {
                    if is_bootstrap_manifest {
                        asset_manager.bootstrap_ready_bytes = asset_manager
                            .bootstrap_ready_bytes
                            .saturating_add(asset.byte_len);
                    }
                } else {
                    let chunk_slots = vec![None; asset.chunk_count as usize];
                    asset_manager.pending_assets.insert(
                        asset.asset_id.clone(),
                        PendingAssetChunks {
                            relative_cache_path: asset.relative_cache_path.clone(),
                            byte_len: asset.byte_len,
                            chunk_count: asset.chunk_count,
                            chunks: chunk_slots,
                            counts_toward_bootstrap: is_bootstrap_manifest,
                        },
                    );
                    record.ready = false;
                    if asset_manager
                        .requested_asset_ids
                        .insert(asset.asset_id.clone())
                    {
                        requested_assets.push(RequestedAsset {
                            asset_id: asset.asset_id.clone(),
                            known_asset_version: asset_manager
                                .cache_index
                                .by_asset_id
                                .get(&asset.asset_id)
                                .map(|entry| entry.asset_version),
                            known_sha256_hex: asset_manager
                                .cache_index
                                .by_asset_id
                                .get(&asset.asset_id)
                                .map(|entry| entry.sha256_hex.clone()),
                        });
                    }
                }
                if is_bootstrap_manifest {
                    asset_manager.bootstrap_total_bytes = asset_manager
                        .bootstrap_total_bytes
                        .saturating_add(asset.byte_len);
                }
                asset_manager
                    .records_by_asset_id
                    .insert(asset.asset_id.clone(), record);
            }
            session.status = format!(
                "Asset stream manifest received ({} assets).",
                manifest.assets.len()
            );
            if !requested_assets.is_empty() {
                let request_message = AssetRequestMessage {
                    requests: requested_assets,
                };
                for mut sender in &mut request_senders {
                    sender.send::<AssetChannel>(request_message.clone());
                }
            }
        }
    }

    for mut receiver in &mut chunk_receivers {
        for chunk in receiver.receive() {
            let mut completed_payload: Option<(String, Vec<u8>)> = None;
            if let Some(pending) = asset_manager.pending_assets.get_mut(&chunk.asset_id) {
                if pending.chunk_count != chunk.chunk_count {
                    continue;
                }
                let idx = chunk.chunk_index as usize;
                if idx >= pending.chunks.len() {
                    continue;
                }
                pending.chunks[idx] = Some(chunk.bytes.clone());
                if pending.chunks.iter().all(Option::is_some) {
                    let mut payload = Vec::<u8>::new();
                    for bytes in pending.chunks.iter().flatten() {
                        payload.extend_from_slice(bytes);
                    }
                    completed_payload = Some((pending.relative_cache_path.clone(), payload));
                }
            } else {
                continue;
            }

            if let Some((relative_cache_path, payload)) = completed_payload {
                let target = std::path::PathBuf::from(&asset_root.0)
                    .join("data/cache_stream")
                    .join(&relative_cache_path);
                if let Some(parent) = target.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&target, &payload);
                session.status = format!("Asset streamed: {}", relative_cache_path);
                let mut ack_to_send: Option<AssetAckMessage> = None;
                if let Some(record) = asset_manager.records_by_asset_id.get_mut(&chunk.asset_id) {
                    let payload_sha = sha256_hex(&payload);
                    if payload_sha != record.sha256_hex {
                        warn!(
                            "client asset checksum mismatch asset_id={} expected={} got={}",
                            chunk.asset_id, record.sha256_hex, payload_sha
                        );
                        continue;
                    }
                    record.ready = true;
                    ack_to_send = Some(AssetAckMessage {
                        asset_id: chunk.asset_id.clone(),
                        asset_version: record.asset_version,
                        sha256_hex: record.sha256_hex.clone(),
                    });
                }
                if let Some(ack) = ack_to_send {
                    asset_manager.cache_index.by_asset_id.insert(
                        ack.asset_id.clone(),
                        AssetCacheIndexRecord {
                            asset_version: ack.asset_version,
                            sha256_hex: ack.sha256_hex.clone(),
                        },
                    );
                    let index_path = cache_index_path(&asset_root.0);
                    let _ = save_cache_index(&index_path, &asset_manager.cache_index);
                    for mut sender in &mut ack_senders {
                        sender.send::<AssetChannel>(ack.clone());
                    }
                }
                if let Some(pending) = asset_manager.pending_assets.remove(&chunk.asset_id)
                    && pending.counts_toward_bootstrap
                {
                    asset_manager.bootstrap_ready_bytes = asset_manager
                        .bootstrap_ready_bytes
                        .saturating_add(pending.byte_len);
                }
                asset_manager.requested_asset_ids.remove(&chunk.asset_id);
                if shaders::shader_materials_enabled()
                    && matches!(
                        chunk.asset_id.as_str(),
                        id if id == default_starfield_shader_asset_id()
                            || id == default_space_background_shader_asset_id()
                            || id == "sprite_pixel_effect_wgsl"
                            || id == "thruster_plume_wgsl"
                            || id == "tactical_map_overlay_wgsl"
                    )
                {
                    shaders::reload_streamed_shaders(&mut shaders_assets, &asset_root.0);
                }
            }
        }
    }
    if asset_manager.bootstrap_manifest_seen
        && !asset_manager.bootstrap_phase_complete
        && asset_manager
            .pending_assets
            .values()
            .all(|pending| !pending.counts_toward_bootstrap)
    {
        info!(
            "client bootstrap asset phase complete (ready_bytes={} total_bytes={})",
            asset_manager.bootstrap_ready_bytes, asset_manager.bootstrap_total_bytes
        );
        asset_manager.bootstrap_phase_complete = true;
    }
}

pub(super) fn ensure_critical_assets_available_system(
    time: Res<'_, Time>,
    mut request_senders: Query<
        '_,
        '_,
        &mut MessageSender<AssetRequestMessage>,
        (With<Client>, With<Connected>),
    >,
    mut request_state: ResMut<'_, CriticalAssetRequestState>,
    asset_manager: Res<'_, LocalAssetManager>,
    asset_root: Res<'_, AssetRootPath>,
    fullscreen_layers: Query<'_, '_, &'_ FullscreenLayer>,
    sprite_shader_asset_ids: Query<'_, '_, &'_ SpriteShaderAssetId>,
    streamed_sprite_shader_asset_ids: Query<'_, '_, &'_ StreamedSpriteShaderAssetId>,
) {
    if request_senders.is_empty() {
        return;
    }
    let mut critical_asset_ids = std::collections::HashSet::<String>::new();
    critical_asset_ids.insert(default_starfield_shader_asset_id().to_string());
    critical_asset_ids.insert(default_space_background_shader_asset_id().to_string());
    for asset_id in KNOWN_RUNTIME_SHADER_ASSET_IDS {
        critical_asset_ids.insert((*asset_id).to_string());
    }
    for layer in &fullscreen_layers {
        if !layer.shader_asset_id.trim().is_empty() {
            critical_asset_ids.insert(layer.shader_asset_id.clone());
        }
    }
    for sprite_shader_asset_id in &sprite_shader_asset_ids {
        if let Some(asset_id) = sprite_shader_asset_id.0.as_ref()
            && !asset_id.trim().is_empty()
        {
            critical_asset_ids.insert(asset_id.clone());
        }
    }
    for streamed in &streamed_sprite_shader_asset_ids {
        if !streamed.0.trim().is_empty() {
            critical_asset_ids.insert(streamed.0.clone());
        }
    }
    let now = time.elapsed_secs_f64();
    let mut requests = Vec::new();

    // Retry any manifest-pending assets periodically. This covers startup races where
    // the manifest arrives before transport message senders are fully wired.
    for asset_id in asset_manager.pending_assets.keys() {
        let known = asset_manager.cache_index.by_asset_id.get(asset_id);
        requests.push(RequestedAsset {
            asset_id: asset_id.clone(),
            known_asset_version: known.map(|entry| entry.asset_version),
            known_sha256_hex: known.map(|entry| entry.sha256_hex.clone()),
        });
    }

    let mut missing_critical = Vec::new();
    for asset_id in critical_asset_ids {
        if !asset_present_on_disk(&asset_id, &asset_manager, &asset_root.0) {
            missing_critical.push(asset_id);
        }
    }
    if requests.is_empty() && missing_critical.is_empty() {
        return;
    }
    if now - request_state.last_request_at_s < 2.0 {
        return;
    }
    request_state.last_request_at_s = now;
    for asset_id in missing_critical {
        let known = asset_manager.cache_index.by_asset_id.get(&asset_id);
        requests.push(RequestedAsset {
            asset_id,
            known_asset_version: known.map(|entry| entry.asset_version),
            known_sha256_hex: known.map(|entry| entry.sha256_hex.clone()),
        });
    }
    requests.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    requests.dedup_by(|a, b| a.asset_id == b.asset_id);
    let request_message = AssetRequestMessage { requests };
    for mut sender in &mut request_senders {
        sender.send::<AssetChannel>(request_message.clone());
    }
}

pub(super) fn asset_present_on_disk(
    asset_id: &str,
    asset_manager: &LocalAssetManager,
    asset_root: &str,
) -> bool {
    let Some(relative_cache_path) = asset_manager
        .records_by_asset_id
        .get(asset_id)
        .map(|record| record.relative_cache_path.as_str())
        .or_else(|| match asset_id {
            id if id == default_starfield_shader_asset_id() => Some("shaders/starfield.wgsl"),
            id if id == default_space_background_shader_asset_id() => {
                Some("shaders/space_background.wgsl")
            }
            "sprite_pixel_effect_wgsl" => Some("shaders/sprite_pixel_effect.wgsl"),
            "thruster_plume_wgsl" => Some("shaders/thruster_plume.wgsl"),
            "weapon_impact_spark_wgsl" => Some("shaders/weapon_impact_spark.wgsl"),
            "tactical_map_overlay_wgsl" => Some("shaders/tactical_map_overlay.wgsl"),
            _ => None,
        })
    else {
        return false;
    };
    let rooted_stream_path = std::path::PathBuf::from(asset_root)
        .join("data/cache_stream")
        .join(relative_cache_path);
    if rooted_stream_path.exists() {
        return true;
    }
    std::path::PathBuf::from(asset_root)
        .join(relative_cache_path)
        .exists()
}
const KNOWN_RUNTIME_SHADER_ASSET_IDS: &[&str] = &[
    "sprite_pixel_effect_wgsl",
    "thruster_plume_wgsl",
    "weapon_impact_spark_wgsl",
    "tactical_map_overlay_wgsl",
];
