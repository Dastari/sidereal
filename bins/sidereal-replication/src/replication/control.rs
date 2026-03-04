use bevy::log::{info, warn};
use bevy::prelude::*;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::server::RawServer;
use lightyear::prelude::{
    ControlledBy, InterpolationTarget, MessageReceiver, NetworkTarget, PredictionTarget, RemoteId,
    Replicate, Server, ServerMultiMessageSender,
};
use sidereal_game::{
    ActionQueue, ControlledEntityGuid, EntityGuid, FlightComputer, OwnerId, PlayerTag,
};
use std::collections::HashMap;

use sidereal_net::{
    ClientControlRequestMessage, ControlChannel, PlayerEntityId, ServerControlAckMessage,
    ServerControlRejectMessage,
};

use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::{
    PendingControlledByBindings, PlayerControlledEntityMap, PlayerRuntimeEntityMap,
    SimulatedControlledEntity, debug_env,
};

#[derive(Resource, Default)]
pub struct ClientControlRequestOrder {
    pub last_request_seq_by_player: HashMap<String, u64>,
}

pub fn init_resources(app: &mut App) {
    app.insert_resource(ClientControlRequestOrder::default());
}

#[doc(hidden)]
pub fn guid_from_entity_id_like(raw: &str) -> Option<String> {
    if let Some(candidate) = raw.split(':').nth(1)
        && uuid::Uuid::parse_str(candidate).is_ok()
    {
        return Some(candidate.to_string());
    }
    if uuid::Uuid::parse_str(raw).is_ok() {
        return Some(raw.to_string());
    }
    None
}

fn control_debug_logging_enabled() -> bool {
    debug_env("SIDEREAL_DEBUG_CONTROL_LOGS")
}

fn clear_controlled_binding_for_client(
    commands: &mut Commands<'_, '_>,
    client_entity: Entity,
    remote_id: RemoteId,
    controlled_entities: &Query<'_, '_, (Entity, Option<&'_ ControlledBy>), With<ActionQueue>>,
    player_entities: &Query<'_, '_, &'_ EntityGuid, With<PlayerTag>>,
) {
    for (entity, controlled_by) in controlled_entities {
        if controlled_by.is_some_and(|binding| binding.owner == client_entity) {
            commands
                .entity(entity)
                .remove::<(ControlledBy, PredictionTarget)>();
            if player_entities.get(entity).is_ok() {
                commands
                    .entity(entity)
                    .remove::<InterpolationTarget>()
                    .insert(Replicate::to_clients(NetworkTarget::Single(remote_id.0)));
            } else {
                commands
                    .entity(entity)
                    .insert(InterpolationTarget::to_clients(NetworkTarget::All));
            }
        }
    }
}

fn neutralize_control_intent_on_handoff(
    commands: &mut Commands<'_, '_>,
    previous_controlled_entity: Entity,
) {
    commands.queue(move |world: &mut World| {
        if let Some(mut queue) = world.get_mut::<ActionQueue>(previous_controlled_entity) {
            queue.clear();
        }
        if let Some(mut flight_computer) =
            world.get_mut::<FlightComputer>(previous_controlled_entity)
        {
            flight_computer.throttle = 0.0;
            flight_computer.yaw_input = 0.0;
            flight_computer.brake_active = true;
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub fn receive_client_control_requests(
    mut commands: Commands<'_, '_>,
    server_query: Query<'_, '_, &'_ Server, With<RawServer>>,
    mut sender: ServerMultiMessageSender<'_, '_, With<lightyear::prelude::client::Connected>>,
    time: Res<'_, Time<Real>>,
    mut last_activity: ResMut<'_, crate::replication::lifecycle::ClientLastActivity>,
    mut receivers: Query<
        '_,
        '_,
        (
            Entity,
            &'_ RemoteId,
            &'_ mut MessageReceiver<ClientControlRequestMessage>,
        ),
        With<ClientOf>,
    >,
    controllable_entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            &'_ OwnerId,
            &'_ SimulatedControlledEntity,
        ),
        With<SimulatedControlledEntity>,
    >,
    controlled_entities: Query<'_, '_, (Entity, Option<&'_ ControlledBy>), With<ActionQueue>>,
    player_guids: Query<'_, '_, &'_ EntityGuid, With<PlayerTag>>,
    player_controlled: Query<'_, '_, &'_ ControlledEntityGuid, With<PlayerTag>>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    mut order_state: ResMut<'_, ClientControlRequestOrder>,
    player_entities: Res<'_, PlayerRuntimeEntityMap>,
    mut controlled_entity_map: ResMut<'_, PlayerControlledEntityMap>,
    mut pending_controlled_by: ResMut<'_, PendingControlledByBindings>,
) {
    let Ok(server) = server_query.single() else {
        return;
    };
    let now_s = time.elapsed_secs_f64();
    for (client_entity, remote_id, mut receiver) in &mut receivers {
        for message in receiver.receive() {
            last_activity.0.insert(client_entity, now_s);
            let target = NetworkTarget::Single(remote_id.0);
            let Some(message_player_id) = PlayerEntityId::parse(message.player_entity_id.as_str())
            else {
                warn!(
                    "replication dropped control request from {:?}: invalid message player id {}",
                    client_entity, message.player_entity_id
                );
                continue;
            };
            let message_player_wire = message_player_id.canonical_wire_id();
            let Some(bound_player) = bindings.by_client_entity.get(&client_entity) else {
                continue;
            };
            let Some(bound_player_id) = PlayerEntityId::parse(bound_player.as_str()) else {
                warn!(
                    "replication dropped control request from {:?}: invalid bound player id {}",
                    client_entity, bound_player
                );
                continue;
            };
            if bound_player_id != message_player_id {
                warn!(
                    "replication dropped client control request from {:?}: player mismatch {} != {}",
                    client_entity, message.player_entity_id, bound_player
                );
                let reject = ServerControlRejectMessage {
                    player_entity_id: message_player_wire,
                    request_seq: message.request_seq,
                    reason: "player_mismatch".to_string(),
                    authoritative_controlled_entity_id: None,
                };
                let _ = sender
                    .send::<ServerControlRejectMessage, ControlChannel>(&reject, server, &target);
                continue;
            }
            if bound_player != &message.player_entity_id {
                warn!(
                    "replication control invariant: canonical match but encoding differs bound={} message={} canonical={}",
                    bound_player,
                    message.player_entity_id,
                    bound_player_id.canonical_wire_id()
                );
            }
            if let Some(last_seq) = order_state
                .last_request_seq_by_player
                .get(bound_player.as_str())
                && message.request_seq <= *last_seq
            {
                if control_debug_logging_enabled() {
                    info!(
                        "control request dropped stale seq player={} seq={} last_seq={}",
                        bound_player, message.request_seq, last_seq
                    );
                }
                let authoritative_controlled = player_entities
                    .by_player_entity_id
                    .get(bound_player)
                    .and_then(|player_entity| player_controlled.get(*player_entity).ok())
                    .and_then(|guid| {
                        let control_guid = guid.0.as_deref()?;
                        let player_guid = bound_player_id.0.to_string();
                        let player_runtime_id = player_guid.clone();
                        if control_guid == player_guid {
                            return Some(player_runtime_id);
                        }
                        controllable_entities
                            .iter()
                            .find(|(_, guid, owner, _)| {
                                PlayerEntityId::parse(owner.0.as_str())
                                    .is_some_and(|owner_id| owner_id == bound_player_id)
                                    && guid.0.to_string() == control_guid
                            })
                            .map(|(_, guid, _, _)| guid.0.to_string())
                    });
                let reject = ServerControlRejectMessage {
                    player_entity_id: bound_player.clone(),
                    request_seq: message.request_seq,
                    reason: "stale_seq".to_string(),
                    authoritative_controlled_entity_id: authoritative_controlled,
                };
                let _ = sender
                    .send::<ServerControlRejectMessage, ControlChannel>(&reject, server, &target);
                continue;
            }

            let Some(&player_entity) = player_entities.by_player_entity_id.get(bound_player) else {
                warn!(
                    "replication dropped control request for {}: no hydrated player entity",
                    bound_player
                );
                let reject = ServerControlRejectMessage {
                    player_entity_id: bound_player.clone(),
                    request_seq: message.request_seq,
                    reason: "missing_player_entity".to_string(),
                    authoritative_controlled_entity_id: None,
                };
                let _ = sender
                    .send::<ServerControlRejectMessage, ControlChannel>(&reject, server, &target);
                continue;
            };
            let Ok(player_guid) = player_guids
                .get(player_entity)
                .map(|guid| guid.0.to_string())
            else {
                warn!(
                    "replication dropped control request for {}: player entity missing EntityGuid",
                    bound_player
                );
                let reject = ServerControlRejectMessage {
                    player_entity_id: bound_player.clone(),
                    request_seq: message.request_seq,
                    reason: "missing_player_guid".to_string(),
                    authoritative_controlled_entity_id: None,
                };
                let _ = sender
                    .send::<ServerControlRejectMessage, ControlChannel>(&reject, server, &target);
                continue;
            };
            let player_runtime_id = player_guid.clone();

            let requested_control_guid = message
                .controlled_entity_id
                .as_deref()
                .and_then(guid_from_entity_id_like);
            let (resolved_control_guid, resolved_target_entity, resolved_runtime_entity_id) =
                if let Some(control_guid) = requested_control_guid.clone() {
                    if control_guid == player_guid {
                        (
                            Some(player_guid.clone()),
                            player_entity,
                            Some(player_runtime_id.clone()),
                        )
                    } else {
                        let requested_target =
                            controllable_entities.iter().find(|(_, guid, owner, _)| {
                                guid.0.to_string() == control_guid
                                    && PlayerEntityId::parse(owner.0.as_str())
                                        .is_some_and(|owner_id| owner_id == bound_player_id)
                            });
                        if let Some((target_entity, target_guid, _, _)) = requested_target {
                            (
                                Some(control_guid),
                                target_entity,
                                Some(target_guid.0.to_string()),
                            )
                        } else {
                            warn!(
                                "replication rejected control request for {} -> {} (target not found or not owned)",
                                bound_player, control_guid
                            );
                            let reject = ServerControlRejectMessage {
                                player_entity_id: bound_player.clone(),
                                request_seq: message.request_seq,
                                reason: "target_not_owned_or_missing".to_string(),
                                authoritative_controlled_entity_id: Some(player_runtime_id.clone()),
                            };
                            let _ = sender.send::<ServerControlRejectMessage, ControlChannel>(
                                &reject, server, &target,
                            );
                            order_state
                                .last_request_seq_by_player
                                .insert(bound_player.clone(), message.request_seq);
                            continue;
                        }
                    }
                } else {
                    (
                        Some(player_guid.clone()),
                        player_entity,
                        Some(player_runtime_id.clone()),
                    )
                };
            let currently_bound = controlled_entity_map
                .by_player_entity_id
                .get(&bound_player_id)
                .copied();
            let currently_bound_entity = currently_bound.unwrap_or(player_entity);
            commands
                .entity(player_entity)
                .insert(ControlledEntityGuid(resolved_control_guid.clone()));

            controlled_entity_map
                .by_player_entity_id
                .insert(bound_player_id, resolved_target_entity);

            let rebind_required = currently_bound_entity != resolved_target_entity;
            if rebind_required {
                neutralize_control_intent_on_handoff(&mut commands, currently_bound_entity);
                clear_controlled_binding_for_client(
                    &mut commands,
                    client_entity,
                    *remote_id,
                    &controlled_entities,
                    &player_guids,
                );
                pending_controlled_by
                    .bindings
                    .retain(|(queued_client, _)| *queued_client != client_entity);
                pending_controlled_by
                    .bindings
                    .push((client_entity, resolved_target_entity));
            }

            if control_debug_logging_enabled() {
                info!(
                    "control route player={} client={:?} seq={} requested_raw={:?} requested_guid={:?} resolved_guid={:?} current_entity={:?} target_entity={:?} rebind_required={}",
                    bound_player,
                    client_entity,
                    message.request_seq,
                    message.controlled_entity_id,
                    requested_control_guid,
                    resolved_control_guid,
                    currently_bound_entity,
                    resolved_target_entity,
                    rebind_required
                );
            }
            order_state
                .last_request_seq_by_player
                .insert(bound_player.clone(), message.request_seq);

            let ack = ServerControlAckMessage {
                player_entity_id: bound_player.clone(),
                request_seq: message.request_seq,
                controlled_entity_id: resolved_runtime_entity_id,
            };
            let _ = sender.send::<ServerControlAckMessage, ControlChannel>(&ack, server, &target);
        }
    }
}
