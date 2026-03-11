use bevy::log::{info, warn};
use bevy::prelude::*;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use lightyear::prelude::client::Connected;
use lightyear::prelude::server::{ClientOf, LinkOf};
use lightyear::prelude::{
    MessageReceiver, NetworkTarget, RemoteId, Server, ServerMultiMessageSender, Unlink,
};
use serde::Deserialize;
use sidereal_game::{AccountId, PlayerTag};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

use sidereal_core::auth::AuthClaims;
use sidereal_net::{
    ClientAuthMessage, ClientDisconnectNotifyMessage, ControlChannel, LIGHTYEAR_PROTOCOL_VERSION,
    PlayerEntityId, ServerSessionDeniedMessage, ServerSessionReadyMessage,
};

use crate::replication::control::{ClientControlRequestOrder, owner_only_replicate};
use crate::replication::input::{
    ClientInputTickTracker, InputRateLimitState, LatestRealtimeInputsByPlayer,
    RealtimeInputActivityByPlayer, canonical_player_entity_id,
};
use crate::replication::lifecycle::ClientLastActivity;
use crate::replication::visibility::{ClientVisibilityRegistry, VisibilityClientContextCache};
use crate::replication::{
    PendingControlledByBindings, PlayerControlledEntityMap, PlayerRuntimeEntityMap,
};

#[derive(Resource, Default)]
pub(crate) struct AuthenticatedClientBindings {
    pub by_client_entity: HashMap<Entity, String>,
    pub by_remote_id: HashMap<lightyear::prelude::PeerId, String>,
}

#[derive(Resource, Default)]
pub(crate) struct PendingAuthAuditState {
    pub last_logged_at_s_by_client_entity: HashMap<Entity, f64>,
}

#[derive(Resource, Default)]
pub(crate) struct SessionReadyThrottleState {
    pub last_sent_at_s_by_client_entity: HashMap<Entity, f64>,
}

pub fn init_resources(app: &mut App) {
    app.insert_resource(AuthenticatedClientBindings::default());
    app.insert_resource(PendingAuthAuditState::default());
    app.insert_resource(SessionReadyThrottleState::default());
}

static MISSING_GATEWAY_JWT_SECRET_WARNED: AtomicBool = AtomicBool::new(false);
pub(crate) const AUTH_CONFIG_DENIED_REASON: &str = "Replication auth is not configured correctly. Check GATEWAY_JWT_SECRET on the replication server and restart it.";

pub(crate) fn configured_gateway_jwt_secret() -> Result<String, &'static str> {
    match std::env::var("GATEWAY_JWT_SECRET") {
        Ok(secret) if secret.len() >= 32 => Ok(secret),
        _ => Err(AUTH_CONFIG_DENIED_REASON),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn cleanup_client_auth_bindings(
    clients: Query<'_, '_, (Entity, &'_ RemoteId), With<ClientOf>>,
    mut removed_clients: RemovedComponents<'_, '_, ClientOf>,
    mut bindings: ResMut<'_, AuthenticatedClientBindings>,
    mut input_tick_tracker: ResMut<'_, ClientInputTickTracker>,
    mut input_rate_limit_state: ResMut<'_, InputRateLimitState>,
    mut latest_realtime_inputs: ResMut<'_, LatestRealtimeInputsByPlayer>,
    mut realtime_input_activity: ResMut<'_, RealtimeInputActivityByPlayer>,
    mut visibility_registry: ResMut<'_, ClientVisibilityRegistry>,
    mut client_context_cache: ResMut<'_, VisibilityClientContextCache>,
    mut control_order: ResMut<'_, ClientControlRequestOrder>,
    mut last_activity: ResMut<'_, ClientLastActivity>,
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
    // Input resources use canonical UUID keys; retain if key's canonical form is live.
    let live_canonical: HashSet<String> = live_player_entity_ids
        .iter()
        .map(|id| canonical_player_entity_id(id))
        .collect();
    input_tick_tracker
        .last_accepted_tick_by_player_entity_id
        .retain(|player_entity_id, _| {
            live_canonical.contains(&canonical_player_entity_id(player_entity_id))
        });
    input_rate_limit_state
        .current_window_index_by_player_entity_id
        .retain(|player_entity_id, _| {
            live_canonical.contains(&canonical_player_entity_id(player_entity_id))
        });
    input_rate_limit_state
        .message_count_in_window_by_player_entity_id
        .retain(|player_entity_id, _| {
            live_canonical.contains(&canonical_player_entity_id(player_entity_id))
        });
    latest_realtime_inputs
        .by_player_entity_id
        .retain(|player_entity_id, _| {
            live_canonical.contains(&player_entity_id.canonical_wire_id())
        });
    realtime_input_activity
        .last_received_at_s_by_player_entity_id
        .retain(|player_entity_id, _| {
            live_canonical.contains(&player_entity_id.canonical_wire_id())
        });
    control_order
        .last_request_seq_by_player
        .retain(|player_entity_id, _| live_player_entity_ids.contains(player_entity_id));
    let disconnected_clients: Vec<Entity> = visibility_registry
        .player_entity_id_by_client
        .keys()
        .filter(|client_entity| !live_clients.contains(client_entity))
        .copied()
        .collect();
    for client_entity in &disconnected_clients {
        visibility_registry.unregister_client(*client_entity);
        client_context_cache.remove_client(*client_entity);
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
    mut bindings: ResMut<'_, AuthenticatedClientBindings>,
    mut visibility_registry: ResMut<'_, ClientVisibilityRegistry>,
    mut client_context_cache: ResMut<'_, VisibilityClientContextCache>,
    mut control_order: ResMut<'_, ClientControlRequestOrder>,
    mut last_activity: ResMut<'_, ClientLastActivity>,
    mut receivers: Query<
        '_,
        '_,
        (
            Entity,
            &'_ RemoteId,
            &'_ mut MessageReceiver<ClientDisconnectNotifyMessage>,
        ),
        With<ClientOf>,
    >,
) {
    for (client_entity, remote_id, mut receiver) in &mut receivers {
        for msg in receiver.receive() {
            info!(
                "replication received client disconnect notify from client_entity={:?} player={}",
                client_entity, msg.player_entity_id
            );
            bindings.by_client_entity.remove(&client_entity);
            bindings.by_remote_id.remove(&remote_id.0);
            visibility_registry.unregister_client(client_entity);
            client_context_cache.remove_client(client_entity);
            control_order
                .last_request_seq_by_player
                .remove(&msg.player_entity_id);
            last_activity.0.remove(&client_entity);
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
    server_query: Query<'_, '_, &'_ Server>,
    mut sender: ServerMultiMessageSender<'_, '_, With<lightyear::prelude::client::Connected>>,
    time: Res<'_, Time<Real>>,
    mut last_activity: ResMut<'_, ClientLastActivity>,
    mut auth_receivers: Query<
        '_,
        '_,
        (
            Entity,
            &'_ LinkOf,
            &'_ RemoteId,
            &'_ mut MessageReceiver<ClientAuthMessage>,
        ),
        With<ClientOf>,
    >,
    controlled_entity_map: Res<'_, PlayerControlledEntityMap>,
    player_entity_map: Res<'_, PlayerRuntimeEntityMap>,
    player_entities: Query<'_, '_, (Entity, &'_ sidereal_game::EntityGuid), With<PlayerTag>>,
    player_accounts: Query<'_, '_, &'_ AccountId, With<PlayerTag>>,
    mut visibility_registry: ResMut<'_, ClientVisibilityRegistry>,
    mut bindings: ResMut<'_, AuthenticatedClientBindings>,
    mut control_order: ResMut<'_, ClientControlRequestOrder>,
    mut ready_throttle: ResMut<'_, SessionReadyThrottleState>,
) {
    let now_s = time.elapsed_secs_f64();
    let jwt_secret = configured_gateway_jwt_secret().ok();
    if jwt_secret.is_none()
        && MISSING_GATEWAY_JWT_SECRET_WARNED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    {
        warn!(
            "replication auth binding denied: missing/invalid GATEWAY_JWT_SECRET (expected >=32 chars)"
        );
    }

    for (client_entity, link_of, remote_id, mut receiver) in &mut auth_receivers {
        let Ok(server) = server_query.get(link_of.server) else {
            warn!(
                "replication auth: missing server entity for client {:?} remote {:?}",
                client_entity, remote_id.0
            );
            continue;
        };
        for message in receiver.receive() {
            last_activity.0.insert(client_entity, now_s);
            let Some(message_player_id) = PlayerEntityId::parse(message.player_entity_id.as_str())
            else {
                warn!(
                    "replication rejected client auth: invalid player id encoding player={}",
                    message.player_entity_id
                );
                continue;
            };
            let message_player_wire = message_player_id.canonical_wire_id();
            let Some(jwt_secret) = jwt_secret.as_ref() else {
                let target = NetworkTarget::Single(remote_id.0);
                let denied = ServerSessionDeniedMessage {
                    player_entity_id: message_player_wire.clone(),
                    reason: AUTH_CONFIG_DENIED_REASON.to_string(),
                };
                if let Err(err) = sender
                    .send::<ServerSessionDeniedMessage, ControlChannel>(&denied, server, &target)
                {
                    warn!(
                        "replication failed sending auth-config session-denied to remote={:?} player={} err={}",
                        remote_id.0, message_player_wire, err
                    );
                }
                continue;
            };
            let claims = match decode_access_token(&message.access_token, jwt_secret) {
                Some(claims) => claims,
                None => {
                    warn!(
                        "replication rejected client auth: invalid token for client {:?}",
                        client_entity
                    );
                    continue;
                }
            };
            let Some(token_player_id) = PlayerEntityId::parse(claims.player_entity_id.as_str())
            else {
                warn!(
                    "replication rejected client auth: invalid token player id encoding token_player={}",
                    claims.player_entity_id
                );
                continue;
            };
            if token_player_id != message_player_id {
                warn!(
                    "replication rejected client auth: token player mismatch token_player={} message_player={}",
                    claims.player_entity_id, message.player_entity_id
                );
                continue;
            }
            if claims.player_entity_id != message.player_entity_id {
                warn!(
                    "replication auth invariant: token/message player encodings differ token_player={} message_player={} canonical={}",
                    claims.player_entity_id, message.player_entity_id, message_player_wire
                );
            }
            let player_entity = player_entity_map
                .by_player_entity_id
                .get(&message_player_wire)
                .copied()
                .or_else(|| {
                    sidereal_runtime_sync::parse_guid_from_entity_id(message_player_wire.as_str())
                        .and_then(|target_guid| {
                            player_entities.iter().find_map(|(entity, guid)| {
                                (guid.0 == target_guid).then_some(entity)
                            })
                        })
                });
            if !claims.sub.is_empty()
                && let Some(player_entity) = player_entity
            {
                let account_id_value = if let Ok(account_id_component) =
                    player_accounts.get(player_entity)
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
                        .entity(player_entity)
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
            if let Some(player_entity) = player_entity {
                // Player anchor entities are owner-only replication targets.
                commands
                    .entity(player_entity)
                    .remove::<lightyear::prelude::InterpolationTarget>()
                    .insert(owner_only_replicate(client_entity));
            } else {
                warn!(
                    "replication auth: player entity not found for {}; owner-only player replication target not applied",
                    message_player_wire
                );
            }

            let already_bound_same_player = bindings
                .by_client_entity
                .get(&client_entity)
                .is_some_and(|bound| bound == &message_player_wire)
                && bindings
                    .by_remote_id
                    .get(&remote_id.0)
                    .is_some_and(|bound| bound == &message_player_wire);
            if already_bound_same_player {
                // Idempotent auth refresh for an already-bound client:
                // keep current visibility/bindings intact and only re-send readiness
                // if enough time has elapsed to make it a meaningful retry.
                visibility_registry.register_client(client_entity, message_player_wire.clone());
                let last_ready_sent_at_s = ready_throttle
                    .last_sent_at_s_by_client_entity
                    .get(&client_entity)
                    .copied()
                    .unwrap_or(f64::NEG_INFINITY);
                if now_s - last_ready_sent_at_s < 2.0 {
                    continue;
                }
                let target = NetworkTarget::Single(remote_id.0);
                let ready = ServerSessionReadyMessage {
                    player_entity_id: message_player_wire.clone(),
                    protocol_version: LIGHTYEAR_PROTOCOL_VERSION,
                };
                if let Err(err) = sender
                    .send::<ServerSessionReadyMessage, ControlChannel>(&ready, server, &target)
                {
                    warn!(
                        "replication failed sending session-ready refresh to remote={:?} player={} err={}",
                        remote_id.0, message_player_wire, err
                    );
                } else {
                    ready_throttle
                        .last_sent_at_s_by_client_entity
                        .insert(client_entity, now_s);
                }
                continue;
            }

            if let Some(bound_player) = bindings.by_remote_id.get(&remote_id.0)
                && bound_player != &message_player_wire
            {
                info!(
                    "replication rebinding remote {:?} from {} to {}",
                    remote_id.0, bound_player, message_player_wire
                );
            }

            bindings
                .by_client_entity
                .insert(client_entity, message_player_wire.clone());
            if let Some(previous_player) = bindings
                .by_remote_id
                .insert(remote_id.0, message_player_wire.clone())
                && previous_player != message_player_wire
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
                .find(|(k, v)| v == &&message_player_wire && *k != &client_entity)
                .map(|(k, _)| *k);
            if let Some(old_entity) = old_client_entity_for_new_player {
                bindings.by_client_entity.remove(&old_entity);
                visibility_registry.unregister_client(old_entity);
            }

            // New authenticated bind is a fresh control session for this player.
            // Reset per-player request ordering so newly started clients (seq from 1)
            // are not rejected as stale against a prior disconnected session.
            control_order
                .last_request_seq_by_player
                .remove(&message_player_wire);

            visibility_registry.register_client(client_entity, message_player_wire.clone());

            if !player_entity_map
                .by_player_entity_id
                .contains_key(&message_player_wire)
            {
                warn!(
                    "replication auth: player entity {} not found in world; denying session",
                    message_player_wire
                );
                let target = NetworkTarget::Single(remote_id.0);
                let denied = ServerSessionDeniedMessage {
                    player_entity_id: message_player_wire.clone(),
                    reason: "Player entity not yet loaded into the world. Please try again."
                        .to_string(),
                };
                if let Err(err) = sender
                    .send::<ServerSessionDeniedMessage, ControlChannel>(&denied, server, &target)
                {
                    warn!(
                        "replication failed sending session-denied to remote={:?} player={} err={}",
                        remote_id.0, message_player_wire, err
                    );
                }
                continue;
            }

            if let Some(&controlled_entity) = controlled_entity_map
                .by_player_entity_id
                .get(&message_player_id)
            {
                pending_controlled_by
                    .bindings
                    .push((client_entity, controlled_entity));
            } else if let Some(&player_entity) = player_entity_map
                .by_player_entity_id
                .get(&message_player_wire)
            {
                pending_controlled_by
                    .bindings
                    .push((client_entity, player_entity));
            }

            info!(
                "replication client authenticated and bound: client={:?} remote={:?} player_entity_id={}",
                client_entity, remote_id.0, message_player_wire
            );

            let target = NetworkTarget::Single(remote_id.0);
            let ready = ServerSessionReadyMessage {
                player_entity_id: message_player_wire.clone(),
                protocol_version: LIGHTYEAR_PROTOCOL_VERSION,
            };
            if let Err(err) =
                sender.send::<ServerSessionReadyMessage, ControlChannel>(&ready, server, &target)
            {
                warn!(
                    "replication failed sending session-ready message to remote={:?} player={} err={}",
                    remote_id.0, message_player_wire, err
                );
            } else {
                ready_throttle
                    .last_sent_at_s_by_client_entity
                    .insert(client_entity, now_s);
            }
        }
    }
}

fn decode_access_token(token: &str, jwt_secret: &str) -> Option<AuthClaims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    match decode::<CompatAuthClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    ) {
        Ok(decoded) => Some(AuthClaims {
            sub: decoded.claims.sub.unwrap_or_default(),
            player_entity_id: decoded.claims.player_entity_id,
            roles: decoded.claims.roles,
            iat: decoded.claims.iat.unwrap_or_default(),
            exp: decoded.claims.exp,
            jti: decoded.claims.jti.unwrap_or_default(),
        }),
        Err(err) => {
            warn!("replication rejected client auth token decode: {}", err);
            None
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn audit_pending_client_auth_state(
    time: Res<'_, Time<Real>>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    mut audit_state: ResMut<'_, PendingAuthAuditState>,
    clients: Query<
        '_,
        '_,
        (Entity, &'_ RemoteId, &'_ MessageReceiver<ClientAuthMessage>),
        (With<ClientOf>, With<Connected>),
    >,
) {
    let now_s = time.elapsed_secs_f64();
    for (client_entity, remote_id, receiver) in &clients {
        if bindings.by_client_entity.contains_key(&client_entity) {
            audit_state
                .last_logged_at_s_by_client_entity
                .remove(&client_entity);
            continue;
        }
        let last_logged_at_s = audit_state
            .last_logged_at_s_by_client_entity
            .get(&client_entity)
            .copied()
            .unwrap_or(f64::NEG_INFINITY);
        if now_s - last_logged_at_s < 1.0 {
            continue;
        }
        info!(
            "replication pending auth state: client={:?} remote={:?} queued_auth_messages={}",
            client_entity,
            remote_id.0,
            receiver.num_messages()
        );
        audit_state
            .last_logged_at_s_by_client_entity
            .insert(client_entity, now_s);
    }
}

#[derive(Debug, Deserialize)]
struct CompatAuthClaims {
    #[serde(default)]
    sub: Option<String>,
    player_entity_id: String,
    #[serde(default)]
    roles: Vec<String>,
    #[serde(default)]
    iat: Option<u64>,
    exp: u64,
    #[serde(default)]
    jti: Option<String>,
}
