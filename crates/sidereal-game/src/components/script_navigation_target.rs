use bevy::math::DVec2;
use bevy::prelude::*;
use bevy::reflect::Reflect;

use crate::EntityGuid;

/// Runtime-only high-level navigation intent emitted by authoritative scripts.
///
/// This is intentionally not persisted or replicated. The authoritative
/// simulation consumes it into capability-specific control state.
#[derive(Debug, Clone, Copy, Component, Reflect, PartialEq)]
#[reflect(Component)]
#[require(EntityGuid)]
pub struct ScriptNavigationTarget {
    pub target_position: DVec2,
}
