use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChannelClass {
    Input,
    Control,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetEnvelope<T> {
    pub protocol_version: u16,
    pub channel: ChannelClass,
    pub source_shard_id: i32,
    pub lease_epoch: u64,
    pub seq: u64,
    pub tick: u64,
    pub payload: T,
}

pub fn encode_envelope_json<T: Serialize>(
    envelope: &NetEnvelope<T>,
) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(envelope)
}

pub fn decode_envelope_json<T: DeserializeOwned>(
    bytes: &[u8],
) -> serde_json::Result<NetEnvelope<T>> {
    serde_json::from_slice(bytes)
}
