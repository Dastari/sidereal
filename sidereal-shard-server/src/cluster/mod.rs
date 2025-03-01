pub mod plugin;
pub mod boundary;
pub mod manager;
pub mod systems;

pub use plugin::ClusterManagerPlugin;
pub use sidereal_core::ecs::components::spatial::{SpatialPosition as Position, BoundaryDirection, Cluster}; 