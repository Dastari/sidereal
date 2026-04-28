use bevy::log::{info, warn};
use bevy::prelude::*;
use lightyear::prelude::server::{ClientOf, LinkOf};
use lightyear::prelude::{
    ControlledBy, InterpolationTarget, Lifetime, MessageReceiver, NetworkTarget, PredictionTarget,
    RemoteId, Replicate, ReplicationState, Server, ServerMultiMessageSender,
};
use sidereal_game::{
    ActionQueue, AfterburnerState, ControlledEntityGuid, EntityGuid, FlightComputer, OwnerId,
    PlayerTag,
};
use std::collections::{HashMap, HashSet};

use sidereal_net::{
    ClientControlRequestMessage, ControlChannel, PlayerEntityId, ServerControlAckMessage,
    ServerControlRejectMessage,
};

use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::input::RealtimeInputCleanupState;
use crate::replication::persistence::{
    PersistenceDirtyState, SimulationPersistenceTimer, persist_entity_snapshot_async,
};
use crate::replication::{
    PlayerControlledEntityMap, PlayerRuntimeEntityMap, SimulatedControlledEntity,
    visibility::{VisibilityMembershipCache, VisibilitySpatialIndex},
};

#[derive(Resource, Default)]
pub struct ClientControlRequestOrder {
    pub last_request_seq_by_player: HashMap<String, u64>,
}

#[derive(Resource, Default)]
pub struct ClientControlLeaseGenerations {
    pub generation_by_player: HashMap<String, u64>,
}

impl ClientControlLeaseGenerations {
    pub(crate) fn ensure_initialized_for_player(&mut self, player_wire: &str) -> u64 {
        let generation = self
            .generation_by_player
            .entry(player_wire.to_string())
            .or_insert(1);
        *generation
    }
}

#[derive(Debug, Clone)]
struct PendingControlAck {
    server_entity: Entity,
    remote_peer_id: lightyear::prelude::PeerId,
    message: ServerControlAckMessage,
}

#[derive(Resource, Default)]
pub struct PendingControlAckQueue {
    queued: Vec<PendingControlAck>,
}

#[derive(Resource, Default)]
pub struct RoleVisibilityRearmState {
    pending_loss_passes: HashMap<(Entity, Entity), u8>,
}

impl RoleVisibilityRearmState {
    const SUPPRESS_MEMBERSHIP_PASSES: u8 = 1;

    fn queue_loss_pass(&mut self, entity: Entity, client_entity: Entity) {
        self.pending_loss_passes
            .insert((entity, client_entity), Self::SUPPRESS_MEMBERSHIP_PASSES);
    }

    pub fn suppress_desired_clients(
        &self,
        entity: Entity,
        desired_visible_clients: &mut HashSet<Entity>,
    ) {
        desired_visible_clients.retain(|client_entity| {
            !self
                .pending_loss_passes
                .contains_key(&(entity, *client_entity))
        });
    }

    pub fn advance_after_membership_pass(&mut self) {
        self.pending_loss_passes.retain(|_, passes| {
            *passes = passes.saturating_sub(1);
            *passes > 0
        });
    }

    #[cfg(test)]
    fn is_pending(&self, entity: Entity, client_entity: Entity) -> bool {
        self.pending_loss_passes
            .contains_key(&(entity, client_entity))
    }
}

pub fn init_resources(app: &mut App) {
    app.insert_resource(ClientControlRequestOrder::default());
    app.insert_resource(ClientControlLeaseGenerations::default());
    app.insert_resource(PendingControlAckQueue::default());
    app.insert_resource(RoleVisibilityRearmState::default());
}

#[doc(hidden)]
pub fn guid_from_entity_id_like(raw: &str) -> Option<String> {
    uuid::Uuid::parse_str(raw).ok().map(|guid| guid.to_string())
}

fn control_target_log_label(value: Option<&str>) -> &str {
    value.unwrap_or("<none>")
}

fn next_control_generation(current_generation: u64, changed: bool) -> u64 {
    match (current_generation, changed) {
        (0, _) => 1,
        (current, true) => current.saturating_add(1),
        (current, false) => current,
    }
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

fn observer_interpolation_target(
    bindings: &AuthenticatedClientBindings,
    owner_client_entity: Entity,
) -> Option<InterpolationTarget> {
    let mut observer_clients = bindings
        .by_client_entity
        .keys()
        .copied()
        .filter(|client_entity| *client_entity != owner_client_entity)
        .collect::<Vec<_>>();
    observer_clients.sort_by_key(|entity| entity.to_bits());
    (!observer_clients.is_empty()).then_some(InterpolationTarget::manual(observer_clients))
}

pub(crate) fn neutralize_control_intent(world: &mut World, controlled_entity: Entity) -> bool {
    let mut changed = false;
    if let Some(mut queue) = world.get_mut::<ActionQueue>(controlled_entity)
        && !queue.pending.is_empty()
    {
        queue.clear();
        changed = true;
    }
    if let Some(mut flight_computer) = world.get_mut::<FlightComputer>(controlled_entity)
        && (flight_computer.throttle != 0.0
            || flight_computer.yaw_input != 0.0
            || flight_computer.brake_active)
    {
        flight_computer.throttle = 0.0;
        flight_computer.yaw_input = 0.0;
        flight_computer.brake_active = false;
        changed = true;
    }
    if let Some(mut afterburner_state) = world.get_mut::<AfterburnerState>(controlled_entity)
        && afterburner_state.active
    {
        afterburner_state.active = false;
        changed = true;
    }
    changed
}

pub(crate) fn queue_neutralize_control_intent(
    commands: &mut Commands<'_, '_>,
    controlled_entity: Entity,
) {
    commands.queue(move |world: &mut World| {
        neutralize_control_intent(world, controlled_entity);
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
    mut lease_generations: ResMut<'_, ClientControlLeaseGenerations>,
    mut pending_acks: ResMut<'_, PendingControlAckQueue>,
    player_entities: Res<'_, PlayerRuntimeEntityMap>,
    mut controlled_entity_map: ResMut<'_, PlayerControlledEntityMap>,
    mut input_cleanup: RealtimeInputCleanupState<'_>,
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
                    control_generation: lease_generations
                        .generation_by_player
                        .get(bound_player.as_str())
                        .copied()
                        .unwrap_or(0),
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
                            .and_then(|player_entity| player_controlled.get(*player_entity).ok())
                            .and_then(|guid| guid.0.as_deref())
                    ),
                );
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
                    control_generation: lease_generations
                        .generation_by_player
                        .get(bound_player.as_str())
                        .copied()
                        .unwrap_or(0),
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
                    control_generation: lease_generations
                        .generation_by_player
                        .get(bound_player.as_str())
                        .copied()
                        .unwrap_or(0),
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
                    control_generation: lease_generations
                        .generation_by_player
                        .get(bound_player.as_str())
                        .copied()
                        .unwrap_or(0),
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
            let current_generation =
                lease_generations.ensure_initialized_for_player(bound_player.as_str());
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
                            info!(
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
                            let reject = ServerControlRejectMessage {
                                player_entity_id: bound_player.clone(),
                                request_seq: message.request_seq,
                                control_generation: current_generation,
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
            let player_guid_for_persistence = player_guid.clone();
            let resolved_control_guid_for_persistence = resolved_control_guid.clone();
            commands.queue(move |world: &mut World| {
                if let Ok(mut player) = world.get_entity_mut(player_entity) {
                    player.insert(ControlledEntityGuid(
                        resolved_control_guid_for_persistence.clone(),
                    ));
                }
                if let Some(mut dirty) = world.get_resource_mut::<PersistenceDirtyState>() {
                    dirty.dirty_entity_ids.insert(player_guid_for_persistence);
                }
                if let Some(mut timer) = world.get_resource_mut::<SimulationPersistenceTimer>() {
                    timer.last_flush_at_s = None;
                }
                persist_entity_snapshot_async(world, player_entity, "control");
            });

            controlled_entity_map
                .by_player_entity_id
                .insert(bound_player_id, resolved_target_entity);

            let rebind_required = currently_bound_entity != resolved_target_entity;
            let control_generation = next_control_generation(current_generation, rebind_required);
            lease_generations
                .generation_by_player
                .insert(bound_player.clone(), control_generation);
            if rebind_required {
                input_cleanup.clear_player(bound_player_id);
                queue_neutralize_control_intent(&mut commands, currently_bound_entity);
                queue_neutralize_control_intent(&mut commands, resolved_target_entity);
            }

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
            order_state
                .last_request_seq_by_player
                .insert(bound_player.clone(), message.request_seq);

            let ack = ServerControlAckMessage {
                player_entity_id: bound_player.clone(),
                request_seq: message.request_seq,
                control_generation,
                controlled_entity_id: resolved_runtime_entity_id,
            };
            pending_acks.queued.push(PendingControlAck {
                server_entity: link_of.server,
                remote_peer_id: remote_id.0,
                message: ack,
            });
        }
    }
}

pub fn flush_pending_control_acks(
    server_query: Query<'_, '_, &'_ Server>,
    mut sender: ServerMultiMessageSender<'_, '_, With<lightyear::prelude::client::Connected>>,
    mut pending_acks: ResMut<'_, PendingControlAckQueue>,
) {
    for pending in pending_acks.queued.drain(..) {
        let Ok(server) = server_query.get(pending.server_entity) else {
            warn!(
                "replication control: missing server entity {:?} while flushing queued ack for player {} seq {}",
                pending.server_entity,
                pending.message.player_entity_id,
                pending.message.request_seq
            );
            continue;
        };
        let target = NetworkTarget::Single(pending.remote_peer_id);
        let _ = sender.send::<ServerControlAckMessage, ControlChannel>(
            &pending.message,
            server,
            &target,
        );
    }
}

fn maybe_set_controlled_by(
    entity_commands: &mut EntityCommands<'_>,
    current: Option<&ControlledBy>,
    desired_owner: Option<Entity>,
) -> bool {
    match desired_owner {
        Some(owner)
            if current.is_some_and(|controlled_by| {
                controlled_by.owner == owner && controlled_by.lifetime == Lifetime::Persistent
            }) =>
        {
            false
        }
        Some(owner) => {
            entity_commands.insert(ControlledBy {
                owner,
                lifetime: Lifetime::Persistent,
            });
            true
        }
        None if current.is_some() => {
            entity_commands.remove::<ControlledBy>();
            true
        }
        None => false,
    }
}

fn maybe_set_replicate(
    entity_commands: &mut EntityCommands<'_>,
    current: Option<&Replicate>,
    desired: &Replicate,
) -> bool {
    if current != Some(desired) {
        entity_commands.insert(desired.clone());
        true
    } else {
        false
    }
}

enum DesiredInterpolationTarget {
    Owner(Entity),
    Network(NetworkTarget),
    Manual(InterpolationTarget),
}

fn maybe_set_prediction_target(
    entity_commands: &mut EntityCommands<'_>,
    current: Option<&PredictionTarget>,
    desired_owner: Option<Entity>,
) -> bool {
    let current_debug = current.map(|target| format!("{target:?}"));
    let desired_debug = desired_owner.map(|owner| format!("{:?}", owner_prediction_target(owner)));
    match desired_owner {
        Some(_owner) if current_debug == desired_debug => false,
        Some(owner) => {
            entity_commands.insert(owner_prediction_target(owner));
            true
        }
        None if current.is_some() => {
            entity_commands.remove::<PredictionTarget>();
            true
        }
        None => false,
    }
}

fn maybe_set_interpolation_target(
    entity_commands: &mut EntityCommands<'_>,
    current: Option<&InterpolationTarget>,
    desired: Option<DesiredInterpolationTarget>,
) -> bool {
    let current_debug = current.map(|target| format!("{target:?}"));
    let desired_debug = desired.as_ref().map(|target| match target {
        DesiredInterpolationTarget::Owner(owner) => {
            format!("{:?}", owner_interpolation_target(*owner))
        }
        DesiredInterpolationTarget::Network(network) => {
            format!("{:?}", InterpolationTarget::to_clients(network.clone()))
        }
        DesiredInterpolationTarget::Manual(target) => format!("{target:?}"),
    });
    match desired {
        Some(DesiredInterpolationTarget::Owner(_owner)) if current_debug == desired_debug => false,
        Some(DesiredInterpolationTarget::Owner(owner)) => {
            entity_commands.insert(owner_interpolation_target(owner));
            true
        }
        Some(DesiredInterpolationTarget::Network(_network)) if current_debug == desired_debug => {
            false
        }
        Some(DesiredInterpolationTarget::Network(network)) => {
            entity_commands.insert(InterpolationTarget::to_clients(network));
            true
        }
        Some(DesiredInterpolationTarget::Manual(_target)) if current_debug == desired_debug => {
            false
        }
        Some(DesiredInterpolationTarget::Manual(target)) => {
            entity_commands.insert(target);
            true
        }
        None if current.is_some() => {
            entity_commands.remove::<InterpolationTarget>();
            true
        }
        None => false,
    }
}

fn collect_visible_clients_for_role_rearm(
    membership_cache: &VisibilityMembershipCache,
    replication_state: &ReplicationState,
    entity: Entity,
) -> Vec<Entity> {
    membership_cache
        .visible_clients(entity)
        .into_iter()
        .flat_map(|clients| clients.iter().copied())
        .filter(|client_entity| replication_state.is_visible(*client_entity))
        .collect()
}

fn rearm_visible_clients_for_role_change(
    membership_cache: &mut VisibilityMembershipCache,
    replication_state: &mut ReplicationState,
    role_rearms: &mut RoleVisibilityRearmState,
    entity: Entity,
) -> usize {
    let visible_clients =
        collect_visible_clients_for_role_rearm(membership_cache, replication_state, entity);
    for client_entity in &visible_clients {
        replication_state.lose_visibility(*client_entity);
        membership_cache.remove_visible_client(entity, *client_entity);
        role_rearms.queue_loss_pass(entity, *client_entity);
    }
    visible_clients.len()
}

fn collect_role_rearm_entities(world: &World, root: Entity) -> Vec<Entity> {
    world
        .get_resource::<VisibilitySpatialIndex>()
        .and_then(|index| index.entities_under_root(root))
        .filter(|entities| !entities.is_empty())
        .unwrap_or_else(|| vec![root])
}

fn rearm_world_entity_for_role_change(world: &mut World, entity: Entity) -> usize {
    world.resource_scope(
        |world, mut membership_cache: Mut<'_, VisibilityMembershipCache>| {
            world.resource_scope(
                |world, mut role_rearms: Mut<'_, RoleVisibilityRearmState>| {
                    let Some(mut replication_state) = world.get_mut::<ReplicationState>(entity)
                    else {
                        return 0;
                    };
                    rearm_visible_clients_for_role_change(
                        &mut membership_cache,
                        &mut replication_state,
                        &mut role_rearms,
                        entity,
                    )
                },
            )
        },
    )
}

fn rearm_visibility_tree_for_role_change(world: &mut World, roots: &[(Entity, String)]) {
    let mut rearmed_entities = HashSet::<Entity>::new();
    for (root, root_guid) in roots {
        let entities = collect_role_rearm_entities(world, *root);
        let mut rearmed_entity_count = 0usize;
        let mut rearmed_visible_client_count = 0usize;
        for entity in entities {
            if !rearmed_entities.insert(entity) {
                continue;
            }
            let rearmed_clients = rearm_world_entity_for_role_change(world, entity);
            if rearmed_clients > 0 {
                rearmed_entity_count = rearmed_entity_count.saturating_add(1);
                rearmed_visible_client_count =
                    rearmed_visible_client_count.saturating_add(rearmed_clients);
            }
        }
        if rearmed_visible_client_count > 0 {
            info!(
                "server control role rearm root_entity={:?} guid={} entities={} visible_clients={}",
                root, root_guid, rearmed_entity_count, rearmed_visible_client_count
            );
        }
    }
}

fn queue_visibility_tree_rearm_for_role_change(
    commands: &mut Commands<'_, '_>,
    roots: Vec<(Entity, String)>,
) {
    if roots.is_empty() {
        return;
    }
    commands.queue(move |world: &mut World| {
        rearm_visibility_tree_for_role_change(world, &roots);
    });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn reconcile_control_replication_roles(
    mut commands: Commands<'_, '_>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    player_entity_map: Res<'_, PlayerRuntimeEntityMap>,
    controlled_entity_map: Res<'_, PlayerControlledEntityMap>,
    entity_guids: Query<'_, '_, &'_ EntityGuid>,
    mut players: Query<
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
            Option<Mut<'_, ReplicationState>>,
        ),
        With<PlayerTag>,
    >,
    mut controlled_entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ SimulatedControlledEntity,
            Option<&'_ ControlledBy>,
            Option<&'_ Replicate>,
            Option<&'_ PredictionTarget>,
            Option<&'_ InterpolationTarget>,
            Option<Mut<'_, ReplicationState>>,
        ),
        Without<PlayerTag>,
    >,
) {
    let mut bound_client_by_player_wire = HashMap::<String, Entity>::new();
    let mut desired_controlled_by_client = HashMap::<Entity, Entity>::new();
    let mut desired_control_guid_by_player = HashMap::<Entity, Option<String>>::new();
    let mut desired_owner_by_entity = HashMap::<Entity, Entity>::new();
    let mut role_rearm_roots = Vec::<(Entity, String)>::new();

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
        replication_state,
    ) in &mut players
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

        let mut replication_topology_changed = false;

        match bound_client {
            Some(client_entity) => {
                maybe_set_controlled_by(
                    &mut entity_commands,
                    current_controlled_by,
                    Some(client_entity),
                );
                let desired_replicate = owner_only_replicate(client_entity);
                let replicate_changed = maybe_set_replicate(
                    &mut entity_commands,
                    current_replicate,
                    &desired_replicate,
                );
                replication_topology_changed |= replicate_changed;

                let prediction_changed = maybe_set_prediction_target(
                    &mut entity_commands,
                    current_prediction,
                    controls_self.then_some(client_entity),
                );
                replication_topology_changed |= prediction_changed;
                let interpolation_changed = maybe_set_interpolation_target(
                    &mut entity_commands,
                    current_interpolation,
                    (!controls_self).then_some(DesiredInterpolationTarget::Owner(client_entity)),
                );
                replication_topology_changed |= interpolation_changed;
            }
            None => {
                maybe_set_controlled_by(&mut entity_commands, current_controlled_by, None);
                let replicate_changed = maybe_set_replicate(
                    &mut entity_commands,
                    current_replicate,
                    &Replicate::to_clients(NetworkTarget::None),
                );
                replication_topology_changed |= replicate_changed;
                let prediction_changed =
                    maybe_set_prediction_target(&mut entity_commands, current_prediction, None);
                replication_topology_changed |= prediction_changed;
                let interpolation_changed = maybe_set_interpolation_target(
                    &mut entity_commands,
                    current_interpolation,
                    None,
                );
                replication_topology_changed |= interpolation_changed;
            }
        }

        if replication_topology_changed && replication_state.is_some() {
            role_rearm_roots.push((entity, player_wire));
        }
    }

    for (
        entity,
        simulated_controlled,
        current_controlled_by,
        current_replicate,
        current_prediction,
        current_interpolation,
        replication_state,
    ) in &mut controlled_entities
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
        let mut replication_topology_changed = false;
        maybe_set_controlled_by(&mut entity_commands, current_controlled_by, desired_owner);
        let replicate_changed = maybe_set_replicate(
            &mut entity_commands,
            current_replicate,
            &Replicate::to_clients(NetworkTarget::All),
        );
        replication_topology_changed |= replicate_changed;

        let desired_interpolation = match desired_owner {
            Some(owner) => observer_interpolation_target(&bindings, owner)
                .map(DesiredInterpolationTarget::Manual),
            None => Some(DesiredInterpolationTarget::Network(NetworkTarget::All)),
        };

        let prediction_changed =
            maybe_set_prediction_target(&mut entity_commands, current_prediction, desired_owner);
        replication_topology_changed |= prediction_changed;
        let interpolation_changed = maybe_set_interpolation_target(
            &mut entity_commands,
            current_interpolation,
            desired_interpolation,
        );
        replication_topology_changed |= interpolation_changed;

        if replication_topology_changed && replication_state.is_some() {
            let entity_guid = entity_guids
                .get(entity)
                .map(|guid| guid.0.to_string())
                .unwrap_or_else(|_| format!("{entity:?}"));
            role_rearm_roots.push((entity, entity_guid));
        }
    }
    queue_visibility_tree_rearm_for_role_change(&mut commands, role_rearm_roots);
}

#[cfg(test)]
mod tests {
    use super::{
        ClientControlLeaseGenerations, RoleVisibilityRearmState,
        collect_visible_clients_for_role_rearm, next_control_generation,
        observer_interpolation_target, owner_interpolation_target, owner_only_replicate,
        owner_prediction_target, rearm_visibility_tree_for_role_change,
        rearm_visible_clients_for_role_change, reconcile_control_replication_roles,
    };
    use crate::replication::auth::AuthenticatedClientBindings;
    use crate::replication::{
        PlayerControlledEntityMap, PlayerRuntimeEntityMap, SimulatedControlledEntity,
        visibility::{VisibilityMembershipCache, VisibilitySpatialIndex},
    };
    use bevy::prelude::*;
    use lightyear::prelude::server::ClientOf;
    use lightyear::prelude::{
        ControlledBy, InterpolationTarget, PeerId, PredictionTarget, RemoteId, Replicate,
        ReplicationState,
    };
    use sidereal_game::{ControlledEntityGuid, EntityGuid, PlayerTag};
    use sidereal_net::PlayerEntityId;
    use std::collections::HashSet;
    use uuid::Uuid;

    #[test]
    fn control_generation_advances_only_when_the_lease_target_changes() {
        assert_eq!(next_control_generation(0, false), 1);
        assert_eq!(next_control_generation(1, false), 1);
        assert_eq!(next_control_generation(1, true), 2);
    }

    #[test]
    fn control_generation_initializes_existing_authoritative_lease() {
        let mut generations = ClientControlLeaseGenerations::default();

        let current =
            generations.ensure_initialized_for_player("1521601b-7e69-4700-853f-eb1eb3a41199");

        assert_eq!(current, 1);
        assert_eq!(next_control_generation(current, true), 2);
        assert_eq!(
            generations
                .generation_by_player
                .get("1521601b-7e69-4700-853f-eb1eb3a41199"),
            Some(&1)
        );
    }

    #[test]
    fn reconcile_assigns_owner_predicted_ship_roles_without_visibility_churn() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(lightyear::prelude::server::ServerPlugins::default());
        app.init_resource::<AuthenticatedClientBindings>();
        app.init_resource::<VisibilityMembershipCache>();
        app.init_resource::<RoleVisibilityRearmState>();
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
            observer_interpolation_target(
                app.world().resource::<AuthenticatedClientBindings>(),
                client
            )
            .map(|target| format!("{target:?}"))
        );
    }

    #[test]
    fn reconcile_assigns_manual_observer_interpolation_targets_for_controlled_ships() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(lightyear::prelude::server::ServerPlugins::default());
        app.init_resource::<AuthenticatedClientBindings>();
        app.init_resource::<VisibilityMembershipCache>();
        app.init_resource::<RoleVisibilityRearmState>();
        app.init_resource::<PlayerControlledEntityMap>();
        app.init_resource::<PlayerRuntimeEntityMap>();
        app.add_systems(Update, reconcile_control_replication_roles);

        let owner_player_id =
            PlayerEntityId(Uuid::parse_str("1521601b-7e69-4700-853f-eb1eb3a41199").unwrap());
        let observer_player_id =
            PlayerEntityId(Uuid::parse_str("7bd0d9cc-42a5-45bb-aef0-8cbf88aa6a44").unwrap());
        let ship_guid = Uuid::parse_str("ce9e421c-8b62-458a-803e-51e9ad272908").unwrap();
        let owner_client = app
            .world_mut()
            .spawn((ClientOf, RemoteId(PeerId::Netcode(42))))
            .id();
        let observer_client = app
            .world_mut()
            .spawn((ClientOf, RemoteId(PeerId::Netcode(43))))
            .id();
        let owner_player_entity = app
            .world_mut()
            .spawn((PlayerTag, EntityGuid(owner_player_id.0)))
            .id();
        let observer_player_entity = app
            .world_mut()
            .spawn((PlayerTag, EntityGuid(observer_player_id.0)))
            .id();
        let ship_entity = app
            .world_mut()
            .spawn((
                EntityGuid(ship_guid),
                SimulatedControlledEntity {
                    player_entity_id: owner_player_id,
                },
            ))
            .id();

        app.world_mut()
            .resource_mut::<AuthenticatedClientBindings>()
            .by_client_entity
            .insert(owner_client, owner_player_id.canonical_wire_id());
        app.world_mut()
            .resource_mut::<AuthenticatedClientBindings>()
            .by_client_entity
            .insert(observer_client, observer_player_id.canonical_wire_id());
        app.world_mut()
            .resource_mut::<PlayerRuntimeEntityMap>()
            .by_player_entity_id
            .insert(owner_player_id.canonical_wire_id(), owner_player_entity);
        app.world_mut()
            .resource_mut::<PlayerRuntimeEntityMap>()
            .by_player_entity_id
            .insert(
                observer_player_id.canonical_wire_id(),
                observer_player_entity,
            );
        app.world_mut()
            .resource_mut::<PlayerControlledEntityMap>()
            .by_player_entity_id
            .insert(owner_player_id, ship_entity);

        app.update();

        let expected = observer_interpolation_target(
            app.world().resource::<AuthenticatedClientBindings>(),
            owner_client,
        )
        .expect("observer interpolation target");
        assert_eq!(
            app.world()
                .get::<InterpolationTarget>(ship_entity)
                .map(|target| format!("{target:?}")),
            Some(format!("{expected:?}"))
        );
    }

    #[test]
    fn reconcile_clears_stale_owner_roles_when_client_binding_is_gone() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(lightyear::prelude::server::ServerPlugins::default());
        app.init_resource::<AuthenticatedClientBindings>();
        app.init_resource::<VisibilityMembershipCache>();
        app.init_resource::<RoleVisibilityRearmState>();
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

    #[test]
    fn visible_role_rearm_targets_only_currently_visible_clients() {
        let entity = Entity::from_bits(10);
        let visible_client = Entity::from_bits(20);
        let invisible_client = Entity::from_bits(30);
        let uncached_client = Entity::from_bits(40);

        let mut membership_cache = VisibilityMembershipCache::default();
        let visible_set = [visible_client, invisible_client]
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        membership_cache.replace_visible_clients(entity, visible_set);

        let mut replication_state = ReplicationState::default();
        replication_state.gain_visibility(visible_client);
        replication_state.lose_visibility(invisible_client);
        replication_state.gain_visibility(uncached_client);

        let visible_clients =
            collect_visible_clients_for_role_rearm(&membership_cache, &replication_state, entity);

        assert_eq!(visible_clients, vec![visible_client]);
    }

    #[test]
    fn role_rearm_sends_loss_before_allowing_regain() {
        let entity = Entity::from_bits(10);
        let visible_client = Entity::from_bits(20);
        let mut membership_cache = VisibilityMembershipCache::default();
        membership_cache.replace_visible_clients(entity, HashSet::from([visible_client]));
        let mut replication_state = ReplicationState::default();
        replication_state.gain_visibility(visible_client);
        let mut role_rearms = RoleVisibilityRearmState::default();

        let rearmed = rearm_visible_clients_for_role_change(
            &mut membership_cache,
            &mut replication_state,
            &mut role_rearms,
            entity,
        );

        assert_eq!(rearmed, 1);
        assert!(!replication_state.is_visible(visible_client));
        assert!(
            membership_cache
                .visible_clients(entity)
                .is_none_or(|clients| !clients.contains(&visible_client))
        );
        assert!(role_rearms.is_pending(entity, visible_client));

        let mut desired_visible_clients = HashSet::from([visible_client]);
        role_rearms.suppress_desired_clients(entity, &mut desired_visible_clients);
        assert!(desired_visible_clients.is_empty());

        role_rearms.advance_after_membership_pass();
        assert!(!role_rearms.is_pending(entity, visible_client));
    }

    #[test]
    fn role_rearm_covers_entities_under_same_visibility_root() {
        let mut world = World::new();
        world.init_resource::<VisibilityMembershipCache>();
        world.init_resource::<RoleVisibilityRearmState>();
        world.init_resource::<VisibilitySpatialIndex>();

        let root = world.spawn(ReplicationState::default()).id();
        let child = world.spawn(ReplicationState::default()).id();
        let visible_client = world.spawn_empty().id();

        world
            .resource_mut::<VisibilitySpatialIndex>()
            .replace_entities_under_root(root, HashSet::from([root, child]));
        world
            .resource_mut::<VisibilityMembershipCache>()
            .replace_visible_clients(root, HashSet::from([visible_client]));
        world
            .resource_mut::<VisibilityMembershipCache>()
            .replace_visible_clients(child, HashSet::from([visible_client]));
        world
            .get_mut::<ReplicationState>(root)
            .unwrap()
            .gain_visibility(visible_client);
        world
            .get_mut::<ReplicationState>(child)
            .unwrap()
            .gain_visibility(visible_client);

        rearm_visibility_tree_for_role_change(&mut world, &[(root, "root-guid".to_string())]);

        assert!(
            !world
                .get::<ReplicationState>(root)
                .unwrap()
                .is_visible(visible_client)
        );
        assert!(
            !world
                .get::<ReplicationState>(child)
                .unwrap()
                .is_visible(visible_client)
        );
        assert!(
            world
                .resource::<VisibilityMembershipCache>()
                .visible_clients(root)
                .is_none_or(|clients| !clients.contains(&visible_client))
        );
        assert!(
            world
                .resource::<VisibilityMembershipCache>()
                .visible_clients(child)
                .is_none_or(|clients| !clients.contains(&visible_client))
        );
        let rearm_state = world.resource::<RoleVisibilityRearmState>();
        assert!(rearm_state.is_pending(root, visible_client));
        assert!(rearm_state.is_pending(child, visible_client));
    }
}
