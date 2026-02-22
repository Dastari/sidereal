use avian3d::prelude::Position;
use bevy::prelude::*;
use lightyear::prelude::{NetworkVisibility, ReplicationState};
use sidereal_game::{
    EntityGuid, FactionId, FactionVisibility, MountedOn, OwnerId, PublicVisibility, ScannerRangeM,
};

use crate::PlayerControlledEntityMap;
use crate::visibility::{
    ClientControlledEntityPositionMap, ClientVisibilityRegistry, DEFAULT_VIEW_RANGE_M,
};

#[derive(Debug, Clone)]
struct PlayerVisibilityContext {
    player_entity_id: String,
    observer_position: Option<Vec3>,
    scanner_range_m: f32,
    player_faction_id: Option<String>,
}

#[allow(clippy::type_complexity)]
pub fn update_network_visibility(
    visibility_registry: Res<'_, ClientVisibilityRegistry>,
    controlled_entity_map: Res<'_, PlayerControlledEntityMap>,
    controlled_positions: Res<'_, ClientControlledEntityPositionMap>,
    controlled_entities: Query<
        '_,
        '_,
        (
            &'_ Position,
            Option<&'_ ScannerRangeM>,
            Option<&'_ FactionId>,
        ),
    >,
    position_by_entity: Query<'_, '_, &'_ Position>,
    entity_guid_with_position: Query<'_, '_, (&'_ EntityGuid, &'_ Position)>,
    mut replicated_entities: Query<
        '_,
        '_,
        (
            &'_ mut ReplicationState,
            Option<&'_ OwnerId>,
            Option<&'_ PublicVisibility>,
            Option<&'_ FactionVisibility>,
            Option<&'_ FactionId>,
            Option<&'_ Position>,
            Option<&'_ ChildOf>,
            Option<&'_ MountedOn>,
        ),
        With<NetworkVisibility>,
    >,
) {
    let position_by_guid = entity_guid_with_position
        .iter()
        .map(|(guid, position)| (guid.0, position.0))
        .collect::<std::collections::HashMap<_, _>>();

    for (client_entity, player_entity_id) in &visibility_registry.player_entity_id_by_client {
        let visibility_context = build_player_visibility_context(
            player_entity_id,
            &controlled_entity_map,
            &controlled_positions,
            &controlled_entities,
        );
        for (
            mut replication_state,
            owner_id,
            public_visibility,
            faction_visibility,
            faction_id,
            own_position,
            child_of,
            mounted_on,
        ) in &mut replicated_entities
        {
            let entity_position = own_position
                .map(|position| position.0)
                .or_else(|| {
                    child_of
                        .and_then(|parent| position_by_entity.get(parent.parent()).ok())
                        .map(|position| position.0)
                })
                .or_else(|| {
                    mounted_on.and_then(|mounted| {
                        position_by_guid.get(&mounted.parent_entity_id).copied()
                    })
                });
            if is_entity_visible_to_player(
                owner_id,
                public_visibility,
                faction_visibility,
                faction_id,
                entity_position,
                &visibility_context,
            ) {
                replication_state.gain_visibility(*client_entity);
            } else {
                replication_state.lose_visibility(*client_entity);
            }
        }
    }
}

fn build_player_visibility_context(
    player_entity_id: &str,
    controlled_entity_map: &PlayerControlledEntityMap,
    controlled_positions: &ClientControlledEntityPositionMap,
    controlled_entities: &Query<
        '_,
        '_,
        (
            &'_ Position,
            Option<&'_ ScannerRangeM>,
            Option<&'_ FactionId>,
        ),
    >,
) -> PlayerVisibilityContext {
    if let Some(controlled_entity) = controlled_entity_map
        .by_player_entity_id
        .get(player_entity_id)
        .copied()
        && let Ok((position, scanner_range, faction_id)) =
            controlled_entities.get(controlled_entity)
    {
        return PlayerVisibilityContext {
            player_entity_id: player_entity_id.to_string(),
            observer_position: Some(position.0),
            scanner_range_m: scanner_range
                .map(|range| range.0.max(DEFAULT_VIEW_RANGE_M))
                .unwrap_or(DEFAULT_VIEW_RANGE_M),
            player_faction_id: faction_id.map(|f| f.0.clone()),
        };
    }

    PlayerVisibilityContext {
        player_entity_id: player_entity_id.to_string(),
        observer_position: controlled_positions.get_position(player_entity_id),
        scanner_range_m: DEFAULT_VIEW_RANGE_M,
        player_faction_id: None,
    }
}

fn is_entity_visible_to_player(
    owner_id: Option<&OwnerId>,
    public_visibility: Option<&PublicVisibility>,
    faction_visibility: Option<&FactionVisibility>,
    faction_id: Option<&FactionId>,
    entity_position: Option<Vec3>,
    visibility_context: &PlayerVisibilityContext,
) -> bool {
    if owner_id.is_some_and(|owner| owner.0 == visibility_context.player_entity_id) {
        return true;
    }
    if public_visibility.is_some() {
        return true;
    }
    if faction_visibility.is_some()
        && let (Some(entity_faction_id), Some(player_faction_id)) = (
            faction_id.map(|f| f.0.as_str()),
            visibility_context.player_faction_id.as_deref(),
        )
        && entity_faction_id == player_faction_id
    {
        return true;
    }
    if let (Some(observer_position), Some(target_position)) =
        (visibility_context.observer_position, entity_position)
    {
        return (target_position - observer_position).length()
            <= visibility_context.scanner_range_m;
    }
    false
}
