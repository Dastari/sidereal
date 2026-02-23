//! Entity archetypes: bundles and spawn helpers per kind.
//! One place per archetype (ship, debris, missiles, etc.) for default loadout and spawning.

pub mod ship;
pub use ship::corvette::*;
