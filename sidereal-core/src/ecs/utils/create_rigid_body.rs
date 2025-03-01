use bevy_rapier2d::prelude::*;
pub fn create_rigid_body(mass: f32, is_fixed: bool) -> RigidBody {
    if is_fixed {
        RigidBody::Fixed
    } else if mass <= 0.0 {
        RigidBody::KinematicVelocityBased
    } else {
        RigidBody::Dynamic
    }
}
