use bevy::log::{info, warn};
use bevy::prelude::*;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{MessageReceiver, RemoteId};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

use sidereal_net::ClientAuthMessage;
use sidereal_persistence::PlayerRuntimeViewState;

use crate::replication::input::ClientInputTickTracker;
use crate::replication::{PendingControlledByBindings, PlayerControlledEntityMap};
use crate::{
    AssetStreamServerState, AuthenticatedClientBindings, ClientVisibilityRegistry,
    PlayerRuntimeViewDirtySet, PlayerRuntimeViewRegistry, unix_epoch_now_i64,
};

#[derive(Debug, serde::Deserialize)]
struct AccessTokenClaims {
    player_entity_id: String,
}

static MISSING_GATEWAY_JWT_SECRET_WARNED: AtomicBool = AtomicBool::new(false);

pub fn cleanup_client_auth_bindings(
    clients: Query<'_, '_, (Entity, &'_ RemoteId), With<ClientOf>>,
    mut bindings: ResMut<'_, AuthenticatedClientBindings>,
    mut input_tick_tracker: ResMut<'_, ClientInputTickTracker>,
    mut stream_state: ResMut<'_, AssetStreamServerState>,
) {
    let live_clients = clients
        .iter()
        .map(|(entity, _)| entity)
        .collect::<HashSet<_>>();
    let live_remote_ids = clients
        .iter()
        .map(|(_, remote_id)| remote_id.0)
        .collect::<HashSet<_>>();
    bindings
        .by_client_entity
        .retain(|client_entity, _| live_clients.contains(client_entity));
    bindings
        .by_remote_id
        .retain(|remote_id, _| live_remote_ids.contains(remote_id));
    let live_player_entity_ids = bindings
        .by_client_entity
        .values()
        .cloned()
        .collect::<HashSet<_>>();
    input_tick_tracker
        .last_accepted_tick_by_player_entity_id
        .retain(|player_entity_id, _| live_player_entity_ids.contains(player_entity_id));
    stream_state
        .sent_asset_ids_by_remote
        .retain(|remote_id, _| live_remote_ids.contains(remote_id));
    stream_state
        .pending_requested_asset_ids_by_remote
        .retain(|remote_id, _| live_remote_ids.contains(remote_id));
    stream_state
        .acked_assets_by_remote
        .retain(|remote_id, _| live_remote_ids.contains(remote_id));
}

#[allow(clippy::too_many_arguments)]
pub fn receive_client_auth_messages(
    mut pending_controlled_by: ResMut<'_, PendingControlledByBindings>,
    mut auth_receivers: Query<
        '_,
        '_,
        (
            Entity,
            &'_ RemoteId,
            &'_ mut MessageReceiver<ClientAuthMessage>,
        ),
        With<ClientOf>,
    >,
    controlled_entity_map: Res<'_, PlayerControlledEntityMap>,
    mut visibility_registry: ResMut<'_, ClientVisibilityRegistry>,
    mut bindings: ResMut<'_, AuthenticatedClientBindings>,
    mut view_registry: ResMut<'_, PlayerRuntimeViewRegistry>,
    mut dirty_view_states: ResMut<'_, PlayerRuntimeViewDirtySet>,
    mut stream_state: ResMut<'_, AssetStreamServerState>,
) {
    let jwt_secret = match std::env::var("GATEWAY_JWT_SECRET") {
        Ok(secret) if secret.len() >= 32 => secret,
        _ => {
            if MISSING_GATEWAY_JWT_SECRET_WARNED
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                warn!(
                    "replication auth binding disabled: missing/invalid GATEWAY_JWT_SECRET (expected >=32 chars)"
                );
            }
            return;
        }
    };

    for (client_entity, remote_id, mut receiver) in &mut auth_receivers {
        for message in receiver.receive() {
            let claims = match decode_access_token(&message.access_token, &jwt_secret) {
                Some(claims) => claims,
                None => {
                    warn!(
                        "replication rejected client auth: invalid token for client {:?}",
                        client_entity
                    );
                    continue;
                }
            };
            if claims.player_entity_id != message.player_entity_id {
                warn!(
                    "replication rejected client auth: token player mismatch for client {:?}",
                    client_entity
                );
                continue;
            }

            if let Some(bound_player) = bindings.by_remote_id.get(&remote_id.0)
                && bound_player != &claims.player_entity_id
            {
                info!(
                    "replication rebinding remote {:?} from {} to {}",
                    remote_id.0, bound_player, claims.player_entity_id
                );
            }

            if let Some(previous_player) = bindings
                .by_client_entity
                .insert(client_entity, claims.player_entity_id.clone())
                && previous_player != claims.player_entity_id.as_str()
            {
                dirty_view_states.player_entity_ids.insert(previous_player);
            }
            if let Some(previous_player) = bindings
                .by_remote_id
                .insert(remote_id.0, claims.player_entity_id.clone())
                && previous_player != claims.player_entity_id.as_str()
            {
                bindings
                    .by_client_entity
                    .retain(|_, v| v != &previous_player);
            }

            let old_client_entity_for_new_player = bindings
                .by_client_entity
                .iter()
                .find(|(k, v)| v == &&claims.player_entity_id && *k != &client_entity)
                .map(|(k, _)| *k);
            if let Some(old_entity) = old_client_entity_for_new_player {
                bindings.by_client_entity.remove(&old_entity);
                visibility_registry.unregister_client(old_entity);
            }

            stream_state.sent_asset_ids_by_remote.remove(&remote_id.0);
            stream_state
                .pending_requested_asset_ids_by_remote
                .remove(&remote_id.0);
            stream_state.acked_assets_by_remote.remove(&remote_id.0);

            visibility_registry.register_client(client_entity, claims.player_entity_id.clone());
            view_registry
                .by_player_entity_id
                .entry(claims.player_entity_id.clone())
                .or_insert_with(|| PlayerRuntimeViewState {
                    player_entity_id: claims.player_entity_id.clone(),
                    updated_at_epoch_s: unix_epoch_now_i64(),
                    ..Default::default()
                });
            dirty_view_states
                .player_entity_ids
                .insert(claims.player_entity_id.clone());

            // Defer ControlledBy to PostUpdate to avoid same-frame replication/hierarchy ordering issues.
            if let Some(&ship_entity) = controlled_entity_map
                .by_player_entity_id
                .get(&claims.player_entity_id)
            {
                pending_controlled_by
                    .bindings
                    .push((client_entity, ship_entity));
            }

            info!(
                "replication client authenticated and bound: client={:?} remote={:?} player_entity_id={}",
                client_entity, remote_id.0, claims.player_entity_id
            );
        }
    }
}

fn decode_access_token(token: &str, jwt_secret: &str) -> Option<AccessTokenClaims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    decode::<AccessTokenClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    )
    .ok()
    .map(|decoded| decoded.claims)
}
