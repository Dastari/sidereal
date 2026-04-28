#[allow(clippy::type_complexity)]
pub fn refresh_visibility_entity_cache(
    mut cache: ResMut<'_, VisibilityEntityCache>,
    mut preparation_metrics: ResMut<'_, VisibilityPreparationMetrics>,
    mut refresh: VisibilityCacheRefreshParams<'_, '_>,
) {
    let started_at = Instant::now();
    let mut cache_upserts = 0usize;
    let mut cache_removals = 0usize;
    let mut dirty_entities = HashSet::<Entity>::new();

    for entity in refresh.removed_replicates.read() {
        if cache.by_entity.remove(&entity).is_some() {
            cache_removals = cache_removals.saturating_add(1);
        }
    }

    for entity in refresh.removed_entity_guid.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_owner_id.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_visibility_range.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_public_visibility.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_faction_visibility.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_faction_id.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_mounted_on.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_parent_guid.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_size.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_runtime_render_layer_definition.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_runtime_render_layer_override.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_static_landmark.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_player_tag.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_fullscreen_layer.read() {
        dirty_entities.insert(entity);
    }

    for (
        entity,
        guid,
        owner_id,
        visibility_range,
        public_visibility,
        faction_visibility,
        faction_id,
        mounted_on,
        parent_guid,
        size,
        runtime_render_layer_definition,
        runtime_render_layer_override,
        static_landmark,
        player_tag,
        fullscreen_layer,
    ) in &refresh.changed_replicated_entities
    {
        cache.by_entity.insert(
            entity,
            build_cached_visibility_entity(
                guid,
                owner_id,
                visibility_range,
                public_visibility,
                faction_visibility,
                faction_id,
                mounted_on,
                parent_guid,
                size,
                runtime_render_layer_definition,
                runtime_render_layer_override,
                static_landmark,
                player_tag,
                fullscreen_layer,
            ),
        );
        cache_upserts = cache_upserts.saturating_add(1);
        dirty_entities.remove(&entity);
    }

    if cache.by_entity.is_empty() {
        for (
            entity,
            guid,
            owner_id,
            visibility_range,
            public_visibility,
            faction_visibility,
            faction_id,
            mounted_on,
            parent_guid,
            size,
            runtime_render_layer_definition,
            runtime_render_layer_override,
            static_landmark,
            player_tag,
            fullscreen_layer,
        ) in &refresh.replicated_entities
        {
            cache.by_entity.insert(
                entity,
                build_cached_visibility_entity(
                    guid,
                    owner_id,
                    visibility_range,
                    public_visibility,
                    faction_visibility,
                    faction_id,
                    mounted_on,
                    parent_guid,
                    size,
                    runtime_render_layer_definition,
                    runtime_render_layer_override,
                    static_landmark,
                    player_tag,
                    fullscreen_layer,
                ),
            );
            cache_upserts = cache_upserts.saturating_add(1);
        }
    } else {
        for entity in dirty_entities {
            if let Ok((
                _,
                guid,
                owner_id,
                visibility_range,
                public_visibility,
                faction_visibility,
                faction_id,
                mounted_on,
                parent_guid,
                size,
                runtime_render_layer_definition,
                runtime_render_layer_override,
                static_landmark,
                player_tag,
                fullscreen_layer,
            )) = refresh.replicated_entities.get(entity)
            {
                cache.by_entity.insert(
                    entity,
                    build_cached_visibility_entity(
                        guid,
                        owner_id,
                        visibility_range,
                        public_visibility,
                        faction_visibility,
                        faction_id,
                        mounted_on,
                        parent_guid,
                        size,
                        runtime_render_layer_definition,
                        runtime_render_layer_override,
                        static_landmark,
                        player_tag,
                        fullscreen_layer,
                    ),
                );
                cache_upserts = cache_upserts.saturating_add(1);
            } else if cache.by_entity.remove(&entity).is_some() {
                cache_removals = cache_removals.saturating_add(1);
            }
        }
    }

    preparation_metrics.cache_refresh_ms = started_at.elapsed().as_secs_f64() * 1000.0;
    preparation_metrics.cache_entries = cache.by_entity.len();
    preparation_metrics.cache_upserts = cache_upserts;
    preparation_metrics.cache_removals = cache_removals;
}

#[allow(clippy::type_complexity)]
pub fn receive_client_local_view_mode_messages(
    time: Res<'_, Time<Real>>,
    mut receivers: Query<
        '_,
        '_,
        (Entity, &'_ mut MessageReceiver<ClientLocalViewModeMessage>),
        With<ClientOf>,
    >,
    bindings: Res<'_, AuthenticatedClientBindings>,
    runtime_cfg: Res<'_, VisibilityRuntimeConfig>,
    mut last_activity: ResMut<'_, ClientLastActivity>,
    mut registry: ResMut<'_, ClientLocalViewModeRegistry>,
    mut delivery_metrics: ResMut<'_, ClientLocalViewDeliveryMetrics>,
) {
    let now_s = time.elapsed_secs_f64();
    for (client_entity, mut receiver) in &mut receivers {
        for message in receiver.receive() {
            let Some(bound_player_id) = bindings.by_client_entity.get(&client_entity) else {
                continue;
            };
            let Some(bound_player_id) = PlayerEntityId::parse(bound_player_id.as_str()) else {
                continue;
            };
            let Some(message_player_id) = PlayerEntityId::parse(message.player_entity_id.as_str())
            else {
                continue;
            };
            if bound_player_id != message_player_id {
                continue;
            }
            last_activity.0.insert(client_entity, now_s);
            let delivery_range = sanitize_client_delivery_range_m(
                message.delivery_range_m,
                message.view_mode,
                &runtime_cfg,
            );
            if delivery_range.was_clamped {
                delivery_metrics.clamped_requests_total =
                    delivery_metrics.clamped_requests_total.saturating_add(1);
            }
            registry.by_client_entity.insert(
                client_entity,
                ClientLocalViewSettings {
                    view_mode: message.view_mode,
                    delivery_range_m: delivery_range.range_m,
                },
            );
        }
    }
}

