use avian2d::prelude::PhysicsSystems;
use bevy::prelude::*;

use crate::bootstrap_runtime;
use crate::replication::persistence::{
    mark_dirty_persistable_entities, mark_dirty_persistable_entities_spatial,
};
use crate::replication::{
    combat, control, input, lifecycle, owner_manifest, persistence, runtime_scripting,
    runtime_state, simulation_entities, tactical, visibility,
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
        app.add_observer(lifecycle::prime_client_link_transport_on_insert);
    }
}

pub(crate) struct ReplicationAuthPlugin;

impl Plugin for ReplicationAuthPlugin {
    fn build(&self, _app: &mut App) {}
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
                combat::mark_new_ballistic_projectiles_prespawned
                    .after(sidereal_game::process_weapon_fire_actions),
                combat::configure_ballistic_projectile_replication
                    .after(combat::mark_new_ballistic_projectiles_prespawned),
                combat::broadcast_weapon_fired_messages.after(sidereal_game::resolve_shot_impacts),
                combat::enqueue_runtime_script_events_from_combat_messages
                    .after(sidereal_game::apply_damage_from_shot_impacts),
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
                simulation_entities::sync_world_entity_transforms_from_world_space,
                runtime_state::update_client_observer_anchor_positions,
                runtime_state::compute_controlled_entity_visibility_ranges,
                visibility::ensure_network_visibility_for_replicated_entities,
                visibility::update_network_visibility,
                owner_manifest::stream_owner_asset_manifest_messages,
                tactical::receive_tactical_resnapshot_requests,
                tactical::stream_tactical_snapshot_messages,
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
