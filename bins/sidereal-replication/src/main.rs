mod bootstrap_runtime;
mod replication;
use crate::replication::{
    assets, auth, control, input, lifecycle, persistence, runtime_state, simulation_entities,
    visibility,
};
use avian2d::prelude::{
    Gravity, PhysicsInterpolationPlugin, PhysicsPlugins, PhysicsSystems, PhysicsTransformPlugin,
};
use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use lightyear::prelude::server::ServerPlugins;
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_game::{HierarchyRebuildEnabled, SiderealGamePlugin};
use sidereal_net::register_lightyear_protocol;
use std::time::Duration;

fn main() {
    let remote_cfg: RemoteInspectConfig = match RemoteInspectConfig::from_env("REPLICATION", 15713)
    {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("invalid REPLICATION BRP config: {err}");
            std::process::exit(2);
        }
    };

    let mut app = App::new();
    // Cap main loop at ~100 Hz so Update (message drain, transport) doesn't spin at full CPU.
    // FixedUpdate remains time-based at 30 Hz. See docs/features/replication_server_cpu_report.md.
    let update_cap_hz = std::env::var("REPLICATION_UPDATE_CAP_HZ")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(100.0)
        .clamp(10.0, 1000.0);
    app.add_plugins(
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / update_cap_hz,
        ))),
    );
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(ScenePlugin);
    app.add_plugins(LogPlugin::default());
    app.add_plugins(SiderealGamePlugin);
    // Prevent server-side Bevy hierarchy components from being replicated.
    app.insert_resource(HierarchyRebuildEnabled(false));
    app.add_plugins(
        PhysicsPlugins::default()
            .with_length_unit(1.0)
            .build()
            .disable::<PhysicsTransformPlugin>()
            .disable::<PhysicsInterpolationPlugin>(),
    );
    app.add_message::<bevy::asset::AssetEvent<Mesh>>();
    app.init_asset::<Mesh>();
    app.insert_resource(Gravity(Vec2::ZERO));
    app.add_plugins(ServerPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / 30.0),
    });
    register_lightyear_protocol(&mut app);
    lifecycle::configure_remote(&mut app, &remote_cfg);

    // Lightyear/Bevy plugins can initialize Fixed time; enforce authoritative 30 Hz after plugin wiring.
    app.insert_resource(Time::<Fixed>::from_hz(30.0));
    init_resources(&mut app);
    register_systems(&mut app);
    app.add_observer(lifecycle::log_replication_client_connected);
    app.add_observer(lifecycle::setup_client_replication_sender);
    app.run();
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
    lifecycle::init_resources(app);
}

fn register_systems(app: &mut App) {
    app.add_systems(
        Startup,
        (
            lifecycle::hydrate_replication_world,
            simulation_entities::hydrate_simulation_entities,
            lifecycle::start_lightyear_server,
            persistence::start_persistence_worker,
            assets::initialize_asset_stream_cache,
        )
            .chain(),
    );
    app.add_systems(
        Startup,
        bootstrap_runtime::start_replication_control_listener,
    );

    app.add_systems(
        Update,
        (
            bevy::ecs::schedule::ApplyDeferred,
            lifecycle::ensure_server_transport_channels,
            auth::receive_client_disconnect_notify,
            auth::cleanup_client_auth_bindings,
            input::receive_latest_realtime_input_messages,
            control::receive_client_control_requests,
            assets::receive_client_asset_requests,
            assets::receive_client_asset_acks,
            input::report_input_drop_metrics,
            persistence::report_persistence_worker_metrics,
            simulation_entities::process_bootstrap_entity_commands,
            runtime_state::log_player_control_state_changes
                .after(simulation_entities::process_bootstrap_entity_commands),
            lifecycle::disconnect_idle_clients,
        )
            .chain(),
    );
    app.add_systems(PostUpdate, auth::receive_client_auth_messages);
    app.add_systems(
        PostUpdate,
        simulation_entities::apply_pending_controlled_by_bindings
            .after(lightyear::prelude::ReplicationBufferSystems::AfterBuffer),
    );

    app.add_systems(
        FixedUpdate,
        (
            assets::stream_bootstrap_assets_to_authenticated_clients,
            assets::send_asset_stream_chunks_paced
                .after(assets::stream_bootstrap_assets_to_authenticated_clients),
        ),
    );
    app.add_systems(
        FixedUpdate,
        (
            simulation_entities::sync_controlled_entity_transforms,
            runtime_state::sync_player_anchor_to_controlled_entity,
            runtime_state::update_client_observer_anchor_positions,
            runtime_state::compute_controlled_entity_scanner_ranges,
            visibility::update_network_visibility,
        )
            .chain()
            .after(PhysicsSystems::Writeback),
    );
    app.add_systems(
        FixedUpdate,
        (
            mark_dirty_persistable_entities,
            mark_dirty_persistable_entities_spatial,
            mark_dirty_persistable_entities_components,
        )
            .after(PhysicsSystems::Writeback),
    );
    app.add_systems(
        FixedUpdate,
        persistence::flush_simulation_state_persistence
            .after(visibility::update_network_visibility),
    );
    app.add_systems(
        FixedUpdate,
        simulation_entities::enforce_planar_motion.before(PhysicsSystems::Prepare),
    );
    app.add_systems(
        FixedUpdate,
        input::drain_native_player_inputs_to_action_queue.before(PhysicsSystems::Prepare),
    );
}

use crate::replication::persistence::{
    mark_dirty_persistable_entities, mark_dirty_persistable_entities_components,
    mark_dirty_persistable_entities_spatial,
};

#[cfg(test)]
mod tests;
