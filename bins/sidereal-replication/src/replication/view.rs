use bevy::log::{info, warn};
use bevy::prelude::*;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::server::RawServer;
use lightyear::prelude::{
    ControlledBy, MessageReceiver, NetworkTarget, RemoteId, Server, ServerMultiMessageSender,
};
use sidereal_game::{ActionQueue, ControlledEntityGuid, EntityGuid, OwnerId, PlayerTag};
use std::collections::HashMap;

use sidereal_net::{
    ClientControlRequestMessage, ControlChannel, ServerControlAckMessage,
    ServerControlRejectMessage,
};

use crate::{
    AuthenticatedClientBindings,
    replication::{
        PendingControlledByBindings, PlayerControlledEntityMap, PlayerRuntimeEntityMap,
        SimulatedControlledEntity,
    },
};

#[derive(Resource, Default)]
pub struct ClientControlRequestOrder {
    pub last_request_seq_by_player: HashMap<String, u64>,
}

fn guid_from_entity_id_like(raw: &str) -> Option<String> {
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
    std::env::var("SIDEREAL_DEBUG_CONTROL_LOGS")
        .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

#[allow(clippy::items_after_test_module)]
#[cfg(test)]
mod tests {
    use super::guid_from_entity_id_like;

    #[test]
    fn parses_prefixed_or_raw_guid() {
        let guid = uuid::Uuid::new_v4();
        assert_eq!(
            guid_from_entity_id_like(&format!("ship:{guid}")),
            Some(guid.to_string())
        );
        assert_eq!(
            guid_from_entity_id_like(&guid.to_string()),
            Some(guid.to_string())
        );
    }

    #[test]
    fn rejects_invalid_identifier() {
        assert_eq!(guid_from_entity_id_like("ship:not-a-guid"), None);
        assert_eq!(guid_from_entity_id_like("definitely-not-a-guid"), None);
    }
}

fn clear_controlled_binding_for_client(
    commands: &mut Commands<'_, '_>,
    client_entity: Entity,
    controlled_entities: &Query<'_, '_, (Entity, Option<&'_ ControlledBy>), With<ActionQueue>>,
) {
    for (entity, controlled_by) in controlled_entities {
        if controlled_by.is_some_and(|binding| binding.owner == client_entity) {
            commands.entity(entity).remove::<ControlledBy>();
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn receive_client_control_requests(
    mut commands: Commands<'_, '_>,
    server_query: Query<'_, '_, &'_ Server, With<RawServer>>,
    mut sender: ServerMultiMessageSender<'_, '_, With<lightyear::prelude::client::Connected>>,
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
    anchor_positions: Query<'_, '_, (&'_ Transform, Option<&'_ avian2d::prelude::Position>)>,
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
    for (client_entity, remote_id, mut receiver) in &mut receivers {
        for message in receiver.receive() {
            let target = NetworkTarget::Single(remote_id.0);
            let Some(bound_player) = bindings.by_client_entity.get(&client_entity) else {
                continue;
            };
            if bound_player != &message.player_entity_id {
                warn!(
                    "replication dropped client control request from {:?}: player mismatch {} != {}",
                    client_entity, message.player_entity_id, bound_player
                );
                let reject = ServerControlRejectMessage {
                    player_entity_id: message.player_entity_id.clone(),
                    request_seq: message.request_seq,
                    reason: "player_mismatch".to_string(),
                    authoritative_controlled_entity_id: None,
                };
                let _ = sender
                    .send::<ServerControlRejectMessage, ControlChannel>(&reject, server, &target);
                continue;
            }
            if let Some(last_seq) = order_state.last_request_seq_by_player.get(bound_player)
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
                        let player_guid = bound_player.strip_prefix("player:")?;
                        let player_runtime_id = format!("player:{player_guid}");
                        if control_guid == player_guid {
                            return Some(player_runtime_id);
                        }
                        controllable_entities
                            .iter()
                            .find(|(_, guid, owner, _)| {
                                owner.0 == *bound_player && guid.0.to_string() == control_guid
                            })
                            .map(|(_, _, _, sim_controlled)| sim_controlled.entity_id.clone())
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
            let player_runtime_id = format!("player:{player_guid}");

            let requested_control_guid = message
                .controlled_entity_id
                .as_deref()
                .and_then(guid_from_entity_id_like);
            let requested_control_raw = message.controlled_entity_id.clone();
            let requested_explicitly_player = requested_control_raw
                .as_deref()
                .is_some_and(|raw| raw.starts_with("player:"));

            let (resolved_control_guid, resolved_target_entity, resolved_runtime_entity_id) =
                if let Some(control_guid) = requested_control_guid.clone() {
                    // Disambiguate by raw identifier intent first.
                    // Some legacy worlds reuse GUIDs across player+ship IDs; GUID-only comparison is ambiguous.
                    if requested_explicitly_player && control_guid == player_guid {
                        (
                            Some(player_guid.clone()),
                            player_entity,
                            Some(player_runtime_id.clone()),
                        )
                    } else {
                        let requested_target =
                            controllable_entities.iter().find(|(_, guid, owner, _)| {
                                guid.0.to_string() == control_guid && owner.0 == *bound_player
                            });
                        if let Some((target_entity, _, _, sim_controlled)) = requested_target {
                            (
                                Some(control_guid),
                                target_entity,
                                Some(sim_controlled.entity_id.clone()),
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
            let currently_bound_entity = controlled_entity_map
                .by_player_entity_id
                .get(bound_player)
                .copied()
                .unwrap_or(player_entity);
            commands
                .entity(player_entity)
                .insert(ControlledEntityGuid(resolved_control_guid.clone()));

            controlled_entity_map
                .by_player_entity_id
                .insert(bound_player.clone(), resolved_target_entity);

            if currently_bound_entity != resolved_target_entity {
                let handoff_anchor_entity = if resolved_target_entity == player_entity {
                    currently_bound_entity
                } else {
                    resolved_target_entity
                };
                if let Ok((anchor_transform, anchor_position)) =
                    anchor_positions.get(handoff_anchor_entity)
                {
                    let anchor_world = anchor_position
                        .map(|position| position.0)
                        .unwrap_or(anchor_transform.translation.truncate());
                    commands.entity(player_entity).insert((
                        Transform::from_translation(anchor_world.extend(0.0)),
                        avian2d::prelude::Position(anchor_world),
                    ));
                }
            }

            // Only rebind ControlledBy when target actually changes; avoid per-update ownership churn
            // that can starve input streams and cause bursty replay behavior.
            let rebind_required = currently_bound_entity != resolved_target_entity;
            if rebind_required {
                clear_controlled_binding_for_client(
                    &mut commands,
                    client_entity,
                    &controlled_entities,
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
                    requested_control_raw,
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
