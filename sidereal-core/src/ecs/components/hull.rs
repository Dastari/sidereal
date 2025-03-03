use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub struct Hull {
    pub width: f32,
    pub height: f32,
    pub blocks: Vec<Block>,
}

#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub struct Block {
    pub x: f32,
    pub y: f32,
    // pub component: Option<Component>,s
    pub direction: Direction,
}

#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub enum Direction {
    Port,
    Starboard,
    Fore,
    Aft,
}
