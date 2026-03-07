use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[sidereal_component_macros::sidereal_component(
    kind = "player_explored_cells",
    persist = true,
    replicate = false,
    visibility = [OwnerOnly]
)]
#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct PlayerExploredCells {
    pub cell_size_m: f32,
    pub chunk_size_cells: u16,
    pub chunks: Vec<PlayerExploredCellsChunk>,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[reflect(Serialize, Deserialize)]
pub enum PlayerExploredCellsChunkEncoding {
    #[default]
    Bitset,
    SparseDeltaVarint,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[reflect(Serialize, Deserialize)]
pub struct PlayerExploredCellsChunk {
    pub chunk_x: i32,
    pub chunk_y: i32,
    pub explored_count: u16,
    pub encoding: PlayerExploredCellsChunkEncoding,
    pub payload_b64: String,
}

impl PlayerExploredCells {
    pub const DEFAULT_FOG_CELL_SIZE_M: f32 = 100.0;
    pub const DEFAULT_CHUNK_SIZE_CELLS: u16 = 64;

    pub fn empty_for_fog() -> Self {
        Self {
            cell_size_m: Self::DEFAULT_FOG_CELL_SIZE_M,
            chunk_size_cells: Self::DEFAULT_CHUNK_SIZE_CELLS,
            chunks: Vec::new(),
        }
    }
}

impl Default for PlayerExploredCells {
    fn default() -> Self {
        Self::empty_for_fog()
    }
}

impl PlayerExploredCellsChunk {
    pub fn payload_bytes(&self) -> Option<Vec<u8>> {
        STANDARD_NO_PAD.decode(self.payload_b64.as_bytes()).ok()
    }
}
