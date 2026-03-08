use bevy::prelude::*;
use lightyear::prelude::server::{ClientOf, LinkOf};
use lightyear::prelude::{
    NetworkTarget, RemoteId, ReplicationState, Server, ServerMultiMessageSender,
};
use serde_json::json;
use sidereal_game::{EntityGuid, OwnerId, ShotFiredEvent, ShotHitEvent, ShotImpactResolvedEvent};
use sidereal_net::{InputChannel, PlayerEntityId, ServerWeaponFiredMessage};

use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::runtime_scripting::{ScriptEvent, ScriptEventQueue};

const TRACER_VISUAL_SPEED_MPS: f32 = 1800.0;
const TRACER_VISUAL_MIN_TTL_S: f32 = 0.01;

pub fn init_resources(_app: &mut App) {}

pub fn broadcast_weapon_fired_messages(
    server_query: Query<'_, '_, &'_ Server>,
    mut sender: ServerMultiMessageSender<'_, '_, With<lightyear::prelude::client::Connected>>,
    mut resolved_events: MessageReader<'_, '_, ShotImpactResolvedEvent>,
    client_remotes: Query<'_, '_, (Entity, &'_ LinkOf, &'_ RemoteId), With<ClientOf>>,
    replicated_entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Option<&'_ OwnerId>,
            &'_ ReplicationState,
        ),
    >,
    bindings: Res<'_, AuthenticatedClientBindings>,
) {
    let client_player_ids = bindings
        .by_client_entity
        .iter()
        .filter_map(|(client_entity, player_entity_id)| {
            PlayerEntityId::parse(player_entity_id.as_str())
                .map(|id| (*client_entity, id.canonical_wire_id()))
        })
        .collect::<std::collections::HashMap<_, _>>();
    let shooter_entity_by_guid = replicated_entities
        .iter()
        .map(|(entity, guid, owner_id, _)| {
            let owner_player_id = owner_id
                .and_then(|owner| PlayerEntityId::parse(owner.0.as_str()))
                .map(|id| id.canonical_wire_id());
            (guid.0, (entity, owner_player_id))
        })
        .collect::<std::collections::HashMap<_, _>>();

    for resolved in resolved_events.read() {
        let travel = resolved.impact_pos - resolved.origin;
        if travel.length_squared() <= f32::EPSILON {
            continue;
        }
        let direction = travel.normalize();
        let initial_velocity = direction * TRACER_VISUAL_SPEED_MPS;
        let impact_distance_m = travel.length().min(resolved.max_range_m.max(1.0));
        let visual_ttl_s =
            (impact_distance_m / TRACER_VISUAL_SPEED_MPS).max(TRACER_VISUAL_MIN_TTL_S);

        let message = ServerWeaponFiredMessage {
            shooter_entity_id: resolved.shooter_guid.to_string(),
            origin_xy: [resolved.origin.x, resolved.origin.y],
            velocity_xy: [initial_velocity.x, initial_velocity.y],
            impact_xy: Some([resolved.impact_pos.x, resolved.impact_pos.y]),
            ttl_s: visual_ttl_s,
        };
        let Some((shooter_entity, shooter_owner_player_id)) =
            shooter_entity_by_guid.get(&resolved.shooter_guid)
        else {
            continue;
        };
        let Ok((_, _, _, shooter_replication_state)) = replicated_entities.get(*shooter_entity)
        else {
            continue;
        };
        for (client_entity, link_of, remote_id) in &client_remotes {
            let Ok(server) = server_query.get(link_of.server) else {
                continue;
            };
            let is_shooter_owner_client =
                shooter_owner_player_id.as_ref().is_some_and(|owner_id| {
                    client_player_ids
                        .get(&client_entity)
                        .is_some_and(|client_player_id| client_player_id == owner_id)
                });
            if !is_shooter_owner_client && !shooter_replication_state.is_visible(client_entity) {
                continue;
            }
            let target = NetworkTarget::Single(remote_id.0);
            let _ =
                sender.send::<ServerWeaponFiredMessage, InputChannel>(&message, server, &target);
        }
    }
}

pub fn enqueue_runtime_script_events_from_combat_messages(
    mut fired_events: MessageReader<'_, '_, ShotFiredEvent>,
    mut resolved_events: MessageReader<'_, '_, ShotImpactResolvedEvent>,
    mut hit_events: MessageReader<'_, '_, ShotHitEvent>,
    mut script_events: ResMut<'_, ScriptEventQueue>,
) {
    for fired in fired_events.read() {
        script_events.pending.push(ScriptEvent {
            event_name: "shot_fired".to_string(),
            payload: json!({
                "shooter_entity_id": fired.shooter_guid.to_string(),
                "weapon_entity_id": fired.weapon_guid.to_string(),
                "origin": { "x": fired.origin.x, "y": fired.origin.y },
                "direction": { "x": fired.direction.x, "y": fired.direction.y },
                "max_range_m": fired.max_range_m,
                "damage_per_shot": fired.damage_per_shot,
            }),
            target_entity_id: None,
        });
    }
    for resolved in resolved_events.read() {
        script_events.pending.push(ScriptEvent {
            event_name: "shot_impact".to_string(),
            payload: json!({
                "shooter_entity_id": resolved.shooter_guid.to_string(),
                "weapon_entity_id": resolved.weapon_guid.to_string(),
                "impact_position": { "x": resolved.impact_pos.x, "y": resolved.impact_pos.y },
                "target_entity_id": resolved.target_guid.map(|guid| guid.to_string()),
                "damage_per_shot": resolved.damage_per_shot,
            }),
            target_entity_id: None,
        });
    }
    for hit in hit_events.read() {
        script_events.pending.push(ScriptEvent {
            event_name: "damage_applied".to_string(),
            payload: json!({
                "shooter_entity_id": hit.shooter_guid.to_string(),
                "target_entity_id": hit.target_guid.map(|guid| guid.to_string()),
                "weapon_entity_id": hit.weapon_guid.to_string(),
                "damage": hit.damage,
            }),
            target_entity_id: None,
        });
    }
}
