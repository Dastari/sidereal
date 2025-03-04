use crate::ecs::components::spatial::{ClusterCoords, Position, SectorCoords};
use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::reflect::Reflect;
use bevy_rapier2d::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Serialize, Deserialize, Clone, Debug, Reflect, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub enum RigidBody {
    #[default]
    Dynamic,
    Static,
    Kinematic,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, Reflect, Default)]
#[require(
    Position,
    SectorCoords,
    ClusterCoords,
    RigidBody,
    Velocity,
    Collider,
    PhysicsState,
    Transform,
    Sleeping,
    Damping,
    GlobalTransform
)]
pub struct PhysicsBody;

#[derive(Component, Clone, Debug, Serialize, Deserialize, Reflect)]

pub struct PhysicsState {
    // Core physics properties
    pub linear_velocity: Vec2,
    pub angular_velocity: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,

    // Mass properties
    pub mass: f32,
    pub center_of_mass: Vec2,

    // Body type
    pub body_type: BodyType,

    // Collider properties
    pub collider_type: ColliderType,
    pub collider_size: Vec2, // For simple shapes like boxes

    // Additional flags
    pub can_sleep: bool,
    pub is_sensor: bool,

    // Timestamp of last Rapier sync
    pub last_sync: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum BodyType {
    Dynamic,
    Static,
    Kinematic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum ColliderType {
    Box,
    Circle,
    Capsule,
    // Add other shapes as needed
}

impl Default for PhysicsState {
    fn default() -> Self {
        Self {
            linear_velocity: Vec2::ZERO,
            angular_velocity: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            mass: 1000.0,
            center_of_mass: Vec2::ZERO,
            body_type: BodyType::Dynamic,
            collider_type: ColliderType::Box,
            collider_size: Vec2::new(10.0, 10.0),
            can_sleep: true,
            is_sensor: false,
            last_sync: 0.0,
        }
    }
}

impl PhysicsState {
    // Helper to create a physics state with common settings for a ship
    pub fn new_ship(width: f32, height: f32, mass: f32) -> Self {
        Self {
            mass,
            collider_type: ColliderType::Box,
            collider_size: Vec2::new(width, height),
            // Other defaults
            ..Default::default()
        }
    }

    // Convert our BodyType to Rapier's RigidBody
    pub fn to_rapier_body_type(&self) -> bevy_rapier2d::dynamics::RigidBody {
        use bevy_rapier2d::dynamics::RigidBody;

        match self.body_type {
            BodyType::Dynamic => RigidBody::Dynamic,
            BodyType::Static => RigidBody::Fixed,
            BodyType::Kinematic => RigidBody::KinematicPositionBased,
        }
    }

    // Create a Rapier collider based on our properties
    pub fn to_rapier_collider(&self) -> bevy_rapier2d::geometry::Collider {
        use bevy_rapier2d::geometry::Collider;

        match self.collider_type {
            ColliderType::Box => {
                Collider::cuboid(self.collider_size.x / 2.0, self.collider_size.y / 2.0)
            }
            ColliderType::Circle => {
                let radius = self.collider_size.x.max(self.collider_size.y) / 2.0;
                Collider::ball(radius)
            }
            ColliderType::Capsule => {
                let half_height = self.collider_size.y / 2.0 - self.collider_size.x / 2.0;
                let radius = self.collider_size.x / 2.0;
                Collider::capsule_y(half_height, radius)
            }
        }
    }
}
