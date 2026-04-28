pub fn ensure_network_visibility_for_replicated_entities(
    mut commands: Commands<'_, '_>,
    query: Query<'_, '_, Entity, (With<Replicate>, Without<NetworkVisibility>)>,
) {
    for entity in &query {
        commands.entity(entity).insert(NetworkVisibility);
    }
}

fn landmark_discovery_due(now_s: f64, last_run_at_s: Option<f64>, interval_s: f64) -> bool {
    last_run_at_s.is_none_or(|last_run_at_s| now_s - last_run_at_s >= interval_s)
}

fn replicated_visibility_world_position(
    position: Option<&Position>,
    world_position: Option<&WorldPosition>,
    global_transform: &GlobalTransform,
) -> Vec3 {
    if let Some(position) = position
        && position.0.is_finite()
    {
        return position.0.extend(0.0).as_vec3();
    }
    if let Some(world_position) = world_position
        && world_position.0.is_finite()
    {
        return world_position.0.extend(0.0).as_vec3();
    }
    let world_pos = global_transform.translation();
    if world_pos.is_finite() {
        world_pos
    } else {
        Vec3::ZERO
    }
}

pub fn refresh_static_landmark_discoveries(
    mut commands: Commands<'_, '_>,
    time: Res<'_, Time>,
    runtime_cfg: Res<'_, VisibilityRuntimeConfig>,
    mut metrics: ResMut<'_, VisibilityLandmarkDiscoveryMetrics>,
    mut last_run_at_s: Local<'_, Option<f64>>,
    mut params: VisibilityLandmarkDiscoveryParams<'_, '_>,
) {
    metrics.landmark_discovery_ms = 0.0;
    metrics.discovered_checks = 0;
    metrics.discovered_new_total = 0;

    let now_s = time.elapsed_secs_f64();
    if !landmark_discovery_due(
        now_s,
        *last_run_at_s,
        runtime_cfg.landmark_discovery_interval_s,
    ) {
        return;
    }
    *last_run_at_s = Some(now_s);

    let started_at = Instant::now();
    let mut static_landmarks_by_entity = HashMap::<Entity, StaticLandmarkCacheEntry>::new();
    let mut visibility_position_by_entity = HashMap::<Entity, Vec3>::new();
    let mut visibility_extent_m_by_entity = HashMap::<Entity, f32>::new();
    let mut entities_by_cell = HashMap::<(i64, i64), Vec<Entity>>::new();
    let mut max_static_landmark_discovery_padding_m = 0.0f32;

    for (entity, position, world_position, global_transform, signal_signature) in
        &params.all_replicated
    {
        let Some(cached) = params.cache.by_entity.get(&entity) else {
            continue;
        };
        let Some((guid, static_landmark)) = cached
            .guid
            .zip(cached.static_landmark.as_ref())
            .map(|(guid, landmark)| (guid, landmark.clone()))
        else {
            continue;
        };
        let effective_world_pos =
            replicated_visibility_world_position(position, world_position, global_transform);
        visibility_position_by_entity.insert(entity, effective_world_pos);
        visibility_extent_m_by_entity.insert(entity, cached.entity_extent_m);
        static_landmarks_by_entity.insert(
            entity,
            (guid, static_landmark.clone(), signal_signature.copied()),
        );
        entities_by_cell
            .entry(cell_key(effective_world_pos, runtime_cfg.cell_size_m))
            .or_default()
            .push(entity);
        let discovery_padding_m = static_landmark_discovery_padding_m(
            cached.entity_extent_m,
            &static_landmark,
            signal_signature,
        );
        max_static_landmark_discovery_padding_m =
            max_static_landmark_discovery_padding_m.max(discovery_padding_m);
    }

    for client_context in params.client_context_cache.by_client.values_mut() {
        let Some(player_entity) = client_context.player_entity else {
            continue;
        };
        let Ok(discovered_component) = params.player_landmark_state.get_mut(player_entity) else {
            continue;
        };
        let mut discovered_component = discovered_component;
        let mut discovered_static_landmarks: HashSet<uuid::Uuid> = discovered_component
            .as_deref()
            .map(|component| component.landmark_entity_ids.iter().copied().collect())
            .unwrap_or_default();
        let mut newly_discovered = Vec::<(uuid::Uuid, Entity, LandmarkDiscoveryCause)>::new();
        let mut discovery_candidates = HashSet::<Entity>::new();
        for (visibility_pos, visibility_range_m) in &client_context.visibility_sources {
            add_entities_in_radius(
                *visibility_pos,
                *visibility_range_m + max_static_landmark_discovery_padding_m,
                runtime_cfg.cell_size_m,
                &entities_by_cell,
                &mut discovery_candidates,
            );
        }
        let discovery_context =
            PlayerVisibilityContextRef::from_cached_client_context(client_context);
        for target_entity in discovery_candidates {
            let Some((target_guid, static_landmark, signal_signature)) =
                static_landmarks_by_entity.get(&target_entity)
            else {
                continue;
            };
            metrics.discovered_checks = metrics.discovered_checks.saturating_add(1);
            if discovered_static_landmarks.contains(target_guid) {
                continue;
            }
            let target_position = visibility_position_by_entity.get(&target_entity).copied();
            let entity_extent_m = visibility_extent_m_by_entity
                .get(&target_entity)
                .copied()
                .unwrap_or(0.0);
            if let Some(discovery_cause) = landmark_discovery_cause(
                target_position,
                entity_extent_m,
                static_landmark,
                signal_signature.as_ref(),
                &discovery_context,
            ) {
                newly_discovered.push((*target_guid, target_entity, discovery_cause));
            }
        }
        metrics.discovered_new_total = metrics
            .discovered_new_total
            .saturating_add(newly_discovered.len());
        if !newly_discovered.is_empty() {
            if let Some(component) = discovered_component.as_deref_mut() {
                for (landmark_id, landmark_entity, discovery_cause) in newly_discovered {
                    if component.insert(landmark_id) {
                        discovered_static_landmarks.insert(landmark_id);
                        if matches!(discovery_cause, LandmarkDiscoveryCause::Direct) {
                            enqueue_static_landmark_discovery_notification(
                                &mut params.notification_queue,
                                client_context.player_entity_id.as_str(),
                                landmark_id,
                                landmark_entity,
                                &static_landmarks_by_entity,
                                &params.landmark_notification_meta,
                            );
                        }
                    }
                }
            } else {
                let mut component = DiscoveredStaticLandmarks::default();
                for (landmark_id, landmark_entity, discovery_cause) in newly_discovered {
                    if component.insert(landmark_id) {
                        discovered_static_landmarks.insert(landmark_id);
                        if matches!(discovery_cause, LandmarkDiscoveryCause::Direct) {
                            enqueue_static_landmark_discovery_notification(
                                &mut params.notification_queue,
                                client_context.player_entity_id.as_str(),
                                landmark_id,
                                landmark_entity,
                                &static_landmarks_by_entity,
                                &params.landmark_notification_meta,
                            );
                        }
                    }
                }
                commands.entity(player_entity).insert(component);
            }
        }
        client_context.discovered_static_landmarks = discovered_static_landmarks;
    }

    metrics.landmark_discovery_ms = started_at.elapsed().as_secs_f64() * 1000.0;
}

fn enqueue_static_landmark_discovery_notification(
    queue: &mut NotificationCommandQueue,
    player_entity_id: &str,
    landmark_id: uuid::Uuid,
    landmark_entity: Entity,
    static_landmarks_by_entity: &HashMap<Entity, StaticLandmarkCacheEntry>,
    landmark_notification_meta: &Query<
        '_,
        '_,
        (
            Option<&DisplayName>,
            Option<&MapIcon>,
            Option<&WorldPosition>,
        ),
    >,
) {
    let Some((_, static_landmark, _)) = static_landmarks_by_entity.get(&landmark_entity) else {
        return;
    };
    let (display_name, map_icon_asset_id, world_position_xy) = landmark_notification_meta
        .get(landmark_entity)
        .map(|(display_name, map_icon, world_position)| {
            (
                display_name.map(|value| value.0.clone()),
                map_icon.map(|value| value.asset_id.clone()),
                world_position.map(|value| [value.0.x, value.0.y]),
            )
        })
        .unwrap_or((None, None, None));
    let landmark_kind = if static_landmark.kind.trim().is_empty() {
        "Landmark".to_string()
    } else {
        static_landmark.kind.clone()
    };
    let display_name = display_name
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            if landmark_kind.trim().is_empty() {
                landmark_id.to_string()
            } else {
                landmark_kind.clone()
            }
        });
    enqueue_player_notification(
        queue,
        landmark_discovery_notification_command(
            player_entity_id,
            landmark_id,
            display_name,
            landmark_kind,
            map_icon_asset_id,
            world_position_xy,
        ),
    );
}

fn landmark_discovery_notification_command(
    player_entity_id: &str,
    landmark_id: uuid::Uuid,
    display_name: String,
    landmark_kind: String,
    map_icon_asset_id: Option<String>,
    world_position_xy: Option<[f64; 2]>,
) -> NotificationCommand {
    NotificationCommand {
        player_entity_id: player_entity_id.to_string(),
        title: "Landmark Discovered".to_string(),
        body: display_name.clone(),
        severity: NotificationSeverity::Info,
        placement: NotificationPlacement::BottomRight,
        image: None,
        payload: NotificationPayload::LandmarkDiscovery {
            entity_guid: landmark_id.to_string(),
            display_name,
            landmark_kind,
            map_icon_asset_id,
            world_position_xy,
        },
        auto_dismiss_after_s: None,
    }
}

impl VisibilityScratch {
    fn clear(&mut self) {
        self.live_clients.clear();
        self.live_client_set.clear();
        self.registered_clients.clear();
        self.all_replicated_entities.clear();
        self.entity_by_guid.clear();
        self.world_position_by_entity.clear();
        self.visibility_position_by_entity.clear();
        self.visibility_extent_m_by_entity.clear();
        self.parent_entity_by_entity.clear();
        self.root_entity_by_entity.clear();
        self.root_public_by_entity.clear();
        self.root_owner_by_entity.clear();
        self.root_faction_by_entity.clear();
        self.pending_world_layer_override_by_entity.clear();
        self.resolved_world_layer_by_entity.clear();
        self.visibility_source_candidates.clear();
        self.visibility_sources_by_owner.clear();
        self.player_faction_by_owner.clear();
        self.entities_by_cell.clear();
        self.owned_entities_by_player.clear();
        self.static_landmarks_by_entity.clear();
        self.max_static_landmark_discovery_padding_m = 0.0;
        self.client_states.clear();
    }
}

