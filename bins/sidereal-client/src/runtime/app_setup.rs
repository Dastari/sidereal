use super::*;

use avian2d::physics_transform::PhysicsTransformConfig;
use avian2d::prelude::*;
use bevy::prelude::*;
use lightyear::avian2d::plugin::AvianReplicationMode;
use lightyear::avian2d::prelude::LightyearAvianPlugin;
use lightyear::frame_interpolation::FrameInterpolationPlugin;
use lightyear::input::native::prelude::InputPlugin as NativeInputPlugin;
use lightyear::prelude::client::{Client, ClientPlugins, Connected};
use sidereal_core::SIM_TICK_HZ;
use sidereal_game::{
    BallisticProjectileSpawnedEvent, CombatAuthorityEnabled, EntityDestroyedEvent,
    FlightFuelConsumptionEnabled, ShotFiredEvent, ShotHitEvent, ShotImpactResolvedEvent,
    SiderealSharedSimulationPlugin, SiderealSimulationSet, SimulationRuntimeRole,
    process_weapon_fire_actions, sync_mounted_hierarchy, update_ballistic_projectiles,
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
    app.insert_resource(FlightFuelConsumptionEnabled(false));
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
    app.insert_resource(ControlBootstrapState::default());
    app.insert_resource(PredictionBootstrapTuning::from_env());
    app.insert_resource(PredictionCorrectionTuning::from_env());
    app.insert_resource(ClientInputTimelineTuning::from_env());
    app.insert_resource(ClientInterpolationTimelineTuning::from_env());
    app.insert_resource(ClientTimelineFocusState::default());
    app.insert_resource(NativePredictionRecoveryTuning::from_env());
    app.insert_resource(NativePredictionRecoveryState::default());
    app.insert_resource(NearbyCollisionProxyTuning::from_env());
    app.insert_resource(RemoteEntityRegistry::default());
}

fn init_debug_and_diagnostics_resources(app: &mut App, headless_transport: bool) {
    app.insert_resource(DebugOverlayState::default());
    app.insert_resource(DebugOverlaySnapshot::default());
    app.insert_resource(AnnotationCalloutRegistry::default());
    app.insert_resource(RuntimeStallDiagnostics::default());
    app.insert_resource(shaders::RuntimeShaderAssignments::default());
    app.insert_resource(shaders::RuntimeShaderAssignmentSyncState::default());
    if headless_transport {
        app.init_resource::<dialog_ui::DialogQueue>();
    }
}

fn init_tactical_resources(app: &mut App) {
    app.insert_resource(NameplateUiState::default());
    app.insert_resource(NameplateRegistry::default());
    app.insert_resource(HudPerfCounters::default());
    app.insert_resource(CharacterSelectionState::default());
    app.insert_resource(FreeCameraState::default());
    app.insert_resource(OwnedEntitiesPanelState::default());
    app.insert_resource(TacticalFogCache::default());
    app.insert_resource(TacticalContactsCache::default());
    app.insert_resource(TacticalResnapshotRequestState::default());
    app.insert_resource(TacticalMapUiState::default());
    app.insert_resource(TacticalSensorRingUiState::default());
    app.insert_resource(ActiveScannerProfileCache::default());
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
    app.insert_resource(Gravity(Vec2::ZERO.into()));
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
    // Sidereal's simulation state is Position/Rotation-owned. In
    // PositionButInterpolateTransform mode Lightyear/Avian enables a pre-physics
    // Transform -> Position sync by default; that lets visual correction or stale
    // render transforms overwrite predicted physics state and causes local control
    // to snap back toward the confirmed visual lane.
    app.world_mut()
        .resource_mut::<PhysicsTransformConfig>()
        .transform_to_position = false;
    app.add_plugins(FrameInterpolationPlugin::<Transform>::default());
    app.add_plugins(NativeInputPlugin::<sidereal_net::PlayerInput>::default());
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
    app.add_plugins(SiderealSharedSimulationPlugin {
        role: SimulationRuntimeRole::ClientPrediction,
    });
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
        )
            .chain()
            .before(SiderealSimulationSet::SimulateGameplay),
    );
    app.add_systems(
        FixedUpdate,
        replication::mark_new_ballistic_projectiles_prespawned
            .after(process_weapon_fire_actions)
            .before(update_ballistic_projectiles)
            .in_set(SiderealSimulationSet::SimulateGameplay),
    );
    app.add_systems(FixedPreUpdate, motion::mark_motion_ownership_dirty_signals);
    app.add_systems(
        PostUpdate,
        sync_mounted_hierarchy.before(bevy::transform::TransformSystems::Propagate),
    );
    app.add_observer(log_client_transport_connected);
    app.add_observer(transport::configure_client_input_timeline_on_add);
    app.add_observer(transport::configure_client_interpolation_timeline_on_add);
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
