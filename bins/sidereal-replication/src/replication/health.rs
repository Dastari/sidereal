use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::replication::SimulatedControlledEntity;
use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::input::ClientInputDropMetrics;
use crate::replication::lifecycle::ClientLastActivity;
use crate::replication::persistence::PersistenceWorkerState;
use crate::replication::runtime_scripting::ScriptRuntimeMetrics;
use crate::replication::simulation_entities::PlayerControlledEntityMap;
use crate::replication::visibility::VisibilityRuntimeMetrics;
use avian2d::prelude::Position;
use avian2d::prelude::RigidBody;
use axum::{Json, Router, extract::State as AxumState, http::StatusCode, routing::get};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use lightyear::prelude::Link;
use lightyear::prelude::client::Connected;
use lightyear::prelude::server::ClientOf;
use serde::Serialize;
use sidereal_game::{
    BallisticProjectile, ControlledEntityGuid, DisplayName, EntityGuid, EntityLabels, MapIcon,
    PlayerTag, ScriptState, ShipTag, SizeM, StaticLandmark, WorldPosition,
};

const DEFAULT_HEALTH_BIND: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 15716);

#[derive(Debug, Clone, Resource)]
pub struct ReplicationHealthServerConfig {
    pub bind_addr: SocketAddr,
}

impl Default for ReplicationHealthServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: DEFAULT_HEALTH_BIND,
        }
    }
}

#[derive(Debug, Clone, Resource)]
pub struct DiagnosticsSnapshotCadence {
    pub health_interval_s: f64,
    pub world_interval_s: f64,
}

impl Default for DiagnosticsSnapshotCadence {
    fn default() -> Self {
        let parse_interval = |name: &str, default_hz: f64| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|hz| hz.is_finite() && *hz > 0.0)
                .map(|hz| 1.0 / hz)
                .unwrap_or(1.0 / default_hz)
        };
        Self {
            health_interval_s: parse_interval("REPLICATION_HEALTH_SNAPSHOT_HZ", 2.0),
            world_interval_s: parse_interval("REPLICATION_WORLD_SNAPSHOT_HZ", 5.0),
        }
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct DiagnosticsSnapshotState {
    pub last_health_at_s: Option<f64>,
    pub last_world_map_at_s: Option<f64>,
    pub last_world_explorer_at_s: Option<f64>,
}

#[derive(Debug, Clone, Default, Resource, Serialize)]
pub struct LuaRuntimeHealthSnapshot {
    pub memory_limit_bytes: u64,
    pub current_memory_bytes: Option<u64>,
    pub interval_runs: u64,
    pub event_runs: u64,
    pub error_count: u64,
    pub reload_count: u64,
    pub last_interval_run_ms: Option<f64>,
    pub last_event_run_ms: Option<f64>,
}

#[derive(Debug, Clone, Default, Resource, Serialize)]
pub struct ReplicationHealthSnapshot {
    pub status: String,
    pub generated_at_unix_ms: u128,
    pub uptime_seconds: u64,
    pub session_count: usize,
    pub users_online: usize,
    pub clients_with_recent_activity: usize,
    pub world_entity_count: usize,
    pub physics_body_count: usize,
    pub scripted_entity_count: usize,
    pub controlled_entity_count: usize,
    pub input_accepted_total: u64,
    pub input_drop_total: u64,
    pub visibility_query_ms: f64,
    pub visibility_apply_ms: f64,
    pub visibility_clients: usize,
    pub visibility_entities: usize,
    pub persistence_enqueued_batches: u64,
    pub persistence_queue_full_events: u64,
    pub persistence_disconnected_events: u64,
    pub persistence_pending_latest: bool,
    pub lua_runtime: LuaRuntimeHealthSnapshot,
}

#[derive(Clone, Default, Resource)]
pub struct SharedHealthSnapshot {
    inner: Arc<RwLock<ReplicationHealthSnapshot>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldMapEntitySnapshot {
    pub guid: String,
    pub display_name: Option<String>,
    pub x: f32,
    pub y: f32,
    pub glyph: char,
    pub color_rgb: (u8, u8, u8),
    pub extent_m: f32,
}

#[derive(Debug, Clone, Default, Resource, Serialize)]
pub struct WorldMapSnapshot {
    pub generated_at_unix_ms: u128,
    pub entities: Vec<WorldMapEntitySnapshot>,
}

#[derive(Clone, Default, Resource)]
pub struct SharedWorldMapSnapshot {
    inner: Arc<RwLock<WorldMapSnapshot>>,
}

#[derive(Debug, Clone, Default, Resource, Serialize)]
pub struct WorldExplorerEntitySnapshot {
    pub guid: String,
    pub display_name: Option<String>,
    pub kind_label: String,
    pub label_group: String,
    pub position_xy: Option<(f32, f32)>,
    pub is_player_anchor: bool,
    pub is_controlled: bool,
    pub latency_ms: Option<u64>,
    pub children: Vec<WorldExplorerEntitySnapshot>,
}

#[derive(Debug, Clone, Default, Resource, Serialize)]
pub struct WorldExplorerGroupSnapshot {
    pub key: String,
    pub label: String,
    pub entities: Vec<WorldExplorerEntitySnapshot>,
}

#[derive(Debug, Clone, Default, Resource, Serialize)]
pub struct WorldExplorerSnapshot {
    pub generated_at_unix_ms: u128,
    pub groups: Vec<WorldExplorerGroupSnapshot>,
}

#[derive(Debug, Clone)]
struct ExplorerEntityMeta {
    guid: String,
    display_name: Option<String>,
    labels: Vec<String>,
    bevy_parent_guid: Option<String>,
    kind_label: String,
    position_xy: Option<(f32, f32)>,
    is_player_anchor: bool,
    latency_ms: Option<u64>,
    controlled_entity_guid: Option<String>,
}

#[derive(Clone, Default, Resource)]
pub struct SharedWorldExplorerSnapshot {
    inner: Arc<RwLock<WorldExplorerSnapshot>>,
}

#[derive(SystemParam)]
pub struct WorldExplorerSnapshotInputs<'w, 's> {
    bindings: Res<'w, AuthenticatedClientBindings>,
    player_entity_map: Res<'w, crate::replication::PlayerRuntimeEntityMap>,
    client_links: Query<'w, 's, &'static Link, (With<ClientOf>, With<Connected>)>,
}

impl SharedWorldMapSnapshot {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store(&self, snapshot: WorldMapSnapshot) {
        let mut inner = self
            .inner
            .write()
            .expect("world map snapshot lock poisoned");
        *inner = snapshot;
    }

    pub fn load(&self) -> WorldMapSnapshot {
        self.inner
            .read()
            .expect("world map snapshot lock poisoned")
            .clone()
    }
}

impl SharedWorldExplorerSnapshot {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store(&self, snapshot: WorldExplorerSnapshot) {
        let mut inner = self
            .inner
            .write()
            .expect("world explorer snapshot lock poisoned");
        *inner = snapshot;
    }

    pub fn load(&self) -> WorldExplorerSnapshot {
        self.inner
            .read()
            .expect("world explorer snapshot lock poisoned")
            .clone()
    }
}

impl SharedHealthSnapshot {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store(&self, snapshot: ReplicationHealthSnapshot) {
        let mut inner = self.inner.write().expect("health snapshot lock poisoned");
        *inner = snapshot;
    }

    pub fn load(&self) -> ReplicationHealthSnapshot {
        self.inner
            .read()
            .expect("health snapshot lock poisoned")
            .clone()
    }
}

#[derive(Debug, Clone, Resource)]
pub struct ReplicationProcessStartedAt(pub std::time::Instant);

pub fn init_resources(app: &mut App) {
    app.insert_resource(DiagnosticsSnapshotCadence::default());
    app.insert_resource(DiagnosticsSnapshotState::default());
    app.insert_resource(ReplicationHealthSnapshot::default());
    app.insert_resource(SharedHealthSnapshot::new());
    app.insert_resource(WorldMapSnapshot::default());
    app.insert_resource(SharedWorldMapSnapshot::new());
    app.insert_resource(WorldExplorerSnapshot::default());
    app.insert_resource(SharedWorldExplorerSnapshot::new());
    app.insert_resource(ReplicationProcessStartedAt(std::time::Instant::now()));
}

#[derive(Clone)]
struct HealthHttpState {
    snapshot: SharedHealthSnapshot,
}

pub fn start_health_server(
    config: Res<'_, ReplicationHealthServerConfig>,
    shared: Res<'_, SharedHealthSnapshot>,
) {
    let bind_addr = config.bind_addr;
    let state = HealthHttpState {
        snapshot: shared.clone(),
    };
    thread::Builder::new()
        .name("replication-health-http".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Runtime::new()
                .expect("failed to create tokio runtime for replication health server");
            runtime.block_on(async move {
                let listener = match tokio::net::TcpListener::bind(bind_addr).await {
                    Ok(listener) => listener,
                    Err(err) => {
                        bevy::log::error!(
                            "replication health endpoint failed to bind {}: {}",
                            bind_addr,
                            err
                        );
                        return;
                    }
                };
                let router = Router::new()
                    .route("/health", get(get_health))
                    .with_state(state);
                if let Err(err) = axum::serve(listener, router).await {
                    bevy::log::error!(
                        "replication health endpoint server stopped {}: {}",
                        bind_addr,
                        err
                    );
                }
            });
        })
        .expect("failed to start replication health server thread");
    bevy::log::info!("replication health endpoint listening on http://{bind_addr}/health");
}

async fn get_health(
    AxumState(state): AxumState<HealthHttpState>,
) -> Result<Json<ReplicationHealthSnapshot>, StatusCode> {
    Ok(Json(state.snapshot.load()))
}

#[allow(clippy::too_many_arguments)]
pub fn update_health_snapshot(
    time: Res<'_, Time<Real>>,
    cadence: Res<'_, DiagnosticsSnapshotCadence>,
    mut cadence_state: ResMut<'_, DiagnosticsSnapshotState>,
    started_at: Res<'_, ReplicationProcessStartedAt>,
    shared: Res<'_, SharedHealthSnapshot>,
    mut snapshot: ResMut<'_, ReplicationHealthSnapshot>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    last_activity: Res<'_, ClientLastActivity>,
    controlled_entities: Res<'_, PlayerControlledEntityMap>,
    input_metrics: Res<'_, ClientInputDropMetrics>,
    visibility_metrics: Res<'_, VisibilityRuntimeMetrics>,
    persistence_state: Res<'_, PersistenceWorkerState>,
    script_metrics: Option<Res<'_, ScriptRuntimeMetrics>>,
    all_entities: Query<'_, '_, Entity>,
    physics_entities: Query<'_, '_, Entity, With<RigidBody>>,
    scripted_entities: Query<'_, '_, Entity, With<ScriptState>>,
) {
    let now_s = time.elapsed_secs_f64();
    if !should_refresh_snapshot(
        now_s,
        &mut cadence_state.last_health_at_s,
        cadence.health_interval_s,
    ) {
        return;
    }
    let unique_users = bindings
        .by_client_entity
        .values()
        .cloned()
        .collect::<std::collections::HashSet<_>>()
        .len();
    let current_script_metrics = script_metrics
        .map(|metrics| metrics.clone())
        .unwrap_or_default();
    let next_snapshot = ReplicationHealthSnapshot {
        status: "ok".to_string(),
        generated_at_unix_ms: unix_time_ms(),
        uptime_seconds: started_at.0.elapsed().as_secs(),
        session_count: bindings.by_client_entity.len(),
        users_online: unique_users,
        clients_with_recent_activity: last_activity.0.len(),
        world_entity_count: all_entities.iter().len(),
        physics_body_count: physics_entities.iter().len(),
        scripted_entity_count: scripted_entities.iter().len(),
        controlled_entity_count: controlled_entities.by_player_entity_id.len(),
        input_accepted_total: input_metrics.accepted_inputs,
        input_drop_total: input_metrics.total_drops(),
        visibility_query_ms: visibility_metrics.query_ms,
        visibility_apply_ms: visibility_metrics.apply_ms,
        visibility_clients: visibility_metrics.clients,
        visibility_entities: visibility_metrics.entities,
        persistence_enqueued_batches: persistence_state.enqueued_batches(),
        persistence_queue_full_events: persistence_state.queue_full_events(),
        persistence_disconnected_events: persistence_state.disconnected_events(),
        persistence_pending_latest: persistence_state.has_latest_pending_batch(),
        lua_runtime: LuaRuntimeHealthSnapshot {
            memory_limit_bytes: current_script_metrics.memory_limit_bytes,
            current_memory_bytes: current_script_metrics.current_memory_bytes,
            interval_runs: current_script_metrics.interval_runs,
            event_runs: current_script_metrics.event_runs,
            error_count: current_script_metrics.error_count,
            reload_count: current_script_metrics.reload_count,
            last_interval_run_ms: current_script_metrics.last_interval_run_ms,
            last_event_run_ms: current_script_metrics.last_event_run_ms,
        },
    };
    *snapshot = next_snapshot.clone();
    shared.store(next_snapshot);
}

#[allow(clippy::type_complexity)]
pub fn update_world_map_snapshot(
    time: Res<'_, Time<Real>>,
    cadence: Res<'_, DiagnosticsSnapshotCadence>,
    mut cadence_state: ResMut<'_, DiagnosticsSnapshotState>,
    shared: Res<'_, SharedWorldMapSnapshot>,
    mut snapshot: ResMut<'_, WorldMapSnapshot>,
    entities: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            Option<&'_ DisplayName>,
            Option<&'_ Position>,
            Option<&'_ WorldPosition>,
            Option<&'_ StaticLandmark>,
            Option<&'_ MapIcon>,
            Option<&'_ SizeM>,
            Has<PlayerTag>,
            Has<ShipTag>,
            Has<BallisticProjectile>,
            Has<SimulatedControlledEntity>,
        ),
    >,
) {
    let now_s = time.elapsed_secs_f64();
    if !should_refresh_snapshot(
        now_s,
        &mut cadence_state.last_world_map_at_s,
        cadence.world_interval_s,
    ) {
        return;
    }
    let entities = entities
        .iter()
        .filter_map(
            |(
                guid,
                display_name,
                position,
                world_position,
                static_landmark,
                map_icon,
                size,
                is_player,
                is_ship,
                is_projectile,
                is_controlled,
            )| {
                let world = position
                    .map(|value| value.0)
                    .or_else(|| world_position.map(|value| value.0))?;
                let (glyph, color_rgb) = classify_world_map_glyph(
                    display_name,
                    static_landmark,
                    map_icon,
                    is_player,
                    is_ship,
                    is_projectile,
                    is_controlled,
                );
                Some(WorldMapEntitySnapshot {
                    guid: guid.0.to_string(),
                    display_name: display_name.map(|value| value.0.clone()),
                    x: world.x,
                    y: world.y,
                    glyph,
                    color_rgb,
                    extent_m: size
                        .map(|value| value.length.max(value.width).max(value.height))
                        .unwrap_or(0.0)
                        .max(1.0),
                })
            },
        )
        .collect::<Vec<_>>();
    let next_snapshot = WorldMapSnapshot {
        generated_at_unix_ms: unix_time_ms(),
        entities,
    };
    *snapshot = next_snapshot.clone();
    shared.store(next_snapshot);
}

#[allow(clippy::type_complexity)]
pub fn update_world_explorer_snapshot(
    time: Res<'_, Time<Real>>,
    cadence: Res<'_, DiagnosticsSnapshotCadence>,
    mut cadence_state: ResMut<'_, DiagnosticsSnapshotState>,
    shared: Res<'_, SharedWorldExplorerSnapshot>,
    mut snapshot: ResMut<'_, WorldExplorerSnapshot>,
    inputs: WorldExplorerSnapshotInputs<'_, '_>,
    entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Option<&'_ EntityLabels>,
            Option<&'_ DisplayName>,
            Option<&'_ Position>,
            Option<&'_ WorldPosition>,
            Option<&'_ ChildOf>,
            Option<&'_ ControlledEntityGuid>,
            Has<PlayerTag>,
            Has<ShipTag>,
            Has<StaticLandmark>,
            Has<BallisticProjectile>,
        ),
    >,
) {
    let now_s = time.elapsed_secs_f64();
    if !should_refresh_snapshot(
        now_s,
        &mut cadence_state.last_world_explorer_at_s,
        cadence.world_interval_s,
    ) {
        return;
    }
    let mut by_guid = std::collections::HashMap::<String, ExplorerEntityMeta>::new();
    let latency_by_player_id = inputs
        .bindings
        .by_client_entity
        .iter()
        .filter_map(|(client_entity, player_entity_id)| {
            inputs
                .client_links
                .get(*client_entity)
                .ok()
                .and_then(|link| u64::try_from(link.stats.rtt.as_millis()).ok())
                .map(|latency_ms| (player_entity_id.clone(), latency_ms))
        })
        .collect::<std::collections::HashMap<_, _>>();
    for (
        entity,
        guid,
        entity_labels,
        display_name,
        position,
        world_position,
        child_of,
        controlled_entity_guid,
        is_player,
        is_ship,
        is_landmark,
        is_projectile,
    ) in &entities
    {
        let guid_string = guid.0.to_string();
        let meta = ExplorerEntityMeta {
            guid: guid_string.clone(),
            display_name: display_name.map(|value| value.0.clone()),
            labels: entity_labels
                .map(|value| value.0.clone())
                .unwrap_or_default(),
            position_xy: position
                .map(|value| (value.0.x, value.0.y))
                .or_else(|| world_position.map(|value| (value.0.x, value.0.y))),
            bevy_parent_guid: child_of.and_then(|value| {
                entities
                    .get(value.parent())
                    .ok()
                    .map(|(_, parent_guid, ..)| parent_guid.0.to_string())
            }),
            kind_label: explorer_kind_label(
                entity_labels,
                is_player,
                is_ship,
                is_landmark,
                is_projectile,
            ),
            is_player_anchor: is_player,
            latency_ms: None,
            controlled_entity_guid: controlled_entity_guid.and_then(|value| value.0.clone()),
        };
        if let Some(player_id) = inputs.bindings.by_client_entity.values().find(|player_id| {
            inputs
                .player_entity_map
                .by_player_entity_id
                .get((*player_id).as_str())
                .is_some_and(|mapped| *mapped == entity)
        }) {
            let mut meta = meta;
            meta.latency_ms = latency_by_player_id.get(player_id).copied();
            by_guid.insert(guid_string, meta);
            continue;
        }
        by_guid.insert(guid_string, meta);
    }

    let groups = build_world_explorer_groups(&by_guid);

    let next_snapshot = WorldExplorerSnapshot {
        generated_at_unix_ms: unix_time_ms(),
        groups,
    };
    *snapshot = next_snapshot.clone();
    shared.store(next_snapshot);
}

fn build_world_explorer_groups(
    by_guid: &std::collections::HashMap<String, ExplorerEntityMeta>,
) -> Vec<WorldExplorerGroupSnapshot> {
    let all_guids = by_guid
        .keys()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    let mut children_by_parent = std::collections::HashMap::<String, Vec<String>>::new();
    let mut roots_by_group = std::collections::HashMap::<String, Vec<String>>::new();
    for guid in &all_guids {
        let Some(meta) = by_guid.get(guid) else {
            continue;
        };
        let parent_guid = meta
            .bevy_parent_guid
            .as_deref()
            .filter(|parent_guid| all_guids.contains(*parent_guid));
        if let Some(parent_guid) = parent_guid {
            children_by_parent
                .entry(parent_guid.to_string())
                .or_default()
                .push(guid.clone());
        } else {
            roots_by_group
                .entry(explorer_group_label(meta))
                .or_default()
                .push(guid.clone());
        }
    }
    for children in children_by_parent.values_mut() {
        children.sort_by(|left, right| explorer_sort_key(left, right, by_guid));
    }
    let mut groups = roots_by_group
        .into_iter()
        .map(|(label, mut roots)| {
            roots.sort_by(|left, right| explorer_sort_key(left, right, by_guid));
            let entities = roots
                .into_iter()
                .filter_map(|guid| {
                    build_world_explorer_entity_recursive(&guid, by_guid, &children_by_parent)
                })
                .collect::<Vec<_>>();
            WorldExplorerGroupSnapshot {
                key: format!("group:{label}"),
                label,
                entities,
            }
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.label.cmp(&right.label));
    groups
}

fn build_world_explorer_entity_recursive(
    guid: &str,
    by_guid: &std::collections::HashMap<String, ExplorerEntityMeta>,
    children_by_parent: &std::collections::HashMap<String, Vec<String>>,
) -> Option<WorldExplorerEntitySnapshot> {
    let meta = by_guid.get(guid)?;
    let mut children = children_by_parent
        .get(guid)
        .into_iter()
        .flatten()
        .filter_map(|child_guid| {
            build_world_explorer_entity_recursive(child_guid, by_guid, children_by_parent)
        })
        .collect::<Vec<_>>();
    children.sort_by(|left, right| {
        left.kind_label
            .cmp(&right.kind_label)
            .then(left.guid.cmp(&right.guid))
    });
    Some(WorldExplorerEntitySnapshot {
        guid: meta.guid.clone(),
        display_name: meta.display_name.clone(),
        kind_label: meta.kind_label.clone(),
        label_group: explorer_group_label(meta),
        position_xy: meta.position_xy,
        is_player_anchor: meta.is_player_anchor,
        is_controlled: meta
            .controlled_entity_guid
            .as_deref()
            .is_some_and(|controlled| controlled == meta.guid),
        latency_ms: meta.latency_ms,
        children,
    })
}

fn explorer_sort_key(
    left: &str,
    right: &str,
    by_guid: &std::collections::HashMap<String, ExplorerEntityMeta>,
) -> std::cmp::Ordering {
    let Some(left_meta) = by_guid.get(left) else {
        return left.cmp(right);
    };
    let Some(right_meta) = by_guid.get(right) else {
        return left.cmp(right);
    };
    let left_anchor = left_meta.is_player_anchor;
    let right_anchor = right_meta.is_player_anchor;
    right_anchor
        .cmp(&left_anchor)
        .then(left_meta.kind_label.cmp(&right_meta.kind_label))
        .then(
            left_meta
                .display_name
                .as_deref()
                .unwrap_or(left)
                .cmp(right_meta.display_name.as_deref().unwrap_or(right)),
        )
}

fn explorer_kind_label(
    labels: Option<&EntityLabels>,
    is_player: bool,
    is_ship: bool,
    is_landmark: bool,
    is_projectile: bool,
) -> String {
    if let Some(labels) = labels {
        if labels.0.iter().any(|label| label == "Ship") {
            return "ship".to_string();
        }
        if labels.0.iter().any(|label| label == "Player") {
            return "player".to_string();
        }
        if let Some(first) = labels.0.first() {
            return first.to_ascii_lowercase();
        }
    }
    if is_player {
        return "player".to_string();
    }
    if is_ship {
        return "ship".to_string();
    }
    if is_landmark {
        return "landmark".to_string();
    }
    if is_projectile {
        return "projectile".to_string();
    }
    "entity".to_string()
}

fn explorer_group_label(meta: &ExplorerEntityMeta) -> String {
    if let Some(first) = meta.labels.first() {
        return first.to_ascii_lowercase();
    }
    meta.kind_label.clone()
}

fn classify_world_map_glyph(
    display_name: Option<&DisplayName>,
    static_landmark: Option<&StaticLandmark>,
    map_icon: Option<&MapIcon>,
    is_player: bool,
    is_ship: bool,
    is_projectile: bool,
    is_controlled: bool,
) -> (char, (u8, u8, u8)) {
    if is_player {
        return ('☻', (96, 165, 250));
    }
    if is_controlled || is_ship {
        return ('◆', (248, 113, 113));
    }
    if is_projectile {
        return ('•', (251, 191, 36));
    }
    if let Some(landmark) = static_landmark {
        let kind = landmark.kind.to_ascii_lowercase();
        if kind.contains("planet") {
            return ('◉', (163, 230, 53));
        }
        if kind.contains("star") || kind.contains("sun") {
            return ('✹', (253, 224, 71));
        }
        if kind.contains("asteroid") {
            return ('⬟', (203, 213, 225));
        }
        return ('●', (148, 163, 184));
    }
    if let Some(icon) = map_icon {
        let asset = icon.asset_id.to_ascii_lowercase();
        if asset.contains("asteroid") {
            return ('⬟', (203, 213, 225));
        }
    }
    if let Some(name) = display_name {
        let lower = name.0.to_ascii_lowercase();
        if lower.contains("asteroid") {
            return ('⬟', (203, 213, 225));
        }
        if lower.contains("planet") {
            return ('◉', (163, 230, 53));
        }
    }
    ('·', (148, 163, 184))
}

fn unix_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn should_refresh_snapshot(now_s: f64, last_at_s: &mut Option<f64>, interval_s: f64) -> bool {
    match *last_at_s {
        Some(last) if now_s - last < interval_s => false,
        _ => {
            *last_at_s = Some(now_s);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ReplicationHealthSnapshot;

    #[test]
    fn health_snapshot_serializes_summary_fields() {
        let snapshot = ReplicationHealthSnapshot {
            users_online: 3,
            session_count: 4,
            ..Default::default()
        };
        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["users_online"], 3);
        assert_eq!(value["session_count"], 4);
        assert!(value.get("sessions").is_none());
    }
}
