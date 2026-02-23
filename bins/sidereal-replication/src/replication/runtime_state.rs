use avian3d::prelude::Position;
use bevy::prelude::*;
use lightyear::prelude::is_in_rollback;
use sidereal_game::{
    EntityGuid, MountedOn, ScannerComponent, ScannerRangeBuff, ScannerRangeM,
    total_scanner_range_for_parent,
};
use sidereal_persistence::PlayerRuntimeViewState;

use crate::replication::SimulatedControlledEntity;
use crate::visibility::{self, ClientControlledEntityPositionMap};
use crate::{PlayerRuntimeViewDirtySet, PlayerRuntimeViewRegistry, unix_epoch_now_i64};

pub fn update_client_controlled_entity_positions(
    entities: Query<'_, '_, (&'_ SimulatedControlledEntity, &'_ Position)>,
    mut position_map: ResMut<'_, ClientControlledEntityPositionMap>,
    mut view_registry: ResMut<'_, PlayerRuntimeViewRegistry>,
    mut dirty_view_states: ResMut<'_, PlayerRuntimeViewDirtySet>,
) {
    for (entity, position) in &entities {
        position_map.update_position(&entity.player_entity_id, position.0);
        let entry = view_registry
            .by_player_entity_id
            .entry(entity.player_entity_id.clone())
            .or_insert_with(|| PlayerRuntimeViewState {
                player_entity_id: entity.player_entity_id.clone(),
                ..Default::default()
            });
        entry.last_controlled_entity_id = Some(entity.entity_id.clone());
        entry.last_camera_position_m = Some([position.0.x, position.0.y, position.0.z]);
        entry.updated_at_epoch_s = unix_epoch_now_i64();
        dirty_view_states
            .player_entity_ids
            .insert(entity.player_entity_id.clone());
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
