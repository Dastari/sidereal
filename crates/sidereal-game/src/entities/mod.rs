//! Entity archetypes: bundles and spawn helpers per kind.
//! One place per archetype (ship, debris, missiles, etc.) for default loadout and spawning.

mod fullscreen_layers;
mod hardpoint;
pub mod ship;
pub use fullscreen_layers::*;
pub use hardpoint::*;
pub use ship::corvette::*;
