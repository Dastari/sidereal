use bevy::prelude::*;
use lightyear::prelude::is_in_rollback;
use sidereal_game::{
    ControlledEntityGuid, EntityGuid, MountedOn, PlayerTag, VisibilityRangeBuffM, VisibilityRangeM,
    total_visibility_range_for_parent,
};

use crate::replication::visibility::ClientObserverAnchorPositionMap;
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
    anchor_positions: Query<
        '_,
        '_,
        (
            Option<&'_ avian2d::prelude::Position>,
            Option<&'_ GlobalTransform>,
            Option<&'_ Transform>,
        ),
    >,
    mut position_map: ResMut<'_, ClientObserverAnchorPositionMap>,
) {
    for (player_entity_id, player_entity) in &player_entities.by_player_entity_id {
        if let Ok((position, global, transform)) = anchor_positions.get(*player_entity) {
            // Contract: observer anchor uses world-space transform; prefer GlobalTransform.
            let world = global
                .map(GlobalTransform::translation)
                .or_else(|| transform.map(|t| t.translation))
                .or_else(|| position.map(|p| p.0.extend(0.0)))
                .unwrap_or(Vec3::ZERO);
            let canonical_player_entity_id =
                sidereal_net::PlayerEntityId::parse(player_entity_id.as_str())
                    .map(sidereal_net::PlayerEntityId::canonical_wire_id)
                    .unwrap_or_else(|| player_entity_id.clone());
            position_map.update_position(player_entity_id, world);
            if canonical_player_entity_id != *player_entity_id {
                position_map.update_position(canonical_player_entity_id.as_str(), world);
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn compute_controlled_entity_visibility_ranges(
    mut commands: Commands<'_, '_>,
    mut controlled_entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Option<&'_ mut VisibilityRangeM>,
            Option<&'_ VisibilityRangeBuffM>,
        ),
        With<SimulatedControlledEntity>,
    >,
    visibility_range_buffs: Query<
        '_,
        '_,
        (&'_ MountedOn, &'_ VisibilityRangeBuffM),
        Without<SimulatedControlledEntity>,
    >,
    rollback_query: Query<'_, '_, (), With<lightyear::prelude::Rollback>>,
) {
    if is_in_rollback(rollback_query) {
        return;
    }
    for (entity, entity_guid, visibility_range, own_buff) in &mut controlled_entities {
        let total_range = total_visibility_range_for_parent(
            entity_guid.0,
            own_buff,
            visibility_range_buffs.iter(),
        );
        if total_range > 0.0 {
            if let Some(mut visibility_range) = visibility_range {
                visibility_range.0 = total_range;
            } else {
                commands
                    .entity(entity)
                    .insert(VisibilityRangeM(total_range));
            }
        } else if visibility_range.is_some() {
            commands.entity(entity).remove::<VisibilityRangeM>();
        }
    }
}
