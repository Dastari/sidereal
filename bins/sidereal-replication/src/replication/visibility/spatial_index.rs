fn summary_logging_enabled() -> bool {
    debug_env("SIDEREAL_REPLICATION_SUMMARY_LOGS")
}

fn debug_visibility_entity_guid() -> Option<uuid::Uuid> {
    static GUID: OnceLock<Option<uuid::Uuid>> = OnceLock::new();
    *GUID.get_or_init(|| {
        std::env::var("SIDEREAL_DEBUG_VIS_ENTITY_GUID")
            .ok()
            .and_then(|raw| uuid::Uuid::parse_str(raw.trim()).ok())
    })
}

fn cell_key(position: Vec3, cell_size_m: f32) -> (i64, i64) {
    (
        (position.x / cell_size_m).floor() as i64,
        (position.y / cell_size_m).floor() as i64,
    )
}

fn add_entities_in_radius(
    center: Vec3,
    radius_m: f32,
    cell_size_m: f32,
    entities_by_cell: &HashMap<(i64, i64), Vec<Entity>>,
    out: &mut HashSet<Entity>,
) {
    let radius = radius_m.max(0.0);
    let cell_radius = (radius / cell_size_m).ceil() as i64;
    let (cx, cy) = cell_key(center, cell_size_m);
    for dx in -cell_radius..=cell_radius {
        for dy in -cell_radius..=cell_radius {
            if let Some(entities) = entities_by_cell.get(&(cx + dx, cy + dy)) {
                out.extend(entities.iter().copied());
            }
        }
    }
}

fn add_cell_keys_in_radius(
    center: Vec3,
    radius_m: f32,
    cell_size_m: f32,
    out: &mut HashSet<(i64, i64)>,
) {
    let radius = radius_m.max(0.0);
    let cell_radius = (radius / cell_size_m).ceil() as i64;
    let (cx, cy) = cell_key(center, cell_size_m);
    for dx in -cell_radius..=cell_radius {
        for dy in -cell_radius..=cell_radius {
            out.insert((cx + dx, cy + dy));
        }
    }
}

fn build_candidate_cells_for_client(
    candidate_mode: VisibilityCandidateMode,
    observer_anchor_position: Option<Vec3>,
    observer_delivery_range_m: f32,
    visibility_sources: &[(Vec3, f32)],
    view_mode: ClientLocalViewMode,
    cell_size_m: f32,
) -> HashSet<(i64, i64)> {
    match candidate_mode {
        VisibilityCandidateMode::FullScan => HashSet::new(),
        VisibilityCandidateMode::SpatialGrid => {
            let mut cells = HashSet::new();
            if let Some(observer_anchor) = observer_anchor_position {
                add_cell_keys_in_radius(
                    observer_anchor,
                    observer_delivery_range_m,
                    cell_size_m,
                    &mut cells,
                );
            }
            if matches!(view_mode, ClientLocalViewMode::Map) {
                for (visibility_pos, visibility_range) in visibility_sources {
                    add_cell_keys_in_radius(
                        *visibility_pos,
                        *visibility_range,
                        cell_size_m,
                        &mut cells,
                    );
                }
            }
            cells
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_candidate_set_for_client(
    candidate_mode: VisibilityCandidateMode,
    player_entity_id: &str,
    observer_anchor_position: Option<Vec3>,
    observer_delivery_range_m: f32,
    visibility_sources: &[(Vec3, f32)],
    view_mode: ClientLocalViewMode,
    cell_size_m: f32,
    all_replicated_entities: &[Entity],
    owned_entities_by_player: &HashMap<String, Vec<Entity>>,
    entities_by_cell: &HashMap<(i64, i64), Vec<Entity>>,
) -> HashSet<Entity> {
    match candidate_mode {
        VisibilityCandidateMode::FullScan => {
            let mut all = HashSet::with_capacity(all_replicated_entities.len());
            all.extend(all_replicated_entities.iter().copied());
            all
        }
        VisibilityCandidateMode::SpatialGrid => {
            let mut candidates = HashSet::new();
            if matches!(view_mode, ClientLocalViewMode::Map)
                && let Some(owned_entities) = owned_entities_by_player.get(player_entity_id)
            {
                candidates.extend(owned_entities.iter().copied());
            }
            if let Some(observer_anchor) = observer_anchor_position {
                add_entities_in_radius(
                    observer_anchor,
                    observer_delivery_range_m,
                    cell_size_m,
                    entities_by_cell,
                    &mut candidates,
                );
            }
            if matches!(view_mode, ClientLocalViewMode::Map) {
                for (visibility_pos, visibility_range) in visibility_sources {
                    add_entities_in_radius(
                        *visibility_pos,
                        *visibility_range,
                        cell_size_m,
                        entities_by_cell,
                        &mut candidates,
                    );
                }
            }
            candidates
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn should_bypass_candidate_filter(
    player_entity_id: &str,
    owner_player_id: Option<&str>,
    is_public_visibility: bool,
    is_faction_visibility: bool,
    is_discovered_static_landmark: bool,
    entity_faction_id: Option<&str>,
    entity_position: Option<Vec3>,
    entity_extent_m: f32,
    visibility_context: &PlayerVisibilityContextRef<'_>,
) -> bool {
    if owner_player_id.is_some_and(|owner| owner == player_entity_id) {
        return true;
    }
    if is_public_visibility {
        return true;
    }
    if is_discovered_static_landmark {
        return true;
    }
    if is_faction_visibility
        && visibility_context
            .player_faction_id
            .zip(entity_faction_id)
            .is_some_and(|(player_faction, entity_faction)| player_faction == entity_faction)
    {
        return true;
    }
    let Some(target_position) = entity_position else {
        return false;
    };
    visibility_context
        .visibility_sources
        .iter()
        .any(|(visibility_pos, visibility_range_m)| {
            (target_position - *visibility_pos).length() <= *visibility_range_m + entity_extent_m
        })
}

fn entity_visibility_extent_m(size: Option<&SizeM>) -> f32 {
    let Some(size) = size else {
        return 0.0;
    };
    let max_dimension = size.length.max(size.width).max(size.height);
    if max_dimension.is_finite() && max_dimension > 0.0 {
        max_dimension * 0.5
    } else {
        0.0
    }
}

fn runtime_layer_parallax_factor(definition: Option<&RuntimeRenderLayerDefinition>) -> f32 {
    definition
        .and_then(|value| value.parallax_factor)
        .unwrap_or(1.0)
        .clamp(0.01, 4.0)
}

fn runtime_layer_screen_scale_factor(definition: Option<&RuntimeRenderLayerDefinition>) -> f32 {
    definition
        .and_then(|value| value.screen_scale_factor)
        .unwrap_or(1.0)
        .clamp(0.01, 64.0)
}

fn max_visual_scale_multiplier(visual_stack: Option<&RuntimeWorldVisualStack>) -> f32 {
    visual_stack
        .map(|stack| {
            stack.passes.iter().fold(1.0_f32, |max_scale, pass| {
                if !pass.enabled {
                    return max_scale;
                }
                max_scale.max(pass.scale_multiplier.unwrap_or(1.0))
            })
        })
        .unwrap_or(1.0)
}

fn effective_discovered_landmark_extent_m(
    entity_extent_m: f32,
    resolved_render_layer: Option<&RuntimeRenderLayerDefinition>,
    visual_stack: Option<&RuntimeWorldVisualStack>,
) -> f32 {
    entity_extent_m
        * runtime_layer_screen_scale_factor(resolved_render_layer)
        * max_visual_scale_multiplier(visual_stack)
}

fn static_landmark_discovery_padding_m(
    entity_extent_m: f32,
    static_landmark: &StaticLandmark,
    signal_signature: Option<&SignalSignature>,
) -> f32 {
    let landmark_radius_m = static_landmark.discovery_radius_m.unwrap_or(0.0).max(0.0);
    let signal_radius_m = signal_signature
        .map(|signal| signal.detection_radius_m.max(0.0))
        .unwrap_or(0.0);
    let uses_extent = static_landmark.use_extent_for_discovery
        || signal_signature.is_some_and(|signal| signal.use_extent_for_detection);
    landmark_radius_m
        + signal_radius_m
        + if uses_extent {
            entity_extent_m.max(0.0)
        } else {
            0.0
        }
}

fn direct_static_landmark_discovery_padding_m(
    entity_extent_m: f32,
    static_landmark: &StaticLandmark,
) -> f32 {
    static_landmark.discovery_radius_m.unwrap_or(0.0).max(0.0)
        + if static_landmark.use_extent_for_discovery {
            entity_extent_m.max(0.0)
        } else {
            0.0
        }
}

fn landmark_discovery_cause(
    entity_position: Option<Vec3>,
    entity_extent_m: f32,
    static_landmark: &StaticLandmark,
    signal_signature: Option<&SignalSignature>,
    visibility_context: &PlayerVisibilityContextRef<'_>,
) -> Option<LandmarkDiscoveryCause> {
    if static_landmark.always_known {
        return Some(LandmarkDiscoveryCause::Direct);
    }
    if !static_landmark.discoverable {
        return None;
    }
    let target_position = entity_position?;
    let direct_extra_radius =
        direct_static_landmark_discovery_padding_m(entity_extent_m, static_landmark);
    let signal_extra_radius =
        static_landmark_discovery_padding_m(entity_extent_m, static_landmark, signal_signature);
    let mut signal_detected = false;
    for (visibility_pos, visibility_range_m) in visibility_context.visibility_sources {
        let distance_m = (target_position - *visibility_pos).length();
        if distance_m <= *visibility_range_m + direct_extra_radius {
            return Some(LandmarkDiscoveryCause::Direct);
        }
        signal_detected |= distance_m <= *visibility_range_m + signal_extra_radius;
    }
    signal_detected.then_some(LandmarkDiscoveryCause::Signal)
}

fn apply_visibility_membership_diff(
    replication_state: &mut ReplicationState,
    current_visible_clients: &mut HashSet<Entity>,
    desired_visible_clients: &HashSet<Entity>,
    visible_gains: &mut usize,
    visible_losses: &mut usize,
) -> usize {
    let gained_clients = desired_visible_clients
        .iter()
        .filter(|client_entity| !current_visible_clients.contains(client_entity))
        .copied()
        .collect::<Vec<_>>();
    let lost_clients = current_visible_clients
        .iter()
        .filter(|client_entity| !desired_visible_clients.contains(client_entity))
        .copied()
        .collect::<Vec<_>>();

    for client_entity in &gained_clients {
        replication_state.gain_visibility(*client_entity);
    }
    for client_entity in &lost_clients {
        replication_state.lose_visibility(*client_entity);
    }

    *visible_gains = visible_gains.saturating_add(gained_clients.len());
    *visible_losses = visible_losses.saturating_add(lost_clients.len());

    current_visible_clients.clear();
    current_visible_clients.extend(desired_visible_clients.iter().copied());

    gained_clients.len()
}

fn queue_visibility_gain_spatial_resend(commands: &mut Commands<'_, '_>, entity: Entity) {
    commands.queue(move |world: &mut World| {
        let Ok(mut entity_mut) = world.get_entity_mut(entity) else {
            return;
        };
        if let Some(mut position) = entity_mut.get_mut::<Position>() {
            position.set_changed();
        }
        if let Some(mut rotation) = entity_mut.get_mut::<Rotation>() {
            rotation.set_changed();
        }
        if let Some(mut linear_velocity) = entity_mut.get_mut::<LinearVelocity>() {
            linear_velocity.set_changed();
        }
        if let Some(mut angular_velocity) = entity_mut.get_mut::<AngularVelocity>() {
            angular_velocity.set_changed();
        }
    });
}

fn remove_entity_from_cell_index(index: &mut VisibilitySpatialIndex, entity: Entity) {
    let Some(cell_key) = index.cell_by_entity.remove(&entity) else {
        return;
    };
    if let Some(entities) = index.entities_by_cell.get_mut(&cell_key) {
        entities.retain(|candidate| *candidate != entity);
        if entities.is_empty() {
            index.entities_by_cell.remove(&cell_key);
        }
    }
}

fn expected_parent_entity_for_spatial_index(
    index: &VisibilitySpatialIndex,
    cache: &VisibilityEntityCache,
    entity: Entity,
) -> Option<Entity> {
    let parent_guid = cache
        .by_entity
        .get(&entity)
        .and_then(|cached| cached.parent_guid)?;
    let parent_entity = index.entity_by_guid.get(&parent_guid).copied()?;
    (parent_entity != entity).then_some(parent_entity)
}

fn spatial_index_requires_full_rebuild(
    index: &VisibilitySpatialIndex,
    cache: &VisibilityEntityCache,
    entity: Entity,
) -> bool {
    let Some(cached) = cache.by_entity.get(&entity) else {
        return true;
    };
    if !index.world_position_by_entity.contains_key(&entity)
        || !index.base_extent_m_by_entity.contains_key(&entity)
        || !index.visibility_position_by_entity.contains_key(&entity)
        || !index.visibility_extent_m_by_entity.contains_key(&entity)
        || !index.root_entity_by_entity.contains_key(&entity)
        || !index.cell_by_entity.contains_key(&entity)
    {
        return true;
    }
    if let Some(guid) = cached.guid
        && index.entity_by_guid.get(&guid).copied() != Some(entity)
    {
        return true;
    }
    index.parent_entity_by_entity.get(&entity).copied()
        != expected_parent_entity_for_spatial_index(index, cache, entity)
}

#[allow(clippy::type_complexity)]
fn rebuild_visibility_spatial_index(
    index: &mut VisibilitySpatialIndex,
    cache: &VisibilityEntityCache,
    all_replicated: &Query<
        '_,
        '_,
        (
            Entity,
            Option<&Position>,
            Option<&WorldPosition>,
            &GlobalTransform,
        ),
        With<Replicate>,
    >,
    cell_size_m: f32,
) {
    index.clear();
    index.cell_size_m = cell_size_m;

    let mut all_entities = Vec::<Entity>::new();
    for (entity, position, world_position, global_transform) in all_replicated {
        all_entities.push(entity);
        let Some(cached) = cache.by_entity.get(&entity) else {
            continue;
        };
        if let Some(guid) = cached.guid {
            index.entity_by_guid.insert(guid, entity);
        }
        let effective_world_pos =
            replicated_visibility_world_position(position, world_position, global_transform);
        index
            .world_position_by_entity
            .insert(entity, effective_world_pos);
        index
            .base_extent_m_by_entity
            .insert(entity, cached.entity_extent_m);
    }

    for &entity in &all_entities {
        let parent_guid = cache
            .by_entity
            .get(&entity)
            .and_then(|cached| cached.parent_guid);
        let Some(parent_guid) = parent_guid else {
            continue;
        };
        let Some(&parent_entity) = index.entity_by_guid.get(&parent_guid) else {
            continue;
        };
        if parent_entity != entity {
            index.parent_entity_by_entity.insert(entity, parent_entity);
        }
    }

    for &entity in &all_entities {
        let root = resolve_mount_root(entity, &index.parent_entity_by_entity);
        index.root_entity_by_entity.insert(entity, root);
        index
            .entities_by_root
            .entry(root)
            .or_default()
            .insert(entity);
    }

    for &entity in &all_entities {
        let root = index
            .root_entity_by_entity
            .get(&entity)
            .copied()
            .unwrap_or(entity);
        let effective_position = index
            .world_position_by_entity
            .get(&root)
            .copied()
            .or_else(|| index.world_position_by_entity.get(&entity).copied())
            .unwrap_or(Vec3::ZERO);
        let effective_extent_m = index
            .base_extent_m_by_entity
            .get(&root)
            .copied()
            .or_else(|| index.base_extent_m_by_entity.get(&entity).copied())
            .unwrap_or(0.0);
        let cell = cell_key(effective_position, cell_size_m);
        index
            .visibility_position_by_entity
            .insert(entity, effective_position);
        index
            .visibility_extent_m_by_entity
            .insert(entity, effective_extent_m);
        index.cell_by_entity.insert(entity, cell);
        index.entities_by_cell.entry(cell).or_default().push(entity);
    }
}

pub fn refresh_visibility_spatial_index(
    mut index: ResMut<'_, VisibilitySpatialIndex>,
    runtime_cfg: Res<'_, VisibilityRuntimeConfig>,
    mut params: VisibilitySpatialIndexRefreshParams<'_, '_>,
) {
    let mut full_rebuild_required =
        index.cell_size_m != runtime_cfg.cell_size_m || index.world_position_by_entity.is_empty();

    for _ in params.removed_replicates.read() {
        full_rebuild_required = true;
    }
    for _ in params.removed_mounted_on.read() {
        full_rebuild_required = true;
    }
    for _ in params.removed_parent_guid.read() {
        full_rebuild_required = true;
    }
    for _ in params.removed_size.read() {
        full_rebuild_required = true;
    }
    for _ in params.removed_entity_guid.read() {
        full_rebuild_required = true;
    }
    for (entity, _, _, _) in &params.changed_replicated {
        if spatial_index_requires_full_rebuild(&index, &params.cache, entity) {
            full_rebuild_required = true;
            break;
        }
    }

    if full_rebuild_required {
        rebuild_visibility_spatial_index(
            &mut index,
            &params.cache,
            &params.all_replicated,
            runtime_cfg.cell_size_m,
        );
        return;
    }

    let mut affected_roots = HashSet::<Entity>::new();
    for (entity, position, world_position, global_transform) in &params.changed_replicated {
        let effective_world_pos =
            replicated_visibility_world_position(position, world_position, global_transform);
        index
            .world_position_by_entity
            .insert(entity, effective_world_pos);
        if let Some(cached) = params.cache.by_entity.get(&entity) {
            index
                .base_extent_m_by_entity
                .insert(entity, cached.entity_extent_m);
        }
        affected_roots.insert(
            index
                .root_entity_by_entity
                .get(&entity)
                .copied()
                .unwrap_or(entity),
        );
    }

    for affected_root in affected_roots {
        let Some(entities_under_root) = index.entities_by_root.get(&affected_root).cloned() else {
            continue;
        };
        let root_position = index
            .world_position_by_entity
            .get(&affected_root)
            .copied()
            .unwrap_or(Vec3::ZERO);
        let root_extent_m = index
            .base_extent_m_by_entity
            .get(&affected_root)
            .copied()
            .unwrap_or(0.0);
        for entity in entities_under_root {
            let effective_position = root_position;
            let effective_extent_m = root_extent_m;
            let next_cell = cell_key(effective_position, runtime_cfg.cell_size_m);
            let previous_cell = index.cell_by_entity.get(&entity).copied();
            if previous_cell != Some(next_cell) {
                remove_entity_from_cell_index(&mut index, entity);
                index.cell_by_entity.insert(entity, next_cell);
                index
                    .entities_by_cell
                    .entry(next_cell)
                    .or_default()
                    .push(entity);
            }
            index
                .visibility_position_by_entity
                .insert(entity, effective_position);
            index
                .visibility_extent_m_by_entity
                .insert(entity, effective_extent_m);
        }
    }
}

