pub(crate) fn should_defer_controlled_predicted_adoption(
    is_local_controlled: bool,
    has_position: bool,
    has_rotation: bool,
    has_linear_velocity: bool,
) -> bool {
    is_local_controlled && (!has_position || !has_rotation || !has_linear_velocity)
}

pub(crate) fn should_defer_spatial_root_adoption(
    is_spatial_root: bool,
    has_position: bool,
    has_rotation: bool,
    has_world_position: bool,
    has_world_rotation: bool,
) -> bool {
    is_spatial_root
        && ((!has_position && !has_world_position) || (!has_rotation && !has_world_rotation))
}

pub(crate) fn is_canonical_runtime_entity_lane(
    is_replicated: bool,
    is_predicted: bool,
    is_interpolated: bool,
) -> bool {
    is_replicated && !is_predicted && !is_interpolated
}

pub(crate) fn is_lightyear_replication_lane(
    is_replicated: bool,
    is_predicted: bool,
    is_interpolated: bool,
) -> bool {
    is_replicated || is_predicted || is_interpolated
}

pub(crate) fn runtime_entity_id_from_guid(
    entity_registry: &RuntimeEntityHierarchy,
    local_player_entity_id: &str,
    guid: &str,
) -> Option<String> {
    // Bare UUID is the canonical entity ID.
    if entity_registry.by_entity_id.contains_key(guid) {
        return Some(guid.to_string());
    }
    if parse_guid_from_entity_id(local_player_entity_id)
        .is_some_and(|player_guid| player_guid.to_string() == guid)
    {
        return Some(local_player_entity_id.to_string());
    }
    None
}

fn ids_refer_to_same_guid(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    parse_guid_from_entity_id(left)
        .zip(parse_guid_from_entity_id(right))
        .is_some_and(|(l, r)| l == r)
}

pub(crate) fn has_local_player_runtime_presence<'a, I>(
    entity_registry: &RuntimeEntityHierarchy,
    local_player_entity_id: &str,
    guid_candidates: I,
) -> bool
where
    I: IntoIterator<Item = &'a EntityGuid>,
{
    if entity_registry
        .by_entity_id
        .contains_key(local_player_entity_id)
    {
        return true;
    }

    let local_player_guid = parse_guid_from_entity_id(local_player_entity_id)
        .or_else(|| uuid::Uuid::parse_str(local_player_entity_id).ok());
    let Some(local_player_guid) = local_player_guid else {
        return false;
    };

    if entity_registry
        .by_entity_id
        .contains_key(local_player_guid.to_string().as_str())
    {
        return true;
    }

    guid_candidates
        .into_iter()
        .any(|entity_guid| entity_guid.0 == local_player_guid)
}

fn resolve_control_target_entity_id(
    entity_registry: &RuntimeEntityHierarchy,
    local_player_entity_id: &str,
    controlled_entity_id: Option<&str>,
) -> Option<String> {
    match controlled_entity_id {
        Some(id) if entity_registry.by_entity_id.contains_key(id) => Some(id.to_string()),
        Some(id) => runtime_entity_id_from_guid(entity_registry, local_player_entity_id, id)
            .or_else(|| Some(id.to_string())),
        None => runtime_entity_id_from_guid(
            entity_registry,
            local_player_entity_id,
            local_player_entity_id,
        )
        .or_else(|| Some(local_player_entity_id.to_string())),
    }
}
