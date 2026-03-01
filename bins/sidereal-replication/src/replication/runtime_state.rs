use bevy::prelude::*;
use lightyear::prelude::is_in_rollback;
use sidereal_game::{
    ControlledEntityGuid, EntityGuid, MountedOn, PlayerTag, ScannerComponent, ScannerRangeBuff,
    ScannerRangeM, total_scanner_range_for_parent,
};

use crate::replication::visibility::{ClientObserverAnchorPositionMap, DEFAULT_VIEW_RANGE_M};
use crate::replication::{PlayerRuntimeEntityMap, SimulatedControlledEntity, debug_env};

#[derive(Resource, Default)]
pub struct PlayerControlDebugState {
    pub last_controlled_guid_by_player: std::collections::HashMap<String, Option<String>>,
}

fn control_debug_logging_enabled() -> bool {
    debug_env("SIDEREAL_DEBUG_CONTROL_LOGS")
}

pub fn init_resources(app: &mut App) {
    app.insert_resource(PlayerControlDebugState::default());
}

#[allow(clippy::type_complexity)]
pub fn sync_player_anchor_to_controlled_entity(
    mut players: Query<
        '_,
        '_,
        (
            &EntityGuid,
            &ControlledEntityGuid,
            &mut Transform,
            Option<&mut avian2d::prelude::Position>,
        ),
        With<PlayerTag>,
    >,
    controlled_entities: Query<
        '_,
        '_,
        (&EntityGuid, &Transform, Option<&avian2d::prelude::Position>),
        (With<SimulatedControlledEntity>, Without<PlayerTag>),
    >,
    rollback_query: Query<'_, '_, (), With<lightyear::prelude::Rollback>>,
) {
    if is_in_rollback(rollback_query) {
        return;
    }

    let mut controlled_world_by_guid = std::collections::HashMap::<uuid::Uuid, Vec2>::new();
    for (guid, transform, position) in &controlled_entities {
        let world = position
            .map(|position| position.0)
            .unwrap_or(transform.translation.truncate());
        controlled_world_by_guid.insert(guid.0, world);
    }

    for (player_guid, controlled_guid, mut player_transform, player_position) in &mut players {
        let Some(control_guid_raw) = controlled_guid.0.as_deref() else {
            continue;
        };
        let Ok(control_guid) = uuid::Uuid::parse_str(control_guid_raw) else {
            continue;
        };
        if control_guid == player_guid.0 {
            continue;
        }
        let Some(target_world) = controlled_world_by_guid.get(&control_guid).copied() else {
            continue;
        };
        player_transform.translation.x = target_world.x;
        player_transform.translation.y = target_world.y;
        player_transform.translation.z = 0.0;
        if let Some(mut player_position) = player_position {
            player_position.0 = target_world;
        }
    }
}

pub fn log_player_control_state_changes(
    players: Query<'_, '_, (&Name, Option<&ControlledEntityGuid>), With<PlayerTag>>,
    mut debug_state: ResMut<'_, PlayerControlDebugState>,
) {
    if !control_debug_logging_enabled() {
        return;
    }

    let mut seen = std::collections::HashSet::new();
    for (name, controlled_guid) in &players {
        let player_entity_id = name.as_str().to_string();
        let current = controlled_guid.and_then(|guid| guid.0.clone());
        let previous = debug_state
            .last_controlled_guid_by_player
            .insert(player_entity_id.clone(), current.clone());
        if previous != Some(current.clone()) {
            info!(
                "replication authoritative control changed player={} previous={:?} current={:?}",
                player_entity_id, previous, current
            );
        }
        seen.insert(player_entity_id);
    }
    debug_state
        .last_controlled_guid_by_player
        .retain(|player_entity_id, _| seen.contains(player_entity_id));
}

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
            DEFAULT_VIEW_RANGE_M,
            own_scanner,
            own_buff,
            scanner_modules.iter(),
        );
    }
}
