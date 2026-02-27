use bevy::prelude::Vec3;

use crate::replication::visibility::{
    PlayerVisibilityContext, VisibilityAuthorization, authorize_visibility,
    is_entity_visible_to_player,
};

fn visibility_context(
    player_entity_id: &str,
    observer_anchor_position: Option<Vec3>,
    player_faction_id: Option<&str>,
    scanner_sources: Vec<(Vec3, f32)>,
) -> PlayerVisibilityContext {
    PlayerVisibilityContext {
        player_entity_id: player_entity_id.to_string(),
        observer_anchor_position,
        scanner_sources,
        player_faction_id: player_faction_id.map(ToString::to_string),
    }
}

#[test]
fn owner_authorization_bypasses_delivery_scope() {
    let ctx = visibility_context("player-a", None, None, vec![]);
    assert_eq!(
        authorize_visibility("player-a", Some("player-a"), false, false, None, None, &ctx),
        Some(VisibilityAuthorization::Owner)
    );
    assert!(is_entity_visible_to_player(
        "player-a",
        Some("player-a"),
        false,
        false,
        None,
        None,
        &ctx
    ));
}

#[test]
fn public_authorization_is_independent_of_delivery_scope() {
    let ctx = visibility_context("player-a", None, None, vec![]);
    assert_eq!(
        authorize_visibility("player-a", None, true, false, None, None, &ctx),
        Some(VisibilityAuthorization::Public)
    );
    assert!(!is_entity_visible_to_player(
        "player-a",
        None,
        true,
        false,
        None,
        Some(Vec3::new(10.0, 0.0, 0.0)),
        &ctx
    ));
}

#[test]
fn faction_authorization_is_independent_of_delivery_scope() {
    let ctx = visibility_context("player-a", None, Some("faction-1"), vec![]);
    assert_eq!(
        authorize_visibility("player-a", None, false, true, Some("faction-1"), None, &ctx),
        Some(VisibilityAuthorization::Faction)
    );
    assert!(!is_entity_visible_to_player(
        "player-a",
        None,
        false,
        true,
        Some("faction-1"),
        Some(Vec3::ZERO),
        &ctx
    ));
}

#[test]
fn scanner_authorization_requires_scanner_coverage() {
    let ctx = visibility_context(
        "player-a",
        Some(Vec3::ZERO),
        None,
        vec![(Vec3::new(1000.0, 0.0, 0.0), 50.0)],
    );
    assert_eq!(
        authorize_visibility(
            "player-a",
            None,
            false,
            false,
            None,
            Some(Vec3::new(0.0, 0.0, 0.0)),
            &ctx
        ),
        None
    );
}

#[test]
fn scanner_authorization_still_requires_delivery_scope() {
    let ctx = visibility_context(
        "player-a",
        Some(Vec3::ZERO),
        None,
        vec![(Vec3::new(1000.0, 0.0, 0.0), 200.0)],
    );
    let target_position = Vec3::new(1050.0, 0.0, 0.0);
    assert_eq!(
        authorize_visibility(
            "player-a",
            None,
            false,
            false,
            None,
            Some(target_position),
            &ctx
        ),
        Some(VisibilityAuthorization::Scanner)
    );
    assert!(!is_entity_visible_to_player(
        "player-a",
        None,
        false,
        false,
        None,
        Some(target_position),
        &ctx
    ));
}

#[test]
fn scanner_authorization_with_missing_observer_anchor_is_culled() {
    let ctx = visibility_context(
        "player-a",
        None,
        None,
        vec![(Vec3::new(1000.0, 0.0, 0.0), 200.0)],
    );
    let target_position = Vec3::new(1050.0, 0.0, 0.0);
    assert_eq!(
        authorize_visibility(
            "player-a",
            None,
            false,
            false,
            None,
            Some(target_position),
            &ctx
        ),
        Some(VisibilityAuthorization::Scanner)
    );
    assert!(!is_entity_visible_to_player(
        "player-a",
        None,
        false,
        false,
        None,
        Some(target_position),
        &ctx
    ));
}

#[test]
fn scanner_authorization_with_player_anchor_in_range_is_visible() {
    let ctx = visibility_context(
        "player-a",
        Some(Vec3::new(1000.0, 0.0, 0.0)),
        None,
        vec![(Vec3::new(1000.0, 0.0, 0.0), 200.0)],
    );
    let target_position = Vec3::new(1050.0, 0.0, 0.0);
    assert!(is_entity_visible_to_player(
        "player-a",
        None,
        false,
        false,
        None,
        Some(target_position),
        &ctx
    ));
}
