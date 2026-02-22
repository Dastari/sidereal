use bevy::log::{error, info, warn};
use bevy::prelude::*;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{
    MessageReceiver, NetworkTarget, RemoteId, Server, ServerMultiMessageSender,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use sidereal_asset_runtime::{
    AssetCatalogEntry, asset_version_from_sha256_hex, default_streamable_asset_sources,
    expand_required_assets, gltf_dependency_relative_paths, sha256_hex,
};
use sidereal_net::{
    AssetAckMessage, AssetRequestMessage, AssetStreamChunkMessage, AssetStreamManifestMessage,
    ControlChannel,
};

use crate::{
    AssetDependencyMap, AssetStreamServerState, AuthenticatedClientBindings, RawServer,
    default_corvette_asset_id, default_space_background_shader_asset_id,
    default_starfield_shader_asset_id,
};

const ASSET_STREAM_CHUNK_BYTES: usize = 1024;

fn always_required_stream_asset_ids() -> [&'static str; 3] {
    [
        default_corvette_asset_id(),
        default_starfield_shader_asset_id(),
        default_space_background_shader_asset_id(),
    ]
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

pub fn receive_client_asset_requests(
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
    for (client_entity, remote_id, mut receiver) in &mut receivers {
        for message in receiver.receive() {
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
            info!(
                "replication received asset requests remote={:?} player={} count={}",
                remote_id.0, bound_player, accepted
            );
        }
    }
}

pub fn receive_client_asset_acks(
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
    for (client_entity, remote_id, mut receiver) in &mut receivers {
        for message in receiver.receive() {
            let Some(bound_player) = bindings.by_client_entity.get(&client_entity) else {
                continue;
            };
            stream_state
                .acked_assets_by_remote
                .entry(remote_id.0)
                .or_default()
                .insert(message.asset_id.clone(), message.asset_version);
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

#[allow(clippy::too_many_arguments)]
pub fn stream_bootstrap_assets_to_authenticated_clients(
    server_query: Query<'_, '_, &'_ Server, With<RawServer>>,
    mut sender: ServerMultiMessageSender<'_, '_, With<lightyear::prelude::client::Connected>>,
    clients: Query<'_, '_, (Entity, &'_ RemoteId), With<ClientOf>>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    dependency_map: Res<'_, AssetDependencyMap>,
    mut stream_state: ResMut<'_, AssetStreamServerState>,
) {
    let Ok(server) = server_query.single() else {
        return;
    };

    for (client_entity, remote_id) in &clients {
        let Some(_bound_player_entity_id) = bindings.by_client_entity.get(&client_entity) else {
            continue;
        };
        let mut required_asset_ids = HashSet::<String>::new();
        required_asset_ids.extend(
            always_required_stream_asset_ids()
                .iter()
                .map(|asset_id| (*asset_id).to_string()),
        );
        let source_by_asset_id = default_streamable_asset_sources()
            .iter()
            .map(|source| (source.asset_id, source))
            .collect::<HashMap<_, _>>();
        let asset_id_by_relative_path = default_streamable_asset_sources()
            .iter()
            .map(|source| (source.relative_cache_path, source.asset_id))
            .collect::<HashMap<_, _>>();
        let mut discovered_dependency_asset_ids = HashSet::<String>::new();
        for asset_id in &required_asset_ids {
            let Some(source) = source_by_asset_id.get(asset_id.as_str()) else {
                continue;
            };
            if !source.relative_cache_path.ends_with(".gltf") {
                continue;
            }
            let Some(gltf_bytes) = load_asset_bytes(source.relative_cache_path) else {
                continue;
            };
            for dep_path in gltf_dependency_relative_paths(source.relative_cache_path, &gltf_bytes)
            {
                if let Some(dep_asset_id) = asset_id_by_relative_path.get(dep_path.as_str()) {
                    discovered_dependency_asset_ids.insert((*dep_asset_id).to_string());
                }
            }
        }
        required_asset_ids.extend(discovered_dependency_asset_ids);
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

        let mut payloads = Vec::<(AssetCatalogEntry, Vec<u8>)>::new();
        for asset in default_streamable_asset_sources()
            .iter()
            .filter(|asset| pending_asset_ids.contains(asset.asset_id))
        {
            let Some(bytes) = load_asset_bytes(asset.relative_cache_path) else {
                warn!(
                    "replication asset stream skipping missing asset {} ({})",
                    asset.asset_id, asset.relative_cache_path
                );
                continue;
            };
            let chunk_count = bytes.len().div_ceil(ASSET_STREAM_CHUNK_BYTES) as u32;
            let sha256 = sha256_hex(&bytes);
            let asset_version = asset_version_from_sha256_hex(&sha256);
            payloads.push((
                AssetCatalogEntry {
                    asset_id: asset.asset_id.to_string(),
                    relative_cache_path: asset.relative_cache_path.to_string(),
                    content_type: asset.content_type.to_string(),
                    byte_len: bytes.len() as u64,
                    chunk_count,
                    asset_version,
                    sha256_hex: sha256,
                },
                bytes,
            ));
        }

        if payloads.is_empty() {
            continue;
        }

        let manifest = AssetStreamManifestMessage {
            assets: payloads.iter().map(|(entry, _)| entry.clone()).collect(),
        };
        let streamed_asset_ids = payloads
            .iter()
            .map(|(entry, _)| entry.asset_id.clone())
            .collect::<Vec<_>>();
        let target = NetworkTarget::Single(remote_id.0);
        if let Err(err) =
            sender.send::<AssetStreamManifestMessage, ControlChannel>(&manifest, server, &target)
        {
            error!("replication failed sending asset manifest: {}", err);
            continue;
        }

        let mut send_failed = false;
        for (entry, bytes) in payloads {
            for (chunk_index, chunk) in bytes.chunks(ASSET_STREAM_CHUNK_BYTES).enumerate() {
                let message = AssetStreamChunkMessage {
                    asset_id: entry.asset_id.clone(),
                    relative_cache_path: entry.relative_cache_path.clone(),
                    chunk_index: chunk_index as u32,
                    chunk_count: entry.chunk_count,
                    bytes: chunk.to_vec(),
                };
                if let Err(err) = sender
                    .send::<AssetStreamChunkMessage, ControlChannel>(&message, server, &target)
                {
                    error!("replication failed sending asset chunk: {}", err);
                    send_failed = true;
                    break;
                }
            }
            if send_failed {
                break;
            }
        }
        if !send_failed {
            let sent_snapshot = {
                let sent_asset_ids = stream_state
                    .sent_asset_ids_by_remote
                    .entry(remote_id.0)
                    .or_default();
                for asset_id in streamed_asset_ids {
                    sent_asset_ids.insert(asset_id);
                }
                sent_asset_ids.clone()
            };
            if let Some(pending_requests) = stream_state
                .pending_requested_asset_ids_by_remote
                .get_mut(&remote_id.0)
            {
                pending_requests.retain(|asset_id| !sent_snapshot.contains(asset_id));
            }
            info!(
                "replication streamed bootstrap assets to remote={:?} assets_total_sent={}",
                remote_id.0,
                sent_snapshot.len()
            );
        }
    }
}
