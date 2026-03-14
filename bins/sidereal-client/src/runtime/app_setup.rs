use super::*;

use avian2d::prelude::*;
use bevy::prelude::*;
use lightyear::avian2d::plugin::AvianReplicationMode;
use lightyear::avian2d::prelude::LightyearAvianPlugin;
use lightyear::frame_interpolation::FrameInterpolationPlugin;
use lightyear::input::native::prelude::NativeStateSequence;
use lightyear::input::plugin::InputPlugin as LightyearInputProtocolPlugin;
use lightyear::prelude::client::{Client, ClientPlugins, Connected};
#[cfg(not(target_arch = "wasm32"))]
use lightyear::prelude::input::native::ClientInputPlugin as NativeClientInputPlugin;
use sidereal_core::SIM_TICK_HZ;
use sidereal_game::{
    BallisticProjectileSpawnedEvent, CombatAuthorityEnabled, EntityDestroyedEvent, ShotFiredEvent,
    ShotHitEvent, ShotImpactResolvedEvent, apply_engine_thrust, bootstrap_weapon_cooldown_state,
    clamp_angular_velocity, process_character_movement_actions, process_flight_actions,
    process_weapon_fire_actions, stabilize_idle_motion, sync_mounted_hierarchy,
    tick_weapon_cooldowns, update_ballistic_projectiles,
};
use sidereal_net::register_lightyear_client_protocol;
use sidereal_runtime_sync::RuntimeEntityHierarchy;
use std::time::Duration;

fn init_transport_resources(
    app: &mut App,
    headless_transport: bool,
    gateway_http_adapter: GatewayHttpAdapter,
    asset_cache_adapter: AssetCacheAdapter,
) {
    app.insert_resource(ClientSession::default());
    app.insert_resource(PendingDisconnectNotify::default());
    app.insert_resource(PendingDisconnectNotifySent::default());
    app.insert_resource(LogoutCleanupRequested::default());
    app.insert_resource(DisconnectRequest::default());
    app.insert_resource(PauseMenuState::default());
    app.insert_resource(ClientNetworkTick::default());
    app.insert_resource(ClientInputAckTracker::default());
    app.insert_resource(ClientInputSendState::default());
    app.insert_resource(ClientAuthSyncState::default());
    app.insert_resource(SessionReadyWatchdogConfig::from_env());
    app.insert_resource(SessionReadyWatchdogState::default());
    app.insert_resource(gateway_http_adapter);
    app.insert_resource(asset_cache_adapter);
    app.insert_resource(HeadlessTransportMode(headless_transport));
}

fn init_asset_runtime_resources(app: &mut App, asset_root: String) {
    app.insert_resource(AssetRootPath(asset_root));
    app.insert_resource(assets::LocalAssetManager::default());
    app.insert_resource(assets::AssetCatalogHotReloadState::default());
    app.insert_resource(assets::RuntimeAssetDependencyState::default());
    app.insert_resource(assets::RuntimeAssetDependencyDirtyState::default());
    app.insert_resource(assets::RuntimeAssetNetIndicatorState::default());
    app.insert_resource(assets::RuntimeAssetHttpFetchState::default());
    app.insert_resource(RuntimeAssetPerfCounters::default());
    app.insert_resource(OwnedAssetManifestCache::default());
    audio::init_audio_runtime(app);
}

fn init_control_and_prediction_resources(app: &mut App) {
    app.insert_resource(CombatAuthorityEnabled(false));
    app.insert_resource(MotionOwnershipReconcileState {
        dirty: true,
        ..default()
    });
    app.insert_resource(ClientControlRequestState::default());
    app.insert_resource(ClientViewModeState::default());
    app.insert_resource(LocalPlayerViewState::default());
    app.insert_resource(RuntimeEntityHierarchy::default());
    app.insert_resource(BootstrapWatchdogState::default());
    app.insert_resource(DeferredPredictedAdoptionState::default());
    app.insert_resource(PredictionBootstrapTuning::from_env());
    app.insert_resource(PredictionCorrectionTuning::from_env());
    app.insert_resource(NearbyCollisionProxyTuning::from_env());
    app.insert_resource(RemoteEntityRegistry::default());
}

fn init_debug_and_diagnostics_resources(app: &mut App, headless_transport: bool) {
    app.insert_resource(DebugOverlayState::default());
    app.insert_resource(DebugOverlaySnapshot::default());
    app.insert_resource(shaders::RuntimeShaderAssignments::default());
    app.insert_resource(shaders::RuntimeShaderAssignmentSyncState::default());
    if headless_transport {
        app.init_resource::<dialog_ui::DialogQueue>();
    }
}

fn init_tactical_resources(app: &mut App) {
    app.insert_resource(NameplateUiState::default());
    app.insert_resource(HudPerfCounters::default());
    app.insert_resource(CharacterSelectionState::default());
    app.insert_resource(FreeCameraState::default());
    app.insert_resource(OwnedEntitiesPanelState::default());
    app.insert_resource(TacticalFogCache::default());
    app.insert_resource(TacticalContactsCache::default());
    app.insert_resource(TacticalResnapshotRequestState::default());
    app.insert_resource(TacticalMapUiState::default());
    app.insert_resource(SessionReadyState::default());
}

fn init_scene_and_render_resources(app: &mut App) {
    app.insert_resource(FullscreenExternalWorldData::default());
    app.insert_resource(StarfieldMotionState::default());
    app.insert_resource(CameraMotionState::default());
}

pub(crate) fn configure_client_runtime(
    app: &mut App,
    asset_root: String,
    headless_transport: bool,
    gateway_http_adapter: GatewayHttpAdapter,
    asset_cache_adapter: AssetCacheAdapter,
) {
    app.add_plugins(
        PhysicsPlugins::default()
            .with_length_unit(1.0)
            .build()
            .disable::<PhysicsTransformPlugin>()
            .disable::<PhysicsInterpolationPlugin>(),
    );
    app.insert_resource(Gravity(Vec2::ZERO));
    crate::client_core::configure_shared_client_core(app);
    app.add_plugins(ClientPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / f64::from(SIM_TICK_HZ)),
    });
    app.add_plugins(LightyearAvianPlugin {
        replication_mode: AvianReplicationMode::PositionButInterpolateTransform,
        update_syncs_manually: false,
        rollback_resources: false,
        rollback_islands: false,
    });
    app.add_plugins(FrameInterpolationPlugin::<Transform>::default());
    app.add_plugins(LightyearInputProtocolPlugin::<
        NativeStateSequence<sidereal_net::PlayerInput>,
    >::default());
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(NativeClientInputPlugin::<sidereal_net::PlayerInput>::default());
    register_lightyear_client_protocol(app);
    app.add_message::<ShotFiredEvent>();
    app.add_message::<ShotImpactResolvedEvent>();
    app.add_message::<ShotHitEvent>();
    app.add_message::<BallisticProjectileSpawnedEvent>();
    app.add_message::<EntityDestroyedEvent>();
    app.add_message::<combat_messages::RemoteWeaponFiredRuntimeMessage>();
    app.add_message::<combat_messages::RemoteEntityDestructionRuntimeMessage>();

    app.insert_resource(Time::<Fixed>::from_hz(f64::from(SIM_TICK_HZ)));
    init_transport_resources(
        app,
        headless_transport,
        gateway_http_adapter,
        asset_cache_adapter,
    );
    init_asset_runtime_resources(app, asset_root);
    init_control_and_prediction_resources(app);
    init_debug_and_diagnostics_resources(app, headless_transport);
    init_tactical_resources(app);
    init_scene_and_render_resources(app);
    app.add_systems(
        Update,
        (
            audio::sync_audio_catalog_defaults_system,
            audio::queue_audio_asset_demands_system
                .after(audio::sync_audio_catalog_defaults_system),
            audio::sync_audio_runtime_system.after(audio::sync_audio_catalog_defaults_system),
            audio::sync_audio_listener_system.after(audio::sync_audio_runtime_system),
        ),
    );
    app.add_systems(
        Update,
        combat_messages::fanout_remote_weapon_fired_messages_system
            .before(audio::receive_remote_weapon_fire_audio_system)
            .before(visuals::receive_remote_weapon_tracer_messages_system)
            .run_if(in_state(ClientAppState::InWorld)),
    );
    app.add_systems(
        Update,
        combat_messages::fanout_remote_destruction_messages_system
            .before(audio::receive_remote_destruction_audio_system)
            .before(visuals::receive_remote_destruction_effect_messages_system)
            .run_if(in_state(ClientAppState::InWorld)),
    );
    app.add_systems(
        Update,
        (
            audio::receive_local_weapon_fire_audio_system,
            audio::receive_local_destruction_audio_system,
            audio::receive_remote_weapon_fire_audio_system,
            audio::receive_remote_destruction_audio_system,
            audio::debug_audio_probe_system,
        )
            .run_if(in_state(ClientAppState::InWorld)),
    );
    app.add_systems(
        FixedUpdate,
        (
            motion::enforce_motion_ownership_for_world_entities,
            motion::sync_controlled_mass_from_total_mass,
            process_character_movement_actions,
            process_flight_actions,
            bootstrap_weapon_cooldown_state,
            tick_weapon_cooldowns,
            process_weapon_fire_actions,
            replication::mark_new_ballistic_projectiles_prespawned,
            update_ballistic_projectiles,
            apply_engine_thrust,
        )
            .chain()
            .before(avian2d::prelude::PhysicsSystems::StepSimulation),
    );
    app.add_systems(FixedPreUpdate, motion::mark_motion_ownership_dirty_signals);
    app.add_systems(
        FixedUpdate,
        (stabilize_idle_motion, clamp_angular_velocity)
            .chain()
            .after(avian2d::prelude::PhysicsSystems::StepSimulation),
    );
    app.add_systems(
        PostUpdate,
        sync_mounted_hierarchy.before(bevy::transform::TransformSystems::Propagate),
    );
    app.add_observer(log_client_transport_connected);
    app.add_observer(transport::ensure_raw_client_connected_after_linked);
    app.add_plugins(plugins::ClientBootstrapPlugin {
        headless: headless_transport,
    });
    app.add_plugins(plugins::ClientTransportPlugin {
        headless: headless_transport,
    });
    app.add_plugins(plugins::ClientReplicationPlugin {
        headless: headless_transport,
    });
    app.add_plugins(plugins::ClientPredictionPlugin {
        headless: headless_transport,
    });
    if !headless_transport {
        app.add_plugins(post_process::ExplosionDistortionPostProcessPlugin);
        app.add_plugins(plugins::ClientVisualsPlugin);
        app.add_plugins(plugins::ClientLightingPlugin);
        app.add_plugins(plugins::ClientUiPlugin);
        app.add_plugins(plugins::ClientDiagnosticsPlugin);
    }
}

fn log_client_transport_connected(
    trigger: On<Add, Connected>,
    clients: Query<'_, '_, (), With<Client>>,
) {
    if clients.get(trigger.entity).is_ok() {
        info!("client lightyear transport connected");
    }
}
