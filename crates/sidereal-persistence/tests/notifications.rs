use sidereal_persistence::{GraphPersistence, PlayerNotificationRecord};
use uuid::Uuid;

fn test_database_url() -> String {
    std::env::var("SIDEREAL_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("REPLICATION_DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string())
}

#[test]
fn player_notification_history_lifecycle() {
    let database_url = test_database_url();
    let mut persistence = match GraphPersistence::connect(&database_url) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!("skipping notification lifecycle test; postgres unavailable: {err}");
            return;
        }
    };
    if let Err(err) = persistence.ensure_player_notifications_schema() {
        tracing::warn!("skipping notification lifecycle test; schema unavailable: {err}");
        return;
    }

    let notification_id = Uuid::new_v4().to_string();
    let player_entity_id = Uuid::new_v4().to_string();
    let record = PlayerNotificationRecord {
        notification_id: notification_id.clone(),
        player_entity_id: player_entity_id.clone(),
        notification_kind: "landmark_discovery".to_string(),
        severity: "info".to_string(),
        title: "Landmark Discovered".to_string(),
        body: "Aurelia".to_string(),
        image_asset_id: Some("map_icon_planet_svg".to_string()),
        image_alt_text: Some("Planet".to_string()),
        placement: "bottom_right".to_string(),
        payload: serde_json::json!({
            "type": "landmark_discovery",
            "entity_guid": "0012ebad-0000-0000-0000-000000000010",
        }),
        created_at_epoch_s: 1_714_000_000,
        delivered_at_epoch_s: None,
        dismissed_at_epoch_s: None,
    };

    persistence
        .insert_player_notification(&record)
        .expect("notification should insert");
    assert!(
        persistence
            .mark_player_notification_delivered(&player_entity_id, &notification_id, 1_714_000_001)
            .expect("notification should mark delivered")
    );
    assert!(
        persistence
            .mark_player_notification_dismissed(&player_entity_id, &notification_id, 1_714_000_002)
            .expect("notification should mark dismissed")
    );
    assert!(
        !persistence
            .mark_player_notification_dismissed(
                &Uuid::new_v4().to_string(),
                &notification_id,
                1_714_000_003,
            )
            .expect("wrong player should not update")
    );
}
