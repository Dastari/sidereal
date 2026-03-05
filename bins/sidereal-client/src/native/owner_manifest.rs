use bevy::log::warn;
use bevy::prelude::*;
use lightyear::prelude::MessageReceiver;
use lightyear::prelude::client::{Client, Connected};
use sidereal_net::{
    PlayerEntityId, ServerOwnerAssetManifestDeltaMessage, ServerOwnerAssetManifestSnapshotMessage,
};

use super::app_state::ClientSession;
use super::resources::OwnedAssetManifestCache;

const MISMATCH_LOG_INTERVAL_S: f64 = 1.0;

fn local_player_canonical_id(session: &ClientSession) -> Option<String> {
    session
        .player_entity_id
        .as_deref()
        .and_then(PlayerEntityId::parse)
        .map(|id| id.canonical_wire_id())
}

fn apply_owner_manifest_snapshot(
    cache: &mut OwnedAssetManifestCache,
    local_player_id: &str,
    message: &ServerOwnerAssetManifestSnapshotMessage,
) {
    if message.player_entity_id != local_player_id {
        return;
    }
    if message.sequence < cache.sequence {
        return;
    }
    cache.sequence = message.sequence;
    cache.generated_at_tick = message.generated_at_tick;
    cache.assets_by_entity_id.clear();
    for asset in &message.assets {
        cache
            .assets_by_entity_id
            .insert(asset.entity_id.clone(), asset.clone());
    }
}

fn apply_owner_manifest_delta(
    cache: &mut OwnedAssetManifestCache,
    local_player_id: &str,
    message: &ServerOwnerAssetManifestDeltaMessage,
    now_s: f64,
) {
    if message.player_entity_id != local_player_id {
        return;
    }
    if message.base_sequence != cache.sequence {
        if now_s - cache.last_sequence_mismatch_log_at_s >= MISMATCH_LOG_INTERVAL_S {
            warn!(
                "owner manifest delta sequence mismatch: local={} base={} incoming={}; waiting for snapshot",
                cache.sequence, message.base_sequence, message.sequence
            );
            cache.last_sequence_mismatch_log_at_s = now_s;
        }
        return;
    }
    if message.sequence <= cache.sequence {
        return;
    }
    for entity_id in &message.removals {
        cache.assets_by_entity_id.remove(entity_id);
    }
    for asset in &message.upserts {
        cache
            .assets_by_entity_id
            .insert(asset.entity_id.clone(), asset.clone());
    }
    cache.sequence = message.sequence;
    cache.generated_at_tick = message.generated_at_tick;
}

pub fn receive_owner_asset_manifest_messages(
    session: Res<'_, ClientSession>,
    time: Res<'_, Time<Real>>,
    mut cache: ResMut<'_, OwnedAssetManifestCache>,
    mut snapshot_receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<ServerOwnerAssetManifestSnapshotMessage>,
        (With<Client>, With<Connected>),
    >,
    mut delta_receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<ServerOwnerAssetManifestDeltaMessage>,
        (With<Client>, With<Connected>),
    >,
) {
    let now_s = time.elapsed_secs_f64();
    let Some(local_player_id) = local_player_canonical_id(&session) else {
        return;
    };
    if cache.player_entity_id.as_deref() != Some(local_player_id.as_str()) {
        *cache = OwnedAssetManifestCache {
            player_entity_id: Some(local_player_id.clone()),
            ..default()
        };
    }

    for mut receiver in &mut snapshot_receivers {
        for message in receiver.receive() {
            apply_owner_manifest_snapshot(&mut cache, &local_player_id, &message);
        }
    }

    for mut receiver in &mut delta_receivers {
        for message in receiver.receive() {
            apply_owner_manifest_delta(&mut cache, &local_player_id, &message, now_s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_owner_manifest_delta, apply_owner_manifest_snapshot};
    use crate::native::resources::OwnedAssetManifestCache;
    use sidereal_net::{
        OwnedAssetEntry, ServerOwnerAssetManifestDeltaMessage,
        ServerOwnerAssetManifestSnapshotMessage,
    };

    fn entry(entity_id: &str, display_name: &str) -> OwnedAssetEntry {
        OwnedAssetEntry {
            entity_id: entity_id.to_string(),
            display_name: display_name.to_string(),
            kind: "ship".to_string(),
            status: "active".to_string(),
            controlled_by_owner: false,
            last_known_position_xy: None,
            health_ratio: None,
            fuel_ratio: None,
            updated_at_tick: 1,
        }
    }

    #[test]
    fn manifest_delta_adds_spawned_ship_for_owned_selection_panel() {
        let player_id = "11111111-1111-1111-1111-111111111111".to_string();
        let mut cache = OwnedAssetManifestCache {
            player_entity_id: Some(player_id.clone()),
            ..OwnedAssetManifestCache::default()
        };
        apply_owner_manifest_snapshot(
            &mut cache,
            &player_id,
            &ServerOwnerAssetManifestSnapshotMessage {
                player_entity_id: player_id.clone(),
                sequence: 1,
                assets: vec![entry(
                    "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                    "Starter Corvette",
                )],
                generated_at_tick: 10,
            },
        );

        let spawned_entity_id = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".to_string();
        apply_owner_manifest_delta(
            &mut cache,
            &player_id,
            &ServerOwnerAssetManifestDeltaMessage {
                player_entity_id: player_id.clone(),
                sequence: 2,
                base_sequence: 1,
                upserts: vec![entry(spawned_entity_id.as_str(), "Dashboard Corvette")],
                removals: Vec::new(),
                generated_at_tick: 11,
            },
            10.0,
        );

        assert!(
            cache
                .assets_by_entity_id
                .contains_key(spawned_entity_id.as_str())
        );
    }
}
