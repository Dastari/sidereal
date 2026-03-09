mod bootstrap_runtime;
mod plugins;
mod replication;
use crate::replication::{
    assets, auth, control, input, lifecycle, owner_manifest, persistence, runtime_scripting,
    runtime_state, scripting, simulation_entities, tactical, visibility,
};
use avian2d::prelude::{Gravity, PhysicsInterpolationPlugin, PhysicsPlugins, PhysicsSystems};
use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::log::tracing_subscriber::fmt::writer::MakeWriterExt;
use bevy::log::{BoxedFmtLayer, LogPlugin};
use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use lightyear::input::native::prelude::NativeStateSequence;
use lightyear::input::plugin::InputPlugin as LightyearInputProtocolPlugin;
use lightyear::prelude::server::ServerPlugins;
use sidereal_core::logging::prepare_timestamped_log_file;
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_game::{HierarchyRebuildEnabled, SiderealGamePlugin};
use sidereal_net::register_lightyear_server_protocol;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

static REPLICATION_LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

fn main() {
    let run_log = match prepare_timestamped_log_file("sidereal-replication") {
        Ok(run_log) => run_log,
        Err(err) => {
            eprintln!("failed to create replication log file: {err}");
            std::process::exit(2);
        }
    };
    if REPLICATION_LOG_PATH.set(run_log.path.clone()).is_err() {
        eprintln!("replication log path initialized more than once");
        std::process::exit(2);
    }
    drop(run_log.file);

    let remote_cfg: RemoteInspectConfig = match RemoteInspectConfig::from_env("REPLICATION", 15713)
    {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("invalid REPLICATION BRP config: {err}");
            std::process::exit(2);
        }
    };

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
    app.add_plugins(SiderealGamePlugin);
    // Prevent server-side Bevy hierarchy components from being replicated.
    app.insert_resource(HierarchyRebuildEnabled(false));
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
        tick_duration: Duration::from_secs_f64(1.0 / 60.0),
    });
    app.add_plugins(LightyearInputProtocolPlugin::<
        NativeStateSequence<sidereal_net::PlayerInput>,
    >::default());
    register_lightyear_server_protocol(&mut app);
    lifecycle::configure_remote(&mut app, &remote_cfg);

    // Lightyear/Bevy plugins can initialize Fixed time; enforce authoritative 60 Hz after plugin wiring.
    app.insert_resource(Time::<Fixed>::from_hz(60.0));
    init_resources(&mut app);
    register_plugins(&mut app);
    app.run();
}

fn replication_fmt_layer(_app: &mut App) -> Option<BoxedFmtLayer> {
    let log_path = REPLICATION_LOG_PATH
        .get()
        .expect("replication log path should be initialized");
    let log_file = OpenOptions::new()
        .append(true)
        .open(log_path)
        .unwrap_or_else(|err| {
            panic!(
                "failed to open replication log file {}: {err}",
                log_path.display()
            )
        });
    Some(Box::new(
        bevy::log::tracing_subscriber::fmt::Layer::default()
            .with_writer(std::io::stderr.and(log_file)),
    ))
}

fn init_resources(app: &mut App) {
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
}

fn register_plugins(app: &mut App) {
    app.add_plugins(plugins::ReplicationLifecyclePlugin);
    app.add_plugins(plugins::ReplicationAuthPlugin);
    app.add_plugins(plugins::ReplicationInputPlugin);
    app.add_plugins(plugins::ReplicationControlPlugin);
    app.add_plugins(plugins::ReplicationRuntimeScriptingPlugin);
    app.add_plugins(plugins::ReplicationVisibilityPlugin);
    app.add_plugins(plugins::ReplicationPersistencePlugin);
    app.add_plugins(plugins::ReplicationBootstrapBridgePlugin);
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
            simulation_entities::process_bootstrap_entity_commands,
            lifecycle::ensure_entity_scoped_replication_groups
                .after(simulation_entities::process_bootstrap_entity_commands),
            runtime_state::log_player_control_state_changes
                .after(lifecycle::ensure_entity_scoped_replication_groups),
            lifecycle::disconnect_idle_clients,
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
