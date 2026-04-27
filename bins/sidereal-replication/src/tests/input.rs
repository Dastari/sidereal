use crate::replication::PlayerControlledEntityMap;
use crate::replication::SimulatedControlledEntity;
use crate::replication::control::ClientControlLeaseGenerations;
use crate::replication::input::{
    ClientInputDropMetrics, InputActivityLogState, InputRateLimitState, InputValidationFailure,
    LatestRealtimeInput, LatestRealtimeInputsByPlayer, MAX_ACTIONS_PER_PACKET,
    MAX_MESSAGES_PER_SECOND, RealtimeInputActivityByPlayer, RealtimeInputTimeoutSeconds,
    canonical_controlled_entity_id, drain_realtime_player_inputs_to_action_queue,
    validate_input_message,
};
use bevy::prelude::*;
use sidereal_game::{ActionQueue, EntityAction, EntityGuid, PlayerTag};
use sidereal_net::{ClientRealtimeInputMessage, PlayerEntityId, RuntimeEntityId};

fn message_with(tick: u64, actions: usize) -> ClientRealtimeInputMessage {
    ClientRealtimeInputMessage {
        player_entity_id: "11111111-1111-1111-1111-111111111111".to_string(),
        controlled_entity_id: "22222222-2222-2222-2222-222222222222".to_string(),
        control_generation: 1,
        actions: vec![EntityAction::LongitudinalNeutral; actions],
        tick,
    }
}

#[test]
fn validation_rejects_duplicate_and_future_ticks() {
    let mut rate_limit = InputRateLimitState::default();
    let duplicate = message_with(10, 1);
    let future = message_with(20, 1);
    assert_eq!(
        validate_input_message(&duplicate, Some(10), 1.0, &mut rate_limit),
        Err(InputValidationFailure::DuplicateOrOutOfOrder)
    );
    assert_eq!(
        validate_input_message(&future, Some(10), 1.0, &mut rate_limit),
        Err(InputValidationFailure::FutureTick)
    );
}

#[test]
fn validation_rejects_oversized_and_rate_limited() {
    let mut rate_limit = InputRateLimitState::default();
    let oversized = message_with(11, MAX_ACTIONS_PER_PACKET + 1);
    assert_eq!(
        validate_input_message(&oversized, Some(10), 1.0, &mut rate_limit),
        Err(InputValidationFailure::OversizedPacket)
    );

    let normal = message_with(11, 1);
    for _ in 0..MAX_MESSAGES_PER_SECOND {
        let result = validate_input_message(&normal, Some(10), 2.0, &mut rate_limit);
        assert_eq!(result, Ok(()));
    }
    assert_eq!(
        validate_input_message(&normal, Some(10), 2.0, &mut rate_limit),
        Err(InputValidationFailure::RateLimited)
    );
}

#[test]
fn canonical_controlled_entity_id_accepts_only_canonical_uuids() {
    let player_id = PlayerEntityId::parse("11111111-1111-1111-1111-111111111111").unwrap();
    assert_eq!(
        canonical_controlled_entity_id("11111111-1111-1111-1111-111111111111", player_id),
        Some(RuntimeEntityId(player_id.0))
    );

    assert_eq!(
        canonical_controlled_entity_id("ship:22222222-2222-2222-2222-222222222222", player_id),
        None
    );
    assert_eq!(
        canonical_controlled_entity_id("22222222-2222-2222-2222-222222222222", player_id),
        RuntimeEntityId::parse("22222222-2222-2222-2222-222222222222")
    );
}

#[test]
fn drain_keeps_fresh_realtime_input_before_timeout() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(PlayerControlledEntityMap::default());
    app.insert_resource(ClientControlLeaseGenerations::default());
    app.insert_resource(LatestRealtimeInputsByPlayer::default());
    app.insert_resource(RealtimeInputActivityByPlayer::default());
    app.insert_resource(RealtimeInputTimeoutSeconds(0.35));
    app.insert_resource(ClientInputDropMetrics::default());
    app.insert_resource(InputActivityLogState::default());
    app.add_systems(Update, drain_realtime_player_inputs_to_action_queue);

    let player_id = PlayerEntityId::parse("11111111-1111-1111-1111-111111111111").unwrap();
    let player_guid = player_id.0;
    let player_entity = app
        .world_mut()
        .spawn((EntityGuid(player_guid), PlayerTag, ActionQueue::default()))
        .id();
    app.world_mut()
        .resource_mut::<PlayerControlledEntityMap>()
        .by_player_entity_id
        .insert(player_id, player_entity);
    app.world_mut()
        .resource_mut::<LatestRealtimeInputsByPlayer>()
        .by_player_entity_id
        .insert(
            player_id,
            LatestRealtimeInput {
                tick: 7,
                controlled_entity_id: RuntimeEntityId(player_guid),
                control_generation: 0,
                actions: vec![EntityAction::Forward],
            },
        );
    app.world_mut()
        .resource_mut::<RealtimeInputActivityByPlayer>()
        .last_received_at_s_by_player_entity_id
        .insert(player_id, 0.0);

    app.update();

    let queue = app.world().get::<ActionQueue>(player_entity).unwrap();
    assert_eq!(queue.pending, vec![EntityAction::Forward]);
}

#[test]
fn drain_clears_stale_realtime_input_after_timeout() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(PlayerControlledEntityMap::default());
    app.insert_resource(ClientControlLeaseGenerations::default());
    app.insert_resource(LatestRealtimeInputsByPlayer::default());
    app.insert_resource(RealtimeInputActivityByPlayer::default());
    app.insert_resource(RealtimeInputTimeoutSeconds(0.35));
    app.insert_resource(ClientInputDropMetrics::default());
    app.insert_resource(InputActivityLogState::default());
    app.add_systems(Update, drain_realtime_player_inputs_to_action_queue);

    let player_id = PlayerEntityId::parse("11111111-1111-1111-1111-111111111111").unwrap();
    let player_guid = player_id.0;
    let player_entity = app
        .world_mut()
        .spawn((
            EntityGuid(player_guid),
            PlayerTag,
            ActionQueue {
                pending: vec![EntityAction::Forward],
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<PlayerControlledEntityMap>()
        .by_player_entity_id
        .insert(player_id, player_entity);
    app.world_mut()
        .resource_mut::<LatestRealtimeInputsByPlayer>()
        .by_player_entity_id
        .insert(
            player_id,
            LatestRealtimeInput {
                tick: 7,
                controlled_entity_id: RuntimeEntityId(player_guid),
                control_generation: 0,
                actions: vec![EntityAction::Forward],
            },
        );
    app.world_mut()
        .resource_mut::<RealtimeInputActivityByPlayer>()
        .last_received_at_s_by_player_entity_id
        .insert(player_id, -1.0);

    app.update();

    let queue = app.world().get::<ActionQueue>(player_entity).unwrap();
    assert!(queue.pending.is_empty());
}

#[test]
fn drain_rejects_stale_generation_input_during_control_handoff() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(PlayerControlledEntityMap::default());
    app.insert_resource(ClientControlLeaseGenerations::default());
    app.insert_resource(LatestRealtimeInputsByPlayer::default());
    app.insert_resource(RealtimeInputActivityByPlayer::default());
    app.insert_resource(RealtimeInputTimeoutSeconds(0.35));
    app.insert_resource(ClientInputDropMetrics::default());
    app.insert_resource(InputActivityLogState::default());
    app.add_systems(Update, drain_realtime_player_inputs_to_action_queue);

    let player_id = PlayerEntityId::parse("11111111-1111-1111-1111-111111111111").unwrap();
    let ship_a_id = RuntimeEntityId::parse("22222222-2222-2222-2222-222222222222").unwrap();
    let ship_b_id = RuntimeEntityId::parse("33333333-3333-3333-3333-333333333333").unwrap();
    let ship_b_entity = app
        .world_mut()
        .spawn((
            EntityGuid(ship_b_id.0),
            SimulatedControlledEntity {
                player_entity_id: player_id,
            },
            ActionQueue::default(),
        ))
        .id();
    app.world_mut()
        .resource_mut::<PlayerControlledEntityMap>()
        .by_player_entity_id
        .insert(player_id, ship_b_entity);
    app.world_mut()
        .resource_mut::<ClientControlLeaseGenerations>()
        .generation_by_player
        .insert(player_id.canonical_wire_id(), 2);
    app.world_mut()
        .resource_mut::<LatestRealtimeInputsByPlayer>()
        .by_player_entity_id
        .insert(
            player_id,
            LatestRealtimeInput {
                tick: 11,
                controlled_entity_id: ship_a_id,
                control_generation: 1,
                actions: vec![EntityAction::Forward],
            },
        );
    app.world_mut()
        .resource_mut::<RealtimeInputActivityByPlayer>()
        .last_received_at_s_by_player_entity_id
        .insert(player_id, 0.0);

    app.update();

    let queue = app.world().get::<ActionQueue>(ship_b_entity).unwrap();
    assert!(queue.pending.is_empty());
    assert_eq!(
        app.world()
            .resource::<ClientInputDropMetrics>()
            .stale_control_generation,
        1
    );
}

#[test]
fn drain_rejects_target_mismatch_with_matching_control_generation() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(PlayerControlledEntityMap::default());
    app.insert_resource(ClientControlLeaseGenerations::default());
    app.insert_resource(LatestRealtimeInputsByPlayer::default());
    app.insert_resource(RealtimeInputActivityByPlayer::default());
    app.insert_resource(RealtimeInputTimeoutSeconds(0.35));
    app.insert_resource(ClientInputDropMetrics::default());
    app.insert_resource(InputActivityLogState::default());
    app.add_systems(Update, drain_realtime_player_inputs_to_action_queue);

    let player_id = PlayerEntityId::parse("11111111-1111-1111-1111-111111111111").unwrap();
    let ship_a_id = RuntimeEntityId::parse("22222222-2222-2222-2222-222222222222").unwrap();
    let ship_b_id = RuntimeEntityId::parse("33333333-3333-3333-3333-333333333333").unwrap();
    let ship_b_entity = app
        .world_mut()
        .spawn((
            EntityGuid(ship_b_id.0),
            SimulatedControlledEntity {
                player_entity_id: player_id,
            },
            ActionQueue::default(),
        ))
        .id();
    app.world_mut()
        .resource_mut::<PlayerControlledEntityMap>()
        .by_player_entity_id
        .insert(player_id, ship_b_entity);
    app.world_mut()
        .resource_mut::<ClientControlLeaseGenerations>()
        .generation_by_player
        .insert(player_id.canonical_wire_id(), 2);
    app.world_mut()
        .resource_mut::<LatestRealtimeInputsByPlayer>()
        .by_player_entity_id
        .insert(
            player_id,
            LatestRealtimeInput {
                tick: 11,
                controlled_entity_id: ship_a_id,
                control_generation: 2,
                actions: vec![EntityAction::Forward],
            },
        );
    app.world_mut()
        .resource_mut::<RealtimeInputActivityByPlayer>()
        .last_received_at_s_by_player_entity_id
        .insert(player_id, 0.0);

    app.update();

    let queue = app.world().get::<ActionQueue>(ship_b_entity).unwrap();
    assert!(queue.pending.is_empty());
    assert_eq!(
        app.world()
            .resource::<ClientInputDropMetrics>()
            .controlled_target_mismatch,
        1
    );
}
