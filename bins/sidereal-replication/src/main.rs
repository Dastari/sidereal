mod bootstrap_runtime;
mod config;
mod log_buffer;
mod plugins;
mod replication;
mod tui;
use crate::config::CliAction;
use crate::replication::{
    assets, auth, control, health, input, lifecycle, owner_manifest, persistence,
    runtime_scripting, runtime_state, scripting, simulation_entities, tactical, visibility,
};
use avian2d::prelude::{Gravity, PhysicsInterpolationPlugin, PhysicsPlugins, PhysicsSystems};
use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::log::info;
use bevy::log::{BoxedFmtLayer, LogPlugin};
use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use lightyear::input::native::prelude::InputPlugin as NativeInputPlugin;
use lightyear::prelude::server::ServerPlugins;
use sidereal_core::SIM_TICK_HZ;
use sidereal_core::logging::prepare_timestamped_log_file;
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_game::{HierarchyRebuildEnabled, SiderealGamePlugin};
use sidereal_net::register_lightyear_server_protocol;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use crate::log_buffer::{ReplicationLogFanout, init_global_log_buffer, set_log_to_stderr};

static REPLICATION_LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static REPLICATION_LOG_FANOUT: OnceLock<ReplicationLogFanout> = OnceLock::new();

#[derive(Debug, Clone, Copy, Resource)]
#[allow(dead_code)]
struct HeadlessMode(pub bool);

fn main() {
    let config = match config::apply_process_cli() {
        Ok(CliAction::Run(config)) => *config,
        Ok(CliAction::Help(text)) => {
            println!("{text}");
            return;
        }
        Err(err) => {
            emit_startup_tracing_error(&err);
            std::process::exit(2);
        }
    };
    config.apply_env();
    let run_log = match prepare_timestamped_log_file("sidereal-replication") {
        Ok(run_log) => run_log,
        Err(err) => {
            emit_startup_tracing_error(&format!("failed to create replication log file: {err}"));
            std::process::exit(2);
        }
    };
    if REPLICATION_LOG_PATH.set(run_log.path.clone()).is_err() {
        emit_startup_tracing_error("replication log path initialized more than once");
        std::process::exit(2);
    }
    let shared_log_buffer = init_global_log_buffer();
    let log_fanout = ReplicationLogFanout::new(run_log.file, shared_log_buffer.clone());
    if REPLICATION_LOG_FANOUT.set(log_fanout).is_err() {
        emit_startup_tracing_error("replication log fanout initialized more than once");
        std::process::exit(2);
    }

    let remote_cfg: RemoteInspectConfig = match RemoteInspectConfig::from_env("REPLICATION", 15713)
    {
        Ok(cfg) => cfg,
        Err(err) => {
            emit_startup_tracing_error(&format!("invalid REPLICATION BRP config: {err}"));
            std::process::exit(2);
        }
    };
    let interactive_terminal = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
    let headless_mode = config.headless || !interactive_terminal;
    set_log_to_stderr(headless_mode);

    let mut app = App::new();
    app.init_resource::<bevy::transform::StaticTransformOptimizations>();
    // Run Update uncapped by default so transport/message drain does not artificially bunch work
    // behind a scheduler sleep while FixedUpdate is still trying to maintain 60 Hz simulation.
    // Sidereal can opt back into a cap via REPLICATION_UPDATE_CAP_HZ if profiling shows a real
    // idle-spin issue, but the default should not throttle replication responsiveness.
    let update_runner = std::env::var("REPLICATION_UPDATE_CAP_HZ")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|hz| hz.is_finite() && *hz > 0.0)
        .map(|hz| ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / hz)))
        .unwrap_or_default();
    app.add_plugins(MinimalPlugins.set(update_runner));
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(ScenePlugin);
    app.add_plugins(LogPlugin {
        // Suppress noisy transient UDP send errors (EAGAIN / os error 11) from lightyear transport.
        filter: "info,lightyear_udp::server=off,postgres::config=warn".to_string(),
        fmt_layer: replication_fmt_layer,
        ..Default::default()
    });
    // Replication must not rebuild Bevy ChildOf/Children locally because those
    // relationships can leak into network replication. UUID-based ParentGuid /
    // MountedOn remains the only cross-runtime hierarchy contract, including for
    // Lua-authored world-init entities.
    app.insert_resource(HierarchyRebuildEnabled(false));
    app.add_plugins(SiderealGamePlugin);
    app.add_plugins(
        PhysicsPlugins::default()
            .with_length_unit(1.0)
            .build()
            .disable::<PhysicsInterpolationPlugin>(),
    );
    app.add_message::<bevy::asset::AssetEvent<Mesh>>();
    app.init_asset::<Mesh>();
    app.insert_resource(Gravity(Vec2::ZERO));
    app.add_plugins(ServerPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / f64::from(SIM_TICK_HZ)),
    });
    app.add_plugins(NativeInputPlugin::<sidereal_net::PlayerInput>::default());
    register_lightyear_server_protocol(&mut app);
    lifecycle::configure_remote(&mut app, &remote_cfg);

    // Lightyear/Bevy plugins can initialize Fixed time; enforce the shared authoritative tick after plugin wiring.
    app.insert_resource(Time::<Fixed>::from_hz(f64::from(SIM_TICK_HZ)));
    app.insert_resource(shared_log_buffer.clone());
    app.insert_resource(HeadlessMode(headless_mode));
    app.insert_resource(health::ReplicationHealthServerConfig {
        bind_addr: config.health_bind,
    });
    init_resources(&mut app);
    register_plugins(&mut app);
    if headless_mode {
        info!("sidereal-replication headless mode active");
    } else {
        let command_sender = app
            .world()
            .resource::<replication::admin::AdminCommandBusSender>()
            .clone();
        let shared_health = app
            .world()
            .resource::<health::SharedHealthSnapshot>()
            .clone();
        let shared_world_map = app
            .world()
            .resource::<health::SharedWorldMapSnapshot>()
            .clone();
        let shared_world_explorer = app
            .world()
            .resource::<health::SharedWorldExplorerSnapshot>()
            .clone();
        if let Err(err) = tui::start(
            shared_log_buffer.clone(),
            command_sender,
            shared_health,
            shared_world_explorer,
            shared_world_map,
        ) {
            info!("sidereal-replication TUI startup failed; continuing headless: {err}");
        }
    }
    app.run();
}

fn emit_startup_tracing_error(message: &str) {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .without_time()
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        tracing::error!("{message}");
    });
}

fn replication_fmt_layer(_app: &mut App) -> Option<BoxedFmtLayer> {
    let fanout = REPLICATION_LOG_FANOUT
        .get()
        .expect("replication log fanout should be initialized")
        .clone();
    Some(Box::new(
        bevy::log::tracing_subscriber::fmt::Layer::default()
            .with_writer(move || fanout.make_writer()),
    ))
}

fn init_resources(app: &mut App) {
    replication::admin::init_resources(app);
    visibility::init_resources(app);
    simulation_entities::init_resources(app);
    auth::init_resources(app);
    assets::init_resources(app);
    input::init_resources(app);
    persistence::init_resources(app);
    control::init_resources(app);
    runtime_state::init_resources(app);
    scripting::init_resources(app);
    runtime_scripting::init_resources(app);
    owner_manifest::init_resources(app);
    tactical::init_resources(app);
    lifecycle::init_resources(app);
    health::init_resources(app);
}

fn register_plugins(app: &mut App) {
    app.add_plugins(plugins::ReplicationLifecyclePlugin);
    app.add_plugins(plugins::ReplicationDiagnosticsPlugin);
    app.add_plugins(plugins::ReplicationAuthPlugin);
    app.add_plugins(plugins::ReplicationInputPlugin);
    app.add_plugins(plugins::ReplicationControlPlugin);
    app.add_plugins(plugins::ReplicationRuntimeScriptingPlugin);
    app.add_plugins(plugins::ReplicationVisibilityPlugin);
    app.add_plugins(plugins::ReplicationPersistencePlugin);
    app.add_systems(
        Update,
        (
            bevy::ecs::schedule::ApplyDeferred,
            lifecycle::ensure_server_transport_channels,
            lifecycle::ensure_server_message_components,
            auth::receive_client_auth_messages,
            auth::audit_pending_client_auth_state,
            auth::receive_client_disconnect_notify,
            auth::cleanup_client_auth_bindings,
            assets::request_script_catalog_reload_on_disk_changes_system,
            assets::poll_runtime_asset_catalog_changes_system,
            input::receive_latest_realtime_input_messages,
            control::receive_client_control_requests,
            visibility::receive_client_local_view_mode_messages,
            input::report_input_drop_metrics,
            persistence::report_persistence_worker_metrics,
            lifecycle::disconnect_idle_clients,
        )
            .chain(),
    );
    app.add_systems(
        Update,
        (
            simulation_entities::process_bootstrap_entity_commands,
            lifecycle::ensure_entity_scoped_replication_groups,
            control::reconcile_control_replication_roles,
            auth::sync_visibility_registry_with_authenticated_clients,
            control::flush_pending_control_acks,
            runtime_state::log_player_control_state_changes,
        )
            .chain(),
    );
    app.add_systems(
        FixedUpdate,
        simulation_entities::enforce_planar_motion.before(PhysicsSystems::Prepare),
    );
}

#[cfg(test)]
mod tests;
