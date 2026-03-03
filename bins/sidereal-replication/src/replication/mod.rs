pub mod assets;
pub mod auth;
pub mod combat;
pub mod control;
pub mod input;
pub mod lifecycle;
pub mod persistence;
pub mod runtime_state;
pub mod simulation_entities;
pub mod visibility;

pub use simulation_entities::{
    PendingControlledByBindings, PlayerControlledEntityMap, PlayerRuntimeEntityMap,
    SimulatedControlledEntity,
};

use std::sync::OnceLock;

pub fn debug_env(name: &'static str) -> bool {
    static SIDEREAL_DEBUG_CONTROL_LOGS: OnceLock<bool> = OnceLock::new();
    static SIDEREAL_DEBUG_INPUT_LOGS: OnceLock<bool> = OnceLock::new();
    static SIDEREAL_REPLICATION_SUMMARY_LOGS: OnceLock<bool> = OnceLock::new();

    let parse =
        |var: &str| std::env::var(var).is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    match name {
        "SIDEREAL_DEBUG_CONTROL_LOGS" => *SIDEREAL_DEBUG_CONTROL_LOGS.get_or_init(|| parse(name)),
        "SIDEREAL_DEBUG_INPUT_LOGS" => *SIDEREAL_DEBUG_INPUT_LOGS.get_or_init(|| parse(name)),
        "SIDEREAL_REPLICATION_SUMMARY_LOGS" => {
            *SIDEREAL_REPLICATION_SUMMARY_LOGS.get_or_init(|| parse(name))
        }
        _ => parse(name),
    }
}
