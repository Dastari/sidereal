#![cfg(feature = "lightyear_protocol")]

use bevy::prelude::App;
use lightyear::prelude::client::ClientPlugins;
use lightyear::prelude::server::ServerPlugins;
use sidereal_game::EntityAction;
use sidereal_net::{
    NotificationPayload, NotificationPlacement, NotificationSeverity, PlayerInput,
    ServerNotificationMessage, register_lightyear_client_protocol,
    register_lightyear_server_protocol,
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
