use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::replication::SimulatedControlledEntity;
use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::input::{
    ClientInputDropMetrics, LatestRealtimeInputsByPlayer, RealtimeInputActivityByPlayer,
    RealtimeInputTimeoutSeconds,
};
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
use sidereal_core::SIM_TICK_HZ;
use sidereal_game::{
    BallisticProjectile, ControlledEntityGuid, DisplayName, EntityGuid, EntityLabels, MapIcon,
    MountedOn, ParentGuid, PlayerTag, ScriptState, ShipTag, SizeM, StaticLandmark, WorldPosition,
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
    pub input_future_tick_drop_total: u64,
    pub input_duplicate_or_out_of_order_drop_total: u64,
    pub input_rate_limited_drop_total: u64,
    pub input_oversized_packet_drop_total: u64,
    pub input_empty_after_filter_drop_total: u64,
    pub input_unbound_client_drop_total: u64,
    pub input_spoofed_player_drop_total: u64,
    pub input_controlled_target_mismatch_total: u64,
    pub input_players_tracked: usize,
    pub input_stale_players: usize,
    pub input_oldest_age_ms: f64,
    pub fixed_tick_budget_ms: f64,
    pub fixed_tick_last_wall_ms: f64,
    pub fixed_tick_max_wall_ms: f64,
    pub fixed_tick_over_budget_total: u64,
    pub fixed_ticks_total: u64,
    pub fixed_ticks_last_update: u32,
    pub fixed_ticks_max_per_update: u32,
    pub fixed_multi_step_updates_total: u64,
    pub visibility_query_ms: f64,
    pub visibility_apply_ms: f64,
    pub visibility_cache_refresh_ms: f64,
    pub visibility_client_context_refresh_ms: f64,
    pub visibility_landmark_discovery_ms: f64,
    pub visibility_clients: usize,
    pub visibility_entities: usize,
    pub visibility_candidates_total: usize,
    pub visibility_candidates_per_client: f64,
    pub visibility_occupied_cells: usize,
    pub visibility_max_entities_per_cell: usize,
    pub visibility_visible_gains: usize,
    pub visibility_visible_losses: usize,
    pub persistence_enqueued_batches: u64,
    pub persistence_queue_full_events: u64,
    pub persistence_disconnected_events: u64,
    pub persistence_pending_latest: bool,
    pub lua_runtime: LuaRuntimeHealthSnapshot,
}

#[derive(Debug, Clone, Resource, Default)]
pub struct FixedStepRuntimeMetrics {
    pub fixed_ticks_total: u64,
    pub fixed_ticks_last_update: u32,
    pub max_fixed_ticks_per_update: u32,
    pub multi_step_updates_total: u64,
    pub last_fixed_tick_wall_ms: f64,
    pub max_fixed_tick_wall_ms: f64,
    pub over_budget_ticks_total: u64,
    current_update_fixed_ticks: u32,
    current_fixed_tick_started_at: Option<std::time::Instant>,
}

#[derive(Clone, Default, Resource)]
pub struct SharedHealthSnapshot {
    inner: Arc<RwLock<ReplicationHealthSnapshot>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldMapEntitySnapshot {
    pub guid: String,
    pub display_name: Option<String>,
    pub x: f64,
    pub y: f64,
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
    pub position_xy: Option<(f64, f64)>,
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
    parent_guid: Option<String>,
    kind_label: String,
    position_xy: Option<(f64, f64)>,
    is_player_anchor: bool,
    latency_ms: Option<u64>,
    controlled_entity_guid: Option<String>,
}

type WorldExplorerEntityQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static EntityGuid,
        Option<&'static EntityLabels>,
        Option<&'static DisplayName>,
        Option<&'static Position>,
        Option<&'static WorldPosition>,
        Option<&'static MountedOn>,
        Option<&'static ParentGuid>,
        Option<&'static ChildOf>,
        Option<&'static ControlledEntityGuid>,
        Has<PlayerTag>,
        Has<ShipTag>,
        Has<StaticLandmark>,
        Has<BallisticProjectile>,
    ),
>;

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

#[derive(SystemParam)]
pub struct HealthSnapshotInputs<'w, 's> {
    bindings: Res<'w, AuthenticatedClientBindings>,
    last_activity: Res<'w, ClientLastActivity>,
    controlled_entities: Res<'w, PlayerControlledEntityMap>,
    input_metrics: Res<'w, ClientInputDropMetrics>,
    latest_realtime_inputs: Res<'w, LatestRealtimeInputsByPlayer>,
    realtime_input_activity: Res<'w, RealtimeInputActivityByPlayer>,
    realtime_input_timeout: Res<'w, RealtimeInputTimeoutSeconds>,
    fixed_metrics: Res<'w, FixedStepRuntimeMetrics>,
    visibility_metrics: Res<'w, VisibilityRuntimeMetrics>,
    persistence_state: Res<'w, PersistenceWorkerState>,
    script_metrics: Option<Res<'w, ScriptRuntimeMetrics>>,
    all_entities: Query<'w, 's, Entity>,
    physics_entities: Query<'w, 's, Entity, With<RigidBody>>,
    scripted_entities: Query<'w, 's, Entity, With<ScriptState>>,
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
    app.insert_resource(FixedStepRuntimeMetrics::default());
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
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
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

pub fn advance_fixed_step_update_frame(mut metrics: ResMut<'_, FixedStepRuntimeMetrics>) {
    let fixed_ticks_this_update = std::mem::take(&mut metrics.current_update_fixed_ticks);
    metrics.fixed_ticks_last_update = fixed_ticks_this_update;
    metrics.max_fixed_ticks_per_update = metrics
        .max_fixed_ticks_per_update
        .max(fixed_ticks_this_update);
    if fixed_ticks_this_update > 1 {
        metrics.multi_step_updates_total = metrics.multi_step_updates_total.saturating_add(1);
    }
}

pub fn begin_fixed_step_diagnostics(mut metrics: ResMut<'_, FixedStepRuntimeMetrics>) {
    metrics.fixed_ticks_total = metrics.fixed_ticks_total.saturating_add(1);
    metrics.current_update_fixed_ticks = metrics.current_update_fixed_ticks.saturating_add(1);
    metrics.current_fixed_tick_started_at = Some(std::time::Instant::now());
}

pub fn end_fixed_step_diagnostics(mut metrics: ResMut<'_, FixedStepRuntimeMetrics>) {
    let Some(started_at) = metrics.current_fixed_tick_started_at.take() else {
        return;
    };
    let wall_ms = started_at.elapsed().as_secs_f64() * 1000.0;
    metrics.last_fixed_tick_wall_ms = wall_ms;
    metrics.max_fixed_tick_wall_ms = metrics.max_fixed_tick_wall_ms.max(wall_ms);
    let budget_ms = 1000.0 / f64::from(SIM_TICK_HZ);
    if wall_ms > budget_ms {
        metrics.over_budget_ticks_total = metrics.over_budget_ticks_total.saturating_add(1);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_health_snapshot(
    time: Res<'_, Time<Real>>,
    cadence: Res<'_, DiagnosticsSnapshotCadence>,
    mut cadence_state: ResMut<'_, DiagnosticsSnapshotState>,
    started_at: Res<'_, ReplicationProcessStartedAt>,
    shared: Res<'_, SharedHealthSnapshot>,
    mut snapshot: ResMut<'_, ReplicationHealthSnapshot>,
    inputs: HealthSnapshotInputs<'_, '_>,
) {
    let now_s = time.elapsed_secs_f64();
    if !should_refresh_snapshot(
        now_s,
        &mut cadence_state.last_health_at_s,
        cadence.health_interval_s,
    ) {
        return;
    }
    let unique_users = inputs
        .bindings
        .by_client_entity
        .values()
        .cloned()
        .collect::<std::collections::HashSet<_>>()
        .len();
    let current_script_metrics = inputs
        .script_metrics
        .map(|metrics| metrics.clone())
        .unwrap_or_default();
    let input_players_tracked = inputs.latest_realtime_inputs.by_player_entity_id.len();
    let mut input_stale_players = 0usize;
    let mut input_oldest_age_ms = 0.0f64;
    for last_received_at_s in inputs
        .realtime_input_activity
        .last_received_at_s_by_player_entity_id
        .values()
    {
        let age_s = (now_s - *last_received_at_s).max(0.0);
        if age_s > inputs.realtime_input_timeout.0 {
            input_stale_players = input_stale_players.saturating_add(1);
        }
        input_oldest_age_ms = input_oldest_age_ms.max(age_s * 1000.0);
    }
    let fixed_tick_budget_ms = 1000.0 / f64::from(SIM_TICK_HZ);
    let next_snapshot = ReplicationHealthSnapshot {
        status: "ok".to_string(),
        generated_at_unix_ms: unix_time_ms(),
        uptime_seconds: started_at.0.elapsed().as_secs(),
        session_count: inputs.bindings.by_client_entity.len(),
        users_online: unique_users,
        clients_with_recent_activity: inputs.last_activity.0.len(),
        world_entity_count: inputs.all_entities.iter().len(),
        physics_body_count: inputs.physics_entities.iter().len(),
        scripted_entity_count: inputs.scripted_entities.iter().len(),
        controlled_entity_count: inputs.controlled_entities.by_player_entity_id.len(),
        input_accepted_total: inputs.input_metrics.accepted_inputs,
        input_drop_total: inputs.input_metrics.total_drops(),
        input_future_tick_drop_total: inputs.input_metrics.future_tick,
        input_duplicate_or_out_of_order_drop_total: inputs
            .input_metrics
            .duplicate_or_out_of_order_tick,
        input_rate_limited_drop_total: inputs.input_metrics.rate_limited,
        input_oversized_packet_drop_total: inputs.input_metrics.oversized_packet,
        input_empty_after_filter_drop_total: inputs.input_metrics.empty_after_filter,
        input_unbound_client_drop_total: inputs.input_metrics.unbound_client,
        input_spoofed_player_drop_total: inputs.input_metrics.spoofed_player_id,
        input_controlled_target_mismatch_total: inputs.input_metrics.controlled_target_mismatch,
        input_players_tracked,
        input_stale_players,
        input_oldest_age_ms,
        fixed_tick_budget_ms,
        fixed_tick_last_wall_ms: inputs.fixed_metrics.last_fixed_tick_wall_ms,
        fixed_tick_max_wall_ms: inputs.fixed_metrics.max_fixed_tick_wall_ms,
        fixed_tick_over_budget_total: inputs.fixed_metrics.over_budget_ticks_total,
        fixed_ticks_total: inputs.fixed_metrics.fixed_ticks_total,
        fixed_ticks_last_update: inputs.fixed_metrics.fixed_ticks_last_update,
        fixed_ticks_max_per_update: inputs.fixed_metrics.max_fixed_ticks_per_update,
        fixed_multi_step_updates_total: inputs.fixed_metrics.multi_step_updates_total,
        visibility_query_ms: inputs.visibility_metrics.query_ms,
        visibility_apply_ms: inputs.visibility_metrics.apply_ms,
        visibility_cache_refresh_ms: inputs.visibility_metrics.cache_refresh_ms,
        visibility_client_context_refresh_ms: inputs.visibility_metrics.client_context_refresh_ms,
        visibility_landmark_discovery_ms: inputs.visibility_metrics.landmark_discovery_ms,
        visibility_clients: inputs.visibility_metrics.clients,
        visibility_entities: inputs.visibility_metrics.entities,
        visibility_candidates_total: inputs.visibility_metrics.candidates_total,
        visibility_candidates_per_client: inputs.visibility_metrics.candidates_per_client,
        visibility_occupied_cells: inputs.visibility_metrics.occupied_cells,
        visibility_max_entities_per_cell: inputs.visibility_metrics.max_entities_per_cell,
        visibility_visible_gains: inputs.visibility_metrics.visible_gains,
        visibility_visible_losses: inputs.visibility_metrics.visible_losses,
        persistence_enqueued_batches: inputs.persistence_state.enqueued_batches(),
        persistence_queue_full_events: inputs.persistence_state.queue_full_events(),
        persistence_disconnected_events: inputs.persistence_state.disconnected_events(),
        persistence_pending_latest: inputs.persistence_state.has_latest_pending_batch(),
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
    entities: WorldExplorerEntityQuery<'_, '_>,
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
        mounted_on,
        parent_guid,
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
            parent_guid: resolved_explorer_parent_guid(
                mounted_on,
                parent_guid,
                child_of,
                &entities,
            ),
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
            .parent_guid
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
        .then(left.cmp(right))
}

fn resolved_explorer_parent_guid(
    mounted_on: Option<&MountedOn>,
    parent_guid: Option<&ParentGuid>,
    child_of: Option<&ChildOf>,
    entities: &WorldExplorerEntityQuery<'_, '_>,
) -> Option<String> {
    mounted_on
        .map(|mounted_on| mounted_on.parent_entity_id.to_string())
        .or_else(|| parent_guid.map(|parent_guid| parent_guid.0.to_string()))
        .or_else(|| {
            child_of.and_then(|value| {
                entities
                    .get(value.parent())
                    .ok()
                    .map(|(_, parent_guid, ..)| parent_guid.0.to_string())
            })
        })
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
    use std::collections::HashMap;

    use super::{ExplorerEntityMeta, ReplicationHealthSnapshot, build_world_explorer_groups};

    #[test]
    fn health_snapshot_serializes_summary_fields() {
        let snapshot = ReplicationHealthSnapshot {
            users_online: 3,
            session_count: 4,
            fixed_ticks_last_update: 2,
            input_rate_limited_drop_total: 5,
            ..Default::default()
        };
        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["users_online"], 3);
        assert_eq!(value["session_count"], 4);
        assert_eq!(value["fixed_ticks_last_update"], 2);
        assert_eq!(value["input_rate_limited_drop_total"], 5);
        assert!(value.get("sessions").is_none());
    }

    #[test]
    fn world_explorer_groups_use_parent_guid_hierarchy_and_stable_order() {
        let root_guid = "00000000-0000-0000-0000-000000000001".to_string();
        let engine_guid = "00000000-0000-0000-0000-000000000002".to_string();
        let shield_guid = "00000000-0000-0000-0000-000000000003".to_string();
        let asteroid_guid = "00000000-0000-0000-0000-000000000004".to_string();
        let mut by_guid = HashMap::new();
        by_guid.insert(
            root_guid.clone(),
            ExplorerEntityMeta {
                guid: root_guid.clone(),
                display_name: Some("Ship".to_string()),
                labels: vec!["Ship".to_string()],
                parent_guid: None,
                kind_label: "ship".to_string(),
                position_xy: None,
                is_player_anchor: false,
                latency_ms: None,
                controlled_entity_guid: None,
            },
        );
        by_guid.insert(
            shield_guid.clone(),
            ExplorerEntityMeta {
                guid: shield_guid.clone(),
                display_name: Some("Module".to_string()),
                labels: vec!["Shield".to_string()],
                parent_guid: Some(root_guid.clone()),
                kind_label: "module".to_string(),
                position_xy: None,
                is_player_anchor: false,
                latency_ms: None,
                controlled_entity_guid: None,
            },
        );
        by_guid.insert(
            engine_guid.clone(),
            ExplorerEntityMeta {
                guid: engine_guid.clone(),
                display_name: Some("Module".to_string()),
                labels: vec!["Engine".to_string()],
                parent_guid: Some(root_guid.clone()),
                kind_label: "module".to_string(),
                position_xy: None,
                is_player_anchor: false,
                latency_ms: None,
                controlled_entity_guid: None,
            },
        );
        by_guid.insert(
            asteroid_guid.clone(),
            ExplorerEntityMeta {
                guid: asteroid_guid.clone(),
                display_name: Some("Asteroid".to_string()),
                labels: vec!["Asteroid".to_string()],
                parent_guid: None,
                kind_label: "asteroid".to_string(),
                position_xy: None,
                is_player_anchor: false,
                latency_ms: None,
                controlled_entity_guid: None,
            },
        );

        let groups = build_world_explorer_groups(&by_guid);
        let ship = groups
            .iter()
            .flat_map(|group| &group.entities)
            .find(|entity| entity.guid == root_guid)
            .expect("ship root");
        assert_eq!(ship.children.len(), 2);
        assert_eq!(ship.children[0].guid, engine_guid);
        assert_eq!(ship.children[1].guid, shield_guid);
        assert!(groups.iter().any(|group| {
            group
                .entities
                .iter()
                .any(|entity| entity.guid == asteroid_guid)
        }));
    }
}
