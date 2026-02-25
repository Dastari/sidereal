mod bootstrap_runtime;
mod replication;
mod visibility;

use crate::replication::assets::{
    StreamableAssetCache, initialize_asset_stream_cache, receive_client_asset_acks,
    receive_client_asset_requests, send_asset_stream_chunks_paced,
    stream_bootstrap_assets_to_authenticated_clients,
};
use crate::replication::auth::{cleanup_client_auth_bindings, receive_client_auth_messages};
use crate::replication::input::{
    ClientInputDropMetrics, ClientInputDropMetricsLogState, ClientInputTickTracker,
    InputActivityLogState, LatestRealtimeInputsByPlayer,
    drain_native_player_inputs_to_action_queue, receive_latest_realtime_input_messages,
    report_input_drop_metrics,
};
use crate::replication::lifecycle::{
    configure_remote, hydrate_replication_world, log_replication_client_connected,
    setup_client_replication_sender, start_lightyear_server,
};
use crate::replication::persistence::{
    PersistenceDirtyState, PersistenceWorkerState, SimulationPersistenceTimer,
    flush_simulation_state_persistence, mark_dirty_persistable_entities_gameplay,
    mark_dirty_persistable_entities_modules, mark_dirty_persistable_entities_runtime_state,
    mark_dirty_persistable_entities_spatial, report_persistence_worker_metrics,
    start_persistence_worker,
};
use crate::replication::physics_runtime::{
    enforce_planar_ship_motion, sync_simulated_ship_components,
};
use crate::replication::runtime_state::{
    PlayerControlDebugState, compute_controlled_entity_scanner_ranges,
    log_player_control_state_changes, sync_player_anchor_to_controlled_entity,
    update_client_observer_anchor_positions,
};
use crate::replication::simulation_entities::{
    PendingControlledByBindings, PlayerControlledEntityMap, PlayerRuntimeEntityMap,
    apply_pending_controlled_by_bindings, hydrate_simulation_entities,
    process_bootstrap_entity_commands,
};
use crate::replication::transport::ensure_server_transport_channels;
use crate::replication::view::ClientControlRequestOrder;
use crate::replication::view::receive_client_control_requests;
use crate::replication::visibility::{VisibilityScratch, update_network_visibility};
use avian3d::prelude::{
    Gravity, PhysicsInterpolationPlugin, PhysicsPlugins, PhysicsSystems, PhysicsTransformPlugin,
};
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::log::LogPlugin;
use bevy::log::info;
use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use lightyear::prelude::ReplicationBufferSystems;
use lightyear::prelude::server::ServerPlugins;
use sidereal_asset_runtime::default_asset_dependencies;
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_game::SiderealGamePlugin;
use sidereal_net::register_lightyear_protocol;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use visibility::{ClientObserverAnchorPositionMap, ClientVisibilityRegistry};

#[derive(Debug, Resource, Clone)]
#[allow(dead_code)]
pub(crate) struct BrpAuthToken(String);

#[derive(Debug, Resource, Clone, Copy)]
#[allow(dead_code)]
struct HydratedEntityCount(usize);

#[derive(Debug, Component)]
#[allow(dead_code)]
struct HydratedGraphEntity {
    entity_id: String,
    labels: Vec<String>,
    component_count: usize,
}

#[derive(Resource, Default)]
struct AuthenticatedClientBindings {
    by_client_entity: HashMap<Entity, String>,
    by_remote_id: HashMap<lightyear::prelude::PeerId, String>,
}

/// Chunk queued for paced sending to avoid UDP send-buffer overflow (EAGAIN).
pub(crate) struct PendingAssetChunk {
    pub(crate) asset_id: String,
    pub(crate) relative_cache_path: String,
    pub(crate) chunk_index: u32,
    pub(crate) chunk_count: u32,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Resource, Default)]
struct AssetStreamServerState {
    sent_asset_ids_by_remote: HashMap<lightyear::prelude::PeerId, HashSet<String>>,
    pending_requested_asset_ids_by_remote: HashMap<lightyear::prelude::PeerId, HashSet<String>>,
    acked_assets_by_remote: HashMap<lightyear::prelude::PeerId, HashMap<String, u64>>,
    /// Chunks to send per remote; drained at a fixed rate per frame to avoid EAGAIN.
    pub(crate) pending_chunks_by_remote:
        HashMap<lightyear::prelude::PeerId, std::collections::VecDeque<PendingAssetChunk>>,
}

#[derive(Resource, Default)]
struct AssetDependencyMap {
    dependencies_by_asset_id: HashMap<String, Vec<String>>,
}

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
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(ScenePlugin);
    app.add_plugins(LogPlugin::default());
    app.add_plugins(SiderealGamePlugin);
    app.add_plugins(
        PhysicsPlugins::default()
            .with_length_unit(1.0)
            .build()
            .disable::<PhysicsTransformPlugin>()
            .disable::<PhysicsInterpolationPlugin>(),
    );
    app.add_message::<bevy::asset::AssetEvent<Mesh>>();
    app.init_asset::<Mesh>();
    app.insert_resource(Gravity(Vec3::ZERO));
    app.add_plugins(ServerPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / 30.0),
    });
    register_lightyear_protocol(&mut app);
    configure_remote(&mut app, &remote_cfg);
    // (rust-analyzer may show "expected &[], found &[; 1]" here; compiler accepts it — macro expansion quirk)
    info!("replication running native world-sync runtime path");
    // Lightyear/Bevy plugins can initialize Fixed time; enforce authoritative 30 Hz after plugin wiring.
    app.insert_resource(Time::<Fixed>::from_hz(30.0));
    app.add_systems(
        Startup,
        (
            start_persistence_worker,
            hydrate_replication_world,
            hydrate_simulation_entities,
            start_lightyear_server,
        )
            .chain(),
    );
    app.add_systems(
        Startup,
        bootstrap_runtime::start_replication_control_listener,
    );
    app.add_systems(Startup, initialize_asset_stream_cache);
    app.add_observer(log_replication_client_connected);
    app.add_observer(setup_client_replication_sender);
    app.insert_resource(ClientVisibilityRegistry::default());
    app.insert_resource(VisibilityScratch::default());
    app.insert_resource(ClientObserverAnchorPositionMap::default());
    app.insert_resource(PlayerControlledEntityMap::default());
    app.insert_resource(PlayerRuntimeEntityMap::default());
    app.insert_resource(AuthenticatedClientBindings::default());
    app.insert_resource(AssetStreamServerState::default());
    app.insert_resource(StreamableAssetCache::default());
    app.insert_resource(ClientInputTickTracker::default());
    app.insert_resource(ClientInputDropMetrics::default());
    app.insert_resource(ClientInputDropMetricsLogState::default());
    app.insert_resource(InputActivityLogState::default());
    app.insert_resource(LatestRealtimeInputsByPlayer::default());
    app.insert_resource(AssetDependencyMap {
        dependencies_by_asset_id: default_asset_dependencies(),
    });
    app.insert_resource(PersistenceWorkerState::default());
    app.insert_resource(PersistenceDirtyState::default());
    app.insert_resource(SimulationPersistenceTimer::default());
    app.insert_resource(PendingControlledByBindings::default());
    app.insert_resource(ClientControlRequestOrder::default());
    app.insert_resource(PlayerControlDebugState::default());
    app.add_systems(
        Update,
        (
            ensure_server_transport_channels,
            cleanup_client_auth_bindings,
            receive_client_auth_messages,
            receive_latest_realtime_input_messages,
            receive_client_control_requests,
            receive_client_asset_requests,
            receive_client_asset_acks,
            report_input_drop_metrics,
            report_persistence_worker_metrics,
            process_bootstrap_entity_commands,
            log_player_control_state_changes.after(process_bootstrap_entity_commands),
        )
            .chain(),
    );
    app.add_systems(
        FixedUpdate,
        (
            stream_bootstrap_assets_to_authenticated_clients,
            send_asset_stream_chunks_paced.after(stream_bootstrap_assets_to_authenticated_clients),
        ),
    );
    app.add_systems(
        FixedUpdate,
        (
            sync_simulated_ship_components,
            sync_player_anchor_to_controlled_entity,
            update_client_observer_anchor_positions,
            compute_controlled_entity_scanner_ranges,
            update_network_visibility,
        )
            .chain()
            .after(PhysicsSystems::Writeback),
    );
    app.add_systems(
        FixedUpdate,
        (
            mark_dirty_persistable_entities_spatial,
            mark_dirty_persistable_entities_runtime_state,
            mark_dirty_persistable_entities_modules,
            mark_dirty_persistable_entities_gameplay,
        )
            .after(PhysicsSystems::Writeback),
    );
    app.add_systems(
        FixedUpdate,
        flush_simulation_state_persistence.after(update_network_visibility),
    );
    app.add_systems(
        FixedUpdate,
        enforce_planar_ship_motion.before(PhysicsSystems::Prepare),
    );
    app.add_systems(
        FixedUpdate,
        drain_native_player_inputs_to_action_queue.before(PhysicsSystems::Prepare),
    );
    app.add_systems(
        PostUpdate,
        apply_pending_controlled_by_bindings.after(ReplicationBufferSystems::AfterBuffer),
    );
    app.run();
}

#[cfg(test)]
mod tests;
