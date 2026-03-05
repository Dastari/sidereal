use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[reflect(Serialize, Deserialize)]
pub struct VisibilityGridCell {
    pub x: i64,
    pub y: i64,
}

#[sidereal_component_macros::sidereal_component(
    kind = "visibility_spatial_grid",
    persist = true,
    replicate = true,
    predict = true,
    visibility = [OwnerOnly]
)]
#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct VisibilitySpatialGrid {
    pub candidate_mode: String,
    pub cell_size_m: f32,
    pub delivery_range_m: f32,
    pub queried_cells: Vec<VisibilityGridCell>,
}
