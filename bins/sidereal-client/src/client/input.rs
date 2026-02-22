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
