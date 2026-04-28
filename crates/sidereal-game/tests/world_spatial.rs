use avian2d::prelude::Rotation;
use sidereal_game::{WorldRotation, resolve_world_rotation_rad};

#[test]
fn world_rotation_resolution_prefers_avian_rotation() {
    let avian_rotation = Rotation::radians(1.25);
    let world_rotation = WorldRotation(-0.75);

    assert_eq!(
        resolve_world_rotation_rad(Some(&avian_rotation), Some(&world_rotation)),
        Some(1.25)
    );
}

#[test]
fn world_rotation_resolution_falls_back_to_static_world_rotation() {
    let world_rotation = WorldRotation(-0.75);

    assert_eq!(
        resolve_world_rotation_rad(None, Some(&world_rotation)),
        Some(-0.75)
    );
}
