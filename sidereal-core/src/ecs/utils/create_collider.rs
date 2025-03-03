use bevy_rapier2d::prelude::*;
pub fn create_collider(radius: f32) -> Collider {
    Collider::ball(radius)
}
