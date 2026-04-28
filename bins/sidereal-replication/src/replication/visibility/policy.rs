#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn is_entity_visible_to_player(
    player_entity_id: &str,
    owner_player_id: Option<&str>,
    is_public_visibility: bool,
    is_faction_visibility: bool,
    is_discovered_static_landmark: bool,
    entity_faction_id: Option<&str>,
    entity_position: Option<Vec3>,
    entity_extent_m: f32,
    visibility_context: &PlayerVisibilityContextRef<'_>,
    delivery_range_m: f32,
    owner_bypasses_delivery_scope: bool,
) -> bool {
    // Safety check for mismatched context call-site.
    if visibility_context.player_entity_id != player_entity_id {
        return false;
    }

    let authorization = authorize_visibility(
        player_entity_id,
        owner_player_id,
        is_public_visibility,
        is_faction_visibility,
        is_discovered_static_landmark,
        entity_faction_id,
        entity_position,
        entity_extent_m,
        visibility_context,
    );
    if authorization.is_none() {
        return false;
    }

    if owner_bypasses_delivery_scope
        && matches!(authorization, Some(VisibilityAuthorization::Owner))
    {
        return true;
    }

    passes_delivery_scope(
        entity_position,
        entity_extent_m,
        visibility_context,
        delivery_range_m,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VisibilityAuthorization {
    Owner,
    Public,
    Faction,
    DiscoveredStaticLandmark,
    Range,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct VisibilityEvaluation {
    authorization: Option<VisibilityAuthorization>,
    bypass_candidate: bool,
    delivery_ok: bool,
    should_be_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreparedLandmarkVisibilityPolicy {
    None,
    AlwaysKnown,
    PlayerDiscovered(uuid::Uuid),
}

#[derive(Debug, Clone, PartialEq)]
enum PreparedEntityApplyPolicy {
    OwnerOnlyAnchor { owner_player_id: Option<String> },
    GlobalVisible,
    PublicVisible(PreparedPublicEntityApplyPolicy),
    FactionVisible(PreparedFactionEntityApplyPolicy),
    DiscoveredLandmark(PreparedDiscoveredLandmarkApplyPolicy),
    RangeChecked(PreparedRangeCheckedEntityApplyPolicy),
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedConditionalEntityApplyCommon {
    owner_player_id: Option<String>,
    entity_position: Option<Vec3>,
    authorization_extent_m: f32,
    controlled_owner_client: Option<Entity>,
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedLandmarkDeliveryPolicy {
    visibility_policy: PreparedLandmarkVisibilityPolicy,
    discovered_extent_m: f32,
    discovered_delivery_scale: f32,
}

impl PreparedLandmarkDeliveryPolicy {
    fn is_discovered_for_client(&self, client_context: &CachedClientVisibilityContext) -> bool {
        match self.visibility_policy {
            PreparedLandmarkVisibilityPolicy::None => false,
            PreparedLandmarkVisibilityPolicy::AlwaysKnown => true,
            PreparedLandmarkVisibilityPolicy::PlayerDiscovered(guid) => {
                client_context.discovered_static_landmarks.contains(&guid)
            }
        }
    }

    fn delivery_profile_for_client(
        &self,
        client_context: &CachedClientVisibilityContext,
        default_extent_m: f32,
        default_delivery_range_m: f32,
    ) -> (bool, f32, f32) {
        if self.is_discovered_for_client(client_context) {
            (
                true,
                self.discovered_extent_m,
                default_delivery_range_m * self.discovered_delivery_scale,
            )
        } else {
            (false, default_extent_m, default_delivery_range_m)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedPublicEntityApplyPolicy {
    common: PreparedConditionalEntityApplyCommon,
    landmark_delivery: Option<PreparedLandmarkDeliveryPolicy>,
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedFactionEntityApplyPolicy {
    common: PreparedConditionalEntityApplyCommon,
    entity_faction_id: Option<String>,
    landmark_delivery: Option<PreparedLandmarkDeliveryPolicy>,
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedDiscoveredLandmarkApplyPolicy {
    common: PreparedConditionalEntityApplyCommon,
    landmark_delivery: PreparedLandmarkDeliveryPolicy,
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedRangeCheckedEntityApplyPolicy {
    common: PreparedConditionalEntityApplyCommon,
}

impl PreparedEntityApplyPolicy {
    fn owner_player_id(&self) -> Option<&str> {
        match self {
            Self::OwnerOnlyAnchor { owner_player_id } => owner_player_id.as_deref(),
            Self::GlobalVisible => None,
            Self::PublicVisible(policy) => policy.common.owner_player_id.as_deref(),
            Self::FactionVisible(policy) => policy.common.owner_player_id.as_deref(),
            Self::DiscoveredLandmark(policy) => policy.common.owner_player_id.as_deref(),
            Self::RangeChecked(policy) => policy.common.owner_player_id.as_deref(),
        }
    }

    fn controlled_owner_client(&self) -> Option<Entity> {
        match self {
            Self::OwnerOnlyAnchor { .. } | Self::GlobalVisible => None,
            Self::PublicVisible(policy) => policy.common.controlled_owner_client,
            Self::FactionVisible(policy) => policy.common.controlled_owner_client,
            Self::DiscoveredLandmark(policy) => policy.common.controlled_owner_client,
            Self::RangeChecked(policy) => policy.common.controlled_owner_client,
        }
    }

    fn entity_position(&self) -> Option<Vec3> {
        match self {
            Self::OwnerOnlyAnchor { .. } | Self::GlobalVisible => None,
            Self::PublicVisible(policy) => policy.common.entity_position,
            Self::FactionVisible(policy) => policy.common.entity_position,
            Self::DiscoveredLandmark(policy) => policy.common.entity_position,
            Self::RangeChecked(policy) => policy.common.entity_position,
        }
    }

    fn is_public_visibility(&self) -> bool {
        matches!(self, Self::PublicVisible(_))
    }

    fn is_faction_visibility(&self) -> bool {
        matches!(self, Self::FactionVisible(_))
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_entity_apply_policy(
    cached: &CachedVisibilityEntity,
    root_public: bool,
    root_owner_player_id: Option<&String>,
    root_faction_id: Option<&String>,
    entity_position: Option<Vec3>,
    entity_extent_m: f32,
    resolved_world_layer: Option<&RuntimeRenderLayerDefinition>,
    runtime_world_visual_stack: Option<&RuntimeWorldVisualStack>,
    controlled_by: Option<&ControlledBy>,
) -> PreparedEntityApplyPolicy {
    // Keep entity policy preparation outside the per-client apply loop. If future
    // work adds more visibility fast paths, extend these prepared buckets rather
    // than pushing more root/public/faction/landmark branching back into the hot loop.
    let is_public = cached.public_visibility || root_public;
    let mut owner_player_id = cached
        .owner_player_id
        .clone()
        .or_else(|| root_owner_player_id.cloned());
    let entity_faction_id = cached
        .faction_id
        .clone()
        .or_else(|| root_faction_id.cloned());
    if cached.is_player_tag {
        if owner_player_id.is_none() {
            owner_player_id = cached.guid.map(|guid| guid.to_string());
        }
        return PreparedEntityApplyPolicy::OwnerOnlyAnchor { owner_player_id };
    }
    if cached.is_global_render_config {
        return PreparedEntityApplyPolicy::GlobalVisible;
    }
    let landmark_policy = match (cached.static_landmark.as_ref(), cached.guid) {
        (Some(landmark), _) if landmark.always_known => {
            PreparedLandmarkVisibilityPolicy::AlwaysKnown
        }
        (Some(_), Some(guid)) => PreparedLandmarkVisibilityPolicy::PlayerDiscovered(guid),
        _ => PreparedLandmarkVisibilityPolicy::None,
    };
    let common = PreparedConditionalEntityApplyCommon {
        owner_player_id,
        entity_position,
        authorization_extent_m: entity_extent_m,
        controlled_owner_client: controlled_by.map(|binding| binding.owner),
    };
    let landmark_delivery = (!matches!(landmark_policy, PreparedLandmarkVisibilityPolicy::None))
        .then(|| PreparedLandmarkDeliveryPolicy {
            visibility_policy: landmark_policy,
            discovered_extent_m: effective_discovered_landmark_extent_m(
                entity_extent_m,
                resolved_world_layer,
                runtime_world_visual_stack,
            ),
            discovered_delivery_scale: 1.0 / runtime_layer_parallax_factor(resolved_world_layer),
        });

    if is_public {
        return PreparedEntityApplyPolicy::PublicVisible(PreparedPublicEntityApplyPolicy {
            common,
            landmark_delivery,
        });
    }
    if cached.faction_visibility {
        return PreparedEntityApplyPolicy::FactionVisible(PreparedFactionEntityApplyPolicy {
            common,
            entity_faction_id,
            landmark_delivery,
        });
    }
    if let Some(landmark_delivery) = landmark_delivery {
        return PreparedEntityApplyPolicy::DiscoveredLandmark(
            PreparedDiscoveredLandmarkApplyPolicy {
                common,
                landmark_delivery,
            },
        );
    }
    PreparedEntityApplyPolicy::RangeChecked(PreparedRangeCheckedEntityApplyPolicy { common })
}

fn evaluate_prepared_entity_policy_for_client(
    prepared_policy: &PreparedEntityApplyPolicy,
    client_context: &CachedClientVisibilityContext,
    visibility_context: &PlayerVisibilityContextRef<'_>,
    owner_bypasses_delivery_scope: bool,
) -> VisibilityEvaluation {
    match prepared_policy {
        PreparedEntityApplyPolicy::OwnerOnlyAnchor { .. }
        | PreparedEntityApplyPolicy::GlobalVisible => VisibilityEvaluation {
            authorization: None,
            bypass_candidate: false,
            delivery_ok: false,
            should_be_visible: false,
        },
        PreparedEntityApplyPolicy::PublicVisible(policy) => {
            let authorization = authorize_owner_visibility(
                visibility_context.player_entity_id,
                policy.common.owner_player_id.as_deref(),
            )
            .or(Some(VisibilityAuthorization::Public));
            let (_, delivery_extent_m, delivery_range_m) = policy
                .landmark_delivery
                .as_ref()
                .map(|landmark| {
                    landmark.delivery_profile_for_client(
                        client_context,
                        policy.common.authorization_extent_m,
                        client_context.delivery_range_m,
                    )
                })
                .unwrap_or((
                    false,
                    policy.common.authorization_extent_m,
                    client_context.delivery_range_m,
                ));
            finalize_visibility_evaluation(
                authorization,
                policy.common.entity_position,
                delivery_extent_m,
                visibility_context,
                delivery_range_m,
                owner_bypasses_delivery_scope,
            )
        }
        PreparedEntityApplyPolicy::FactionVisible(policy) => {
            let authorization = authorize_owner_visibility(
                visibility_context.player_entity_id,
                policy.common.owner_player_id.as_deref(),
            )
            .or_else(|| {
                authorize_faction_visibility(
                    policy.entity_faction_id.as_deref(),
                    visibility_context,
                )
            });
            let (_, delivery_extent_m, delivery_range_m) = policy
                .landmark_delivery
                .as_ref()
                .map(|landmark| {
                    landmark.delivery_profile_for_client(
                        client_context,
                        policy.common.authorization_extent_m,
                        client_context.delivery_range_m,
                    )
                })
                .unwrap_or((
                    false,
                    policy.common.authorization_extent_m,
                    client_context.delivery_range_m,
                ));
            finalize_visibility_evaluation(
                authorization,
                policy.common.entity_position,
                delivery_extent_m,
                visibility_context,
                delivery_range_m,
                owner_bypasses_delivery_scope,
            )
        }
        PreparedEntityApplyPolicy::DiscoveredLandmark(policy) => {
            let (is_discovered_static_landmark, delivery_extent_m, delivery_range_m) =
                policy.landmark_delivery.delivery_profile_for_client(
                    client_context,
                    policy.common.authorization_extent_m,
                    client_context.delivery_range_m,
                );
            let authorization = authorize_owner_visibility(
                visibility_context.player_entity_id,
                policy.common.owner_player_id.as_deref(),
            )
            .or_else(|| authorize_discovered_landmark_visibility(is_discovered_static_landmark));
            finalize_visibility_evaluation(
                authorization,
                policy.common.entity_position,
                delivery_extent_m,
                visibility_context,
                delivery_range_m,
                owner_bypasses_delivery_scope,
            )
        }
        PreparedEntityApplyPolicy::RangeChecked(policy) => {
            let authorization = authorize_owner_visibility(
                visibility_context.player_entity_id,
                policy.common.owner_player_id.as_deref(),
            )
            .or_else(|| {
                authorize_range_visibility(
                    policy.common.entity_position,
                    policy.common.authorization_extent_m,
                    visibility_context,
                )
            });
            finalize_visibility_evaluation(
                authorization,
                policy.common.entity_position,
                policy.common.authorization_extent_m,
                visibility_context,
                client_context.delivery_range_m,
                owner_bypasses_delivery_scope,
            )
        }
    }
}

fn finalize_visibility_evaluation(
    authorization: Option<VisibilityAuthorization>,
    entity_position: Option<Vec3>,
    delivery_extent_m: f32,
    visibility_context: &PlayerVisibilityContextRef<'_>,
    delivery_range_m: f32,
    owner_bypasses_delivery_scope: bool,
) -> VisibilityEvaluation {
    let delivery_ok = authorization.is_some_and(|authorization| {
        if owner_bypasses_delivery_scope && matches!(authorization, VisibilityAuthorization::Owner)
        {
            return true;
        }
        passes_delivery_scope(
            entity_position,
            delivery_extent_m,
            visibility_context,
            delivery_range_m,
        )
    });
    VisibilityEvaluation {
        authorization,
        bypass_candidate: authorization.is_some(),
        delivery_ok,
        should_be_visible: authorization.is_some() && delivery_ok,
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(test), allow(dead_code))]
fn evaluate_visibility_for_client(
    player_entity_id: &str,
    owner_player_id: Option<&str>,
    is_public_visibility: bool,
    is_faction_visibility: bool,
    is_discovered_static_landmark: bool,
    entity_faction_id: Option<&str>,
    entity_position: Option<Vec3>,
    authorization_extent_m: f32,
    delivery_extent_m: f32,
    visibility_context: &PlayerVisibilityContextRef<'_>,
    delivery_range_m: f32,
    owner_bypasses_delivery_scope: bool,
) -> VisibilityEvaluation {
    let authorization = authorize_visibility(
        player_entity_id,
        owner_player_id,
        is_public_visibility,
        is_faction_visibility,
        is_discovered_static_landmark,
        entity_faction_id,
        entity_position,
        authorization_extent_m,
        visibility_context,
    );
    finalize_visibility_evaluation(
        authorization,
        entity_position,
        delivery_extent_m,
        visibility_context,
        delivery_range_m,
        owner_bypasses_delivery_scope,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn authorize_visibility(
    player_entity_id: &str,
    owner_player_id: Option<&str>,
    is_public_visibility: bool,
    is_faction_visibility: bool,
    is_discovered_static_landmark: bool,
    entity_faction_id: Option<&str>,
    entity_position: Option<Vec3>,
    entity_extent_m: f32,
    visibility_context: &PlayerVisibilityContextRef<'_>,
) -> Option<VisibilityAuthorization> {
    // Ownership/public/faction are policy exceptions and must be evaluated
    // before any spatial delivery narrowing.
    if let Some(authorization) = authorize_owner_visibility(player_entity_id, owner_player_id) {
        return Some(authorization);
    }
    if is_faction_visibility
        && let Some(authorization) =
            authorize_faction_visibility(entity_faction_id, visibility_context)
    {
        return Some(authorization);
    }
    if is_public_visibility {
        return Some(VisibilityAuthorization::Public);
    }
    if let Some(authorization) =
        authorize_discovered_landmark_visibility(is_discovered_static_landmark)
    {
        return Some(authorization);
    }
    authorize_range_visibility(entity_position, entity_extent_m, visibility_context)
}

fn authorize_owner_visibility(
    player_entity_id: &str,
    owner_player_id: Option<&str>,
) -> Option<VisibilityAuthorization> {
    owner_player_id
        .is_some_and(|owner| owner == player_entity_id)
        .then_some(VisibilityAuthorization::Owner)
}

fn authorize_faction_visibility(
    entity_faction_id: Option<&str>,
    visibility_context: &PlayerVisibilityContextRef<'_>,
) -> Option<VisibilityAuthorization> {
    visibility_context
        .player_faction_id
        .zip(entity_faction_id)
        .is_some_and(|(player_faction, entity_faction)| player_faction == entity_faction)
        .then_some(VisibilityAuthorization::Faction)
}

fn authorize_discovered_landmark_visibility(
    is_discovered_static_landmark: bool,
) -> Option<VisibilityAuthorization> {
    is_discovered_static_landmark.then_some(VisibilityAuthorization::DiscoveredStaticLandmark)
}

fn authorize_range_visibility(
    entity_position: Option<Vec3>,
    entity_extent_m: f32,
    visibility_context: &PlayerVisibilityContextRef<'_>,
) -> Option<VisibilityAuthorization> {
    let target_position = entity_position?;
    visibility_context
        .visibility_sources
        .iter()
        .find(|(visibility_pos, visibility_range_m)| {
            (target_position - *visibility_pos).length() <= *visibility_range_m + entity_extent_m
        })
        .map(|_| VisibilityAuthorization::Range)
}

fn passes_delivery_scope(
    entity_position: Option<Vec3>,
    entity_extent_m: f32,
    visibility_context: &PlayerVisibilityContextRef<'_>,
    delivery_range_m: f32,
) -> bool {
    let (Some(observer_anchor_position), Some(target_position)) =
        (visibility_context.observer_anchor_position, entity_position)
    else {
        return false;
    };
    (target_position - observer_anchor_position).length() <= delivery_range_m + entity_extent_m
}

