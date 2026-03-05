use bevy::log::{error, info, warn};
use bevy::prelude::*;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::server::RawServer;
use lightyear::prelude::{
    MessageReceiver, NetworkTarget, RemoteId, Server, ServerMultiMessageSender,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::OnceLock;

use sidereal_asset_runtime::{
    AssetCatalogEntry, asset_version_from_sha256_hex, default_asset_dependencies,
    default_streamable_asset_sources, expand_required_assets, sha256_hex,
};
use sidereal_game::{
    default_corvette_asset_id, default_space_background_shader_asset_id,
    default_starfield_shader_asset_id,
};
use sidereal_net::{
    AssetAckMessage, AssetChannel, AssetRequestMessage, AssetStreamChunkMessage,
    AssetStreamManifestMessage,
};

use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::lifecycle::ClientLastActivity;
use crate::replication::scripting::{load_world_init_config, scripts_root_dir};

/// Chunk queued for paced sending to avoid UDP send-buffer overflow (EAGAIN).
pub(crate) struct PendingAssetChunk {
    pub(crate) asset_id: String,
    pub(crate) relative_cache_path: String,
    pub(crate) chunk_index: u32,
    pub(crate) chunk_count: u32,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Resource, Default)]
pub(crate) struct AssetStreamServerState {
    pub(crate) sent_asset_ids_by_remote: HashMap<lightyear::prelude::PeerId, HashSet<String>>,
    pub(crate) pending_requested_asset_ids_by_remote:
        HashMap<lightyear::prelude::PeerId, HashSet<String>>,
    pub(crate) acked_assets_by_remote: HashMap<lightyear::prelude::PeerId, HashMap<String, u64>>,
    /// Chunks to send per remote; drained at a fixed rate per frame to avoid EAGAIN.
    pub(crate) pending_chunks_by_remote:
        HashMap<lightyear::prelude::PeerId, std::collections::VecDeque<PendingAssetChunk>>,
    /// Consecutive chunk send failures per remote for backoff/drop behavior.
    pub(crate) chunk_send_failures_by_remote: HashMap<lightyear::prelude::PeerId, u32>,
    /// Remaining FixedUpdate ticks to skip chunk sends per remote after a send error.
    pub(crate) chunk_send_backoff_frames_by_remote: HashMap<lightyear::prelude::PeerId, u16>,
}

#[derive(Resource, Default)]
pub(crate) struct AssetDependencyMap {
    pub(crate) dependencies_by_asset_id: HashMap<String, Vec<String>>,
}

pub fn init_resources(app: &mut App) {
    app.insert_resource(AssetStreamServerState::default());
    app.insert_resource(StreamableAssetCache::default());
    app.insert_resource(AssetDependencyMap {
        dependencies_by_asset_id: default_asset_dependencies(),
    });
}

const ASSET_STREAM_CHUNK_BYTES: usize = 1024;
/// Max asset stream chunks sent per remote per frame to avoid UDP send buffer overflow (EAGAIN).
const ASSET_STREAM_CHUNKS_PER_FRAME: usize = 10;
/// Drop queued chunks for a remote after repeated send failures to avoid infinite resend storms.
const MAX_CONSECUTIVE_CHUNK_SEND_FAILURES: u32 = 8;
/// Initial FixedUpdate frames to wait before retrying chunk send after one failure.
const CHUNK_SEND_BACKOFF_INITIAL_FRAMES: u16 = 15;
/// Max FixedUpdate frames to wait between retries for a failing remote.
const CHUNK_SEND_BACKOFF_MAX_FRAMES: u16 = 300;

#[derive(Debug, Clone)]
struct CachedStreamAsset {
    entry: AssetCatalogEntry,
    bytes: Vec<u8>,
}

#[derive(Resource, Default)]
pub struct StreamableAssetCache {
    assets_by_id: HashMap<String, CachedStreamAsset>,
    always_required_asset_ids: HashSet<String>,
}

const CORE_RUNTIME_SHADER_ASSET_IDS: &[&str] = &[
    "sprite_pixel_effect_wgsl",
    "thruster_plume_wgsl",
    "weapon_impact_spark_wgsl",
    "tactical_map_overlay_wgsl",
];

fn always_required_stream_asset_ids() -> Vec<String> {
    let mut required = vec![default_corvette_asset_id().to_string()];
    let mut include_default_space = true;
    let mut include_default_starfield = true;

    let scripts_root = scripts_root_dir();
    match load_world_init_config(&scripts_root) {
        Ok(config) => {
            if config.space_background_shader_asset_id == default_space_background_shader_asset_id()
            {
                include_default_space = false;
            }
            if config.starfield_shader_asset_id == default_starfield_shader_asset_id() {
                include_default_starfield = false;
            }
            required.push(config.space_background_shader_asset_id);
            required.push(config.starfield_shader_asset_id);
            required.extend(config.additional_required_asset_ids);
        }
        Err(err) => {
            warn!(
                "replication asset cache init could not load world init script config; using default shader asset ids: {}",
                err
            );
        }
    }

    if include_default_space {
        required.push(default_space_background_shader_asset_id().to_string());
    }
    if include_default_starfield {
        required.push(default_starfield_shader_asset_id().to_string());
    }
    required.extend(
        CORE_RUNTIME_SHADER_ASSET_IDS
            .iter()
            .map(|asset_id| (*asset_id).to_string()),
    );
    required.sort();
    required.dedup();
    required
}

fn asset_root_dir() -> PathBuf {
    PathBuf::from(std::env::var("ASSET_ROOT").unwrap_or_else(|_| "./data".to_string()))
}

fn load_asset_bytes(relative_cache_path: &str) -> Option<Vec<u8>> {
    let asset_root = asset_root_dir();
    let rooted_path = asset_root.join(relative_cache_path);
    if let Ok(bytes) = std::fs::read(&rooted_path) {
        return Some(bytes);
    }
    let cache_path = asset_root.join("cache_stream").join(relative_cache_path);
    std::fs::read(cache_path).ok()
}

fn asset_stream_debug_logs_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SIDEREAL_DEBUG_ASSET_STREAM_LOGS")
            .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    })
}

pub fn initialize_asset_stream_cache(
    mut cache: ResMut<'_, StreamableAssetCache>,
    _dependency_map: ResMut<'_, AssetDependencyMap>,
) {
    cache.assets_by_id.clear();
    cache.always_required_asset_ids = always_required_stream_asset_ids()
        .into_iter()
        .collect::<HashSet<_>>();

    let streamable_sources = default_streamable_asset_sources();
    for source in streamable_sources {
        let Some(bytes) = load_asset_bytes(source.relative_cache_path) else {
            warn!(
                "replication asset cache init skipping missing asset {} ({})",
                source.asset_id, source.relative_cache_path
            );
            continue;
        };
        let chunk_count = bytes.len().div_ceil(ASSET_STREAM_CHUNK_BYTES) as u32;
        let sha256 = sha256_hex(&bytes);
        let asset_version = asset_version_from_sha256_hex(&sha256);
        cache.assets_by_id.insert(
            source.asset_id.to_string(),
            CachedStreamAsset {
                entry: AssetCatalogEntry {
                    asset_id: source.asset_id.to_string(),
                    relative_cache_path: source.relative_cache_path.to_string(),
                    content_type: source.content_type.to_string(),
                    byte_len: bytes.len() as u64,
                    chunk_count,
                    asset_version,
                    sha256_hex: sha256,
                },
                bytes,
            },
        );
    }

    info!(
        "replication cached streamable assets: loaded={} always_required={}",
        cache.assets_by_id.len(),
        cache.always_required_asset_ids.len()
    );
}

pub fn receive_client_asset_requests(
    time: Res<'_, Time<Real>>,
    mut last_activity: ResMut<'_, ClientLastActivity>,
    mut receivers: Query<
        '_,
        '_,
        (
            Entity,
            &'_ RemoteId,
            &'_ mut MessageReceiver<AssetRequestMessage>,
        ),
        With<ClientOf>,
    >,
    bindings: Res<'_, AuthenticatedClientBindings>,
    mut stream_state: ResMut<'_, AssetStreamServerState>,
) {
    let now_s = time.elapsed_secs_f64();
    for (client_entity, remote_id, mut receiver) in &mut receivers {
        for message in receiver.receive() {
            last_activity.0.insert(client_entity, now_s);
            let Some(bound_player) = bindings.by_client_entity.get(&client_entity) else {
                continue;
            };
            let pending = stream_state
                .pending_requested_asset_ids_by_remote
                .entry(remote_id.0)
                .or_default();
            let mut accepted = 0usize;
            for request in &message.requests {
                pending.insert(request.asset_id.clone());
                accepted += 1;
            }
            if asset_stream_debug_logs_enabled() {
                info!(
                    "replication received asset requests remote={:?} player={} count={}",
                    remote_id.0, bound_player, accepted
                );
            }
        }
    }
}

pub fn receive_client_asset_acks(
    time: Res<'_, Time<Real>>,
    mut last_activity: ResMut<'_, ClientLastActivity>,
    mut receivers: Query<
        '_,
        '_,
        (
            Entity,
            &'_ RemoteId,
            &'_ mut MessageReceiver<AssetAckMessage>,
        ),
        With<ClientOf>,
    >,
    bindings: Res<'_, AuthenticatedClientBindings>,
    mut stream_state: ResMut<'_, AssetStreamServerState>,
) {
    let now_s = time.elapsed_secs_f64();
    for (client_entity, remote_id, mut receiver) in &mut receivers {
        for message in receiver.receive() {
            last_activity.0.insert(client_entity, now_s);
            let Some(bound_player) = bindings.by_client_entity.get(&client_entity) else {
                continue;
            };
            stream_state
                .acked_assets_by_remote
                .entry(remote_id.0)
                .or_default()
                .insert(message.asset_id.clone(), message.asset_version);
            if asset_stream_debug_logs_enabled() {
                info!(
                    "replication received asset ack remote={:?} player={} asset_id={} version={} sha256={}",
                    remote_id.0,
                    bound_player,
                    message.asset_id,
                    message.asset_version,
                    message.sha256_hex
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn stream_bootstrap_assets_to_authenticated_clients(
    server_query: Query<'_, '_, &'_ Server, With<RawServer>>,
    mut sender: ServerMultiMessageSender<'_, '_, With<lightyear::prelude::client::Connected>>,
    clients: Query<'_, '_, (Entity, &'_ RemoteId), With<ClientOf>>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    dependency_map: Res<'_, AssetDependencyMap>,
    cache: Res<'_, StreamableAssetCache>,
    mut stream_state: ResMut<'_, AssetStreamServerState>,
) {
    let Ok(server) = server_query.single() else {
        return;
    };

    for (client_entity, remote_id) in &clients {
        let Some(_bound_player_entity_id) = bindings.by_client_entity.get(&client_entity) else {
            continue;
        };
        if stream_state
            .pending_chunks_by_remote
            .get(&remote_id.0)
            .is_some_and(|q| !q.is_empty())
        {
            continue;
        }
        let required_asset_ids = cache.always_required_asset_ids.clone();
        let required_asset_ids = expand_required_assets(
            &required_asset_ids,
            &dependency_map.dependencies_by_asset_id,
        );
        let requested_asset_ids = stream_state
            .pending_requested_asset_ids_by_remote
            .entry(remote_id.0)
            .or_default()
            .clone();
        let candidate_asset_ids = required_asset_ids
            .union(&requested_asset_ids)
            .cloned()
            .collect::<HashSet<_>>();
        if candidate_asset_ids.is_empty() {
            continue;
        }
        let pending_asset_ids = {
            let sent_asset_ids = stream_state
                .sent_asset_ids_by_remote
                .entry(remote_id.0)
                .or_default();
            candidate_asset_ids
                .into_iter()
                .filter(|asset_id| {
                    requested_asset_ids.contains(asset_id) || !sent_asset_ids.contains(asset_id)
                })
                .collect::<HashSet<_>>()
        };
        if pending_asset_ids.is_empty() {
            continue;
        }

        let mut payloads = Vec::<CachedStreamAsset>::new();
        for asset_id in pending_asset_ids {
            let Some(asset) = cache.assets_by_id.get(asset_id.as_str()) else {
                warn!(
                    "replication asset stream skipping uncached asset {}",
                    asset_id
                );
                continue;
            };
            payloads.push(asset.clone());
        }

        if payloads.is_empty() {
            continue;
        }

        let manifest = AssetStreamManifestMessage {
            assets: payloads.iter().map(|cached| cached.entry.clone()).collect(),
        };
        let streamed_asset_ids = payloads
            .iter()
            .map(|cached| cached.entry.asset_id.clone())
            .collect::<Vec<_>>();
        let target = NetworkTarget::Single(remote_id.0);
        if let Err(err) =
            sender.send::<AssetStreamManifestMessage, AssetChannel>(&manifest, server, &target)
        {
            error!("replication failed sending asset manifest: {}", err);
            continue;
        }

        let queue = stream_state
            .pending_chunks_by_remote
            .entry(remote_id.0)
            .or_default();
        for cached in payloads {
            for (chunk_index, chunk) in cached.bytes.chunks(ASSET_STREAM_CHUNK_BYTES).enumerate() {
                queue.push_back(PendingAssetChunk {
                    asset_id: cached.entry.asset_id.clone(),
                    relative_cache_path: cached.entry.relative_cache_path.clone(),
                    chunk_index: chunk_index as u32,
                    chunk_count: cached.entry.chunk_count,
                    bytes: chunk.to_vec(),
                });
            }
        }
        info!(
            "replication enqueued bootstrap asset chunks for remote={:?} assets={}",
            remote_id.0,
            streamed_asset_ids.len()
        );
    }
}

/// Sends a limited number of queued asset chunks per remote per frame to avoid UDP EAGAIN.
pub fn send_asset_stream_chunks_paced(
    server_query: Query<'_, '_, &'_ Server, With<RawServer>>,
    mut sender: ServerMultiMessageSender<'_, '_, With<lightyear::prelude::client::Connected>>,
    mut stream_state: ResMut<'_, AssetStreamServerState>,
    bindings: Res<'_, AuthenticatedClientBindings>,
) {
    use lightyear::prelude::PeerId;

    let Ok(server) = server_query.single() else {
        return;
    };

    let mut completed_assets: Vec<(PeerId, String)> = Vec::new();
    let mut remotes_to_drop: Vec<PeerId> = Vec::new();

    stream_state
        .pending_chunks_by_remote
        .retain(|remote_id, _| bindings.by_remote_id.contains_key(remote_id));
    stream_state
        .chunk_send_failures_by_remote
        .retain(|remote_id, _| bindings.by_remote_id.contains_key(remote_id));
    stream_state
        .chunk_send_backoff_frames_by_remote
        .retain(|remote_id, _| bindings.by_remote_id.contains_key(remote_id));

    let remote_ids = stream_state
        .pending_chunks_by_remote
        .keys()
        .copied()
        .collect::<Vec<_>>();
    for remote_id in remote_ids {
        if let Some(backoff_frames) = stream_state
            .chunk_send_backoff_frames_by_remote
            .get_mut(&remote_id)
            && *backoff_frames > 0
        {
            *backoff_frames -= 1;
            continue;
        }
        let target = NetworkTarget::Single(remote_id);
        for _ in 0..ASSET_STREAM_CHUNKS_PER_FRAME {
            let Some(pending) = stream_state
                .pending_chunks_by_remote
                .get_mut(&remote_id)
                .and_then(|queue| queue.pop_front())
            else {
                break;
            };
            let message = AssetStreamChunkMessage {
                asset_id: pending.asset_id.clone(),
                relative_cache_path: pending.relative_cache_path.clone(),
                chunk_index: pending.chunk_index,
                chunk_count: pending.chunk_count,
                bytes: pending.bytes,
            };
            if let Err(err) =
                sender.send::<AssetStreamChunkMessage, AssetChannel>(&message, server, &target)
            {
                let failures = {
                    let entry = stream_state
                        .chunk_send_failures_by_remote
                        .entry(remote_id)
                        .or_default();
                    *entry = entry.saturating_add(1);
                    *entry
                };
                let next_backoff = stream_state
                    .chunk_send_backoff_frames_by_remote
                    .get(&remote_id)
                    .copied()
                    .unwrap_or(CHUNK_SEND_BACKOFF_INITIAL_FRAMES);
                let doubled = next_backoff.saturating_mul(2);
                stream_state.chunk_send_backoff_frames_by_remote.insert(
                    remote_id,
                    doubled.clamp(
                        CHUNK_SEND_BACKOFF_INITIAL_FRAMES,
                        CHUNK_SEND_BACKOFF_MAX_FRAMES,
                    ),
                );
                if failures == 1 || (failures % 4 == 0) {
                    warn!(
                        "replication failed sending asset chunk remote={:?} failures={} err={}",
                        remote_id, failures, err
                    );
                }
                if let Some(queue) = stream_state.pending_chunks_by_remote.get_mut(&remote_id) {
                    queue.push_front(PendingAssetChunk {
                        asset_id: pending.asset_id.clone(),
                        relative_cache_path: pending.relative_cache_path.clone(),
                        chunk_index: pending.chunk_index,
                        chunk_count: pending.chunk_count,
                        bytes: message.bytes,
                    });
                }
                if failures >= MAX_CONSECUTIVE_CHUNK_SEND_FAILURES {
                    warn!(
                        "replication dropping queued asset chunks for remote={:?} after {} consecutive send failures",
                        remote_id, failures
                    );
                    remotes_to_drop.push(remote_id);
                }
                break;
            }
            stream_state
                .chunk_send_failures_by_remote
                .remove(&remote_id);
            stream_state
                .chunk_send_backoff_frames_by_remote
                .remove(&remote_id);
            let is_last_chunk = pending.chunk_index + 1 >= pending.chunk_count;
            if is_last_chunk {
                completed_assets.push((remote_id, pending.asset_id));
            }
        }
    }

    for (remote_id, asset_id) in completed_assets {
        stream_state
            .sent_asset_ids_by_remote
            .entry(remote_id)
            .or_default()
            .insert(asset_id.clone());
        if let Some(pending_requests) = stream_state
            .pending_requested_asset_ids_by_remote
            .get_mut(&remote_id)
        {
            pending_requests.remove(&asset_id);
        }
    }

    for remote_id in remotes_to_drop {
        stream_state.pending_chunks_by_remote.remove(&remote_id);
        stream_state
            .pending_requested_asset_ids_by_remote
            .remove(&remote_id);
        stream_state
            .chunk_send_failures_by_remote
            .remove(&remote_id);
        stream_state
            .chunk_send_backoff_frames_by_remote
            .remove(&remote_id);
    }
}
