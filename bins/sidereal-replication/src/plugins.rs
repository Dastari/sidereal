use avian2d::prelude::PhysicsSystems;
use bevy::prelude::*;

use crate::bootstrap_runtime;
use crate::replication::persistence::{
    mark_dirty_persistable_entities, mark_dirty_persistable_entities_spatial,
};
use crate::replication::{
    assets, auth, combat, control, input, lifecycle, persistence, runtime_scripting, runtime_state,
    simulation_entities, visibility,
};

pub(crate) struct ReplicationLifecyclePlugin;

impl Plugin for ReplicationLifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (
                simulation_entities::hydrate_simulation_entities,
                lifecycle::start_lightyear_server,
            )
                .chain(),
        );
        app.add_systems(
            Startup,
            bootstrap_runtime::start_replication_control_listener,
        );
        app.add_observer(lifecycle::log_replication_client_connected);
        app.add_observer(lifecycle::setup_client_replication_sender);
    }
}

pub(crate) struct ReplicationAuthPlugin;

impl Plugin for ReplicationAuthPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, auth::receive_client_auth_messages);
    }
}

pub(crate) struct ReplicationInputPlugin;

impl Plugin for ReplicationInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            input::drain_native_player_inputs_to_action_queue.before(PhysicsSystems::Prepare),
        );
    }
}

pub(crate) struct ReplicationControlPlugin;

impl Plugin for ReplicationControlPlugin {
    fn build(&self, app: &mut App) {
        combat::init_resources(app);
        app.add_systems(
            FixedUpdate,
            (
                control::sync_player_anchor_replication_mode,
                combat::broadcast_weapon_fired_messages
                    .after(sidereal_game::process_weapon_fire_actions),
            )
                .chain()
                .after(PhysicsSystems::Writeback),
        );
    }
}

pub(crate) struct ReplicationVisibilityPlugin;

impl Plugin for ReplicationVisibilityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                simulation_entities::sync_controlled_entity_transforms,
                runtime_state::update_client_observer_anchor_positions,
                runtime_state::compute_controlled_entity_scanner_ranges,
                visibility::ensure_network_visibility_for_replicated_entities,
                visibility::update_network_visibility,
            )
                .chain()
                .after(PhysicsSystems::Writeback),
        );
    }
}

pub(crate) struct ReplicationPersistencePlugin;

impl Plugin for ReplicationPersistencePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, persistence::start_persistence_worker);
        app.add_systems(
            FixedUpdate,
            (
                mark_dirty_persistable_entities,
                mark_dirty_persistable_entities_spatial,
            )
                .after(PhysicsSystems::Writeback),
        );
        app.add_systems(
            FixedUpdate,
            persistence::flush_simulation_state_persistence
                .after(visibility::update_network_visibility),
        );
    }
}

pub(crate) struct ReplicationRuntimeScriptingPlugin;

impl Plugin for ReplicationRuntimeScriptingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                runtime_scripting::refresh_script_world_snapshot,
                runtime_scripting::run_script_intervals,
                runtime_scripting::run_script_events,
                runtime_scripting::apply_script_intents
                    .before(sidereal_game::process_flight_actions),
            )
                .chain()
                .before(PhysicsSystems::Prepare),
        );
    }
}

pub(crate) struct ReplicationAssetsPlugin;

impl Plugin for ReplicationAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, assets::initialize_asset_stream_cache);
        app.add_systems(
            FixedUpdate,
            (
                assets::stream_bootstrap_assets_to_authenticated_clients,
                assets::send_asset_stream_chunks_paced
                    .after(assets::stream_bootstrap_assets_to_authenticated_clients),
            ),
        );
    }
}

pub(crate) struct ReplicationBootstrapBridgePlugin;

impl Plugin for ReplicationBootstrapBridgePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            simulation_entities::apply_pending_controlled_by_bindings
                .after(lightyear::prelude::ReplicationBufferSystems::AfterBuffer),
        );
    }
}
