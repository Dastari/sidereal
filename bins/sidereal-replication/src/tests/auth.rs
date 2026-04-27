use bevy::prelude::*;
use lightyear::prelude::PeerId;

use crate::replication::PlayerControlledEntityMap;
use crate::replication::PlayerRuntimeEntityMap;
use crate::replication::auth::{
    AUTH_CONFIG_DENIED_REASON, AuthenticatedClientBindings, cleanup_client_auth_bindings,
    configured_gateway_jwt_secret, reset_realtime_input_session_for_player,
    sync_visibility_registry_with_authenticated_clients,
};
use crate::replication::control::ClientControlRequestOrder;
use crate::replication::input::{
    ClientInputStreamKey, ClientInputTickTracker, InputRateLimitState,
    LatestRealtimeInputsByPlayer, RealtimeInputActivityByPlayer,
};
use crate::replication::lifecycle::ClientLastActivity;
use crate::replication::visibility::{ClientVisibilityRegistry, VisibilityClientContextCache};
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{RemoteId, ReplicationState};
use sidereal_game::{ActionQueue, AfterburnerState, EntityAction, EntityGuid, FlightComputer};
use sidereal_net::{PlayerEntityId, RuntimeEntityId};

#[test]
fn cleanup_drops_visibility_for_disconnected_client() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<AuthenticatedClientBindings>();
    app.init_resource::<ClientInputTickTracker>();
    app.init_resource::<InputRateLimitState>();
    app.init_resource::<LatestRealtimeInputsByPlayer>();
    app.init_resource::<RealtimeInputActivityByPlayer>();
    app.init_resource::<ClientVisibilityRegistry>();
    app.init_resource::<VisibilityClientContextCache>();
    app.init_resource::<ClientControlRequestOrder>();
    app.init_resource::<ClientLastActivity>();
    app.init_resource::<PlayerControlledEntityMap>();
    app.add_systems(Update, cleanup_client_auth_bindings);

    let client = app
        .world_mut()
        .spawn((ClientOf, RemoteId(PeerId::Netcode(42))))
        .id();
    let replicated = app.world_mut().spawn(ReplicationState::default()).id();

    {
        let mut bindings = app
            .world_mut()
            .resource_mut::<AuthenticatedClientBindings>();
        bindings
            .by_client_entity
            .insert(client, "11111111-1111-1111-1111-111111111111".to_string());
        bindings.by_remote_id.insert(
            PeerId::Netcode(42),
            "11111111-1111-1111-1111-111111111111".to_string(),
        );
    }
    app.world_mut()
        .resource_mut::<ClientVisibilityRegistry>()
        .register_client(client, "11111111-1111-1111-1111-111111111111".to_string());
    app.world_mut()
        .get_mut::<ReplicationState>(replicated)
        .expect("replication state exists")
        .gain_visibility(client);

    app.world_mut().entity_mut(client).despawn();
    app.update();

    // Cleanup removes bindings and registry entries; ReplicationState visibility bits
    // are intentionally left as-is (see cleanup_client_auth_bindings).
    assert!(
        !app.world()
            .resource::<AuthenticatedClientBindings>()
            .by_client_entity
            .contains_key(&client)
    );
    assert!(
        !app.world()
            .resource::<ClientVisibilityRegistry>()
            .player_entity_id_by_client
            .contains_key(&client)
    );
}

#[test]
fn cleanup_neutralizes_disconnected_player_control_intent() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<AuthenticatedClientBindings>();
    app.init_resource::<ClientInputTickTracker>();
    app.init_resource::<InputRateLimitState>();
    app.init_resource::<LatestRealtimeInputsByPlayer>();
    app.init_resource::<RealtimeInputActivityByPlayer>();
    app.init_resource::<ClientVisibilityRegistry>();
    app.init_resource::<VisibilityClientContextCache>();
    app.init_resource::<ClientControlRequestOrder>();
    app.init_resource::<ClientLastActivity>();
    app.init_resource::<PlayerControlledEntityMap>();
    app.add_systems(Update, cleanup_client_auth_bindings);

    let player_id = PlayerEntityId::parse("11111111-1111-1111-1111-111111111111").unwrap();
    let player_wire = player_id.canonical_wire_id();
    let controlled_guid = RuntimeEntityId::parse("22222222-2222-2222-2222-222222222222").unwrap();
    let client = app
        .world_mut()
        .spawn((ClientOf, RemoteId(PeerId::Netcode(42))))
        .id();
    let controlled = app
        .world_mut()
        .spawn((
            EntityGuid(controlled_guid.0),
            ActionQueue {
                pending: vec![EntityAction::Forward, EntityAction::Right],
            },
            FlightComputer {
                profile: "test".to_string(),
                throttle: 1.0,
                yaw_input: -1.0,
                brake_active: true,
                turn_rate_deg_s: 90.0,
            },
            AfterburnerState { active: true },
        ))
        .id();

    app.world_mut()
        .resource_mut::<AuthenticatedClientBindings>()
        .by_client_entity
        .insert(client, player_wire);
    app.world_mut()
        .resource_mut::<PlayerControlledEntityMap>()
        .by_player_entity_id
        .insert(player_id, controlled);

    app.world_mut().entity_mut(client).despawn();
    app.update();

    let queue = app.world().get::<ActionQueue>(controlled).unwrap();
    assert!(queue.pending.is_empty());
    let computer = app.world().get::<FlightComputer>(controlled).unwrap();
    assert_eq!(computer.throttle, 0.0);
    assert_eq!(computer.yaw_input, 0.0);
    assert!(!computer.brake_active);
    let afterburner = app.world().get::<AfterburnerState>(controlled).unwrap();
    assert!(!afterburner.active);
}

#[test]
fn configured_gateway_jwt_secret_rejects_missing_or_short_values() {
    unsafe {
        std::env::remove_var("GATEWAY_JWT_SECRET");
    }
    assert_eq!(
        configured_gateway_jwt_secret().unwrap_err(),
        AUTH_CONFIG_DENIED_REASON
    );

    unsafe {
        std::env::set_var("GATEWAY_JWT_SECRET", "too-short");
    }
    assert_eq!(
        configured_gateway_jwt_secret().unwrap_err(),
        AUTH_CONFIG_DENIED_REASON
    );

    unsafe {
        std::env::set_var("GATEWAY_JWT_SECRET", "0123456789abcdef0123456789abcdef");
    }
    assert_eq!(
        configured_gateway_jwt_secret().as_deref(),
        Ok("0123456789abcdef0123456789abcdef")
    );
}

#[test]
fn visibility_registry_sync_waits_for_authenticated_hydrated_player() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<AuthenticatedClientBindings>();
    app.init_resource::<PlayerRuntimeEntityMap>();
    app.init_resource::<ClientVisibilityRegistry>();
    app.add_systems(Update, sync_visibility_registry_with_authenticated_clients);

    let client = app
        .world_mut()
        .spawn((ClientOf, RemoteId(PeerId::Netcode(42))))
        .id();
    let player_entity_id = "11111111-1111-1111-1111-111111111111".to_string();

    app.world_mut()
        .resource_mut::<AuthenticatedClientBindings>()
        .by_client_entity
        .insert(client, player_entity_id.clone());

    app.update();
    assert!(
        !app.world()
            .resource::<ClientVisibilityRegistry>()
            .player_entity_id_by_client
            .contains_key(&client)
    );

    let player_entity = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<PlayerRuntimeEntityMap>()
        .by_player_entity_id
        .insert(player_entity_id.clone(), player_entity);

    app.update();
    assert_eq!(
        app.world()
            .resource::<ClientVisibilityRegistry>()
            .player_entity_id_by_client
            .get(&client),
        Some(&player_entity_id)
    );
}

#[test]
fn fresh_auth_bind_clears_prior_realtime_input_timeline_for_player() {
    let mut tracker = ClientInputTickTracker::default();
    let mut rate_limit = InputRateLimitState::default();
    let mut latest = LatestRealtimeInputsByPlayer::default();
    let mut activity = RealtimeInputActivityByPlayer::default();

    let player_id = PlayerEntityId::parse("11111111-1111-1111-1111-111111111111").unwrap();
    let controlled_id = RuntimeEntityId::parse("22222222-2222-2222-2222-222222222222").unwrap();
    let other_player_id = PlayerEntityId::parse("33333333-3333-3333-3333-333333333333").unwrap();
    let other_controlled_id =
        RuntimeEntityId::parse("44444444-4444-4444-4444-444444444444").unwrap();
    let player_wire = player_id.canonical_wire_id();
    let client = Entity::from_bits(42);
    let other_client = Entity::from_bits(43);

    tracker.last_accepted_tick_by_stream.insert(
        ClientInputStreamKey {
            client_entity: client,
            player_entity_id: player_id,
            controlled_entity_id: controlled_id,
            control_generation: 2,
        },
        10_218,
    );
    tracker.last_accepted_tick_by_stream.insert(
        ClientInputStreamKey {
            client_entity: other_client,
            player_entity_id: other_player_id,
            controlled_entity_id: other_controlled_id,
            control_generation: 1,
        },
        55,
    );
    rate_limit
        .current_window_index_by_player_entity_id
        .insert(player_wire.clone(), 7);
    rate_limit
        .message_count_in_window_by_player_entity_id
        .insert(player_wire.clone(), 12);
    latest.by_player_entity_id.insert(
        player_id,
        crate::replication::input::LatestRealtimeInput {
            tick: 10_218,
            controlled_entity_id: controlled_id,
            control_generation: 2,
            actions: vec![EntityAction::Forward],
        },
    );
    latest.by_player_entity_id.insert(
        other_player_id,
        crate::replication::input::LatestRealtimeInput {
            tick: 55,
            controlled_entity_id: other_controlled_id,
            control_generation: 1,
            actions: vec![EntityAction::Left],
        },
    );
    activity
        .last_received_at_s_by_player_entity_id
        .insert(player_id, 12.0);

    reset_realtime_input_session_for_player(
        player_id,
        &mut tracker,
        &mut rate_limit,
        &mut latest,
        &mut activity,
    );

    assert!(
        tracker
            .last_accepted_tick_by_stream
            .keys()
            .all(|key| key.player_entity_id != player_id)
    );
    assert_eq!(tracker.last_accepted_tick_by_stream.len(), 1);
    assert!(
        !rate_limit
            .current_window_index_by_player_entity_id
            .contains_key(&player_wire)
    );
    assert!(
        !rate_limit
            .message_count_in_window_by_player_entity_id
            .contains_key(&player_wire)
    );
    assert!(!latest.by_player_entity_id.contains_key(&player_id));
    assert!(latest.by_player_entity_id.contains_key(&other_player_id));
    assert!(
        !activity
            .last_received_at_s_by_player_entity_id
            .contains_key(&player_id)
    );
}
