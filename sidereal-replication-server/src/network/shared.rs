pub use bevy::prelude::*;
pub use bevy_renet::renet::*;
pub use bevy_renet::*;
use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    Ping,
    Pong,
    ShardConnected { shard_id: String },
    RequestWorldState,
    Heartbeat { timestamp: f64 },
    EntityUpdates { updated_entities: Vec<Entity>, timestamp: f64 },
}

pub const PROTOCOL_ID: u64 = 1000;