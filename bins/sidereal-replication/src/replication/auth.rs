use bevy::log::{info, warn};
use bevy::prelude::*;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use lightyear::prelude::server::{ClientOf, RawServer};
use lightyear::prelude::{
    MessageReceiver, NetworkTarget, RemoteId, Replicate, ReplicationState, Server,
    ServerMultiMessageSender, Unlink,
};
use sidereal_game::{AccountId, PlayerTag};
use sidereal_game::{
    ActionQueue, CharacterMovementController, ControlledEntityGuid, EntityGuid, SelectedEntityGuid,
    default_character_movement_action_capabilities,
};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

use sidereal_net::{
    ClientAuthMessage, ClientDisconnectNotifyMessage, ControlChannel, ServerSessionReadyMessage,
};

use crate::replication::input::{
    ClientInputTickTracker, InputRateLimitState, LatestRealtimeInputsByPlayer,
};
use crate::replication::view::ClientControlRequestOrder;
use crate::replication::{
    PendingControlledByBindings, PlayerControlledEntityMap, PlayerRuntimeEntityMap,
};
use crate::{AssetStreamServerState, AuthenticatedClientBindings, ClientVisibilityRegistry};

#[derive(Debug, serde::Deserialize)]
struct AccessTokenClaims {
    sub: String,
    player_entity_id: String,
}

static MISSING_GATEWAY_JWT_SECRET_WARNED: AtomicBool = AtomicBool::new(false);

#[allow(clippy::too_many_arguments)]
pub fn cleanup_client_auth_bindings(
    clients: Query<'_, '_, (Entity, &'_ RemoteId), With<ClientOf>>,
    mut removed_clients: RemovedComponents<'_, '_, ClientOf>,
    mut bindings: ResMut<'_, AuthenticatedClientBindings>,
    mut input_tick_tracker: ResMut<'_, ClientInputTickTracker>,
    mut input_rate_limit_state: ResMut<'_, InputRateLimitState>,
    mut latest_realtime_inputs: ResMut<'_, LatestRealtimeInputsByPlayer>,
    mut stream_state: ResMut<'_, AssetStreamServerState>,
    mut visibility_registry: ResMut<'_, ClientVisibilityRegistry>,
    mut control_order: ResMut<'_, ClientControlRequestOrder>,
    mut last_activity: ResMut<'_, crate::ClientLastActivity>,
) {
    let live_clients = clients
        .iter()
        .map(|(entity, _)| entity)
        .collect::<HashSet<_>>();
    let live_remote_ids = clients
        .iter()
        .map(|(_, remote_id)| remote_id.0)
        .collect::<HashSet<_>>();
    // Drain RemovedComponents<ClientOf> so we don't accumulate stale removals.
    let _: HashSet<_> = removed_clients.read().collect();
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
    input_rate_limit_state
        .current_window_index_by_player_entity_id
        .retain(|player_entity_id, _| live_player_entity_ids.contains(player_entity_id));
    input_rate_limit_state
        .message_count_in_window_by_player_entity_id
        .retain(|player_entity_id, _| live_player_entity_ids.contains(player_entity_id));
    latest_realtime_inputs
        .by_player_entity_id
        .retain(|player_entity_id, _| live_player_entity_ids.contains(player_entity_id));
    control_order
        .last_request_seq_by_player
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
    stream_state
        .pending_chunks_by_remote
        .retain(|remote_id, _| live_remote_ids.contains(remote_id));
    stream_state
        .chunk_send_failures_by_remote
        .retain(|remote_id, _| live_remote_ids.contains(remote_id));
    stream_state
        .chunk_send_backoff_frames_by_remote
        .retain(|remote_id, _| live_remote_ids.contains(remote_id));
    let disconnected_clients: Vec<Entity> = visibility_registry
        .player_entity_id_by_client
        .keys()
        .filter(|client_entity| !live_clients.contains(client_entity))
        .copied()
        .collect();
    for client_entity in &disconnected_clients {
        visibility_registry.unregister_client(*client_entity);
    }
    last_activity
        .0
        .retain(|client_entity, _| live_clients.contains(client_entity));

    // Do not call lose_visibility(stale_client) for each replicated entity here.
    // Doing so causes Lightyear to enqueue a despawn/visibility-revoke per entity for the
    // disconnected client, producing a huge burst of outbound traffic (tens of MiB/s) when
    // the client has already left. The client is already gone (ClientOf removed); we leave
    // ReplicationState visibility bits as-is for the stale client. They are harmless:
    // update_network_visibility only iterates live_clients, and the replication sender
    // for that client is gone, so no traffic is sent for them.
}

/// When the client sends a disconnect notify (logout or window close), Unlink immediately
/// so the server stops sending to that peer without waiting for idle timeout.
pub fn receive_client_disconnect_notify(
    mut commands: Commands<'_, '_>,
    mut receivers: Query<
        '_,
        '_,
        (
            Entity,
            &'_ mut MessageReceiver<ClientDisconnectNotifyMessage>,
        ),
        With<ClientOf>,
    >,
) {
    for (client_entity, mut receiver) in &mut receivers {
        for msg in receiver.receive() {
            info!(
                "replication received client disconnect notify from client_entity={:?} player={}",
                client_entity, msg.player_entity_id
            );
            commands.trigger(Unlink {
                entity: client_entity,
                reason: "client_notify".to_string(),
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn receive_client_auth_messages(
    mut commands: Commands<'_, '_>,
    mut pending_controlled_by: ResMut<'_, PendingControlledByBindings>,
    server_query: Query<'_, '_, &'_ Server, With<RawServer>>,
    mut sender: ServerMultiMessageSender<'_, '_, With<lightyear::prelude::client::Connected>>,
    time: Res<'_, Time<Real>>,
    mut last_activity: ResMut<'_, crate::ClientLastActivity>,
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
    mut player_entity_map: ResMut<'_, PlayerRuntimeEntityMap>,
    player_accounts: Query<'_, '_, &'_ AccountId, With<PlayerTag>>,
    mut visibility_registry: ResMut<'_, ClientVisibilityRegistry>,
    mut bindings: ResMut<'_, AuthenticatedClientBindings>,
    mut stream_state: ResMut<'_, AssetStreamServerState>,
    mut control_order: ResMut<'_, ClientControlRequestOrder>,
    mut replication_states: Query<'_, '_, &'_ mut ReplicationState, With<Replicate>>,
) {
    let now_s = time.elapsed_secs_f64();
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

    let Ok(server) = server_query.single() else {
        return;
    };

    for (client_entity, remote_id, mut receiver) in &mut auth_receivers {
        for message in receiver.receive() {
            last_activity.0.insert(client_entity, now_s);
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
                    "replication rejected client auth: token player mismatch token_player={} message_player={}",
                    claims.player_entity_id, message.player_entity_id
                );
                continue;
            }
            if let Some(player_entity) = player_entity_map
                .by_player_entity_id
                .get(&message.player_entity_id)
            {
                let account_id_value = if let Ok(account_id_component) =
                    player_accounts.get(*player_entity)
                {
                    account_id_component.0.clone()
                } else {
                    // Hardening: if hydration missed AccountId for an existing player entity,
                    // recover from authenticated token subject and patch entity immediately.
                    warn!(
                        "replication auth repair: player {} missing AccountId component; injecting from authenticated token subject",
                        message.player_entity_id
                    );
                    commands
                        .entity(*player_entity)
                        .insert(AccountId(claims.sub.clone()));
                    claims.sub.clone()
                };
                if account_id_value != claims.sub {
                    warn!(
                        "replication rejected client auth: account does not own player entity (account={} player={})",
                        claims.sub, message.player_entity_id
                    );
                    continue;
                }
            }

            if let Some(bound_player) = bindings.by_remote_id.get(&remote_id.0)
                && bound_player != &message.player_entity_id
            {
                info!(
                    "replication rebinding remote {:?} from {} to {}",
                    remote_id.0, bound_player, message.player_entity_id
                );
            }

            bindings
                .by_client_entity
                .insert(client_entity, message.player_entity_id.clone());
            if let Some(previous_player) = bindings
                .by_remote_id
                .insert(remote_id.0, message.player_entity_id.clone())
                && previous_player != message.player_entity_id.as_str()
            {
                bindings
                    .by_client_entity
                    .retain(|_, v| v != &previous_player);
                control_order
                    .last_request_seq_by_player
                    .remove(&previous_player);
            }

            let old_client_entity_for_new_player = bindings
                .by_client_entity
                .iter()
                .find(|(k, v)| v == &&message.player_entity_id && *k != &client_entity)
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
            stream_state.pending_chunks_by_remote.remove(&remote_id.0);
            stream_state
                .chunk_send_failures_by_remote
                .remove(&remote_id.0);
            stream_state
                .chunk_send_backoff_frames_by_remote
                .remove(&remote_id.0);

            // New authenticated bind is a fresh control session for this player.
            // Reset per-player request ordering so newly started clients (seq from 1)
            // are not rejected as stale against a prior disconnected session.
            control_order
                .last_request_seq_by_player
                .remove(&message.player_entity_id);

            visibility_registry.register_client(client_entity, message.player_entity_id.clone());
            // Force a clean visibility handshake for this authenticated client.
            // This guarantees reconnects receive fresh spawn baseline even if the
            // underlying remote link entity was reused across quick logout/login.
            for mut replication_state in &mut replication_states {
                if replication_state.is_visible(client_entity) {
                    replication_state.lose_visibility(client_entity);
                }
            }

            if !player_entity_map
                .by_player_entity_id
                .contains_key(&message.player_entity_id)
            {
                // Fail-open for runtime presence (character only): if bootstrap/runtime timing
                // is late, create the player runtime entity now so session-ready can complete.
                // Ship creation remains bootstrap/hydration-owned and is never synthesized here.
                let player_guid = message
                    .player_entity_id
                    .strip_prefix("player:")
                    .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
                    .unwrap_or_else(uuid::Uuid::new_v4);
                let mut entity_commands = commands.spawn((
                    Name::new(message.player_entity_id.clone()),
                    EntityGuid(player_guid),
                    PlayerTag,
                    AccountId(claims.sub.clone()),
                    ActionQueue::default(),
                    default_character_movement_action_capabilities(),
                    CharacterMovementController { speed_mps: 40.0 },
                    ControlledEntityGuid(Some(player_guid.to_string())),
                    SelectedEntityGuid(None),
                    Transform::default(),
                    avian2d::prelude::Position(Vec2::ZERO),
                ));
                entity_commands.insert(Replicate::to_clients(NetworkTarget::All));
                let player_entity = entity_commands.id();
                player_entity_map
                    .by_player_entity_id
                    .insert(message.player_entity_id.clone(), player_entity);
                warn!(
                    "replication auth repaired missing runtime player entity for {} during bind",
                    message.player_entity_id
                );
            }

            // Defer ControlledBy to PostUpdate to avoid same-frame replication/hierarchy ordering issues.
            if let Some(&ship_entity) = controlled_entity_map
                .by_player_entity_id
                .get(&message.player_entity_id)
            {
                pending_controlled_by
                    .bindings
                    .push((client_entity, ship_entity));
            } else if let Some(&player_entity) = player_entity_map
                .by_player_entity_id
                .get(&message.player_entity_id)
            {
                pending_controlled_by
                    .bindings
                    .push((client_entity, player_entity));
            }

            info!(
                "replication client authenticated and bound: client={:?} remote={:?} player_entity_id={}",
                client_entity, remote_id.0, message.player_entity_id
            );

            let target = NetworkTarget::Single(remote_id.0);
            let ready = ServerSessionReadyMessage {
                player_entity_id: message.player_entity_id.clone(),
            };
            if let Err(err) =
                sender.send::<ServerSessionReadyMessage, ControlChannel>(&ready, server, &target)
            {
                warn!(
                    "replication failed sending session-ready message to remote={:?} player={} err={}",
                    remote_id.0, message.player_entity_id, err
                );
            }
        }
    }
}

fn decode_access_token(token: &str, jwt_secret: &str) -> Option<AccessTokenClaims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    match decode::<AccessTokenClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    ) {
        Ok(decoded) => Some(decoded.claims),
        Err(err) => {
            warn!("replication rejected client auth token decode: {}", err);
            None
        }
    }
}
