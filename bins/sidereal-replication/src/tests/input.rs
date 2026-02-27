use sidereal_game::EntityAction;
use sidereal_net::ClientRealtimeInputMessage;

use crate::replication::input::{
    InputRateLimitState, InputValidationFailure, MAX_ACTIONS_PER_PACKET, MAX_MESSAGES_PER_SECOND,
    validate_input_message,
};

fn message_with(tick: u64, actions: usize) -> ClientRealtimeInputMessage {
    ClientRealtimeInputMessage {
        player_entity_id: "player:test".to_string(),
        controlled_entity_id: "ship:test".to_string(),
        actions: vec![EntityAction::ThrustNeutral; actions],
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
