use sidereal_game::{
    ActionQueue, EntityAction, FLIGHT_CONTROL_ACTIONS, MAX_PENDING_ACTIONS,
    default_flight_action_capabilities,
};

#[test]
fn action_queue_stays_bounded_under_burst() {
    let mut queue = ActionQueue::default();
    for i in 0..(MAX_PENDING_ACTIONS + 32) {
        let action = if i % 2 == 0 {
            EntityAction::Forward
        } else {
            EntityAction::Left
        };
        queue.push(action);
    }
    assert_eq!(queue.pending.len(), MAX_PENDING_ACTIONS);
    assert_eq!(queue.pending[0], EntityAction::Forward);
}

#[test]
fn default_flight_capabilities_match_allowlist() {
    let caps = default_flight_action_capabilities();
    for action in FLIGHT_CONTROL_ACTIONS {
        assert!(caps.can_handle(action));
    }
    assert!(caps.can_handle(EntityAction::AfterburnerOn));
    assert!(caps.can_handle(EntityAction::AfterburnerOff));
    assert!(caps.can_handle(EntityAction::Brake));
    assert!(caps.can_handle(EntityAction::FirePrimary));
    assert!(caps.can_handle(EntityAction::FireSecondary));
}
