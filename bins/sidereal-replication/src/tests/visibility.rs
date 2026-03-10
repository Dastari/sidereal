use bevy::prelude::Vec3;
use std::collections::HashSet;

use crate::replication::visibility::{
    DEFAULT_VIEW_RANGE_M, PlayerVisibilityContext, VisibilityAuthorization, authorize_visibility,
    is_entity_visible_to_player, should_bypass_candidate_filter,
};
use sidereal_net::ClientLocalViewMode;

fn visibility_context(
    player_entity_id: &str,
    observer_anchor_position: Option<Vec3>,
    player_faction_id: Option<&str>,
    visibility_sources: Vec<(Vec3, f32)>,
) -> PlayerVisibilityContext {
    PlayerVisibilityContext {
        player_entity_id: player_entity_id.to_string(),
        observer_anchor_position,
        visibility_sources,
        discovered_static_landmarks: HashSet::new(),
        player_faction_id: player_faction_id.map(ToString::to_string),
        view_mode: ClientLocalViewMode::Tactical,
    }
}

#[test]
fn owner_authorization_still_requires_delivery_scope() {
    let ctx = visibility_context("player-a", None, None, vec![]);
    assert_eq!(
        authorize_visibility(
            "player-a",
            Some("player-a"),
            false,
            false,
            false,
            None,
            None,
            0.0,
            &ctx.as_ref(),
        ),
        Some(VisibilityAuthorization::Owner)
    );
    assert!(!is_entity_visible_to_player(
        "player-a",
        Some("player-a"),
        false,
        false,
        false,
        None,
        Some(Vec3::new(10.0, 0.0, 0.0)),
        0.0,
        &ctx.as_ref(),
        DEFAULT_VIEW_RANGE_M,
        false,
    ));
}

#[test]
fn public_authorization_is_independent_of_delivery_scope() {
    let ctx = visibility_context("player-a", None, None, vec![]);
    assert_eq!(
        authorize_visibility(
            "player-a",
            None,
            true,
            false,
            false,
            None,
            None,
            0.0,
            &ctx.as_ref(),
        ),
        Some(VisibilityAuthorization::Public)
    );
    assert!(!is_entity_visible_to_player(
        "player-a",
        None,
        true,
        false,
        false,
        None,
        Some(Vec3::new(10.0, 0.0, 0.0)),
        0.0,
        &ctx.as_ref(),
        DEFAULT_VIEW_RANGE_M,
        false,
    ));
}

#[test]
fn faction_authorization_is_independent_of_delivery_scope() {
    let ctx = visibility_context("player-a", None, Some("faction-1"), vec![]);
    assert_eq!(
        authorize_visibility(
            "player-a",
            None,
            false,
            true,
            false,
            Some("faction-1"),
            None,
            0.0,
            &ctx.as_ref(),
        ),
        Some(VisibilityAuthorization::Faction)
    );
    assert!(!is_entity_visible_to_player(
        "player-a",
        None,
        false,
        true,
        false,
        Some("faction-1"),
        Some(Vec3::ZERO),
        0.0,
        &ctx.as_ref(),
        DEFAULT_VIEW_RANGE_M,
        false,
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
            false,
            None,
            Some(Vec3::new(0.0, 0.0, 0.0)),
            0.0,
            &ctx.as_ref()
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
            false,
            None,
            Some(target_position),
            0.0,
            &ctx.as_ref()
        ),
        Some(VisibilityAuthorization::Range)
    );
    assert!(!is_entity_visible_to_player(
        "player-a",
        None,
        false,
        false,
        false,
        None,
        Some(target_position),
        0.0,
        &ctx.as_ref(),
        DEFAULT_VIEW_RANGE_M,
        false,
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
            false,
            None,
            Some(target_position),
            0.0,
            &ctx.as_ref()
        ),
        Some(VisibilityAuthorization::Range)
    );
    assert!(!is_entity_visible_to_player(
        "player-a",
        None,
        false,
        false,
        false,
        None,
        Some(target_position),
        0.0,
        &ctx.as_ref(),
        DEFAULT_VIEW_RANGE_M,
        false,
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
        false,
        None,
        Some(target_position),
        0.0,
        &ctx.as_ref(),
        DEFAULT_VIEW_RANGE_M,
        false,
    ));
}

#[test]
fn discovered_static_landmark_authorization_still_requires_delivery_scope() {
    let mut ctx = visibility_context("player-a", None, None, vec![]);
    let landmark_id =
        uuid::Uuid::parse_str("11111111-2222-3333-4444-555555555555").expect("valid landmark guid");
    ctx.discovered_static_landmarks.insert(landmark_id);

    assert_eq!(
        authorize_visibility(
            "player-a",
            None,
            false,
            false,
            true,
            None,
            Some(Vec3::new(10.0, 0.0, 0.0)),
            0.0,
            &ctx.as_ref(),
        ),
        Some(VisibilityAuthorization::DiscoveredStaticLandmark)
    );
    assert!(!is_entity_visible_to_player(
        "player-a",
        None,
        false,
        false,
        true,
        None,
        Some(Vec3::new(10_000.0, 0.0, 0.0)),
        0.0,
        &ctx.as_ref(),
        DEFAULT_VIEW_RANGE_M,
        false,
    ));
}

#[test]
fn discovered_static_landmark_in_delivery_scope_is_visible_without_scanner_coverage() {
    let mut ctx = visibility_context("player-a", Some(Vec3::ZERO), None, vec![]);
    let landmark_id =
        uuid::Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").expect("valid landmark guid");
    ctx.discovered_static_landmarks.insert(landmark_id);

    assert!(is_entity_visible_to_player(
        "player-a",
        None,
        false,
        false,
        true,
        None,
        Some(Vec3::new(100.0, 0.0, 0.0)),
        0.0,
        &ctx.as_ref(),
        DEFAULT_VIEW_RANGE_M,
        false,
    ));
}

#[test]
fn candidate_bypass_triggers_for_owner_public_faction_and_scanner() {
    let owner_ctx = visibility_context("player-a", None, None, vec![]);
    assert!(should_bypass_candidate_filter(
        "player-a",
        Some("player-a"),
        false,
        false,
        false,
        None,
        None,
        0.0,
        &owner_ctx.as_ref()
    ));

    let public_ctx = visibility_context("player-a", None, None, vec![]);
    assert!(should_bypass_candidate_filter(
        "player-a",
        None,
        true,
        false,
        false,
        None,
        None,
        0.0,
        &public_ctx.as_ref()
    ));

    let faction_ctx = visibility_context("player-a", None, Some("faction-1"), vec![]);
    assert!(should_bypass_candidate_filter(
        "player-a",
        None,
        false,
        true,
        false,
        Some("faction-1"),
        Some(Vec3::ZERO),
        0.0,
        &faction_ctx.as_ref()
    ));

    let scanner_ctx = visibility_context(
        "player-a",
        None,
        None,
        vec![(Vec3::new(10.0, 0.0, 0.0), 25.0)],
    );
    assert!(should_bypass_candidate_filter(
        "player-a",
        None,
        false,
        false,
        false,
        None,
        Some(Vec3::new(20.0, 0.0, 0.0)),
        0.0,
        &scanner_ctx.as_ref()
    ));
}

#[test]
fn discovered_landmarks_bypass_candidate_prefilter() {
    let ctx = visibility_context("player-a", None, None, vec![]);
    assert!(should_bypass_candidate_filter(
        "player-a",
        None,
        false,
        false,
        true,
        None,
        Some(Vec3::new(50_000.0, 0.0, 0.0)),
        0.0,
        &ctx.as_ref(),
    ));
}
