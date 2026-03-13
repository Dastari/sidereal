use bevy::prelude::*;
use lightyear::prelude::server::{ClientOf, LinkOf};
use lightyear::prelude::{
    InterpolationTarget, LocalTimeline, NetworkTarget, PreSpawned, RemoteId, Replicate,
    ReplicationState, Server, ServerMultiMessageSender,
};
use serde_json::json;
use sidereal_game::{
    BallisticProjectile, BallisticProjectileSpawnedEvent, BallisticWeapon, EntityDestroyedEvent,
    EntityDestructionStartedEvent, EntityGuid, OwnerId, PublicVisibility, ShotFiredEvent,
    ShotHitEvent, ShotImpactResolvedEvent,
};
use sidereal_net::{
    InputChannel, PlayerEntityId, ServerEntityDestructionMessage, ServerWeaponFiredMessage,
};

use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::control::owner_prediction_target;
use crate::replication::runtime_scripting::{ScriptEvent, ScriptEventQueue};

const TRACER_VISUAL_SPEED_MPS: f32 = 1800.0;
const TRACER_VISUAL_MIN_TTL_S: f32 = 0.01;

pub fn init_resources(_app: &mut App) {}

#[allow(clippy::type_complexity)]
pub fn mark_new_ballistic_projectiles_prespawned(
    mut commands: Commands<'_, '_>,
    timeline: Res<'_, LocalTimeline>,
    projectiles: Query<
        '_,
        '_,
        (Entity, &'_ BallisticProjectile),
        (
            With<BallisticProjectile>,
            Added<BallisticProjectile>,
            Without<PreSpawned>,
        ),
    >,
) {
    for (entity, projectile) in &projectiles {
        commands.entity(entity).insert(PreSpawned::new(
            projectile.prespawn_hash_for_tick(timeline.tick().0),
        ));
    }
}

pub fn configure_ballistic_projectile_replication(
    mut commands: Commands<'_, '_>,
    mut spawned_events: MessageReader<'_, '_, BallisticProjectileSpawnedEvent>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    client_remote_ids: Query<'_, '_, &'_ RemoteId, With<ClientOf>>,
    client_player_ids: Query<'_, '_, Entity, With<ClientOf>>,
    projectiles: Query<'_, '_, (), With<BallisticProjectile>>,
) {
    let mut client_entity_by_player_id = std::collections::HashMap::<String, Entity>::new();
    for client_entity in &client_player_ids {
        if let Some(player_entity_id) = bindings.by_client_entity.get(&client_entity) {
            client_entity_by_player_id.insert(player_entity_id.clone(), client_entity);
        }
    }

    for spawned in spawned_events.read() {
        if projectiles.get(spawned.projectile_entity).is_err() {
            continue;
        }
        let mut entity_commands = commands.entity(spawned.projectile_entity);
        entity_commands.insert((Replicate::to_clients(NetworkTarget::All), PublicVisibility));
        if let Some(owner_id) = spawned.owner_id.as_ref()
            && let Some(client_entity) = client_entity_by_player_id.get(owner_id)
            && let Ok(remote_id) = client_remote_ids.get(*client_entity)
        {
            entity_commands.insert(owner_prediction_target(*client_entity));
            entity_commands.insert(InterpolationTarget::to_clients(
                NetworkTarget::AllExceptSingle(remote_id.0),
            ));
            continue;
        }
        entity_commands.insert(InterpolationTarget::to_clients(NetworkTarget::All));
    }
}

pub fn broadcast_weapon_fired_messages(
    server_query: Query<'_, '_, &'_ Server>,
    mut sender: ServerMultiMessageSender<'_, '_, With<lightyear::prelude::client::Connected>>,
    mut resolved_events: MessageReader<'_, '_, ShotImpactResolvedEvent>,
    weapons: Query<'_, '_, (&'_ EntityGuid, &'_ BallisticWeapon)>,
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
    let weapon_audio_by_guid = weapons
        .iter()
        .filter_map(|(guid, weapon)| {
            resolve_weapon_fire_profile_id(weapon)
                .map(|profile_id| (guid.0, (profile_id.to_string(), weapon.cooldown_seconds())))
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
            weapon_guid: resolved.weapon_guid.to_string(),
            audio_profile_id: weapon_audio_by_guid
                .get(&resolved.weapon_guid)
                .map(|(profile_id, _)| profile_id.clone()),
            cooldown_s: weapon_audio_by_guid
                .get(&resolved.weapon_guid)
                .map(|(_, cooldown_s)| *cooldown_s),
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

fn resolve_weapon_fire_profile_id(weapon: &BallisticWeapon) -> Option<&str> {
    if let Some(profile_id) = weapon.fire_audio_profile_id.as_deref() {
        return Some(profile_id);
    }
    (weapon.weapon_name == "Ballistic Gatling").then_some("weapon.ballistic_gatling")
}

pub fn broadcast_entity_destruction_messages(
    server_query: Query<'_, '_, &'_ Server>,
    mut sender: ServerMultiMessageSender<'_, '_, With<lightyear::prelude::client::Connected>>,
    mut started_events: MessageReader<'_, '_, EntityDestructionStartedEvent>,
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
    let replicated_entity_by_guid = replicated_entities
        .iter()
        .map(|(entity, guid, owner_id, _state)| {
            let owner_player_id = owner_id
                .and_then(|owner| PlayerEntityId::parse(owner.0.as_str()))
                .map(|id| id.canonical_wire_id());
            (guid.0, (entity, owner_player_id))
        })
        .collect::<std::collections::HashMap<_, _>>();

    for started in started_events.read() {
        let message = ServerEntityDestructionMessage {
            entity_id: started.entity_guid.to_string(),
            origin_xy: [started.effect_origin.x, started.effect_origin.y],
            destruction_profile_id: started.destruction_profile_id.clone(),
        };
        let Some((runtime_entity, owner_player_id)) =
            replicated_entity_by_guid.get(&started.entity_guid)
        else {
            continue;
        };
        let Ok((_, _, _, replication_state)) = replicated_entities.get(*runtime_entity) else {
            continue;
        };
        for (client_entity, link_of, remote_id) in &client_remotes {
            let Ok(server) = server_query.get(link_of.server) else {
                continue;
            };
            let is_owner_client = owner_player_id.as_ref().is_some_and(|owner_id| {
                client_player_ids
                    .get(&client_entity)
                    .is_some_and(|client_player_id| client_player_id == owner_id)
            });
            if !is_owner_client && !replication_state.is_visible(client_entity) {
                continue;
            }
            let target = NetworkTarget::Single(remote_id.0);
            let _ = sender
                .send::<ServerEntityDestructionMessage, InputChannel>(&message, server, &target);
        }
    }
}

pub fn enqueue_runtime_script_events_from_combat_messages(
    mut fired_events: MessageReader<'_, '_, ShotFiredEvent>,
    mut resolved_events: MessageReader<'_, '_, ShotImpactResolvedEvent>,
    mut hit_events: MessageReader<'_, '_, ShotHitEvent>,
    mut destruction_started_events: MessageReader<'_, '_, EntityDestructionStartedEvent>,
    mut destroyed_events: MessageReader<'_, '_, EntityDestroyedEvent>,
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
    for started in destruction_started_events.read() {
        let payload = json!({
            "entity_id": started.entity_guid.to_string(),
            "destruction_profile_id": started.destruction_profile_id.as_str(),
            "destroy_delay_s": started.destroy_delay_s,
            "position": {
                "x": started.effect_origin.x,
                "y": started.effect_origin.y,
            },
        });
        script_events.pending.push(ScriptEvent {
            event_name: "health_depleted".to_string(),
            payload: payload.clone(),
            target_entity_id: Some(started.entity_guid.to_string()),
        });
        script_events.pending.push(ScriptEvent {
            event_name: "before_destroy".to_string(),
            payload,
            target_entity_id: Some(started.entity_guid.to_string()),
        });
    }
    for destroyed in destroyed_events.read() {
        script_events.pending.push(ScriptEvent {
            event_name: "destroyed".to_string(),
            payload: json!({
                "entity_id": destroyed.entity_guid.to_string(),
                "destruction_profile_id": destroyed.destruction_profile_id.as_str(),
                "position": {
                    "x": destroyed.effect_origin.x,
                    "y": destroyed.effect_origin.y,
                },
            }),
            target_entity_id: Some(destroyed.entity_guid.to_string()),
        });
    }
}
