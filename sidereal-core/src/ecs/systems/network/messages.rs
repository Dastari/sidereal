use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use bincode::{Encode, Decode};
use crate::ecs::systems::sectors::SectorCoord;
use crate::plugins::SerializedEntity;


pub const PROTOCOL_ID: u64 = 1000;
#[derive(Event)]
pub struct NetworkMessageEvent {
    pub client_id: u64,
    pub message: NetworkMessage,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub enum NetworkMessage {
    Ping,
    Pong,
    ShardConnected,
    ShardDisconnected,
    RequestWorldState,
    Heartbeat { timestamp: f64 },
    EntityUpdates { updated_entities:  Vec<SerializedEntity> , timestamp: f64 },
    AssignSectors { sectors: Vec<SectorCoord> },
    RevokeSectors { sectors: Vec<SectorCoord> },
    SectorAssignmentConfirm { sectors: Vec<SectorCoord> },
    SectorLoadReport { load_factor: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntityWrapper(Entity);

impl From<Entity> for EntityWrapper {
    fn from(entity: Entity) -> Self {
        EntityWrapper(entity)
    }
}

impl From<EntityWrapper> for Entity {
    fn from(wrapper: EntityWrapper) -> Self {
        wrapper.0
    }
}

impl Serialize for EntityWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.to_bits().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EntityWrapper {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = u64::deserialize(deserializer)?;
        Ok(EntityWrapper(Entity::from_bits(bits)))
    }
}

impl Encode for EntityWrapper {
    fn encode<E: bincode::enc::Encoder>(&self, encoder: &mut E) -> Result<(), bincode::error::EncodeError> {
        self.0.to_bits().encode(encoder)
    }
}

impl<CTX> Decode<CTX> for EntityWrapper {
    fn decode<D: bincode::de::Decoder<Context = CTX>>(decoder: &mut D) -> Result<Self, bincode::error::DecodeError> {
        let bits = u64::decode(decoder)?;
        Ok(EntityWrapper(Entity::from_bits(bits)))
    }
}

impl<'de, CTX> bincode::BorrowDecode<'de, CTX> for EntityWrapper {
    fn borrow_decode<D>(decoder: &mut D) -> Result<Self, bincode::error::DecodeError>
    where
        D: bincode::de::Decoder<Context = CTX> + bincode::de::BorrowDecoder<'de, Context = CTX>,
    {
        let bits = <u64 as bincode::BorrowDecode<'de, CTX>>::borrow_decode(decoder)?;
        Ok(EntityWrapper(Entity::from_bits(bits)))
    }
}



