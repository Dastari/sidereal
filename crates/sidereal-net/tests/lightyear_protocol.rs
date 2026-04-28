#![cfg(feature = "lightyear_protocol")]

use bevy::prelude::App;
use lightyear::prelude::client::ClientPlugins;
use lightyear::prelude::server::ServerPlugins;
use lightyear::prelude::{
    MessageReceiver, MessageSender, Transport, client::Client, server::ClientOf,
};
use sidereal_game::{ActionQueue, EntityAction};
use sidereal_net::{
    ClientAuthMessage, ClientNotificationDismissedMessage, ClientRealtimeInputMessage,
    ManifestChannel, NotificationPayload, NotificationPlacement, NotificationSeverity, PlayerInput,
    ServerNotificationMessage, ServerOwnerAssetManifestDeltaMessage,
    ServerTacticalContactsDeltaMessage, TacticalDeltaChannel, register_lightyear_client_protocol,
    register_lightyear_server_protocol, replace_action_queue_from_actions,
    replace_action_queue_from_player_input,
};

#[test]
fn lightyear_server_protocol_registration_registers_messages() {
    let mut app = App::new();
    app.add_plugins(ServerPlugins::default());
    register_lightyear_server_protocol(&mut app);
}

#[test]
fn lightyear_client_protocol_registration_registers_messages() {
    let mut app = App::new();
    app.add_plugins(ClientPlugins::default());
    register_lightyear_client_protocol(&mut app);
}

#[test]
fn lightyear_server_protocol_registers_one_way_message_components() {
    let mut app = App::new();
    app.add_plugins(ServerPlugins::default());
    register_lightyear_server_protocol(&mut app);

    let client = app.world_mut().spawn(ClientOf).id();
    let entity = app.world().entity(client);

    assert!(entity.contains::<MessageReceiver<ClientAuthMessage>>());
    assert!(entity.contains::<MessageReceiver<ClientRealtimeInputMessage>>());
    assert!(entity.contains::<MessageReceiver<ClientNotificationDismissedMessage>>());
    assert!(!entity.contains::<MessageSender<ClientAuthMessage>>());
    assert!(!entity.contains::<MessageSender<ClientRealtimeInputMessage>>());
    assert!(!entity.contains::<MessageSender<ClientNotificationDismissedMessage>>());

    assert!(entity.contains::<MessageSender<ServerNotificationMessage>>());
    assert!(entity.contains::<MessageSender<ServerOwnerAssetManifestDeltaMessage>>());
    assert!(entity.contains::<MessageSender<ServerTacticalContactsDeltaMessage>>());
    assert!(!entity.contains::<MessageReceiver<ServerNotificationMessage>>());
    assert!(!entity.contains::<MessageReceiver<ServerOwnerAssetManifestDeltaMessage>>());
    assert!(!entity.contains::<MessageReceiver<ServerTacticalContactsDeltaMessage>>());
}

#[test]
fn lightyear_client_protocol_registers_one_way_message_components() {
    let mut app = App::new();
    app.add_plugins(ClientPlugins::default());
    register_lightyear_client_protocol(&mut app);

    let client = app.world_mut().spawn(Client::default()).id();
    let entity = app.world().entity(client);

    assert!(entity.contains::<MessageSender<ClientAuthMessage>>());
    assert!(entity.contains::<MessageSender<ClientRealtimeInputMessage>>());
    assert!(entity.contains::<MessageSender<ClientNotificationDismissedMessage>>());
    assert!(!entity.contains::<MessageReceiver<ClientAuthMessage>>());
    assert!(!entity.contains::<MessageReceiver<ClientRealtimeInputMessage>>());
    assert!(!entity.contains::<MessageReceiver<ClientNotificationDismissedMessage>>());

    assert!(entity.contains::<MessageReceiver<ServerNotificationMessage>>());
    assert!(entity.contains::<MessageReceiver<ServerOwnerAssetManifestDeltaMessage>>());
    assert!(entity.contains::<MessageReceiver<ServerTacticalContactsDeltaMessage>>());
    assert!(!entity.contains::<MessageSender<ServerNotificationMessage>>());
    assert!(!entity.contains::<MessageSender<ServerOwnerAssetManifestDeltaMessage>>());
    assert!(!entity.contains::<MessageSender<ServerTacticalContactsDeltaMessage>>());
}

#[test]
fn lightyear_protocol_registers_one_way_server_delivery_channels() {
    let mut server_app = App::new();
    server_app.add_plugins(ServerPlugins::default());
    register_lightyear_server_protocol(&mut server_app);
    let server_client = server_app
        .world_mut()
        .spawn((ClientOf, Transport::default()))
        .id();
    server_app.update();
    let server_entity = server_app.world().entity(server_client);
    let server_transport = server_entity.get::<Transport>().expect("transport exists");
    assert!(server_transport.has_sender::<TacticalDeltaChannel>());
    assert!(!server_transport.has_receiver::<TacticalDeltaChannel>());
    assert!(server_transport.has_sender::<ManifestChannel>());
    assert!(!server_transport.has_receiver::<ManifestChannel>());

    let mut client_app = App::new();
    client_app.add_plugins(ClientPlugins::default());
    register_lightyear_client_protocol(&mut client_app);
    let client = client_app
        .world_mut()
        .spawn((Client::default(), Transport::default()))
        .id();
    client_app.update();
    let client_entity = client_app.world().entity(client);
    let client_transport = client_entity.get::<Transport>().expect("transport exists");
    assert!(!client_transport.has_sender::<TacticalDeltaChannel>());
    assert!(client_transport.has_receiver::<TacticalDeltaChannel>());
    assert!(!client_transport.has_sender::<ManifestChannel>());
    assert!(client_transport.has_receiver::<ManifestChannel>());
}

#[test]
fn player_input_matches_axis_mapping() {
    let player_input = PlayerInput::from_axis_inputs(1.0, -1.0, false, false, false);
    assert_eq!(
        player_input.actions,
        vec![
            EntityAction::Forward,
            EntityAction::Right,
            EntityAction::AfterburnerOff
        ]
    );
}

#[test]
fn player_input_replaces_action_queue_snapshot() {
    let mut queue = ActionQueue {
        pending: vec![EntityAction::FirePrimary],
    };
    let input = PlayerInput {
        actions: vec![EntityAction::Forward, EntityAction::Left],
    };

    replace_action_queue_from_player_input(&mut queue, &input);

    assert_eq!(queue.pending, input.actions);

    replace_action_queue_from_actions(&mut queue, &[EntityAction::LongitudinalNeutral]);

    assert_eq!(queue.pending, vec![EntityAction::LongitudinalNeutral]);
}

#[test]
fn notification_defaults_match_toast_lane() {
    assert_eq!(NotificationSeverity::default(), NotificationSeverity::Info);
    assert_eq!(
        NotificationPlacement::default(),
        NotificationPlacement::BottomRight
    );
}

#[test]
fn notification_message_roundtrips_through_json() {
    let message = ServerNotificationMessage {
        notification_id: "11111111-2222-3333-4444-555555555555".to_string(),
        player_entity_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
        title: "Landmark Discovered".to_string(),
        body: "Aurelia".to_string(),
        severity: NotificationSeverity::Info,
        placement: NotificationPlacement::BottomRight,
        image: None,
        payload: NotificationPayload::LandmarkDiscovery {
            entity_guid: "0012ebad-0000-0000-0000-000000000010".to_string(),
            display_name: "Aurelia".to_string(),
            landmark_kind: "Planet".to_string(),
            map_icon_asset_id: Some("map_icon_planet_svg".to_string()),
            world_position_xy: Some([8000.0, 0.0]),
        },
        created_at_epoch_s: 1_714_000_000,
        auto_dismiss_after_s: Some(5.0),
    };

    let encoded = serde_json::to_string(&message).expect("notification should serialize");
    let decoded: ServerNotificationMessage =
        serde_json::from_str(&encoded).expect("notification should deserialize");

    assert_eq!(decoded, message);
    assert_eq!(decoded.payload.kind(), "landmark_discovery");
}
