use bevy::prelude::*;
use bevy::reflect::Reflect;

/// Runtime-only marker for entities allowed to execute shared motion/combat writer systems.
/// Not persisted or replicated.
#[derive(Debug, Clone, Copy, Component, Reflect, Default)]
#[reflect(Component, Default)]
pub struct SimulationMotionWriter;
