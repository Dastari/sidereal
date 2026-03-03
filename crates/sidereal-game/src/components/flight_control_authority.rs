use bevy::prelude::*;

/// Runtime-only marker that grants permission to execute flight motion writer systems.
///
/// This is intentionally non-persisted and non-replicated; each runtime determines
/// authoritative writers locally (server: simulation authority, client: local controlled root).
#[derive(Component, Debug, Default, Clone, Copy, Reflect)]
#[reflect(Component, Default)]
pub struct FlightControlAuthority;
