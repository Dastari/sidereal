use bevy::prelude::*;
use lightyear::prelude::is_in_rollback;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{
    ControlledBy, InterpolationTarget, PredictionTarget, RemoteId, Replicate, ReplicationState,
};
use sidereal_game::{
    ControlledEntityGuid, EntityGuid, MountedOn, PlayerTag, VisibilityRangeBuffM, VisibilityRangeM,
    total_visibility_range_for_parent,
};

use crate::replication::visibility::ClientObserverAnchorPositionMap;
use crate::replication::{PlayerRuntimeEntityMap, SimulatedControlledEntity, debug_env};

type ControlledTargetDebugQueryItem<'a> = (
    Entity,
    &'a EntityGuid,
    Has<Replicate>,
    Has<PredictionTarget>,
    Has<InterpolationTarget>,
    Option<&'a ControlledBy>,
    Option<&'a ReplicationState>,
);

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
    player_entities: Res<'_, PlayerRuntimeEntityMap>,
    controlled_entity_map: Res<'_, crate::replication::PlayerControlledEntityMap>,
    controlled_targets: Query<
        '_,
        '_,
        ControlledTargetDebugQueryItem<'_>,
        With<SimulatedControlledEntity>,
    >,
    client_remote_ids: Query<'_, '_, &'_ RemoteId, With<ClientOf>>,
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
            if let Some(player_entity) = player_entities
                .by_player_entity_id
                .get(player_entity_id.as_str())
            {
                // Sidereal supports dynamic control handoff between the persisted player anchor
                // and owned ships. Future audits need this detail because the player anchor and
                // controlled ship intentionally use different replication modes.
                let mapped_entity = sidereal_net::PlayerEntityId::parse(player_entity_id.as_str())
                    .and_then(|parsed_player_id| {
                        controlled_entity_map
                            .by_player_entity_id
                            .get(&parsed_player_id)
                            .copied()
                    });
                let mut detail_logged = false;
                let effective_target = mapped_entity.unwrap_or(*player_entity);
                if let Ok((
                    entity,
                    guid,
                    has_replicate,
                    has_prediction_target,
                    has_interpolation_target,
                    controlled_by,
                    replication_state,
                )) = controlled_targets.get(effective_target)
                {
                    let controlling_remote = controlled_by
                        .and_then(|binding| client_remote_ids.get(binding.owner).ok())
                        .map(|remote| format!("{:?}", remote.0))
                        .unwrap_or_else(|| "<none>".to_string());
                    let visibility_for_owner = controlled_by
                        .map(|binding| {
                            replication_state
                                .map(|state| state.is_visible(binding.owner))
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    let authority_for_owner = controlled_by
                        .map(|binding| {
                            replication_state
                                .map(|state| state.has_authority(binding.owner))
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    info!(
                        "replication authoritative control target detail player={} target_entity={:?} target_guid={} replicate={} prediction_target={} interpolation_target={} controlled_by_client={:?} controlled_by_remote={} visible_for_owner={} authority_for_owner={}",
                        player_entity_id,
                        entity,
                        guid.0,
                        has_replicate,
                        has_prediction_target,
                        has_interpolation_target,
                        controlled_by.map(|binding| binding.owner),
                        controlling_remote,
                        visibility_for_owner,
                        authority_for_owner,
                    );
                    detail_logged = true;
                }
                if !detail_logged {
                    info!(
                        "replication authoritative control target detail player={} target_entity={:?} detail_unavailable=true",
                        player_entity_id, effective_target
                    );
                }
            }
        }
        seen.insert(player_entity_id);
    }
    debug_state
        .last_controlled_guid_by_player
        .retain(|player_entity_id, _| seen.contains(player_entity_id));
}

pub fn update_client_observer_anchor_positions(
    player_entities: Res<'_, PlayerRuntimeEntityMap>,
    controlled_entity_map: Res<'_, crate::replication::PlayerControlledEntityMap>,
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
        let canonical_player_entity_id =
            sidereal_net::PlayerEntityId::parse(player_entity_id.as_str())
                .map(sidereal_net::PlayerEntityId::canonical_wire_id)
                .unwrap_or_else(|| player_entity_id.clone());
        let controlled_entity = sidereal_net::PlayerEntityId::parse(player_entity_id.as_str())
            .and_then(|player_id| {
                controlled_entity_map
                    .by_player_entity_id
                    .get(&player_id)
                    .copied()
            });
        let observer_anchor_entities = controlled_entity
            .into_iter()
            .chain(std::iter::once(*player_entity));

        for observer_anchor_entity in observer_anchor_entities {
            let Ok((position, global, transform)) = anchor_positions.get(observer_anchor_entity)
            else {
                continue;
            };
            // Contract: observer anchor follows the currently controlled entity when one exists.
            // Fall back to the persisted player anchor for free-roam or incomplete bootstrap.
            let world = global
                .map(GlobalTransform::translation)
                .or_else(|| transform.map(|t| t.translation))
                .or_else(|| position.map(|p| p.0.extend(0.0)))
                .unwrap_or(Vec3::ZERO);
            position_map.update_position(player_entity_id, world);
            if canonical_player_entity_id != *player_entity_id {
                position_map.update_position(canonical_player_entity_id.as_str(), world);
            }
            break;
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

#[cfg(test)]
mod tests {
    use super::update_client_observer_anchor_positions;
    use crate::replication::PlayerRuntimeEntityMap;
    use crate::replication::simulation_entities::PlayerControlledEntityMap;
    use crate::replication::visibility::ClientObserverAnchorPositionMap;
    use avian2d::prelude::Position;
    use bevy::prelude::*;
    use sidereal_net::PlayerEntityId;
    use uuid::Uuid;

    #[test]
    fn observer_anchor_prefers_controlled_entity_position() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<PlayerRuntimeEntityMap>();
        app.init_resource::<PlayerControlledEntityMap>();
        app.init_resource::<ClientObserverAnchorPositionMap>();
        app.add_systems(Update, update_client_observer_anchor_positions);

        let player_id =
            PlayerEntityId(Uuid::parse_str("1521601b-7e69-4700-853f-eb1eb3a41199").unwrap());
        let player_entity = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.0, 0.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 0.0)),
            ))
            .id();
        let ship_entity = app
            .world_mut()
            .spawn((
                Position(Vec2::new(250.0, -125.0)),
                Transform::from_xyz(250.0, -125.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(250.0, -125.0, 0.0)),
            ))
            .id();

        app.world_mut()
            .resource_mut::<PlayerRuntimeEntityMap>()
            .by_player_entity_id
            .insert(player_id.canonical_wire_id(), player_entity);
        app.world_mut()
            .resource_mut::<PlayerControlledEntityMap>()
            .by_player_entity_id
            .insert(player_id, ship_entity);

        app.update();

        let stored = app
            .world()
            .resource::<ClientObserverAnchorPositionMap>()
            .get_position(player_id.canonical_wire_id().as_str());
        assert_eq!(stored, Some(Vec3::new(250.0, -125.0, 0.0)));
    }

    #[test]
    fn observer_anchor_falls_back_to_player_anchor_when_not_controlling_ship() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<PlayerRuntimeEntityMap>();
        app.init_resource::<PlayerControlledEntityMap>();
        app.init_resource::<ClientObserverAnchorPositionMap>();
        app.add_systems(Update, update_client_observer_anchor_positions);

        let player_id =
            PlayerEntityId(Uuid::parse_str("8e5fa817-a5a6-48e1-bb54-8d3f59df1ea4").unwrap());
        let player_entity = app
            .world_mut()
            .spawn((
                Position(Vec2::new(42.0, 84.0)),
                Transform::from_xyz(42.0, 84.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(42.0, 84.0, 0.0)),
            ))
            .id();

        app.world_mut()
            .resource_mut::<PlayerRuntimeEntityMap>()
            .by_player_entity_id
            .insert(player_id.canonical_wire_id(), player_entity);

        app.update();

        let stored = app
            .world()
            .resource::<ClientObserverAnchorPositionMap>()
            .get_position(player_id.canonical_wire_id().as_str());
        assert_eq!(stored, Some(Vec3::new(42.0, 84.0, 0.0)));
    }
}
