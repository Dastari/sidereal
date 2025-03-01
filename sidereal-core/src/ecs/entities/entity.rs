use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use crate::ecs::utils::{create_rigid_body, create_collider};

/// Bundle for easily spawning a physics-enabled space entity
#[derive(Bundle)]
pub struct EntityBundle {
    // Rapier physics components
    rigid_body: RigidBody,
    collider: Collider,
    velocity: Velocity,
    // Bevy rendering components
    transform: Transform,
    global_transform: GlobalTransform,
    visibility: Visibility,
}

impl EntityBundle {
    pub fn new(position: Vec2, radius: f32, mass: f32, is_fixed: bool, initial_velocity: Option<Vec2>) -> Self {
        let vel = initial_velocity.unwrap_or(Vec2::ZERO);
        
        Self {
            rigid_body: create_rigid_body(mass, is_fixed),
            collider: create_collider(radius),
            velocity: Velocity { linvel: vel, angvel: 0.0 },
            transform: Transform::from_xyz(position.x, position.y, 0.0),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::default(),
        }
    }
    
    // Helper methods for specific entity types
    pub fn planet(position: Vec2, radius: f32, mass: f32) -> Self {
        Self::new(position, radius, mass, false, None)
    }
    
    pub fn star(position: Vec2, radius: f32, mass: f32) -> Self {
        Self::new(position, radius, mass, true, None)
    }
}