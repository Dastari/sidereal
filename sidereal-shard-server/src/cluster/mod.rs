pub mod boundary;
pub mod manager;
pub mod plugin;
pub mod systems;

pub use plugin::ClusterManagerPlugin;
pub use sidereal_core::ecs::components::spatial::{
    BoundaryDirection, Cluster, SpatialPosition as Position,
};
