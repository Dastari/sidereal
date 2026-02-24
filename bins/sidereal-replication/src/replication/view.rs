use bevy::log::warn;
use bevy::prelude::*;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{ControlledBy, MessageReceiver};
use sidereal_game::{
    ControlledEntityGuid, EntityGuid, FocusedEntityGuid, OwnerId, SelectedEntityGuid,
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

fn clear_controlled_binding_for_client(
    commands: &mut Commands<'_, '_>,
    client_entity: Entity,
    controlled_entities: &Query<
        '_,
        '_,
        (Entity, Option<&'_ ControlledBy>),
        With<SimulatedControlledEntity>,
    >,
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
    ship_entities: Query<
        '_,
        '_,
        (Entity, &'_ EntityGuid, &'_ OwnerId),
        With<SimulatedControlledEntity>,
    >,
    controlled_entities: Query<
        '_,
        '_,
        (Entity, Option<&'_ ControlledBy>),
        With<SimulatedControlledEntity>,
    >,
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

            commands.entity(player_entity).insert((
                FocusedEntityGuid(focused_guid),
                SelectedEntityGuid(selected_guid),
                Transform::from_translation(Vec3::from_array(message.camera_position_m)),
            ));

            clear_controlled_binding_for_client(&mut commands, client_entity, &controlled_entities);
            pending_controlled_by
                .bindings
                .retain(|(queued_client, _)| *queued_client != client_entity);

            if let Some(control_guid) = requested_control_guid {
                let target = ship_entities.iter().find(|(_, guid, owner)| {
                    guid.0.to_string() == control_guid && owner.0 == *bound_player
                });
                if let Some((target_entity, _, _)) = target {
                    commands
                        .entity(player_entity)
                        .insert(ControlledEntityGuid(Some(control_guid.clone())));
                    controlled_entity_map
                        .by_player_entity_id
                        .insert(bound_player.clone(), target_entity);
                    pending_controlled_by
                        .bindings
                        .push((client_entity, target_entity));
                } else {
                    commands
                        .entity(player_entity)
                        .insert(ControlledEntityGuid(None));
                    controlled_entity_map
                        .by_player_entity_id
                        .remove(bound_player);
                }
            } else {
                commands
                    .entity(player_entity)
                    .insert(ControlledEntityGuid(None));
                controlled_entity_map
                    .by_player_entity_id
                    .remove(bound_player);
            }
        }
    }
}
