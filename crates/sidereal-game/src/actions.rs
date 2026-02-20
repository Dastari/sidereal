//! Entity Action System
//!
//! Architecture:
//! - Client input (keys/mouse) → bindings → EntityAction enums
//! - EntityActions are sent to controlled entity via network or local queue
//! - Entities dispatch actions to components that register as handlers
//! - Handlers implement capability-specific logic (e.g., FlightComputer → Engine → fuel check → apply force)
//!
//! Design principles:
//! - Actions are high-level intent (ThrustForward, FireWeapon, ActivateShield)
//! - No direct force/velocity manipulation from input layer
//! - Components declare which actions they handle
//! - Fuel, power, cooldown, and other constraints are checked at handler level
//! - Same action pipeline works for player input, AI commands, and scripted sequences

use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

/// High-level action that can be sent to any entity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
#[reflect(Serialize, Deserialize)]
pub enum EntityAction {
    // === Flight control ===
    /// Thrust forward (throttle positive)
    ThrustForward,
    /// Thrust reverse (throttle negative)
    ThrustReverse,
    /// Stop all thrust (throttle zero)
    ThrustNeutral,
    /// Active flight-computer braking to drive linear velocity toward zero
    Brake,
    /// Yaw left (turn counterclockwise)
    YawLeft,
    /// Yaw right (turn clockwise)
    YawRight,
    /// Stop yaw input
    YawNeutral,

    // === Combat (future) ===
    /// Fire primary weapon group
    FirePrimary,
    /// Fire secondary weapon group
    FireSecondary,
    /// Activate shield
    ActivateShield,
    /// Deactivate shield
    DeactivateShield,

    // === Utility (future) ===
    /// Activate tractor beam
    ActivateTractor,
    /// Deactivate tractor beam
    DeactivateTractor,
    /// Activate scanner
    ActivateScanner,
    /// Deploy cargo
    DeployCargo,

    // === Navigation (future) ===
    /// Engage autopilot to target
    EngageAutopilot,
    /// Disengage autopilot
    DisengageAutopilot,
    /// Dock with target
    InitiateDocking,
}

/// Component that queues pending actions for an entity
#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct ActionQueue {
    /// Actions to process this tick
    pub pending: Vec<EntityAction>,
}

impl ActionQueue {
    pub fn push(&mut self, action: EntityAction) {
        self.pending.push(action);
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }

    pub fn drain(&mut self) -> std::vec::Drain<'_, EntityAction> {
        self.pending.drain(..)
    }
}

/// Component that declares which actions an entity can handle
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct ActionCapabilities {
    /// Set of actions this entity can process
    pub supported: Vec<EntityAction>,
}

impl ActionCapabilities {
    pub fn can_handle(&self, action: EntityAction) -> bool {
        self.supported.contains(&action)
    }
}

/// System to validate and log unsupported actions
pub fn validate_action_capabilities(
    query: Query<(Entity, &ActionQueue, Option<&ActionCapabilities>)>,
) {
    for (entity, queue, capabilities) in &query {
        if queue.pending.is_empty() {
            continue;
        }

        let Some(caps) = capabilities else {
            warn!(
                entity = ?entity,
                actions = ?queue.pending,
                "entity received actions but has no ActionCapabilities component"
            );
            continue;
        };

        for action in &queue.pending {
            if !caps.can_handle(*action) {
                warn!(
                    entity = ?entity,
                    action = ?action,
                    "entity received unsupported action"
                );
            }
        }
    }
}
