use avian2d::prelude::{AngularInertia, AngularVelocity, LinearVelocity, Mass, Position, Rotation};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{
    ControlledBy, MessageReceiver, NetworkVisibility, Replicate, ReplicationState,
};
use sidereal_game::{
    DiscoveredStaticLandmarks, DisplayName, EntityGuid, FactionId, FactionVisibility,
    FullscreenLayer, MapIcon, MountedOn, OwnerId, ParentGuid, PlayerTag, PublicVisibility,
    RENDER_DOMAIN_FULLSCREEN, RENDER_PHASE_FULLSCREEN_BACKGROUND,
    RENDER_PHASE_FULLSCREEN_FOREGROUND, RuntimeRenderLayerDefinition, RuntimeRenderLayerOverride,
    RuntimeWorldVisualStack, SignalSignature, SizeM, StaticLandmark, VisibilityDisclosure,
    VisibilityGridCell, VisibilityRangeM, VisibilityRangeSource, VisibilitySpatialGrid,
    WorldPosition, default_main_world_render_layer,
};
use sidereal_net::{
    ClientLocalViewMode, ClientLocalViewModeMessage, NotificationPayload, NotificationPlacement,
    NotificationSeverity, PlayerEntityId,
};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::time::Instant;

use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::debug_env;
use crate::replication::lifecycle::ClientLastActivity;
use crate::replication::notifications::{
    NotificationCommand, NotificationCommandQueue, enqueue_player_notification,
};
use crate::replication::{PlayerRuntimeEntityMap, SimulatedControlledEntity};

pub const DEFAULT_VIEW_RANGE_M: f32 = 300.0;
pub const DEFAULT_DELIVERY_RANGE_MAX_M: f32 = 50_000.0;
const DEFAULT_VISIBILITY_CELL_SIZE_M: f32 = 2000.0;
const DEFAULT_LANDMARK_DISCOVERY_INTERVAL_S: f64 = 0.25;
const CLIENT_DELIVERY_RANGE_MIN_M: f32 = 1.0;

fn canonical_player_entity_id(id: &str) -> String {
    sidereal_net::PlayerEntityId::parse(id)
        .map(sidereal_net::PlayerEntityId::canonical_wire_id)
        .unwrap_or_else(|| id.to_string())
}

fn parse_delivery_range_m(raw: Option<&str>) -> Option<f32> {
    raw.and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn delivery_range_m_from_env() -> f32 {
    parse_delivery_range_m(
        std::env::var("SIDEREAL_VISIBILITY_DELIVERY_RANGE_M")
            .ok()
            .as_deref(),
    )
    .unwrap_or(DEFAULT_VIEW_RANGE_M)
}

fn delivery_range_max_m_from_env() -> f32 {
    parse_delivery_range_m(
        std::env::var("REPLICATION_VISIBILITY_DELIVERY_RANGE_MAX_M")
            .ok()
            .as_deref(),
    )
    .unwrap_or(DEFAULT_DELIVERY_RANGE_MAX_M)
}

fn parse_cell_size_m(raw: Option<&str>) -> Option<f32> {
    raw.and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite() && *value >= 50.0)
}

fn cell_size_m_from_env() -> f32 {
    parse_cell_size_m(
        std::env::var("SIDEREAL_VISIBILITY_CELL_SIZE_M")
            .ok()
            .as_deref(),
    )
    .unwrap_or(DEFAULT_VISIBILITY_CELL_SIZE_M)
}

fn bypass_all_visibility_filters_from_env() -> bool {
    if !cfg!(test) {
        return false;
    }
    std::env::var("SIDEREAL_VISIBILITY_BYPASS_ALL")
        .ok()
        .is_some_and(|raw| {
            let normalized = raw.trim().to_ascii_lowercase();
            normalized == "1" || normalized == "true" || normalized == "on"
        })
}

