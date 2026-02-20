use bevy::prelude::App;
use core::time::Duration;
use lightyear::prelude::{
    AppChannelExt, AppMessageExt, ChannelMode, ChannelSettings, NetworkDirection, ReliableSettings,
};
use serde::{Deserialize, Serialize};
use sidereal_game::EntityAction;

use crate::WorldStateDelta;

/// Client sends input actions to replication server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientInputMessage {
    pub player_entity_id: String,
    pub actions: Vec<EntityAction>,
    pub tick: u64,
}

/// Client authenticates replication session and binds transport identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientAuthMessage {
    pub player_entity_id: String,
    pub access_token: String,
}

/// Replication sends state to clients
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplicationStateMessage {
    pub tick: u64,
    pub world_json: Vec<u8>,
}

impl ReplicationStateMessage {
    pub fn from_world(tick: u64, world: &WorldStateDelta) -> serde_json::Result<Self> {
        Ok(Self {
            tick,
            world_json: serde_json::to_vec(world)?,
        })
    }

    pub fn decode_world(&self) -> serde_json::Result<WorldStateDelta> {
        serde_json::from_slice(&self.world_json)
    }
}

impl ClientInputMessage {
    pub fn from_axis_inputs(
        player_entity_id: String,
        tick: u64,
        thrust: f32,
        turn: f32,
        brake: bool,
    ) -> Self {
        let mut actions = Vec::new();
        if brake {
            actions.push(EntityAction::Brake);
        } else if thrust > 0.0 {
            actions.push(EntityAction::ThrustForward);
        } else if thrust < 0.0 {
            actions.push(EntityAction::ThrustReverse);
        } else {
            actions.push(EntityAction::ThrustNeutral);
        }

        if turn > 0.0 {
            actions.push(EntityAction::YawLeft);
        } else if turn < 0.0 {
            actions.push(EntityAction::YawRight);
        } else {
            actions.push(EntityAction::YawNeutral);
        }

        Self {
            player_entity_id,
            actions,
            tick,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum LightyearWireMessage {
    ClientAuth(ClientAuthMessage),
    ClientInput(ClientInputMessage),
    ReplicationState(ReplicationStateMessage),
}

#[derive(Debug)]
pub struct ControlChannel;
#[derive(Debug)]
pub struct InputChannel;
#[derive(Debug)]
pub struct StateChannel;

pub fn register_lightyear_protocol(app: &mut App) {
    app.register_message::<ClientAuthMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<ClientInputMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<ReplicationStateMessage>()
        .add_direction(NetworkDirection::Bidirectional);

    app.add_channel::<ControlChannel>(ChannelSettings {
        mode: ChannelMode::UnorderedReliable(ReliableSettings::default()),
        send_frequency: Duration::default(),
        priority: 8.0,
    })
    .add_direction(NetworkDirection::Bidirectional);
    app.add_channel::<InputChannel>(ChannelSettings {
        mode: ChannelMode::UnorderedUnreliable,
        send_frequency: Duration::default(),
        priority: 10.0,
    })
    .add_direction(NetworkDirection::Bidirectional);
    app.add_channel::<StateChannel>(ChannelSettings {
        mode: ChannelMode::UnorderedUnreliable,
        send_frequency: Duration::default(),
        priority: 10.0,
    })
    .add_direction(NetworkDirection::Bidirectional);
}

pub fn encode_wire_message(message: &LightyearWireMessage) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(message)
}

pub fn decode_wire_message(bytes: &[u8]) -> serde_json::Result<LightyearWireMessage> {
    serde_json::from_slice(bytes)
}
