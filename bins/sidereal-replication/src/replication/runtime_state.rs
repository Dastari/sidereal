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
