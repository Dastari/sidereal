#[cfg(test)]
mod tests {
    use super::*;

    fn cached_client_visibility_context(
        player_entity_id: &str,
        player_faction_id: Option<&str>,
        delivery_range_m: f32,
        visibility_sources: Vec<(Vec3, f32)>,
        discovered_static_landmarks: impl IntoIterator<Item = uuid::Uuid>,
    ) -> CachedClientVisibilityContext {
        CachedClientVisibilityContext {
            player_entity_id: player_entity_id.to_string(),
            player_entity: None,
            observer_anchor_position: Some(Vec3::ZERO),
            visibility_sources,
            discovered_static_landmarks: discovered_static_landmarks.into_iter().collect(),
            player_faction_id: player_faction_id.map(str::to_string),
            view_mode: ClientLocalViewMode::Tactical,
            delivery_range_m,
        }
    }

    fn test_runtime_config() -> VisibilityRuntimeConfig {
        VisibilityRuntimeConfig {
            candidate_mode: VisibilityCandidateMode::SpatialGrid,
            delivery_range_m: DEFAULT_VIEW_RANGE_M,
            delivery_range_max_m: DEFAULT_DELIVERY_RANGE_MAX_M,
            cell_size_m: DEFAULT_VISIBILITY_CELL_SIZE_M,
            landmark_discovery_interval_s: DEFAULT_LANDMARK_DISCOVERY_INTERVAL_S,
            bypass_all_filters: false,
        }
    }

    #[test]
    fn candidate_mode_defaults_to_spatial_grid() {
        assert_eq!(
            VisibilityCandidateMode::from_raw(None),
            VisibilityCandidateMode::SpatialGrid
        );
    }

    #[test]
    fn candidate_mode_parses_full_aliases() {
        assert_eq!(
            VisibilityCandidateMode::from_raw(Some("full_scan")),
            VisibilityCandidateMode::FullScan
        );
        assert_eq!(
            VisibilityCandidateMode::from_raw(Some("full")),
            VisibilityCandidateMode::FullScan
        );
    }

    #[test]
    fn candidate_mode_unknown_values_fall_back_to_spatial_grid() {
        assert_eq!(
            VisibilityCandidateMode::from_raw(Some("grid")),
            VisibilityCandidateMode::SpatialGrid
        );
        assert_eq!(
            VisibilityCandidateMode::from_raw(Some("random")),
            VisibilityCandidateMode::SpatialGrid
        );
    }

    #[test]
    fn parse_cell_size_requires_minimum_and_finite_value() {
        assert_eq!(parse_cell_size_m(Some("49.9")), None);
        assert_eq!(parse_cell_size_m(Some("2000")), Some(2000.0));
        assert_eq!(parse_cell_size_m(Some("NaN")), None);
    }

    #[test]
    fn client_delivery_range_clamps_non_finite_and_large_values() {
        let cfg = test_runtime_config();

        let huge = sanitize_client_delivery_range_m(f32::MAX, ClientLocalViewMode::Tactical, &cfg);
        assert_eq!(huge.range_m, DEFAULT_DELIVERY_RANGE_MAX_M);
        assert!(huge.was_clamped);

        let infinity =
            sanitize_client_delivery_range_m(f32::INFINITY, ClientLocalViewMode::Tactical, &cfg);
        assert_eq!(infinity.range_m, DEFAULT_VIEW_RANGE_M);
        assert!(infinity.was_clamped);

        let nan = sanitize_client_delivery_range_m(f32::NAN, ClientLocalViewMode::Tactical, &cfg);
        assert_eq!(nan.range_m, DEFAULT_VIEW_RANGE_M);
        assert!(nan.was_clamped);
    }

    #[test]
    fn clamped_client_delivery_range_bounds_candidate_cells() {
        let cfg = test_runtime_config();
        let observer = Vec3::ZERO;
        let inside_cap = Entity::from_raw_u32(10).expect("valid entity id");
        let outside_cap = Entity::from_raw_u32(11).expect("valid entity id");
        let mut scratch = VisibilityScratch::default();
        scratch
            .entities_by_cell
            .insert((25_i64, 0_i64), vec![inside_cap]);
        scratch
            .entities_by_cell
            .insert((26_i64, 0_i64), vec![outside_cap]);

        let sanitized =
            sanitize_client_delivery_range_m(f32::MAX, ClientLocalViewMode::Tactical, &cfg);
        let candidates = build_candidate_set_for_client(
            VisibilityCandidateMode::SpatialGrid,
            "11111111-1111-1111-1111-111111111111",
            Some(observer),
            sanitized.range_m,
            &[],
            ClientLocalViewMode::Tactical,
            DEFAULT_VISIBILITY_CELL_SIZE_M,
            &scratch.all_replicated_entities,
            &scratch.owned_entities_by_player,
            &scratch.entities_by_cell,
        );

        assert!(candidates.contains(&inside_cap));
        assert!(!candidates.contains(&outside_cap));
    }

    #[test]
    fn cell_key_uses_i64_for_large_coordinates() {
        let position = Vec3::new(5.0e12, -5.0e12, 0.0);
        let key = cell_key(position, 2000.0);
        assert!(key.0 > i64::from(i32::MAX));
        assert!(key.1 < i64::from(i32::MIN));
    }

    #[test]
    fn add_entities_in_radius_uses_configured_cell_size() {
        let center = Vec3::new(0.0, 0.0, 0.0);
        let near = Entity::from_raw_u32(1).expect("valid entity id");
        let far = Entity::from_raw_u32(2).expect("valid entity id");
        let mut grid = HashMap::new();
        grid.insert((0_i64, 0_i64), vec![near]);
        grid.insert((2_i64, 0_i64), vec![far]);

        let mut out = HashSet::new();
        add_entities_in_radius(center, 500.0, 1000.0, &grid, &mut out);
        assert!(out.contains(&near));
        assert!(!out.contains(&far));
    }

    #[test]
    fn candidate_set_uses_configured_delivery_range_for_observer_anchor() {
        let observer = Vec3::ZERO;
        let candidate = Entity::from_raw_u32(3).expect("valid entity id");
        let mut scratch = VisibilityScratch::default();
        // Candidate is two cells away on X when cell size is 1000m.
        scratch
            .entities_by_cell
            .insert((2_i64, 0_i64), vec![candidate]);

        let short = build_candidate_set_for_client(
            VisibilityCandidateMode::SpatialGrid,
            "11111111-1111-1111-1111-111111111111",
            Some(observer),
            500.0,
            &[],
            ClientLocalViewMode::Tactical,
            1000.0,
            &scratch.all_replicated_entities,
            &scratch.owned_entities_by_player,
            &scratch.entities_by_cell,
        );
        let long = build_candidate_set_for_client(
            VisibilityCandidateMode::SpatialGrid,
            "11111111-1111-1111-1111-111111111111",
            Some(observer),
            2500.0,
            &[],
            ClientLocalViewMode::Tactical,
            1000.0,
            &scratch.all_replicated_entities,
            &scratch.owned_entities_by_player,
            &scratch.entities_by_cell,
        );

        assert!(!short.contains(&candidate));
        assert!(long.contains(&candidate));
    }

    #[test]
    fn candidate_cells_include_observer_region_only() {
        let observer = Vec3::new(0.0, 0.0, 0.0);
        let cells = build_candidate_cells_for_client(
            VisibilityCandidateMode::SpatialGrid,
            Some(observer),
            1000.0,
            &[],
            ClientLocalViewMode::Tactical,
            1000.0,
        );
        assert!(cells.contains(&(0, 0)));
        assert!(cells.contains(&(1, 0)));
        assert!(!cells.contains(&(2, 0)));
    }

    #[test]
    fn delivery_scope_includes_entity_extent() {
        let visibility_sources = Vec::new();
        let visibility_context = PlayerVisibilityContextRef {
            player_entity_id: "11111111-1111-1111-1111-111111111111",
            observer_anchor_position: Some(Vec3::ZERO),
            visibility_sources: &visibility_sources,
            player_faction_id: None,
            view_mode: ClientLocalViewMode::Tactical,
        };

        assert!(passes_delivery_scope(
            Some(Vec3::new(1000.0, 0.0, 0.0)),
            100.0,
            &visibility_context,
            900.0,
        ));
    }

    #[test]
    fn authorization_range_includes_entity_extent() {
        let visibility_sources = vec![(Vec3::ZERO, 900.0)];
        let visibility_context = PlayerVisibilityContextRef {
            player_entity_id: "11111111-1111-1111-1111-111111111111",
            observer_anchor_position: Some(Vec3::ZERO),
            visibility_sources: &visibility_sources,
            player_faction_id: None,
            view_mode: ClientLocalViewMode::Tactical,
        };

        assert_eq!(
            authorize_visibility(
                "11111111-1111-1111-1111-111111111111",
                None,
                false,
                false,
                false,
                None,
                Some(Vec3::new(1000.0, 0.0, 0.0)),
                100.0,
                &visibility_context,
            ),
            Some(VisibilityAuthorization::Range)
        );
    }

    #[test]
    fn evaluate_visibility_reuses_authorization_for_candidate_and_delivery() {
        let visibility_sources = vec![(Vec3::new(1000.0, 0.0, 0.0), 200.0)];
        let visibility_context = PlayerVisibilityContextRef {
            player_entity_id: "11111111-1111-1111-1111-111111111111",
            observer_anchor_position: Some(Vec3::new(1000.0, 0.0, 0.0)),
            visibility_sources: &visibility_sources,
            player_faction_id: None,
            view_mode: ClientLocalViewMode::Tactical,
        };

        let visible = evaluate_visibility_for_client(
            visibility_context.player_entity_id,
            None,
            false,
            false,
            false,
            None,
            Some(Vec3::new(1050.0, 0.0, 0.0)),
            0.0,
            0.0,
            &visibility_context,
            DEFAULT_VIEW_RANGE_M,
            false,
        );
        assert_eq!(visible.authorization, Some(VisibilityAuthorization::Range));
        assert!(visible.bypass_candidate);
        assert!(visible.delivery_ok);
        assert!(visible.should_be_visible);

        let hidden = evaluate_visibility_for_client(
            visibility_context.player_entity_id,
            Some(visibility_context.player_entity_id),
            false,
            false,
            false,
            None,
            Some(Vec3::new(5_000.0, 0.0, 0.0)),
            0.0,
            0.0,
            &visibility_context,
            DEFAULT_VIEW_RANGE_M,
            false,
        );
        assert_eq!(hidden.authorization, Some(VisibilityAuthorization::Owner));
        assert!(hidden.bypass_candidate);
        assert!(!hidden.delivery_ok);
        assert!(!hidden.should_be_visible);

        let map_owner = evaluate_visibility_for_client(
            visibility_context.player_entity_id,
            Some(visibility_context.player_entity_id),
            false,
            false,
            false,
            None,
            Some(Vec3::new(5_000.0, 0.0, 0.0)),
            0.0,
            0.0,
            &PlayerVisibilityContextRef {
                view_mode: ClientLocalViewMode::Map,
                ..visibility_context
            },
            DEFAULT_VIEW_RANGE_M,
            true,
        );
        assert_eq!(
            map_owner.authorization,
            Some(VisibilityAuthorization::Owner)
        );
        assert!(map_owner.bypass_candidate);
        assert!(map_owner.delivery_ok);
        assert!(map_owner.should_be_visible);
    }

    #[test]
    fn prepare_entity_apply_policy_classifies_special_and_conditional_entities() {
        let player_anchor_guid = uuid::Uuid::new_v4();
        let player_anchor = CachedVisibilityEntity {
            guid: Some(player_anchor_guid),
            is_player_tag: true,
            ..Default::default()
        };
        let PreparedEntityApplyPolicy::OwnerOnlyAnchor { owner_player_id } =
            prepare_entity_apply_policy(
                &player_anchor,
                false,
                None,
                None,
                Some(Vec3::ZERO),
                4.0,
                None,
                None,
                None,
            )
        else {
            panic!("expected owner-only anchor policy");
        };
        let expected_owner_id = player_anchor_guid.to_string();
        assert_eq!(owner_player_id.as_deref(), Some(expected_owner_id.as_str()));

        let global = CachedVisibilityEntity {
            is_global_render_config: true,
            ..Default::default()
        };
        assert!(matches!(
            prepare_entity_apply_policy(
                &global,
                false,
                None,
                None,
                Some(Vec3::ZERO),
                4.0,
                None,
                None,
                None,
            ),
            PreparedEntityApplyPolicy::GlobalVisible
        ));

        let public_landmark = CachedVisibilityEntity {
            guid: Some(uuid::Uuid::new_v4()),
            static_landmark: Some(StaticLandmark {
                kind: "Landmark".to_string(),
                discoverable: true,
                always_known: false,
                discovery_radius_m: None,
                use_extent_for_discovery: false,
            }),
            public_visibility: true,
            ..Default::default()
        };
        let PreparedEntityApplyPolicy::PublicVisible(policy) = prepare_entity_apply_policy(
            &public_landmark,
            false,
            None,
            None,
            Some(Vec3::ZERO),
            6.0,
            None,
            None,
            None,
        ) else {
            panic!("expected public-visible policy");
        };
        assert!(matches!(
            policy
                .landmark_delivery
                .as_ref()
                .map(|landmark| landmark.visibility_policy),
            Some(PreparedLandmarkVisibilityPolicy::PlayerDiscovered(_))
        ));
        assert_eq!(policy.common.authorization_extent_m, 6.0);

        let faction_visible = CachedVisibilityEntity {
            faction_visibility: true,
            faction_id: Some("alpha".to_string()),
            ..Default::default()
        };
        let PreparedEntityApplyPolicy::FactionVisible(policy) = prepare_entity_apply_policy(
            &faction_visible,
            false,
            None,
            None,
            Some(Vec3::ZERO),
            8.0,
            None,
            None,
            None,
        ) else {
            panic!("expected faction-visible policy");
        };
        assert_eq!(policy.entity_faction_id.as_deref(), Some("alpha"));
        assert!(policy.landmark_delivery.is_none());

        let discovered_landmark_guid = uuid::Uuid::new_v4();
        let discovered_landmark = CachedVisibilityEntity {
            guid: Some(discovered_landmark_guid),
            static_landmark: Some(StaticLandmark {
                kind: "Landmark".to_string(),
                discoverable: true,
                always_known: false,
                discovery_radius_m: None,
                use_extent_for_discovery: false,
            }),
            ..Default::default()
        };
        let PreparedEntityApplyPolicy::DiscoveredLandmark(policy) = prepare_entity_apply_policy(
            &discovered_landmark,
            false,
            None,
            None,
            Some(Vec3::ZERO),
            5.0,
            None,
            None,
            None,
        ) else {
            panic!("expected discovered-landmark policy");
        };
        assert!(matches!(
            policy.landmark_delivery.visibility_policy,
            PreparedLandmarkVisibilityPolicy::PlayerDiscovered(guid)
                if guid == discovered_landmark_guid
        ));

        let range_checked = CachedVisibilityEntity::default();
        assert!(matches!(
            prepare_entity_apply_policy(
                &range_checked,
                false,
                None,
                None,
                Some(Vec3::ZERO),
                3.0,
                None,
                None,
                None,
            ),
            PreparedEntityApplyPolicy::RangeChecked(_)
        ));
    }

    #[test]
    fn prepared_policy_evaluation_preserves_specialized_authorization_paths() {
        let public_policy = prepare_entity_apply_policy(
            &CachedVisibilityEntity {
                public_visibility: true,
                ..Default::default()
            },
            false,
            None,
            None,
            Some(Vec3::new(100.0, 0.0, 0.0)),
            0.0,
            None,
            None,
            None,
        );
        let public_client =
            cached_client_visibility_context("player-public", None, 300.0, Vec::new(), []);
        let public_visibility_context =
            PlayerVisibilityContextRef::from_cached_client_context(&public_client);
        let public_eval = evaluate_prepared_entity_policy_for_client(
            &public_policy,
            &public_client,
            &public_visibility_context,
            false,
        );
        assert_eq!(
            public_eval.authorization,
            Some(VisibilityAuthorization::Public)
        );
        assert!(public_eval.bypass_candidate);
        assert!(public_eval.should_be_visible);

        let faction_policy = prepare_entity_apply_policy(
            &CachedVisibilityEntity {
                faction_visibility: true,
                faction_id: Some("alpha".to_string()),
                ..Default::default()
            },
            false,
            None,
            None,
            Some(Vec3::new(100.0, 0.0, 0.0)),
            0.0,
            None,
            None,
            None,
        );
        let faction_client = cached_client_visibility_context(
            "player-faction",
            Some("alpha"),
            300.0,
            Vec::new(),
            [],
        );
        let faction_visibility_context =
            PlayerVisibilityContextRef::from_cached_client_context(&faction_client);
        let faction_eval = evaluate_prepared_entity_policy_for_client(
            &faction_policy,
            &faction_client,
            &faction_visibility_context,
            false,
        );
        assert_eq!(
            faction_eval.authorization,
            Some(VisibilityAuthorization::Faction)
        );
        assert!(faction_eval.should_be_visible);

        let other_faction_client = cached_client_visibility_context(
            "player-faction-other",
            Some("beta"),
            300.0,
            Vec::new(),
            [],
        );
        let other_faction_visibility_context =
            PlayerVisibilityContextRef::from_cached_client_context(&other_faction_client);
        let other_faction_eval = evaluate_prepared_entity_policy_for_client(
            &faction_policy,
            &other_faction_client,
            &other_faction_visibility_context,
            false,
        );
        assert_eq!(other_faction_eval.authorization, None);
        assert!(!other_faction_eval.bypass_candidate);
        assert!(!other_faction_eval.should_be_visible);

        let discovered_landmark_guid = uuid::Uuid::new_v4();
        let discovered_policy = prepare_entity_apply_policy(
            &CachedVisibilityEntity {
                guid: Some(discovered_landmark_guid),
                static_landmark: Some(StaticLandmark {
                    kind: "Landmark".to_string(),
                    discoverable: true,
                    always_known: false,
                    discovery_radius_m: None,
                    use_extent_for_discovery: false,
                }),
                ..Default::default()
            },
            false,
            None,
            None,
            Some(Vec3::new(100.0, 0.0, 0.0)),
            0.0,
            None,
            None,
            None,
        );
        let discovered_client = cached_client_visibility_context(
            "player-discovered",
            None,
            300.0,
            Vec::new(),
            [discovered_landmark_guid],
        );
        let discovered_visibility_context =
            PlayerVisibilityContextRef::from_cached_client_context(&discovered_client);
        let discovered_eval = evaluate_prepared_entity_policy_for_client(
            &discovered_policy,
            &discovered_client,
            &discovered_visibility_context,
            false,
        );
        assert_eq!(
            discovered_eval.authorization,
            Some(VisibilityAuthorization::DiscoveredStaticLandmark)
        );
        assert!(discovered_eval.should_be_visible);

        let undiscovered_client =
            cached_client_visibility_context("player-undiscovered", None, 300.0, Vec::new(), []);
        let undiscovered_visibility_context =
            PlayerVisibilityContextRef::from_cached_client_context(&undiscovered_client);
        let undiscovered_eval = evaluate_prepared_entity_policy_for_client(
            &discovered_policy,
            &undiscovered_client,
            &undiscovered_visibility_context,
            false,
        );
        assert_eq!(undiscovered_eval.authorization, None);
        assert!(!undiscovered_eval.should_be_visible);

        let range_policy = prepare_entity_apply_policy(
            &CachedVisibilityEntity::default(),
            false,
            None,
            None,
            Some(Vec3::new(150.0, 0.0, 0.0)),
            10.0,
            None,
            None,
            None,
        );
        let range_client = cached_client_visibility_context(
            "player-range",
            None,
            300.0,
            vec![(Vec3::ZERO, 200.0)],
            [],
        );
        let range_visibility_context =
            PlayerVisibilityContextRef::from_cached_client_context(&range_client);
        let range_eval = evaluate_prepared_entity_policy_for_client(
            &range_policy,
            &range_client,
            &range_visibility_context,
            false,
        );
        assert_eq!(
            range_eval.authorization,
            Some(VisibilityAuthorization::Range)
        );
        assert!(range_eval.should_be_visible);
    }

    #[test]
    fn membership_diff_applies_only_changes() {
        let client_a = Entity::from_raw_u32(1).expect("valid entity id");
        let client_b = Entity::from_raw_u32(2).expect("valid entity id");
        let mut replication_state = ReplicationState::default();
        let mut current_visible_clients = HashSet::from([client_a]);
        replication_state.gain_visibility(client_a);
        let desired_visible_clients = HashSet::from([client_b]);
        let mut visible_gains = 0usize;
        let mut visible_losses = 0usize;

        let gained_count = apply_visibility_membership_diff(
            &mut replication_state,
            &mut current_visible_clients,
            &desired_visible_clients,
            &mut visible_gains,
            &mut visible_losses,
        );
        assert_eq!(gained_count, 1);

        assert_eq!(visible_gains, 1);
        assert_eq!(visible_losses, 1);
        assert!(replication_state.is_visible(client_b));
        assert!(!replication_state.is_visible(client_a));
        assert_eq!(current_visible_clients, desired_visible_clients);
    }

    #[test]
    fn spatial_index_rebuild_check_accepts_matching_cached_entry() {
        let entity = Entity::from_raw_u32(11).expect("valid entity id");
        let guid = uuid::Uuid::new_v4();
        let mut index = VisibilitySpatialIndex::default();
        index.entity_by_guid.insert(guid, entity);
        index.world_position_by_entity.insert(entity, Vec3::ZERO);
        index.base_extent_m_by_entity.insert(entity, 5.0);
        index
            .visibility_position_by_entity
            .insert(entity, Vec3::ZERO);
        index.visibility_extent_m_by_entity.insert(entity, 5.0);
        index.root_entity_by_entity.insert(entity, entity);
        index.cell_by_entity.insert(entity, (0, 0));
        index.entities_by_cell.insert((0, 0), vec![entity]);
        index
            .entities_by_root
            .entry(entity)
            .or_default()
            .insert(entity);

        let mut cache = VisibilityEntityCache::default();
        cache.by_entity.insert(
            entity,
            CachedVisibilityEntity {
                guid: Some(guid),
                entity_extent_m: 5.0,
                ..Default::default()
            },
        );

        assert!(!spatial_index_requires_full_rebuild(&index, &cache, entity));
    }

    #[test]
    fn spatial_index_rebuild_check_rejects_missing_index_entry() {
        let entity = Entity::from_raw_u32(12).expect("valid entity id");
        let guid = uuid::Uuid::new_v4();
        let mut cache = VisibilityEntityCache::default();
        cache.by_entity.insert(
            entity,
            CachedVisibilityEntity {
                guid: Some(guid),
                entity_extent_m: 5.0,
                ..Default::default()
            },
        );

        assert!(spatial_index_requires_full_rebuild(
            &VisibilitySpatialIndex::default(),
            &cache,
            entity,
        ));
    }

    #[test]
    fn resolved_parent_guid_prefers_mounted_on() {
        let mounted_parent = uuid::Uuid::new_v4();
        let fallback_parent = uuid::Uuid::new_v4();
        assert_eq!(
            resolved_parent_guid(
                Some(&MountedOn {
                    parent_entity_id: mounted_parent,
                    hardpoint_id: "test-hardpoint".to_string(),
                }),
                Some(&ParentGuid(fallback_parent))
            ),
            Some(mounted_parent)
        );
    }

    #[test]
    fn fullscreen_phase_runtime_layer_is_global_render_config() {
        let definition = RuntimeRenderLayerDefinition {
            layer_id: "bg_starfield".to_string(),
            phase: RENDER_PHASE_FULLSCREEN_BACKGROUND.to_string(),
            material_domain: RENDER_DOMAIN_FULLSCREEN.to_string(),
            enabled: true,
            ..default()
        };
        assert!(is_global_render_config_entity(false, Some(&definition)));
        assert!(is_global_render_config_entity(true, None));
    }

    #[test]
    fn visibility_world_position_prefers_static_world_position_over_stale_global_transform() {
        let world_position = WorldPosition(Vec2::new(8000.0, 0.0).into());
        let stale_global = GlobalTransform::from_translation(Vec3::ZERO);

        let resolved =
            replicated_visibility_world_position(None, Some(&world_position), &stale_global);

        assert_eq!(resolved, Vec3::new(8000.0, 0.0, 0.0));
    }

    #[test]
    fn landmark_signal_signature_extends_discovery_overlap() {
        let landmark = StaticLandmark {
            kind: "Planet".to_string(),
            discoverable: true,
            always_known: false,
            discovery_radius_m: None,
            use_extent_for_discovery: false,
        };
        let signal = SignalSignature {
            strength: 1.0,
            detection_radius_m: 1000.0,
            use_extent_for_detection: false,
        };
        let context = cached_client_visibility_context(
            "player-a",
            None,
            300.0,
            vec![(Vec3::ZERO, 300.0)],
            [],
        );
        let visibility_context = PlayerVisibilityContextRef::from_cached_client_context(&context);

        assert_eq!(
            landmark_discovery_cause(
                Some(Vec3::new(1200.0, 0.0, 0.0)),
                0.0,
                &landmark,
                None,
                &visibility_context,
            ),
            None
        );
        assert_eq!(
            landmark_discovery_cause(
                Some(Vec3::new(1200.0, 0.0, 0.0)),
                0.0,
                &landmark,
                Some(&signal),
                &visibility_context,
            ),
            Some(LandmarkDiscoveryCause::Signal)
        );
    }

    #[test]
    fn landmark_signal_discovery_padding_uses_extent_once() {
        let landmark = StaticLandmark {
            kind: "Planet".to_string(),
            discoverable: true,
            always_known: false,
            discovery_radius_m: Some(200.0),
            use_extent_for_discovery: true,
        };
        let signal = SignalSignature {
            strength: 1.0,
            detection_radius_m: 1000.0,
            use_extent_for_detection: true,
        };

        assert_eq!(
            static_landmark_discovery_padding_m(500.0, &landmark, Some(&signal)),
            1700.0
        );
    }

    #[test]
    fn landmark_discovery_notification_uses_landmark_payload() {
        let landmark_id = uuid::Uuid::new_v4();
        let command = landmark_discovery_notification_command(
            "11111111-1111-1111-1111-111111111111",
            landmark_id,
            "Aurelia".to_string(),
            "Planet".to_string(),
            Some("map_icon_planet_svg".to_string()),
            Some([8000.0, 0.0]),
        );

        assert_eq!(command.title, "Landmark Discovered");
        assert_eq!(command.body, "Aurelia");
        assert_eq!(command.severity, NotificationSeverity::Info);
        assert_eq!(command.placement, NotificationPlacement::BottomRight);
        assert!(matches!(
            command.payload,
            NotificationPayload::LandmarkDiscovery {
                entity_guid,
                display_name,
                landmark_kind,
                map_icon_asset_id: Some(_),
                world_position_xy: Some(_),
            } if entity_guid == landmark_id.to_string()
                && display_name == "Aurelia"
                && landmark_kind == "Planet"
        ));
    }
}
