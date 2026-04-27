use super::backend::AudioBackendResource;
use super::catalog::AudioCatalogState;
use super::settings::{AudioBusSettings, AudioSettings};
use super::state::AudioAssetDemandState;
#[cfg(not(target_arch = "wasm32"))]
use crate::runtime::assets::LocalAssetManager;
use crate::runtime::assets::{AssetCatalogHotReloadState, RuntimeAssetDependencyState};
use crate::runtime::combat_messages::{
    RemoteEntityDestructionRuntimeMessage, RemoteWeaponFiredRuntimeMessage,
};
use crate::runtime::components::GameplayCamera;
#[cfg(not(target_arch = "wasm32"))]
use crate::runtime::resources::{AssetCacheAdapter, AssetRootPath};
use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use sidereal_game::{BallisticWeapon, EntityGuid};
use sidereal_game::{EntityDestroyedEvent, ShotFiredEvent};
use std::collections::HashSet;

#[cfg(not(target_arch = "wasm32"))]
use super::native_backend::{
    AudioAssetResolver, DebugProbeMode, LoopEmitterRequest, OneShotRequest,
};

pub(crate) fn sync_audio_catalog_defaults_system(
    catalog: Res<'_, AudioCatalogState>,
    mut settings: ResMut<'_, AudioSettings>,
) {
    let Some(version) = catalog.version.as_ref() else {
        return;
    };
    if settings.initialized_catalog_version.as_deref() == Some(version.as_str()) {
        return;
    }
    if let Some(registry) = catalog.registry() {
        for bus in &registry.buses {
            settings
                .buses
                .entry(bus.bus_id.clone())
                .or_insert(AudioBusSettings {
                    volume_db: bus.default_volume_db.unwrap_or(0.0),
                    muted: bus.muted.unwrap_or(false),
                });
        }
    }
    settings.initialized_catalog_version = Some(version.clone());
}

pub(crate) fn sync_audio_runtime_system(
    mut backend: NonSendMut<'_, AudioBackendResource>,
    catalog: Res<'_, AudioCatalogState>,
    settings: Res<'_, AudioSettings>,
    time: Res<'_, Time>,
) {
    backend.sync_graph(&catalog, &settings);
    backend.tick(time.elapsed_secs_f64(), &settings);
}

pub(crate) fn sync_audio_listener_system(
    mut backend: NonSendMut<'_, AudioBackendResource>,
    camera_query: Query<'_, '_, &'_ GlobalTransform, With<GameplayCamera>>,
) {
    let Some(transform) = camera_query.iter().next() else {
        return;
    };
    let (_, rotation, translation) = transform.to_scale_rotation_translation();
    backend.sync_listener(translation, rotation);
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn ensure_menu_music_system(
    mut backend: NonSendMut<'_, AudioBackendResource>,
    catalog: Res<'_, AudioCatalogState>,
    settings: Res<'_, AudioSettings>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
    mut demand: ResMut<'_, AudioAssetDemandState>,
) {
    ensure_music_profile(
        &mut backend,
        &catalog,
        &settings,
        &asset_root,
        &asset_manager,
        *cache_adapter,
        &mut demand,
        "music.menu.standard",
        "main",
    );
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn ensure_menu_music_system() {}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn ensure_world_music_system(
    mut backend: NonSendMut<'_, AudioBackendResource>,
    catalog: Res<'_, AudioCatalogState>,
    settings: Res<'_, AudioSettings>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
    mut demand: ResMut<'_, AudioAssetDemandState>,
) {
    ensure_music_profile(
        &mut backend,
        &catalog,
        &settings,
        &asset_root,
        &asset_manager,
        *cache_adapter,
        &mut demand,
        "music.world.deep_space",
        "main",
    );
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn ensure_world_music_system() {}

pub(crate) fn debug_audio_probe_system(
    #[cfg(not(target_arch = "wasm32"))] keys: Res<'_, ButtonInput<KeyCode>>,
    #[cfg(not(target_arch = "wasm32"))] mut backend: NonSendMut<'_, AudioBackendResource>,
    #[cfg(not(target_arch = "wasm32"))] catalog: Res<'_, AudioCatalogState>,
    #[cfg(not(target_arch = "wasm32"))] asset_root: Res<'_, AssetRootPath>,
    #[cfg(not(target_arch = "wasm32"))] asset_manager: Res<'_, LocalAssetManager>,
    #[cfg(not(target_arch = "wasm32"))] cache_adapter: Res<'_, AssetCacheAdapter>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let resolver = AudioAssetResolver {
            asset_root: &asset_root.0,
            asset_manager: &asset_manager,
            cache_adapter: *cache_adapter,
        };
        if keys.just_pressed(KeyCode::F6) {
            backend.play_debug_probe(
                DebugProbeMode::Nonspatial,
                "destruction.asteroid.default",
                "explode",
                &catalog,
                &resolver,
            );
        }
        if keys.just_pressed(KeyCode::F7) {
            backend.play_debug_probe(
                DebugProbeMode::SpatialAtListener,
                "destruction.asteroid.default",
                "explode",
                &catalog,
                &resolver,
            );
        }
        if keys.just_pressed(KeyCode::F8) {
            backend.play_debug_probe(
                DebugProbeMode::SpatialOffsetRight,
                "destruction.asteroid.default",
                "explode",
                &catalog,
                &resolver,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn receive_local_destruction_audio_system(
    mut backend: NonSendMut<'_, AudioBackendResource>,
    catalog: Res<'_, AudioCatalogState>,
    settings: Res<'_, AudioSettings>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
    mut demand: ResMut<'_, AudioAssetDemandState>,
    mut events: MessageReader<'_, '_, EntityDestroyedEvent>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    let resolver = AudioAssetResolver {
        asset_root: &asset_root.0,
        asset_manager: &asset_manager,
        cache_adapter: *cache_adapter,
    };
    for event in events.read() {
        #[cfg(not(target_arch = "wasm32"))]
        backend.play_one_shot(
            OneShotRequest {
                profile_id: event.destruction_profile_id.as_str(),
                cue_id: "explode",
                position: Some(event.effect_origin.as_vec2()),
            },
            &resolver,
            &catalog,
            &settings,
            &mut demand.desired_asset_ids,
        );
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn receive_local_destruction_audio_system(
    mut events: MessageReader<'_, '_, EntityDestroyedEvent>,
) {
    for _ in events.read() {}
}

#[allow(clippy::too_many_arguments)]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn receive_remote_destruction_audio_system(
    mut backend: NonSendMut<'_, AudioBackendResource>,
    catalog: Res<'_, AudioCatalogState>,
    settings: Res<'_, AudioSettings>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
    mut demand: ResMut<'_, AudioAssetDemandState>,
    mut events: MessageReader<'_, '_, RemoteEntityDestructionRuntimeMessage>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    let resolver = AudioAssetResolver {
        asset_root: &asset_root.0,
        asset_manager: &asset_manager,
        cache_adapter: *cache_adapter,
    };
    for event in events.read() {
        let message = &event.message;
        #[cfg(not(target_arch = "wasm32"))]
        backend.play_one_shot(
            OneShotRequest {
                profile_id: message.destruction_profile_id.as_str(),
                cue_id: "explode",
                position: Some(Vec2::new(
                    message.origin_xy[0] as f32,
                    message.origin_xy[1] as f32,
                )),
            },
            &resolver,
            &catalog,
            &settings,
            &mut demand.desired_asset_ids,
        );
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn receive_remote_destruction_audio_system(
    mut events: MessageReader<'_, '_, RemoteEntityDestructionRuntimeMessage>,
) {
    for _ in events.read() {}
}

#[allow(clippy::too_many_arguments)]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn receive_local_weapon_fire_audio_system(
    mut backend: NonSendMut<'_, AudioBackendResource>,
    catalog: Res<'_, AudioCatalogState>,
    settings: Res<'_, AudioSettings>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
    time: Res<'_, Time>,
    mut demand: ResMut<'_, AudioAssetDemandState>,
    weapons: Query<'_, '_, (&'_ BallisticWeapon, &'_ EntityGuid)>,
    mut events: MessageReader<'_, '_, ShotFiredEvent>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    let resolver = AudioAssetResolver {
        asset_root: &asset_root.0,
        asset_manager: &asset_manager,
        cache_adapter: *cache_adapter,
    };
    for event in events.read() {
        let Ok((weapon, weapon_guid)) = weapons.get(event.weapon_entity) else {
            continue;
        };
        let Some(profile_id) = resolve_weapon_fire_profile_id(weapon) else {
            continue;
        };
        let release_timeout_s = (weapon.cooldown_seconds() as f64 * 1.75).max(0.14);
        #[cfg(not(target_arch = "wasm32"))]
        backend.trigger_loop_emitter(
            LoopEmitterRequest {
                key: weapon_guid.0.to_string(),
                profile_id,
                cue_id: "fire",
                position: event.origin.as_vec2(),
                release_timeout_s,
                now_s: time.elapsed_secs_f64(),
            },
            &resolver,
            &catalog,
            &settings,
            &mut demand.desired_asset_ids,
        );
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn receive_local_weapon_fire_audio_system(
    mut events: MessageReader<'_, '_, ShotFiredEvent>,
) {
    for _ in events.read() {}
}

#[allow(clippy::too_many_arguments)]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn receive_remote_weapon_fire_audio_system(
    mut backend: NonSendMut<'_, AudioBackendResource>,
    catalog: Res<'_, AudioCatalogState>,
    settings: Res<'_, AudioSettings>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
    time: Res<'_, Time>,
    mut demand: ResMut<'_, AudioAssetDemandState>,
    mut events: MessageReader<'_, '_, RemoteWeaponFiredRuntimeMessage>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    let resolver = AudioAssetResolver {
        asset_root: &asset_root.0,
        asset_manager: &asset_manager,
        cache_adapter: *cache_adapter,
    };
    for event in events.read() {
        let message = &event.message;
        let Some(profile_id) = message.audio_profile_id.as_deref() else {
            continue;
        };
        let cooldown_s = message.cooldown_s.unwrap_or(60.0 / 750.0);
        #[cfg(not(target_arch = "wasm32"))]
        backend.trigger_loop_emitter(
            LoopEmitterRequest {
                key: message.weapon_guid.clone(),
                profile_id,
                cue_id: "fire",
                position: Vec2::new(message.origin_xy[0] as f32, message.origin_xy[1] as f32),
                release_timeout_s: (f64::from(cooldown_s) * 1.75).max(0.14),
                now_s: time.elapsed_secs_f64(),
            },
            &resolver,
            &catalog,
            &settings,
            &mut demand.desired_asset_ids,
        );
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn receive_remote_weapon_fire_audio_system(
    mut events: MessageReader<'_, '_, RemoteWeaponFiredRuntimeMessage>,
) {
    for _ in events.read() {}
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_weapon_fire_profile_id(weapon: &BallisticWeapon) -> Option<&str> {
    if let Some(profile_id) = weapon.fire_audio_profile_id.as_deref() {
        return Some(profile_id);
    }
    // Transitional fallback for pre-audio-profile authored gatlings until all bundles carry
    // explicit fire_audio_profile_id values.
    (weapon.weapon_name == "Ballistic Gatling").then_some("weapon.ballistic_gatling")
}

pub(crate) fn queue_audio_asset_demands_system(
    catalog: Res<'_, AudioCatalogState>,
    mut dependency_state: ResMut<'_, RuntimeAssetDependencyState>,
    mut hot_reload: ResMut<'_, AssetCatalogHotReloadState>,
    demand: Res<'_, AudioAssetDemandState>,
) {
    let mut requested_asset_ids = HashSet::new();
    for asset_id in &demand.desired_asset_ids {
        requested_asset_ids.insert(asset_id.clone());
    }
    for asset_id in &demand.critical_asset_ids {
        requested_asset_ids.insert(asset_id.clone());
        dependency_state.critical_asset_ids.insert(asset_id.clone());
    }
    for asset_id in requested_asset_ids {
        dependency_state
            .candidate_asset_ids
            .insert(asset_id.clone());
        dependency_state
            .lower_value_asset_ids
            .insert(asset_id.clone());
        hot_reload.forced_asset_ids.insert(asset_id);
    }
    if let Some(profile_asset_ids) = catalog.profile_asset_ids("music.menu.standard") {
        for asset_id in profile_asset_ids {
            dependency_state
                .candidate_asset_ids
                .insert(asset_id.clone());
            dependency_state
                .lower_value_asset_ids
                .insert(asset_id.clone());
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg(not(target_arch = "wasm32"))]
fn ensure_music_profile(
    backend: &mut NonSendMut<'_, AudioBackendResource>,
    catalog: &AudioCatalogState,
    settings: &AudioSettings,
    asset_root: &AssetRootPath,
    asset_manager: &LocalAssetManager,
    cache_adapter: AssetCacheAdapter,
    demand: &mut AudioAssetDemandState,
    profile_id: &str,
    cue_id: &str,
) {
    let Some(profile_asset_ids) = catalog.profile_asset_ids(profile_id) else {
        return;
    };
    if catalog.cue(profile_id, cue_id).is_none() {
        return;
    }
    if !profile_asset_ids
        .iter()
        .all(|asset_id| asset_manager.is_asset_ready(asset_id))
    {
        return;
    }
    let resolver = AudioAssetResolver {
        asset_root: &asset_root.0,
        asset_manager,
        cache_adapter,
    };
    backend.ensure_music(
        profile_id,
        cue_id,
        catalog,
        settings,
        &resolver,
        &mut demand.desired_asset_ids,
        &mut demand.critical_asset_ids,
    );
}
