use serde::{Deserialize, Serialize};

pub mod auth;
pub mod bootstrap_wire;
pub mod gateway_dtos;
pub mod net_envelope;
pub mod remote_inspect;

pub const PROTOCOL_VERSION: u16 = 1;
pub const SIM_TICK_HZ: u16 = 30;
pub const RENDER_TARGET_HZ: u16 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub uuid::Uuid);

impl EntityId {
    pub fn new_v4() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub fn is_nil(self) -> bool {
        self.0.is_nil()
    }
}
