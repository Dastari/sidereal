use bevy::prelude::*;

pub mod actions;
pub mod corvette;
pub mod flight;
pub mod generated;
pub mod mass;

// Re-export commonly used items
pub use actions::*;
pub use corvette::*;
pub use generated::components::*;
pub use mass::recompute_total_mass;

// Re-export flight systems (not components, those come from generated)
pub use flight::{apply_engine_thrust, process_flight_actions};

pub struct SiderealGamePlugin;

impl Plugin for SiderealGamePlugin {
    fn build(&self, app: &mut App) {
        generated::components::register_generated_components(app);

        // Register action system types
        app.register_type::<EntityAction>()
            .register_type::<ActionQueue>()
            .register_type::<ActionCapabilities>();

        // Register action system (runs in FixedUpdate for determinism)
        app.add_systems(
            FixedUpdate,
            (
                validate_action_capabilities,
                process_flight_actions,
                recompute_total_mass,
                apply_engine_thrust,
            )
                .chain(),
        );
    }
}
