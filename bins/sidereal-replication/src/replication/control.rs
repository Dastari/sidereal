use bevy::log::{info, warn};
use bevy::prelude::*;
use lightyear::prelude::server::{ClientOf, LinkOf};
use lightyear::prelude::{
    ControlledBy, InterpolationTarget, Lifetime, MessageReceiver, NetworkTarget, PredictionTarget,
    RemoteId, Replicate, Server, ServerMultiMessageSender,
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
    PlayerControlledEntityMap, PlayerRuntimeEntityMap, SimulatedControlledEntity, debug_env,
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
    uuid::Uuid::parse_str(raw).ok().map(|guid| guid.to_string())
}

fn control_debug_logging_enabled() -> bool {
    debug_env("SIDEREAL_DEBUG_CONTROL_LOGS")
}

fn control_target_log_label(value: Option<&str>) -> &str {
    value.unwrap_or("<none>")
}

pub(crate) fn owner_only_replicate(client_entity: Entity) -> Replicate {
    // Sidereal dynamic handoff already knows the concrete ReplicationSender entity that owns the
    // lane. Prefer Manual(sender_entity) over peer-id NetworkTarget here so control rebinding does
    // not depend on PeerMetadata mapping timing.
    Replicate::manual(vec![client_entity])
}

pub(crate) fn owner_prediction_target(client_entity: Entity) -> PredictionTarget {
    // Same rationale as owner_only_replicate(): dynamic owner prediction is a sender-local concern.
    // Using the resolved sender entity keeps Sidereal aligned with Lightyear's target model while
    // avoiding an extra remote-id->sender lookup during handoff/application.
    PredictionTarget::manual(vec![client_entity])
}

pub(crate) fn owner_interpolation_target(client_entity: Entity) -> InterpolationTarget {
    // The persisted player anchor is owner-only. When it is not the active predicted entity we keep
    // it interpolated only for that owner rather than broadcasting it as a generic observer target.
    InterpolationTarget::manual(vec![client_entity])
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
    server_query: Query<'_, '_, &'_ Server>,
    mut sender: ServerMultiMessageSender<'_, '_, With<lightyear::prelude::client::Connected>>,
    time: Res<'_, Time<Real>>,
    mut last_activity: ResMut<'_, crate::replication::lifecycle::ClientLastActivity>,
    mut receivers: Query<
        '_,
        '_,
        (
            Entity,
            &'_ LinkOf,
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
    player_guids: Query<'_, '_, &'_ EntityGuid, With<PlayerTag>>,
    player_controlled: Query<'_, '_, &'_ ControlledEntityGuid, With<PlayerTag>>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    mut order_state: ResMut<'_, ClientControlRequestOrder>,
    player_entities: Res<'_, PlayerRuntimeEntityMap>,
    mut controlled_entity_map: ResMut<'_, PlayerControlledEntityMap>,
) {
    let now_s = time.elapsed_secs_f64();
    for (client_entity, link_of, remote_id, mut receiver) in &mut receivers {
        let Ok(server) = server_query.get(link_of.server) else {
            warn!(
                "replication control: missing server entity for client {:?} remote {:?}",
                client_entity, remote_id.0
            );
            continue;
        };
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
            let requested_raw = message.controlled_entity_id.clone();
            let requested_control_guid = message
                .controlled_entity_id
                .as_deref()
                .and_then(guid_from_entity_id_like);
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
                        "server control handover stale player={} client={:?} remote={} seq={} previous_seq={} requested_raw={} requested_guid={} previous_controlled={} result=reject reason=stale_seq",
                        bound_player,
                        client_entity,
                        remote_id.0,
                        message.request_seq,
                        last_seq,
                        control_target_log_label(requested_raw.as_deref()),
                        control_target_log_label(requested_control_guid.as_deref()),
                        control_target_log_label(
                            player_entities
                                .by_player_entity_id
                                .get(bound_player)
                                .and_then(|player_entity| player_controlled
                                    .get(*player_entity)
                                    .ok())
                                .and_then(|guid| guid.0.as_deref())
                        ),
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
            let previous_controlled_guid = player_controlled
                .get(player_entity)
                .ok()
                .and_then(|guid| guid.0.clone())
                .or_else(|| Some(player_runtime_id.clone()));
            if control_debug_logging_enabled() {
                info!(
                    "server control handover request player={} client={:?} remote={} seq={} previous_controlled={} requested_raw={} requested_guid={}",
                    bound_player,
                    client_entity,
                    remote_id.0,
                    message.request_seq,
                    control_target_log_label(previous_controlled_guid.as_deref()),
                    control_target_log_label(requested_raw.as_deref()),
                    control_target_log_label(requested_control_guid.as_deref()),
                );
            }
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
                            if control_debug_logging_enabled() {
                                warn!(
                                    "server control handover reject player={} client={:?} remote={} seq={} previous_controlled={} requested_raw={} requested_guid={} result=reject reason=target_not_owned_or_missing authoritative_controlled={}",
                                    bound_player,
                                    client_entity,
                                    remote_id.0,
                                    message.request_seq,
                                    control_target_log_label(previous_controlled_guid.as_deref()),
                                    control_target_log_label(requested_raw.as_deref()),
                                    control_target_log_label(requested_control_guid.as_deref()),
                                    player_runtime_id,
                                );
                            }
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
            }

            if control_debug_logging_enabled() {
                info!(
                    "server control handover resolved player={} client={:?} remote={} seq={} previous_controlled={} requested_raw={} requested_guid={} resolved_guid={} previous_entity={:?} target_entity={:?} rebind_required={} result=ack authoritative_controlled={}",
                    bound_player,
                    client_entity,
                    remote_id.0,
                    message.request_seq,
                    control_target_log_label(previous_controlled_guid.as_deref()),
                    control_target_log_label(requested_raw.as_deref()),
                    control_target_log_label(requested_control_guid.as_deref()),
                    control_target_log_label(resolved_control_guid.as_deref()),
                    currently_bound_entity,
                    resolved_target_entity,
                    rebind_required,
                    control_target_log_label(resolved_runtime_entity_id.as_deref())
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

fn maybe_set_controlled_by(
    entity_commands: &mut EntityCommands<'_>,
    current: Option<&ControlledBy>,
    desired_owner: Option<Entity>,
) {
    match desired_owner {
        Some(owner)
            if current.is_some_and(|controlled_by| {
                controlled_by.owner == owner && controlled_by.lifetime == Lifetime::Persistent
            }) => {}
        Some(owner) => {
            entity_commands.insert(ControlledBy {
                owner,
                lifetime: Lifetime::Persistent,
            });
        }
        None if current.is_some() => {
            entity_commands.remove::<ControlledBy>();
        }
        None => {}
    }
}

fn maybe_set_replicate(
    entity_commands: &mut EntityCommands<'_>,
    current: Option<&Replicate>,
    desired: &Replicate,
) {
    if current != Some(desired) {
        entity_commands.insert(desired.clone());
    }
}

enum DesiredInterpolationTarget {
    Owner(Entity),
    Network(NetworkTarget),
}

fn maybe_set_prediction_target(
    entity_commands: &mut EntityCommands<'_>,
    current: Option<&PredictionTarget>,
    desired_owner: Option<Entity>,
) {
    let current_debug = current.map(|target| format!("{target:?}"));
    let desired_debug = desired_owner.map(|owner| format!("{:?}", owner_prediction_target(owner)));
    match desired_owner {
        Some(owner) if current_debug == desired_debug => {}
        Some(owner) => {
            entity_commands.insert(owner_prediction_target(owner));
        }
        None if current.is_some() => {
            entity_commands.remove::<PredictionTarget>();
        }
        None => {}
    }
}

fn maybe_set_interpolation_target(
    entity_commands: &mut EntityCommands<'_>,
    current: Option<&InterpolationTarget>,
    desired: Option<DesiredInterpolationTarget>,
) {
    let current_debug = current.map(|target| format!("{target:?}"));
    let desired_debug = desired.as_ref().map(|target| match target {
        DesiredInterpolationTarget::Owner(owner) => {
            format!("{:?}", owner_interpolation_target(*owner))
        }
        DesiredInterpolationTarget::Network(network) => {
            format!("{:?}", InterpolationTarget::to_clients(network.clone()))
        }
    });
    match desired {
        Some(DesiredInterpolationTarget::Owner(owner)) if current_debug == desired_debug => {}
        Some(DesiredInterpolationTarget::Owner(owner)) => {
            entity_commands.insert(owner_interpolation_target(owner));
        }
        Some(DesiredInterpolationTarget::Network(network)) if current_debug == desired_debug => {}
        Some(DesiredInterpolationTarget::Network(network)) => {
            entity_commands.insert(InterpolationTarget::to_clients(network));
        }
        None if current.is_some() => {
            entity_commands.remove::<InterpolationTarget>();
        }
        None => {}
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn reconcile_control_replication_roles(
    mut commands: Commands<'_, '_>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    player_entity_map: Res<'_, PlayerRuntimeEntityMap>,
    controlled_entity_map: Res<'_, PlayerControlledEntityMap>,
    client_remote_ids: Query<'_, '_, &'_ RemoteId, With<ClientOf>>,
    entity_guids: Query<'_, '_, &'_ EntityGuid>,
    players: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Option<&'_ ControlledEntityGuid>,
            Option<&'_ ControlledBy>,
            Option<&'_ Replicate>,
            Option<&'_ PredictionTarget>,
            Option<&'_ InterpolationTarget>,
        ),
        With<PlayerTag>,
    >,
    controlled_entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ SimulatedControlledEntity,
            Option<&'_ ControlledBy>,
            Option<&'_ Replicate>,
            Option<&'_ PredictionTarget>,
            Option<&'_ InterpolationTarget>,
        ),
        Without<PlayerTag>,
    >,
) {
    let mut bound_client_by_player_wire = HashMap::<String, Entity>::new();
    let mut desired_controlled_by_client = HashMap::<Entity, Entity>::new();
    let mut desired_control_guid_by_player = HashMap::<Entity, Option<String>>::new();
    let mut desired_owner_by_entity = HashMap::<Entity, Entity>::new();

    for (client_entity, player_wire) in &bindings.by_client_entity {
        let Some(player_id) = PlayerEntityId::parse(player_wire.as_str()) else {
            continue;
        };
        let Some(&player_entity) = player_entity_map.by_player_entity_id.get(player_wire) else {
            continue;
        };
        let desired_controlled_entity = controlled_entity_map
            .by_player_entity_id
            .get(&player_id)
            .copied()
            .unwrap_or(player_entity);
        let desired_control_guid = entity_guids
            .get(desired_controlled_entity)
            .ok()
            .map(|guid| guid.0.to_string());

        bound_client_by_player_wire.insert(player_wire.clone(), *client_entity);
        desired_controlled_by_client.insert(*client_entity, desired_controlled_entity);
        desired_control_guid_by_player.insert(player_entity, desired_control_guid);
        desired_owner_by_entity.insert(desired_controlled_entity, *client_entity);
    }

    for (
        entity,
        player_guid,
        current_control_guid,
        current_controlled_by,
        current_replicate,
        current_prediction,
        current_interpolation,
    ) in &players
    {
        let player_wire = player_guid.0.to_string();
        let bound_client = bound_client_by_player_wire
            .get(player_wire.as_str())
            .copied();
        let desired_control_guid = desired_control_guid_by_player
            .get(&entity)
            .cloned()
            .flatten()
            .or_else(|| Some(player_wire.clone()));
        let controls_self = desired_control_guid
            .as_deref()
            .and_then(guid_from_entity_id_like)
            .is_none_or(|guid| guid == player_wire);

        let mut entity_commands = commands.entity(entity);
        if current_control_guid.and_then(|value| value.0.as_ref()) != desired_control_guid.as_ref()
        {
            entity_commands.insert(ControlledEntityGuid(desired_control_guid));
        }

        match bound_client {
            Some(client_entity) => {
                maybe_set_controlled_by(
                    &mut entity_commands,
                    current_controlled_by,
                    Some(client_entity),
                );
                let desired_replicate = owner_only_replicate(client_entity);
                maybe_set_replicate(&mut entity_commands, current_replicate, &desired_replicate);

                maybe_set_prediction_target(
                    &mut entity_commands,
                    current_prediction,
                    controls_self.then_some(client_entity),
                );
                maybe_set_interpolation_target(
                    &mut entity_commands,
                    current_interpolation,
                    (!controls_self).then_some(DesiredInterpolationTarget::Owner(client_entity)),
                );
            }
            None => {
                maybe_set_controlled_by(&mut entity_commands, current_controlled_by, None);
                maybe_set_replicate(
                    &mut entity_commands,
                    current_replicate,
                    &Replicate::to_clients(NetworkTarget::None),
                );
                maybe_set_prediction_target(&mut entity_commands, current_prediction, None);
                maybe_set_interpolation_target(&mut entity_commands, current_interpolation, None);
            }
        }
    }

    for (
        entity,
        simulated_controlled,
        current_controlled_by,
        current_replicate,
        current_prediction,
        current_interpolation,
    ) in &controlled_entities
    {
        let desired_owner = desired_owner_by_entity
            .get(&entity)
            .copied()
            .filter(|owner| {
                bindings
                    .by_client_entity
                    .get(owner)
                    .and_then(|player_wire| PlayerEntityId::parse(player_wire.as_str()))
                    .is_some_and(|player_id| player_id == simulated_controlled.player_entity_id)
            });

        let mut entity_commands = commands.entity(entity);
        maybe_set_controlled_by(&mut entity_commands, current_controlled_by, desired_owner);
        maybe_set_replicate(
            &mut entity_commands,
            current_replicate,
            &Replicate::to_clients(NetworkTarget::All),
        );

        let desired_interpolation = match desired_owner {
            Some(owner) => client_remote_ids.get(owner).ok().map(|remote_id| {
                DesiredInterpolationTarget::Network(NetworkTarget::AllExceptSingle(remote_id.0))
            }),
            None => Some(DesiredInterpolationTarget::Network(NetworkTarget::All)),
        };

        maybe_set_prediction_target(&mut entity_commands, current_prediction, desired_owner);
        maybe_set_interpolation_target(
            &mut entity_commands,
            current_interpolation,
            desired_interpolation,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        owner_interpolation_target, owner_only_replicate, owner_prediction_target,
        reconcile_control_replication_roles,
    };
    use crate::replication::auth::AuthenticatedClientBindings;
    use crate::replication::{
        PlayerControlledEntityMap, PlayerRuntimeEntityMap, SimulatedControlledEntity,
    };
    use bevy::prelude::*;
    use lightyear::prelude::server::ClientOf;
    use lightyear::prelude::{
        ControlledBy, InterpolationTarget, PeerId, PredictionTarget, RemoteId, Replicate,
    };
    use sidereal_game::{ControlledEntityGuid, EntityGuid, PlayerTag};
    use sidereal_net::PlayerEntityId;
    use uuid::Uuid;

    #[test]
    fn reconcile_assigns_owner_predicted_ship_roles_without_visibility_churn() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(lightyear::prelude::server::ServerPlugins::default());
        app.init_resource::<AuthenticatedClientBindings>();
        app.init_resource::<PlayerControlledEntityMap>();
        app.init_resource::<PlayerRuntimeEntityMap>();
        app.add_systems(Update, reconcile_control_replication_roles);

        let player_id =
            PlayerEntityId(Uuid::parse_str("1521601b-7e69-4700-853f-eb1eb3a41199").unwrap());
        let ship_guid = Uuid::parse_str("ce9e421c-8b62-458a-803e-51e9ad272908").unwrap();
        let client = app
            .world_mut()
            .spawn((ClientOf, RemoteId(PeerId::Netcode(42))))
            .id();
        let player_entity = app
            .world_mut()
            .spawn((PlayerTag, EntityGuid(player_id.0)))
            .id();
        let ship_entity = app
            .world_mut()
            .spawn((
                EntityGuid(ship_guid),
                SimulatedControlledEntity {
                    player_entity_id: player_id,
                },
            ))
            .id();

        app.world_mut()
            .resource_mut::<AuthenticatedClientBindings>()
            .by_client_entity
            .insert(client, player_id.canonical_wire_id());
        app.world_mut()
            .resource_mut::<PlayerRuntimeEntityMap>()
            .by_player_entity_id
            .insert(player_id.canonical_wire_id(), player_entity);
        app.world_mut()
            .resource_mut::<PlayerControlledEntityMap>()
            .by_player_entity_id
            .insert(player_id, ship_entity);

        app.update();

        let player_control_guid = app
            .world()
            .get::<ControlledEntityGuid>(player_entity)
            .expect("player control guid");
        assert_eq!(
            player_control_guid.0.as_deref(),
            Some(ship_guid.to_string().as_str())
        );
        assert_eq!(
            app.world().get::<ControlledBy>(player_entity),
            Some(&ControlledBy {
                owner: client,
                lifetime: lightyear::prelude::Lifetime::Persistent,
            })
        );
        assert_eq!(
            app.world().get::<Replicate>(player_entity),
            Some(&owner_only_replicate(client))
        );
        assert_eq!(
            app.world()
                .get::<InterpolationTarget>(player_entity)
                .map(|target| format!("{target:?}")),
            Some(format!("{:?}", owner_interpolation_target(client)))
        );
        assert!(app.world().get::<PredictionTarget>(player_entity).is_none());

        assert_eq!(
            app.world().get::<ControlledBy>(ship_entity),
            Some(&ControlledBy {
                owner: client,
                lifetime: lightyear::prelude::Lifetime::Persistent,
            })
        );
        assert_eq!(
            app.world().get::<Replicate>(ship_entity),
            Some(&Replicate::to_clients(
                lightyear::prelude::NetworkTarget::All
            ))
        );
        assert_eq!(
            app.world()
                .get::<PredictionTarget>(ship_entity)
                .map(|target| format!("{target:?}")),
            Some(format!("{:?}", owner_prediction_target(client)))
        );
        assert_eq!(
            app.world()
                .get::<InterpolationTarget>(ship_entity)
                .map(|target| format!("{target:?}")),
            Some(format!(
                "{:?}",
                InterpolationTarget::to_clients(
                    lightyear::prelude::NetworkTarget::AllExceptSingle(PeerId::Netcode(42)),
                )
            ))
        );
    }

    #[test]
    fn reconcile_clears_stale_owner_roles_when_client_binding_is_gone() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(lightyear::prelude::server::ServerPlugins::default());
        app.init_resource::<AuthenticatedClientBindings>();
        app.init_resource::<PlayerControlledEntityMap>();
        app.init_resource::<PlayerRuntimeEntityMap>();
        app.add_systems(Update, reconcile_control_replication_roles);

        let player_id =
            PlayerEntityId(Uuid::parse_str("1521601b-7e69-4700-853f-eb1eb3a41199").unwrap());
        let client = app.world_mut().spawn_empty().id();
        let player_entity = app
            .world_mut()
            .spawn((
                PlayerTag,
                EntityGuid(player_id.0),
                ControlledBy {
                    owner: client,
                    lifetime: lightyear::prelude::Lifetime::Persistent,
                },
                owner_only_replicate(client),
                owner_prediction_target(client),
            ))
            .id();
        let ship_entity = app
            .world_mut()
            .spawn((
                EntityGuid(Uuid::parse_str("ce9e421c-8b62-458a-803e-51e9ad272908").unwrap()),
                SimulatedControlledEntity {
                    player_entity_id: player_id,
                },
                ControlledBy {
                    owner: client,
                    lifetime: lightyear::prelude::Lifetime::Persistent,
                },
                owner_prediction_target(client),
                Replicate::to_clients(lightyear::prelude::NetworkTarget::All),
                InterpolationTarget::to_clients(
                    lightyear::prelude::NetworkTarget::AllExceptSingle(PeerId::Netcode(42)),
                ),
            ))
            .id();

        app.world_mut()
            .resource_mut::<PlayerRuntimeEntityMap>()
            .by_player_entity_id
            .insert(player_id.canonical_wire_id(), player_entity);
        app.world_mut()
            .resource_mut::<PlayerControlledEntityMap>()
            .by_player_entity_id
            .insert(player_id, ship_entity);
        app.world_mut().entity_mut(client).despawn();

        app.update();

        assert!(app.world().get::<ControlledBy>(player_entity).is_none());
        assert!(app.world().get::<PredictionTarget>(player_entity).is_none());
        assert!(
            app.world()
                .get::<InterpolationTarget>(player_entity)
                .is_none()
        );
        assert_eq!(
            app.world().get::<Replicate>(player_entity),
            Some(&Replicate::to_clients(
                lightyear::prelude::NetworkTarget::None
            ))
        );

        assert!(app.world().get::<ControlledBy>(ship_entity).is_none());
        assert!(app.world().get::<PredictionTarget>(ship_entity).is_none());
        assert_eq!(
            app.world()
                .get::<InterpolationTarget>(ship_entity)
                .map(|target| format!("{target:?}")),
            Some(format!(
                "{:?}",
                InterpolationTarget::to_clients(lightyear::prelude::NetworkTarget::All,)
            ))
        );
    }
}
