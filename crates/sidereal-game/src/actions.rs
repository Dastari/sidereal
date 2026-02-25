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
    /// Legacy alias for `Forward`.
    /// Thrust forward (throttle positive)
    ThrustForward,
    /// Legacy alias for `Backward`.
    /// Thrust reverse (throttle negative)
    ThrustReverse,
    /// Legacy alias for `LongitudinalNeutral`.
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

const MAX_PENDING_ACTIONS: usize = 128;

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

pub const FLIGHT_CONTROL_ACTIONS: [EntityAction; 7] = [
    EntityAction::Forward,
    EntityAction::Backward,
    EntityAction::LongitudinalNeutral,
    EntityAction::Left,
    EntityAction::Right,
    EntityAction::LateralNeutral,
    EntityAction::Brake,
];

pub const LEGACY_FLIGHT_CONTROL_ACTIONS: [EntityAction; 7] = [
    EntityAction::ThrustForward,
    EntityAction::ThrustReverse,
    EntityAction::ThrustNeutral,
    EntityAction::Brake,
    EntityAction::YawLeft,
    EntityAction::YawRight,
    EntityAction::YawNeutral,
];

pub fn is_flight_control_action(action: EntityAction) -> bool {
    FLIGHT_CONTROL_ACTIONS.contains(&action) || LEGACY_FLIGHT_CONTROL_ACTIONS.contains(&action)
}

pub fn default_flight_action_capabilities() -> ActionCapabilities {
    let mut supported = FLIGHT_CONTROL_ACTIONS.to_vec();
    supported.extend(LEGACY_FLIGHT_CONTROL_ACTIONS);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_queue_stays_bounded_under_burst() {
        let mut queue = ActionQueue::default();
        for i in 0..(MAX_PENDING_ACTIONS + 32) {
            let action = if i % 2 == 0 {
                EntityAction::ThrustForward
            } else {
                EntityAction::YawLeft
            };
            queue.push(action);
        }
        assert_eq!(queue.pending.len(), MAX_PENDING_ACTIONS);
        assert_eq!(queue.pending[0], EntityAction::ThrustForward);
    }

    #[test]
    fn default_flight_capabilities_match_allowlist() {
        let caps = default_flight_action_capabilities();
        for action in FLIGHT_CONTROL_ACTIONS {
            assert!(caps.can_handle(action));
        }
        for action in LEGACY_FLIGHT_CONTROL_ACTIONS {
            assert!(caps.can_handle(action));
        }
        assert!(caps.can_handle(EntityAction::Brake));
        assert!(!caps.can_handle(EntityAction::FirePrimary));
    }
}
