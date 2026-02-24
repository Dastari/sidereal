use bevy::prelude::*;

pub mod actions;
pub mod component_meta;
pub mod components;
pub mod entities;
pub mod flight;
pub mod generated;
pub mod mass;
pub mod scanner;

// Re-export commonly used items
pub use actions::*;
pub use component_meta::*;
pub use components::*;
pub use entities::*;
pub use generated::components::*;
pub use mass::recompute_total_mass;
pub use scanner::{apply_range_buff, compute_scanner_contribution, total_scanner_range_for_parent};

// Re-export flight systems (not components, those come from generated)
pub use flight::{
    angular_inertia_from_size, apply_engine_thrust, clamp_angular_velocity, process_flight_actions,
    sanitize_planar_angular_velocity, stabilize_idle_motion,
};

/// Registers gameplay component types and reflection metadata only.
/// Does NOT add simulation systems. Use on the client where simulation
/// is server-authoritative and local flight systems must not run.
pub struct SiderealGameCorePlugin;

impl Plugin for SiderealGameCorePlugin {
    fn build(&self, app: &mut App) {
        generated::components::register_generated_components(app);

        app.register_type::<EntityAction>()
            .register_type::<ActionQueue>()
            .register_type::<ActionCapabilities>();
    }
}

/// Full gameplay plugin: component registration + simulation systems.
/// Use on server/replication where authoritative simulation runs.
pub struct SiderealGamePlugin;

impl Plugin for SiderealGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SiderealGameCorePlugin);

        app.add_systems(
            FixedUpdate,
            (
                validate_action_capabilities,
                process_flight_actions,
                recompute_total_mass,
                apply_engine_thrust,
            )
                .chain()
                .before(avian3d::prelude::PhysicsSystems::StepSimulation),
        );
        app.add_systems(
            FixedUpdate,
            (stabilize_idle_motion, clamp_angular_velocity)
                .chain()
                .after(avian3d::prelude::PhysicsSystems::StepSimulation),
        );
    }
}
