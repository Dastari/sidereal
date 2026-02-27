use bevy::prelude::World;
use sidereal_game::entities::ship::corvette::{
    CorvetteOverrides, corvette_random_spawn_position, default_corvette_mass_kg, spawn_corvette,
};
use uuid::Uuid;

#[test]
fn corvette_bundle_spawn_with_overrides() {
    let mut world = World::new();
    let mut commands = world.commands();

    let overrides = CorvetteOverrides::for_player(Uuid::new_v4(), "player:test-123".to_string(), 1)
        .with_display_name("Test Ship");

    let (ship_guid, module_guids) = spawn_corvette(&mut commands, overrides);

    assert_ne!(ship_guid, Uuid::nil());
    assert_ne!(module_guids.flight_computer, Uuid::nil());
    assert_ne!(module_guids.engine_left, Uuid::nil());
    assert_ne!(module_guids.engine_right, Uuid::nil());
}

#[test]
fn corvette_total_mass() {
    let hull_mass = default_corvette_mass_kg();
    let total = hull_mass + 50.0 + 2.0 * 500.0 + 2.0 * 1100.0;
    assert_eq!(total, 18_250.0);
}

#[test]
fn corvette_spawn_position_deterministic() {
    let account_id = Uuid::new_v4();
    let pos = corvette_random_spawn_position(account_id);
    assert!(pos.x >= -500.0 && pos.x <= 500.0);
    assert!(pos.y >= -500.0 && pos.y <= 500.0);
    assert_eq!(pos.z, 0.0);
    assert_eq!(pos, corvette_random_spawn_position(account_id));
}
