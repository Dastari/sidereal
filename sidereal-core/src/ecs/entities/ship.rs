use crate::ecs::components::name::Name;
use crate::ecs::components::physics::PhysicsBody;
use crate::ecs::components::hull::Hull;
use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

#[derive(Component, Reflect)]
#[require(Name, Velocity, Hull, PhysicsBody)]
pub struct Ship;    

