use avian2d::prelude::{LinearVelocity, Position, Rotation};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use sidereal_game::{
    ActionQueue, AmmoCount, BallisticProjectile, BallisticWeapon, EntityAction, EntityGuid,
    Hardpoint, MountedOn, OwnerId, ParentGuid, SimulationMotionWriter, WeaponCooldownState,
    bootstrap_weapon_cooldown_state, process_weapon_fire_actions,
};
use uuid::Uuid;

fn spawn_weapon_fixture(app: &mut App, projectile_speed_mps: f32) -> (Entity, Entity) {
    let shooter_guid = Uuid::new_v4();
    let hardpoint_guid = Uuid::new_v4();
    let weapon_guid = Uuid::new_v4();

    let shooter = app
        .world_mut()
        .spawn((
            EntityGuid(shooter_guid),
            Position(Vec2::new(10.0, -20.0)),
            Rotation::from(Quat::IDENTITY),
            LinearVelocity(Vec2::new(35.0, -5.0)),
            OwnerId("player-1".to_string()),
            SimulationMotionWriter,
            ActionQueue {
                pending: vec![EntityAction::FirePrimary],
            },
        ))
        .id();
    app.world_mut().spawn((
        EntityGuid(hardpoint_guid),
        ParentGuid(shooter_guid),
        Hardpoint {
            hardpoint_id: "weapon_fore_center".to_string(),
            offset_m: Vec3::new(0.0, 8.0, 0.0),
            local_rotation: Quat::IDENTITY,
        },
    ));
    let weapon = app
        .world_mut()
        .spawn((
            EntityGuid(weapon_guid),
            MountedOn {
                parent_entity_id: shooter_guid,
                hardpoint_id: "weapon_fore_center".to_string(),
            },
            BallisticWeapon {
                weapon_name: "Test Weapon".to_string(),
                rpm: 600.0,
                damage_per_shot: 12.0,
                max_range_m: 120.0,
                projectile_speed_mps,
                spread_rad: 0.0,
                damage_type: sidereal_game::DamageType::Ballistic,
            },
            WeaponCooldownState::default(),
            AmmoCount::new(5, 5),
        ))
        .id();

    (shooter, weapon)
}

#[test]
fn projectile_weapon_fire_spawns_projectile_with_inherited_velocity() {
    let mut app = App::new();
    app.add_message::<sidereal_game::ShotFiredEvent>();
    app.add_message::<sidereal_game::BallisticProjectileSpawnedEvent>();
    let (_shooter, weapon) = spawn_weapon_fixture(&mut app, 360.0);

    let _ = app
        .world_mut()
        .run_system_once(bootstrap_weapon_cooldown_state);
    let _ = app.world_mut().run_system_once(process_weapon_fire_actions);

    let mut projectile_query = app
        .world_mut()
        .query::<(&BallisticProjectile, &LinearVelocity)>();
    let projectiles = projectile_query.iter(app.world()).collect::<Vec<_>>();
    assert_eq!(projectiles.len(), 1);
    let (projectile, linear_velocity) = projectiles[0];
    assert_eq!(projectile.damage_per_hit, 12.0);
    assert!((linear_velocity.0.x - 35.0).abs() < 0.001);
    assert!((linear_velocity.0.y - 355.0).abs() < 0.001);

    let ammo = app.world().entity(weapon).get::<AmmoCount>().unwrap();
    assert_eq!(ammo.current, 4);
}

#[test]
fn zero_speed_weapon_remains_hitscan_and_spawns_no_projectile_entity() {
    let mut app = App::new();
    app.add_message::<sidereal_game::ShotFiredEvent>();
    app.add_message::<sidereal_game::BallisticProjectileSpawnedEvent>();
    let (_shooter, weapon) = spawn_weapon_fixture(&mut app, 0.0);

    let _ = app.world_mut().run_system_once(process_weapon_fire_actions);

    let projectile_count = app
        .world_mut()
        .query::<&BallisticProjectile>()
        .iter(app.world())
        .count();
    assert_eq!(projectile_count, 0);

    let cooldown = app
        .world()
        .entity(weapon)
        .get::<WeaponCooldownState>()
        .unwrap();
    assert!(cooldown.remaining_s > 0.0);
}
