//! Entity Action System
//!
//! Architecture:
//! - Client input (keys/mouse) → bindings → EntityAction enums
//! - EntityActions are sent to controlled entity via network or local queue
//! - Entities dispatch actions to components that register as handlers
//! - Handlers implement capability-specific logic (e.g., FlightComputer → Engine → fuel check → apply force)
//!
//! Design principles:
//! - Actions are high-level intent (Forward, FireWeapon, ActivateShield)
//! - No direct force/velocity manipulation from input layer
//! - Components declare which actions they handle
//! - Fuel, power, cooldown, and other constraints are checked at handler level
//! - Same action pipeline works for player input, AI commands, and scripted sequences

use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::{ActionCapabilities, ActionQueue};

/// High-level action that can be sent to any entity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
#[reflect(Serialize, Deserialize)]
pub enum EntityAction {
    // === Generic movement verbs (canonical) ===
    /// Move/drive forward intent.
    Forward,
    /// Move/drive backward intent.
    Backward,
    /// Stop forward/backward intent.
    LongitudinalNeutral,
    /// Turn/strafe left intent.
    Left,
    /// Turn/strafe right intent.
    Right,
    /// Stop left/right intent.
    LateralNeutral,

    // === Flight control ===
    /// Active flight-computer braking to drive linear velocity toward zero
    Brake,
    /// Enable afterburner while held/active.
    AfterburnerOn,
    /// Disable afterburner when released/inactive.
    AfterburnerOff,

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

pub const MAX_PENDING_ACTIONS: usize = 128;

impl ActionQueue {
    pub fn push(&mut self, action: EntityAction) {
        if self.pending.len() >= MAX_PENDING_ACTIONS {
            // Keep queue bounded under adverse network/input conditions.
            self.pending.remove(0);
        }
        self.pending.push(action);
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }

    pub fn drain(&mut self) -> std::vec::Drain<'_, EntityAction> {
        self.pending.drain(..)
    }
}

pub const FLIGHT_CONTROL_ACTIONS: [EntityAction; 9] = [
    EntityAction::Forward,
    EntityAction::Backward,
    EntityAction::LongitudinalNeutral,
    EntityAction::Left,
    EntityAction::Right,
    EntityAction::LateralNeutral,
    EntityAction::Brake,
    EntityAction::AfterburnerOn,
    EntityAction::AfterburnerOff,
];
pub const WEAPON_ACTIONS: [EntityAction; 2] =
    [EntityAction::FirePrimary, EntityAction::FireSecondary];

pub fn is_flight_control_action(action: EntityAction) -> bool {
    FLIGHT_CONTROL_ACTIONS.contains(&action)
}

pub fn default_flight_action_capabilities() -> ActionCapabilities {
    let mut supported = FLIGHT_CONTROL_ACTIONS.to_vec();
    supported.extend(WEAPON_ACTIONS);
    ActionCapabilities { supported }
}

pub fn default_character_movement_action_capabilities() -> ActionCapabilities {
    ActionCapabilities {
        supported: FLIGHT_CONTROL_ACTIONS.to_vec(),
    }
}

impl ActionCapabilities {
    pub fn can_handle(&self, action: EntityAction) -> bool {
        self.supported.contains(&action)
    }
}

/// Legacy compatibility hook kept as a no-op: unsupported actions are ignored.
pub fn validate_action_capabilities(
    _query: Query<(Entity, &ActionQueue, Option<&ActionCapabilities>)>,
) {
}
