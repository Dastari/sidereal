use bevy::prelude::*;

/// Replication-side asset delivery moved to gateway HTTP routes (`/assets/<asset_guid>`).
///
/// This module remains as the replication init hook so startup wiring stays stable while
/// asset delivery state is fully owned by gateway/runtime asset registry systems.
pub fn init_resources(_app: &mut App) {}
