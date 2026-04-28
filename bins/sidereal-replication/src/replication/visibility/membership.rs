#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn update_network_visibility(
    mut commands: Commands<'_, '_>,
    time: Res<'_, Time>,
    runtime_cfg: Res<'_, VisibilityRuntimeConfig>,
    preparation_metrics: Res<'_, VisibilityPreparationMetrics>,
    landmark_metrics: Res<'_, VisibilityLandmarkDiscoveryMetrics>,
    mut telemetry_state: ResMut<'_, VisibilityTelemetryLogState>,
    mut role_rearms: ResMut<'_, crate::replication::control::RoleVisibilityRearmState>,
    params: VisibilityUpdateParams<'_, '_>,
) {
    let clients = params.clients;
    let cache = params.cache;
    let mut client_context_cache = params.client_context_cache;
    let mut membership_cache = params.membership_cache;
    let spatial_index = params.spatial_index;
    let visibility_registry = params.visibility_registry;
    let mut view_mode_registry = params.view_mode_registry;
    let local_view_delivery_metrics = params.local_view_delivery_metrics;
    let player_entities = params.player_entities;
    let mut scratch = params.scratch;
    let observer_anchor_positions = params.observer_anchor_positions;
    let player_visibility_state = params.player_visibility_state;
    let player_landmark_state = params.player_landmark_state;
    let all_replicated = params.all_replicated;
    let mut replicated_entities = params.replicated_entities;
    let started_at = Instant::now();
    let mut client_cache_upserts = 0usize;
    let mut visible_gains = 0usize;
    let mut visible_losses = 0usize;
    scratch.clear();
    scratch.live_clients.extend(clients.iter());
    let live_clients_snapshot = scratch.live_clients.clone();
    scratch.live_client_set.extend(live_clients_snapshot);
    view_mode_registry
        .by_client_entity
        .retain(|client, _| scratch.live_client_set.contains(client));

    // Drop stale registry entries for clients that have disconnected but have not yet
    // been cleaned by auth cleanup pass in this frame.
    let registered_clients = visibility_registry
        .player_entity_id_by_client
        .iter()
        .filter_map(|(client, player_id)| {
            if scratch.live_client_set.contains(client) {
                Some((*client, player_id.clone()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    scratch.registered_clients.extend(registered_clients);

    let mut runtime_layer_definitions_by_id =
        HashMap::<String, RuntimeRenderLayerDefinition>::new();
    runtime_layer_definitions_by_id.insert(
        sidereal_game::DEFAULT_MAIN_WORLD_LAYER_ID.to_string(),
        default_main_world_render_layer(),
    );

    // 1) Build policy/runtime lookup state for all replicated entities while reading
    // stable spatial state from the persistent visibility index.
    for (entity, _position, _world_position, _global_transform) in &all_replicated {
        let Some(cached) = cache.by_entity.get(&entity) else {
            continue;
        };
        if let Some(definition) = cached.runtime_render_layer_definition.as_ref() {
            runtime_layer_definitions_by_id.insert(definition.layer_id.clone(), definition.clone());
        }
        scratch.all_replicated_entities.push(entity);
        if let Some(guid) = cached.guid
            && let Some(static_landmark) = cached.static_landmark.as_ref()
        {
            let discovery_padding_m =
                static_landmark_discovery_padding_m(cached.entity_extent_m, static_landmark, None);
            scratch.max_static_landmark_discovery_padding_m = scratch
                .max_static_landmark_discovery_padding_m
                .max(discovery_padding_m);
            scratch
                .static_landmarks_by_entity
                .insert(entity, (guid, static_landmark.clone(), None));
        }
        scratch
            .root_public_by_entity
            .insert(entity, cached.public_visibility);
        if let Some(faction) = cached.faction_id.as_ref() {
            scratch
                .root_faction_by_entity
                .insert(entity, faction.clone());
        }
        if let Some(owner) = cached.owner_player_id.as_ref() {
            let canonical_owner = owner.clone();
            scratch
                .root_owner_by_entity
                .insert(entity, canonical_owner.clone());
            scratch
                .owned_entities_by_player
                .entry(canonical_owner.clone())
                .or_default()
                .push(entity);
            if let Some(faction) = cached.faction_id.as_ref() {
                scratch
                    .player_faction_by_owner
                    .entry(canonical_owner)
                    .or_insert_with(|| faction.clone());
            }
        }
        if let Some(override_layer) = cached.pending_world_layer_override.as_ref() {
            scratch
                .pending_world_layer_override_by_entity
                .insert(entity, override_layer.clone());
        }
        if let (Some(owner), Some(range)) =
            (cached.owner_player_id.as_ref(), cached.visibility_range_m)
            && range > 0.0
        {
            scratch
                .visibility_source_candidates
                .push((entity, owner.clone(), range));
        }
    }

    let pending_layer_overrides = scratch
        .pending_world_layer_override_by_entity
        .iter()
        .map(|(entity, layer_id)| (*entity, layer_id.clone()))
        .collect::<Vec<_>>();
    for (entity, layer_id) in pending_layer_overrides {
        if scratch.resolved_world_layer_by_entity.contains_key(&entity) {
            continue;
        }
        if let Some(definition) = runtime_layer_definitions_by_id.get(layer_id.as_str()) {
            scratch
                .resolved_world_layer_by_entity
                .insert(entity, definition.clone());
        }
    }

    let all_replicated_entities = scratch.all_replicated_entities.clone();
    let live_entity_set = all_replicated_entities
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    membership_cache
        .by_entity
        .retain(|entity, visible_clients| {
            visible_clients.retain(|client_entity| scratch.live_client_set.contains(client_entity));
            live_entity_set.contains(entity)
        });

    // 2) Build visibility sources from owned entities with a resolved effective visibility range.
    // Runtime-controlled roots normally carry the aggregated VisibilityRangeM, but dynamic
    // hydration/control handoff can briefly leave the hierarchy root cache behind the range-bearing
    // entity. In that case the range-bearing entity remains the source instead of dropping the
    // player's scanner/visibility source for a tick.
    let visibility_source_candidates = scratch.visibility_source_candidates.clone();
    let mut emitted_visibility_sources = HashSet::<(String, Entity)>::new();
    for (entity, canonical_owner, range) in &visibility_source_candidates {
        let root = spatial_index
            .root_entity_by_entity
            .get(entity)
            .copied()
            .unwrap_or(*entity);
        let root_has_matching_source = cache.by_entity.get(&root).is_some_and(|cached_root| {
            cached_root
                .owner_player_id
                .as_ref()
                .is_some_and(|owner| owner == canonical_owner)
                && cached_root
                    .visibility_range_m
                    .is_some_and(|root_range| root_range > 0.0)
        });
        let source_entity = if root_has_matching_source {
            root
        } else {
            *entity
        };
        if !emitted_visibility_sources.insert((canonical_owner.clone(), source_entity)) {
            continue;
        }
        let Some(position) = spatial_index
            .world_position_by_entity
            .get(&source_entity)
            .or_else(|| spatial_index.world_position_by_entity.get(entity))
            .copied()
        else {
            continue;
        };
        scratch
            .visibility_sources_by_owner
            .entry(canonical_owner.clone())
            .or_default()
            .push((position, *range));
    }
    let scratch_build_ms = started_at.elapsed().as_secs_f64() * 1000.0;

    let candidate_started_at = Instant::now();
    let client_context_refresh_started_at = Instant::now();
    let live_client_count_before_retain = client_context_cache.by_client.len();
    client_context_cache
        .by_client
        .retain(|client_entity, _| scratch.live_client_set.contains(client_entity));
    let client_cache_removals_local =
        live_client_count_before_retain.saturating_sub(client_context_cache.by_client.len());
    let registered_clients = scratch.registered_clients.clone();
    for (client_entity, player_entity_id) in &registered_clients {
        let canonical_player_id = canonical_player_entity_id(player_entity_id.as_str());
        let visibility_sources = scratch
            .visibility_sources_by_owner
            .get(canonical_player_id.as_str())
            .cloned()
            .unwrap_or_default();
        let observer_anchor_position = observer_anchor_positions
            .get_position(canonical_player_id.as_str())
            .or_else(|| observer_anchor_positions.get_position(player_entity_id.as_str()));
        let player_faction_id = scratch
            .player_faction_by_owner
            .get(canonical_player_id.as_str())
            .cloned();
        let local_view_settings = view_mode_registry
            .by_client_entity
            .get(client_entity)
            .copied()
            .unwrap_or(ClientLocalViewSettings {
                view_mode: ClientLocalViewMode::Tactical,
                delivery_range_m: runtime_cfg.delivery_range_m,
            });
        let local_view_mode = local_view_settings.view_mode;
        let client_delivery_range_m = local_view_settings.delivery_range_m;
        let player_entity = player_entities
            .by_player_entity_id
            .get(canonical_player_id.as_str())
            .copied()
            .or_else(|| {
                player_entities
                    .by_player_entity_id
                    .get(player_entity_id.as_str())
                    .copied()
            });
        let next_context = CachedClientVisibilityContext {
            player_entity_id: canonical_player_id.clone(),
            player_entity,
            observer_anchor_position,
            visibility_sources,
            discovered_static_landmarks: player_entity
                .and_then(|player_entity| {
                    player_landmark_state
                        .get(player_entity)
                        .ok()
                        .and_then(|component| {
                            component.map(|component| {
                                component.landmark_entity_ids.iter().copied().collect()
                            })
                        })
                })
                .unwrap_or_default(),
            player_faction_id,
            view_mode: local_view_mode,
            delivery_range_m: client_delivery_range_m,
        };
        let should_upsert =
            client_context_cache.by_client.get(client_entity) != Some(&next_context);
        if should_upsert {
            client_context_cache
                .by_client
                .insert(*client_entity, next_context);
            client_cache_upserts = client_cache_upserts.saturating_add(1);
        }
    }
    let client_context_refresh_ms =
        client_context_refresh_started_at.elapsed().as_secs_f64() * 1000.0;
    let client_cache_entries = client_context_cache.by_client.len();
    let client_cache_removals = client_cache_removals_local;

    for (client_entity, _) in &registered_clients {
        let Some(client_context) = client_context_cache.by_client.get(client_entity) else {
            continue;
        };
        let candidates = build_candidate_set_for_client(
            runtime_cfg.candidate_mode,
            client_context.player_entity_id.as_str(),
            client_context.observer_anchor_position,
            client_context.delivery_range_m,
            &client_context.visibility_sources,
            client_context.view_mode,
            runtime_cfg.cell_size_m,
            &scratch.all_replicated_entities,
            &scratch.owned_entities_by_player,
            &spatial_index.entities_by_cell,
        );
        let candidate_cells = build_candidate_cells_for_client(
            runtime_cfg.candidate_mode,
            client_context.observer_anchor_position,
            client_context.delivery_range_m,
            &client_context.visibility_sources,
            client_context.view_mode,
            runtime_cfg.cell_size_m,
        );
        scratch.client_states.push(ClientVisibilityComputedState {
            client_entity: *client_entity,
            candidate_entities: candidates,
            candidate_cells,
        });
    }
    let discovery_and_candidate_ms = candidate_started_at.elapsed().as_secs_f64() * 1000.0;

    let disclosure_started_at = Instant::now();
    for client_state in &scratch.client_states {
        let Some(client_context) = client_context_cache
            .by_client
            .get(&client_state.client_entity)
        else {
            continue;
        };
        let Some(player_entity) = client_context.player_entity else {
            continue;
        };
        let visibility_sources = client_context
            .visibility_sources
            .iter()
            .map(|(position, range_m)| VisibilityRangeSource {
                x: position.x,
                y: position.y,
                z: position.z,
                range_m: *range_m,
            })
            .collect::<Vec<_>>();
        let mut queried_cells = client_state
            .candidate_cells
            .iter()
            .copied()
            .map(|(x, y)| VisibilityGridCell { x, y })
            .collect::<Vec<_>>();
        queried_cells.sort_by_key(|cell| (cell.x, cell.y));

        let next_grid = VisibilitySpatialGrid {
            candidate_mode: runtime_cfg.candidate_mode.as_str().to_string(),
            cell_size_m: runtime_cfg.cell_size_m,
            delivery_range_m: client_context.delivery_range_m,
            queried_cells,
        };
        let next_disclosure = VisibilityDisclosure { visibility_sources };

        let Ok((existing_grid, existing_disclosure)) = player_visibility_state.get(player_entity)
        else {
            continue;
        };
        let mut entity_commands = commands.entity(player_entity);
        if existing_grid.is_none_or(|current| current != &next_grid) {
            entity_commands.insert(next_grid);
        }
        if existing_disclosure.is_none_or(|current| current != &next_disclosure) {
            entity_commands.insert(next_disclosure);
        }
    }
    let disclosure_sync_ms = disclosure_started_at.elapsed().as_secs_f64() * 1000.0;

    // Cache client buckets once so owner-only and owner-map fast paths do not keep
    // rediscovering the same client subsets while iterating every replicated entity.
    let mut client_entities_by_player_id = HashMap::<String, Vec<Entity>>::new();
    let mut map_mode_client_entities_by_player_id = HashMap::<String, Vec<Entity>>::new();
    for client_state in &scratch.client_states {
        let Some(client_context) = client_context_cache
            .by_client
            .get(&client_state.client_entity)
        else {
            continue;
        };
        client_entities_by_player_id
            .entry(client_context.player_entity_id.clone())
            .or_default()
            .push(client_state.client_entity);
        if matches!(client_context.view_mode, ClientLocalViewMode::Map) {
            map_mode_client_entities_by_player_id
                .entry(client_context.player_entity_id.clone())
                .or_default()
                .push(client_state.client_entity);
        }
    }

    let apply_started_at = Instant::now();
    for (entity, mut replication_state, controlled_by, runtime_world_visual_stack) in
        &mut replicated_entities
    {
        let Some(cached) = cache.by_entity.get(&entity) else {
            continue;
        };
        let current_visible_clients = membership_cache
            .by_entity
            .get(&entity)
            .cloned()
            .unwrap_or_default();
        let tracked_guid = cached.guid;
        let debug_track_this_entity =
            debug_visibility_entity_guid().is_some_and(|tracked| Some(tracked) == tracked_guid);
        let root_entity = spatial_index
            .root_entity_by_entity
            .get(&entity)
            .copied()
            .unwrap_or(entity);

        let entity_position = spatial_index
            .visibility_position_by_entity
            .get(&entity)
            .copied();
        let entity_extent_m = spatial_index
            .visibility_extent_m_by_entity
            .get(&entity)
            .copied()
            .unwrap_or(cached.entity_extent_m);
        let mut desired_visible_clients = HashSet::<Entity>::new();
        let resolved_world_layer = scratch
            .resolved_world_layer_by_entity
            .get(&entity)
            .or_else(|| scratch.resolved_world_layer_by_entity.get(&root_entity))
            .or_else(|| {
                cached
                    .pending_world_layer_override
                    .as_ref()
                    .and_then(|layer_id| runtime_layer_definitions_by_id.get(layer_id))
            });
        let prepared_policy = prepare_entity_apply_policy(
            cached,
            scratch
                .root_public_by_entity
                .get(&root_entity)
                .copied()
                .unwrap_or(false),
            scratch.root_owner_by_entity.get(&root_entity),
            scratch.root_faction_by_entity.get(&root_entity),
            entity_position,
            entity_extent_m,
            resolved_world_layer,
            runtime_world_visual_stack,
            controlled_by,
        );

        if runtime_cfg.bypass_all_filters {
            for client_state in &scratch.client_states {
                desired_visible_clients.insert(client_state.client_entity);
            }
            role_rearms.suppress_desired_clients(entity, &mut desired_visible_clients);
            let current_visible_clients = membership_cache.by_entity.entry(entity).or_default();
            let gained_count = apply_visibility_membership_diff(
                &mut replication_state,
                current_visible_clients,
                &desired_visible_clients,
                &mut visible_gains,
                &mut visible_losses,
            );
            if gained_count > 0 && entity_position.is_some() {
                // Some stationary spatial roots were observed to spawn for newly visible clients
                // with a default origin pose until a later movement delta arrived. Force one
                // resend of the current replicated motion state on visibility gain so late-join
                // observers receive an authoritative bootstrap even when the entity is idle.
                queue_visibility_gain_spatial_resend(&mut commands, entity);
            }
            continue;
        }

        match &prepared_policy {
            PreparedEntityApplyPolicy::OwnerOnlyAnchor { owner_player_id } => {
                if let Some(owner_player_id) = owner_player_id.as_ref()
                    && let Some(owner_clients) =
                        client_entities_by_player_id.get(owner_player_id.as_str())
                {
                    for client_entity in owner_clients {
                        desired_visible_clients.insert(*client_entity);
                    }
                }
            }
            PreparedEntityApplyPolicy::GlobalVisible => {
                for client_state in &scratch.client_states {
                    desired_visible_clients.insert(client_state.client_entity);
                }
            }
            PreparedEntityApplyPolicy::PublicVisible(_)
            | PreparedEntityApplyPolicy::FactionVisible(_)
            | PreparedEntityApplyPolicy::DiscoveredLandmark(_)
            | PreparedEntityApplyPolicy::RangeChecked(_) => {
                // Owner-in-map-view is a stable fast path once authorization resolves
                // to Owner. Seed those clients up front so the generic loop focuses on
                // client-varying range/faction/discovery work instead of repeating the
                // same owner-map bypass check for every client candidate.
                let owner_map_clients =
                    prepared_policy
                        .owner_player_id()
                        .and_then(|owner_player_id| {
                            map_mode_client_entities_by_player_id.get(owner_player_id)
                        });
                if let Some(owner_map_clients) = owner_map_clients {
                    for client_entity in owner_map_clients {
                        desired_visible_clients.insert(*client_entity);
                    }
                }
                for client_state in &scratch.client_states {
                    let client_entity = client_state.client_entity;
                    if owner_map_clients.is_some_and(|clients| clients.contains(&client_entity)) {
                        continue;
                    }
                    let Some(client_context) = client_context_cache.by_client.get(&client_entity)
                    else {
                        continue;
                    };
                    if prepared_policy.controlled_owner_client() == Some(client_entity) {
                        // Hard guarantee: the owning client must always receive state for
                        // their currently controlled entity, independent of visibility/range.
                        desired_visible_clients.insert(client_entity);
                        continue;
                    }
                    let visibility_context =
                        PlayerVisibilityContextRef::from_cached_client_context(client_context);
                    let in_candidates = client_state.candidate_entities.contains(&entity);
                    let visibility_eval = evaluate_prepared_entity_policy_for_client(
                        &prepared_policy,
                        client_context,
                        &visibility_context,
                        matches!(visibility_context.view_mode, ClientLocalViewMode::Map),
                    );
                    if !in_candidates && !visibility_eval.bypass_candidate {
                        if debug_track_this_entity {
                            info!(
                                "vis-debug guid={} client_entity={:?} player={} in_candidates={} bypass_candidate={} owner={:?} public={} faction_visible={} entity_pos={:?} anchor_pos={:?} result=lose(candidate)",
                                tracked_guid
                                    .map(|g| g.to_string())
                                    .unwrap_or_else(|| "<none>".to_string()),
                                client_entity,
                                visibility_context.player_entity_id,
                                in_candidates,
                                visibility_eval.bypass_candidate,
                                prepared_policy.owner_player_id(),
                                prepared_policy.is_public_visibility(),
                                prepared_policy.is_faction_visibility(),
                                prepared_policy.entity_position(),
                                visibility_context.observer_anchor_position,
                            );
                        }
                        continue;
                    }
                    if visibility_eval.should_be_visible {
                        desired_visible_clients.insert(client_entity);
                    }
                    if debug_track_this_entity {
                        info!(
                            "vis-debug guid={} client_entity={:?} player={} in_candidates={} bypass_candidate={} owner={:?} public={} faction_visible={} authorization={:?} delivery_ok={} entity_pos={:?} anchor_pos={:?} currently_visible={} result={}",
                            tracked_guid
                                .map(|g| g.to_string())
                                .unwrap_or_else(|| "<none>".to_string()),
                            client_entity,
                            visibility_context.player_entity_id,
                            in_candidates,
                            visibility_eval.bypass_candidate,
                            prepared_policy.owner_player_id(),
                            prepared_policy.is_public_visibility(),
                            prepared_policy.is_faction_visibility(),
                            visibility_eval.authorization,
                            visibility_eval.delivery_ok,
                            prepared_policy.entity_position(),
                            visibility_context.observer_anchor_position,
                            current_visible_clients.contains(&client_entity),
                            if visibility_eval.should_be_visible {
                                "gain/keep"
                            } else {
                                "lose"
                            }
                        );
                    }
                }
            }
        }
        role_rearms.suppress_desired_clients(entity, &mut desired_visible_clients);
        let current_visible_clients = membership_cache.by_entity.entry(entity).or_default();
        let gained_count = apply_visibility_membership_diff(
            &mut replication_state,
            current_visible_clients,
            &desired_visible_clients,
            &mut visible_gains,
            &mut visible_losses,
        );
        if gained_count > 0 && entity_position.is_some() {
            queue_visibility_gain_spatial_resend(&mut commands, entity);
        }
    }
    let apply_ms = apply_started_at.elapsed().as_secs_f64() * 1000.0;
    let occupied_cells = spatial_index.entities_by_cell.len();
    let max_entities_per_cell = spatial_index
        .entities_by_cell
        .values()
        .map(Vec::len)
        .max()
        .unwrap_or(0);

    if summary_logging_enabled() {
        let now_s = time.elapsed_secs_f64();
        const LOG_INTERVAL_S: f64 = 5.0;
        if now_s - telemetry_state.last_logged_at_s >= LOG_INTERVAL_S {
            telemetry_state.last_logged_at_s = now_s;
            let clients_count = scratch.client_states.len();
            let entities_count = scratch.all_replicated_entities.len();
            let candidates_total = scratch
                .client_states
                .iter()
                .map(|state| state.candidate_entities.len())
                .sum::<usize>();
            let candidates_per_client = if clients_count > 0 {
                candidates_total as f64 / clients_count as f64
            } else {
                0.0
            };
            let (delivery_min, delivery_avg, delivery_max) = if scratch.client_states.is_empty() {
                (
                    runtime_cfg.delivery_range_m as f64,
                    runtime_cfg.delivery_range_m as f64,
                    runtime_cfg.delivery_range_m as f64,
                )
            } else {
                let mut values = scratch
                    .client_states
                    .iter()
                    .filter_map(|state| {
                        client_context_cache
                            .by_client
                            .get(&state.client_entity)
                            .map(|context| context.delivery_range_m as f64)
                    })
                    .collect::<Vec<_>>();
                if values.is_empty() {
                    (
                        runtime_cfg.delivery_range_m as f64,
                        runtime_cfg.delivery_range_m as f64,
                        runtime_cfg.delivery_range_m as f64,
                    )
                } else {
                    values.sort_by(|a, b| a.total_cmp(b));
                    let min = *values
                        .first()
                        .unwrap_or(&(runtime_cfg.delivery_range_m as f64));
                    let max = *values
                        .last()
                        .unwrap_or(&(runtime_cfg.delivery_range_m as f64));
                    let avg = values.iter().sum::<f64>() / values.len() as f64;
                    (min, avg, max)
                }
            };
            info!(
                "replication visibility summary mode={} bypass_all={} delivery_range_m[min/avg/max]={:.1}/{:.1}/{:.1} delivery_range_clamped_requests_total={} query_ms={:.2} cache_refresh_ms={:.2} cache_upserts={} cache_removals={} client_context_refresh_ms={:.2} client_cache_entries={} client_cache_upserts={} client_cache_removals={} landmark_discovery_ms={:.2} landmark_discovery_checks={} landmark_discovery_new_total={} clients={} entities={} candidates_per_client={:.1} occupied_cells={} max_entities_per_cell={} visible_gains={} visible_losses={}",
                runtime_cfg.candidate_mode.as_str(),
                runtime_cfg.bypass_all_filters,
                delivery_min,
                delivery_avg,
                delivery_max,
                local_view_delivery_metrics.clamped_requests_total,
                started_at.elapsed().as_secs_f64() * 1000.0,
                preparation_metrics.cache_refresh_ms,
                preparation_metrics.cache_upserts,
                preparation_metrics.cache_removals,
                client_context_refresh_ms,
                client_cache_entries,
                client_cache_upserts,
                client_cache_removals,
                landmark_metrics.landmark_discovery_ms,
                landmark_metrics.discovered_checks,
                landmark_metrics.discovered_new_total,
                clients_count,
                entities_count,
                candidates_per_client,
                occupied_cells,
                max_entities_per_cell,
                visible_gains,
                visible_losses
            );
        }
    }

    let clients_count = scratch.client_states.len();
    let entities_count = scratch.all_replicated_entities.len();
    let candidates_total = scratch
        .client_states
        .iter()
        .map(|state| state.candidate_entities.len())
        .sum::<usize>();
    let candidates_per_client = if clients_count > 0 {
        candidates_total as f64 / clients_count as f64
    } else {
        0.0
    };
    let (delivery_min, delivery_avg, delivery_max) = if scratch.client_states.is_empty() {
        (
            runtime_cfg.delivery_range_m as f64,
            runtime_cfg.delivery_range_m as f64,
            runtime_cfg.delivery_range_m as f64,
        )
    } else {
        let mut values = scratch
            .client_states
            .iter()
            .filter_map(|state| {
                client_context_cache
                    .by_client
                    .get(&state.client_entity)
                    .map(|context| context.delivery_range_m as f64)
            })
            .collect::<Vec<_>>();
        if values.is_empty() {
            (
                runtime_cfg.delivery_range_m as f64,
                runtime_cfg.delivery_range_m as f64,
                runtime_cfg.delivery_range_m as f64,
            )
        } else {
            values.sort_by(|a, b| a.total_cmp(b));
            let min = *values
                .first()
                .unwrap_or(&(runtime_cfg.delivery_range_m as f64));
            let max = *values
                .last()
                .unwrap_or(&(runtime_cfg.delivery_range_m as f64));
            let avg = values.iter().sum::<f64>() / values.len() as f64;
            (min, avg, max)
        }
    };
    commands.insert_resource(VisibilityRuntimeMetrics {
        cache_refresh_ms: preparation_metrics.cache_refresh_ms,
        cache_upserts: preparation_metrics.cache_upserts,
        cache_removals: preparation_metrics.cache_removals,
        client_context_refresh_ms,
        client_cache_entries,
        client_cache_upserts,
        client_cache_removals,
        landmark_discovery_ms: landmark_metrics.landmark_discovery_ms,
        query_ms: started_at.elapsed().as_secs_f64() * 1000.0,
        scratch_build_ms,
        discovery_and_candidate_ms,
        disclosure_sync_ms,
        apply_ms,
        clients: clients_count,
        entities: entities_count,
        candidates_total,
        candidates_per_client,
        discovered_checks: landmark_metrics.discovered_checks,
        discovered_new_total: landmark_metrics.discovered_new_total,
        delivery_range_min_m: delivery_min,
        delivery_range_avg_m: delivery_avg,
        delivery_range_max_m: delivery_max,
        delivery_range_clamped_requests_total: local_view_delivery_metrics.clamped_requests_total,
        occupied_cells,
        max_entities_per_cell,
        visible_gains,
        visible_losses,
    });
    role_rearms.advance_after_membership_pass();
}

/// Resolves the mount root entity by traversing the parent chain (MountedOn).
/// The root is used for owner/public/faction inheritance and to derive the
/// effective visibility position/extent for mounted children.
fn resolve_mount_root(entity: Entity, parent_entity_by_entity: &HashMap<Entity, Entity>) -> Entity {
    let mut current = entity;
    let mut visited = std::collections::HashSet::new();
    while let Some(&parent) = parent_entity_by_entity.get(&current) {
        if !visited.insert(current) {
            break;
        }
        current = parent;
    }
    current
}
