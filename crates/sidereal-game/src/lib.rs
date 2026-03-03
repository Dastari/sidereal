use bevy::prelude::*;

pub mod actions;
pub mod character_movement;
pub mod combat;
pub mod component_meta;
pub mod components;
pub mod entities;
pub mod flight;
pub mod generated;
pub mod hierarchy;
pub mod mass;
pub mod scanner;

// Re-export commonly used items
pub use actions::*;
pub use character_movement::{
    process_character_movement_actions, sync_player_to_controlled_entity,
};
pub use combat::{
    ShotFiredEvent, ShotHitEvent, bootstrap_legacy_ballistic_weapon_ranges,
    bootstrap_weapon_cooldown_state, process_weapon_fire_actions, tick_weapon_cooldowns,
};
pub use component_meta::*;
pub use components::*;
pub use entities::*;
pub use generated::components::*;
pub use hierarchy::sync_mounted_hierarchy;
pub use mass::{
    bootstrap_collision_profiles_from_aabb, bootstrap_legacy_corvette_collision_aabb,
    bootstrap_legacy_corvette_collision_outline, bootstrap_root_dynamic_entity_colliders,
    bootstrap_ship_mass_components, collider_from_collision_shape, recompute_total_mass,
};
pub use scanner::{apply_range_buff, compute_scanner_contribution, total_scanner_range_for_parent};

// Re-export flight systems (not components, those come from generated)
pub use flight::{
    angular_inertia_from_size, apply_engine_thrust, clamp_angular_velocity,
    compute_brake_decel_accel_mps2, grant_flight_control_authority_system, process_flight_actions,
    revoke_stale_flight_control_authority_system, sanitize_planar_angular_velocity,
    stabilize_idle_motion,
};

/// Controls whether local Bevy hierarchy reconstruction runs in this runtime.
/// Replication server disables this to avoid leaking ChildOf/Children into network replication.
#[derive(Resource, Debug, Clone, Copy)]
pub struct HierarchyRebuildEnabled(pub bool);

impl Default for HierarchyRebuildEnabled {
    fn default() -> Self {
        Self(true)
    }
}

fn hierarchy_rebuild_enabled(flag: Option<Res<HierarchyRebuildEnabled>>) -> bool {
    flag.map(|v| v.0).unwrap_or(true)
}

/// Registers gameplay component types and reflection metadata only.
/// Does NOT add simulation systems. Use on the client where simulation
/// is server-authoritative and local flight systems must not run.
pub struct SiderealGameCorePlugin;

impl Plugin for SiderealGameCorePlugin {
    fn build(&self, app: &mut App) {
        generated::components::register_generated_components(app);

        app.register_type::<EntityAction>()
            .register_type::<ActionQueue>()
            .register_type::<ActionCapabilities>()
            .register_type::<FlightControlAuthority>();
    }
}

/// Full gameplay plugin: component registration + simulation systems.
/// Use on server/replication where authoritative simulation runs.
pub struct SiderealGamePlugin;

impl Plugin for SiderealGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SiderealGameCorePlugin);
        app.insert_resource(HierarchyRebuildEnabled::default());

        app.add_systems(
            PostUpdate,
            (
                bootstrap_ship_mass_components,
                bootstrap_collision_profiles_from_aabb,
                bootstrap_legacy_corvette_collision_aabb,
                bootstrap_legacy_corvette_collision_outline,
                bootstrap_legacy_ballistic_weapon_ranges,
                bootstrap_root_dynamic_entity_colliders,
                sync_mounted_hierarchy
                    .before(bevy::transform::TransformSystems::Propagate)
                    .run_if(hierarchy_rebuild_enabled),
            ),
        );
        app.add_systems(
            FixedUpdate,
            (
                grant_flight_control_authority_system,
                revoke_stale_flight_control_authority_system,
                validate_action_capabilities,
                sync_player_to_controlled_entity,
                process_character_movement_actions,
                process_flight_actions,
                bootstrap_weapon_cooldown_state,
                tick_weapon_cooldowns,
                process_weapon_fire_actions,
                recompute_total_mass,
                apply_engine_thrust,
            )
                .chain()
                .before(avian2d::prelude::PhysicsSystems::StepSimulation),
        );
        app.add_systems(
            FixedUpdate,
            (stabilize_idle_motion, clamp_angular_velocity)
                .chain()
                .after(avian2d::prelude::PhysicsSystems::StepSimulation),
        );
    }
}
