use avian3d::prelude::{AngularVelocity, LinearVelocity, Position, Rotation};
use bevy::prelude::*;

use crate::replication::SimulatedControlledEntity;

#[allow(clippy::type_complexity)]
pub fn sync_simulated_ship_components(
    mut ships: Query<
        '_,
        '_,
        (&'_ Position, &'_ Rotation, &'_ mut Transform),
        With<SimulatedControlledEntity>,
    >,
) {
    for (position, rotation, mut transform) in &mut ships {
        let mut planar_position = position.0;
        if !planar_position.is_finite() {
            planar_position = Vec3::ZERO;
        }
        planar_position.z = 0.0;
        let safe_rotation = if rotation.0.is_finite() {
            rotation.0
        } else {
            Quat::IDENTITY
        };
        let mut heading = safe_rotation.to_euler(EulerRot::ZYX).0;
        if !heading.is_finite() {
            heading = 0.0;
        }
        transform.translation = planar_position;
        transform.rotation = Quat::from_rotation_z(heading);
    }
}

pub fn enforce_planar_ship_motion(
    mut ships: Query<
        '_,
        '_,
        (
            &'_ mut Position,
            &'_ mut LinearVelocity,
            &'_ mut Rotation,
            &'_ mut AngularVelocity,
        ),
        With<SimulatedControlledEntity>,
    >,
) {
    for (mut position, mut velocity, mut rotation, mut angular_velocity) in &mut ships {
        if !position.0.is_finite() {
            position.0 = Vec3::ZERO;
        }
        if !velocity.0.is_finite() {
            velocity.0 = Vec3::ZERO;
        }
        if !angular_velocity.0.is_finite() {
            angular_velocity.0 = Vec3::ZERO;
        }
        position.0.z = 0.0;
        velocity.0.z = 0.0;
        angular_velocity.0.x = 0.0;
        angular_velocity.0.y = 0.0;
        let mut heading = if rotation.0.is_finite() {
            rotation.0.to_euler(EulerRot::ZYX).0
        } else {
            0.0
        };
        if !heading.is_finite() {
            heading = 0.0;
        }
        rotation.0 = Quat::from_rotation_z(heading);
    }
}
