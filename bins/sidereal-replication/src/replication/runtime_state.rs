use bevy::prelude::*;
use lightyear::prelude::is_in_rollback;
use sidereal_game::{
    EntityGuid, MountedOn, ScannerComponent, ScannerRangeBuff, ScannerRangeM,
    total_scanner_range_for_parent,
};

use crate::replication::{PlayerRuntimeEntityMap, SimulatedControlledEntity};
use crate::visibility::{self, ClientObserverAnchorPositionMap};

pub fn update_client_observer_anchor_positions(
    player_entities: Res<'_, PlayerRuntimeEntityMap>,
    camera_transforms: Query<'_, '_, &'_ Transform>,
    mut position_map: ResMut<'_, ClientObserverAnchorPositionMap>,
) {
    for (player_entity_id, player_entity) in &player_entities.by_player_entity_id {
        if let Ok(camera_transform) = camera_transforms.get(*player_entity) {
            position_map.update_position(player_entity_id, camera_transform.translation);
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn compute_controlled_entity_scanner_ranges(
    mut controlled_entities: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            &'_ mut ScannerRangeM,
            Option<&'_ ScannerComponent>,
            Option<&'_ ScannerRangeBuff>,
        ),
        With<SimulatedControlledEntity>,
    >,
    scanner_modules: Query<
        '_,
        '_,
        (
            &'_ MountedOn,
            &'_ ScannerComponent,
            Option<&'_ ScannerRangeBuff>,
        ),
        Without<SimulatedControlledEntity>,
    >,
    rollback_query: Query<'_, '_, (), With<lightyear::prelude::Rollback>>,
) {
    if is_in_rollback(rollback_query) {
        return;
    }
    for (entity_guid, mut scanner_range, own_scanner, own_buff) in &mut controlled_entities {
        scanner_range.0 = total_scanner_range_for_parent(
            entity_guid.0,
            visibility::DEFAULT_VIEW_RANGE_M,
            own_scanner,
            own_buff,
            scanner_modules.iter(),
        );
    }
}
