use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Component, Reflect, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct Hull {
    pub width: f32,
    pub height: f32,
    pub blocks: Vec<Block>,
}

impl Default for Hull {
    fn default() -> Self {
        Self {
            width: 10.0,
            height: 10.0,
            blocks: vec![Block::default()],
        }
    }
}

#[derive(Component, Reflect, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct Block {
    pub x: f32,
    pub y: f32,
    pub direction: Direction,
}

impl Default for Block {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            direction: Direction::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
#[reflect(Serialize, Deserialize)]
pub enum Direction {
    Port,
    Starboard,
    Fore,
    Aft,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Fore
    }
}
