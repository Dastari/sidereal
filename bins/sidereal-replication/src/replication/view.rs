use bevy::log::{info, warn};
use bevy::prelude::*;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{ControlledBy, MessageReceiver};
use sidereal_game::{
    ActionQueue, ControlledEntityGuid, EntityGuid, FocusedEntityGuid, OwnerId, PlayerTag,
    SelectedEntityGuid,
};

use sidereal_net::ClientViewUpdateMessage;

use crate::{
    AuthenticatedClientBindings,
    replication::{
        PendingControlledByBindings, PlayerControlledEntityMap, PlayerRuntimeEntityMap,
        SimulatedControlledEntity,
    },
};

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
pub fn receive_client_view_updates(
    mut commands: Commands<'_, '_>,
    mut receivers: Query<
        '_,
        '_,
        (Entity, &'_ mut MessageReceiver<ClientViewUpdateMessage>),
        With<ClientOf>,
    >,
    controllable_entities: Query<
        '_,
        '_,
        (Entity, &'_ EntityGuid, &'_ OwnerId),
        With<SimulatedControlledEntity>,
    >,
    controlled_entities: Query<'_, '_, (Entity, Option<&'_ ControlledBy>), With<ActionQueue>>,
    anchor_positions: Query<'_, '_, (&'_ Transform, Option<&'_ avian3d::prelude::Position>)>,
    player_guids: Query<'_, '_, &'_ EntityGuid, With<PlayerTag>>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    player_entities: Res<'_, PlayerRuntimeEntityMap>,
    mut controlled_entity_map: ResMut<'_, PlayerControlledEntityMap>,
    mut pending_controlled_by: ResMut<'_, PendingControlledByBindings>,
) {
    for (client_entity, mut receiver) in &mut receivers {
        for message in receiver.receive() {
            let Some(bound_player) = bindings.by_client_entity.get(&client_entity) else {
                continue;
            };
            if bound_player != &message.player_entity_id {
                warn!(
                    "replication dropped client view update from {:?}: player mismatch {} != {}",
                    client_entity, message.player_entity_id, bound_player
                );
                continue;
            }

            let Some(&player_entity) = player_entities.by_player_entity_id.get(bound_player) else {
                warn!(
                    "replication dropped view update for {}: no hydrated player entity",
                    bound_player
                );
                continue;
            };
            let Ok(player_guid) = player_guids
                .get(player_entity)
                .map(|guid| guid.0.to_string())
            else {
                warn!(
                    "replication dropped view update for {}: player entity missing EntityGuid",
                    bound_player
                );
                continue;
            };

            let focused_guid = message
                .focused_entity_id
                .as_deref()
                .and_then(guid_from_entity_id_like);
            let selected_guid = message
                .selected_entity_id
                .as_deref()
                .and_then(guid_from_entity_id_like);
            let requested_control_guid = message
                .controlled_entity_id
                .as_deref()
                .and_then(guid_from_entity_id_like);
            let requested_control_raw = message.controlled_entity_id.clone();
            let requested_explicitly_player = requested_control_raw
                .as_deref()
                .is_some_and(|raw| raw.starts_with("player:"));

            commands.entity(player_entity).insert((
                FocusedEntityGuid(focused_guid),
                SelectedEntityGuid(selected_guid),
            ));

            let (resolved_control_guid, resolved_target_entity) = if let Some(control_guid) =
                requested_control_guid.clone()
            {
                // Disambiguate by raw identifier intent first.
                // Some legacy worlds reuse GUIDs across player+ship IDs; GUID-only comparison is ambiguous.
                if requested_explicitly_player && control_guid == player_guid {
                    (Some(player_guid.clone()), player_entity)
                } else {
                    let target = controllable_entities.iter().find(|(_, guid, owner)| {
                        guid.0.to_string() == control_guid && owner.0 == *bound_player
                    });
                    if let Some((target_entity, _, _)) = target {
                        (Some(control_guid), target_entity)
                    } else {
                        warn!(
                            "replication rejected control request for {} -> {} (target not found or not owned)",
                            bound_player, control_guid
                        );
                        (Some(player_guid.clone()), player_entity)
                    }
                }
            } else {
                (Some(player_guid.clone()), player_entity)
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
                        .unwrap_or(anchor_transform.translation);
                    commands.entity(player_entity).insert((
                        Transform::from_translation(anchor_world),
                        avian3d::prelude::Position(anchor_world),
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
                    "control route player={} client={:?} requested_raw={:?} requested_guid={:?} resolved_guid={:?} current_entity={:?} target_entity={:?} rebind_required={}",
                    bound_player,
                    client_entity,
                    requested_control_raw,
                    requested_control_guid,
                    resolved_control_guid,
                    currently_bound_entity,
                    resolved_target_entity,
                    rebind_required
                );
            }
        }
    }
}
