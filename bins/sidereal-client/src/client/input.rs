use bevy::prelude::{ButtonInput, KeyCode};
use sidereal_net::PlayerInput;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputAxes {
    pub thrust: f32,
    pub turn: f32,
    pub brake: bool,
}

pub fn player_input_from_keyboard(
    input: Option<&ButtonInput<KeyCode>>,
) -> (PlayerInput, InputAxes) {
    let brake = input.is_some_and(|keys| keys.pressed(KeyCode::Space));
    let thrust = if brake {
        0.0
    } else if input.is_some_and(|keys| keys.pressed(KeyCode::KeyW)) {
        1.0
    } else if input.is_some_and(|keys| keys.pressed(KeyCode::KeyS)) {
        -0.7
    } else {
        0.0
    };
    let turn = if input.is_some_and(|keys| keys.pressed(KeyCode::KeyA)) {
        1.0
    } else if input.is_some_and(|keys| keys.pressed(KeyCode::KeyD)) {
        -1.0
    } else {
        0.0
    };
    let axes = InputAxes {
        thrust,
        turn,
        brake,
    };
    (PlayerInput::from_axis_inputs(thrust, turn, brake), axes)
}

pub fn neutral_player_input() -> (PlayerInput, InputAxes) {
    let axes = InputAxes {
        thrust: 0.0,
        turn: 0.0,
        brake: false,
    };
    (PlayerInput::from_axis_inputs(0.0, 0.0, false), axes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_input_has_no_actions() {
        let (input, axes) = neutral_player_input();
        assert_eq!(axes.thrust, 0.0);
        assert_eq!(axes.turn, 0.0);
        assert!(!axes.brake);
        assert!(input.actions.iter().all(|action| {
            matches!(
                action,
                sidereal_game::EntityAction::ThrustNeutral
                    | sidereal_game::EntityAction::YawNeutral
                    | sidereal_game::EntityAction::LongitudinalNeutral
                    | sidereal_game::EntityAction::LateralNeutral
            )
        }));
    }
}
