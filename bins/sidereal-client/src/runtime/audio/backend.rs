use super::catalog::AudioCatalogState;
#[cfg(not(target_arch = "wasm32"))]
use super::native_backend::{
    AudioAssetResolver, DebugProbeMode, LoopEmitterRequest, NativeAudioBackend, OneShotRequest,
    load_clip_asset_id,
};
use super::null_backend::NullAudioBackend;
use super::settings::AudioSettings;
use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashSet;

pub(super) enum AudioBackendKind {
    #[cfg(not(target_arch = "wasm32"))]
    Native(Box<NativeAudioBackend>),
    Null(NullAudioBackend),
}

pub(crate) struct AudioBackendResource {
    backend: AudioBackendKind,
    logged_errors: std::collections::HashSet<String>,
}

impl Default for AudioBackendResource {
    fn default() -> Self {
        let backend = {
            #[cfg(not(target_arch = "wasm32"))]
            {
                match NativeAudioBackend::new() {
                    Ok(backend) => AudioBackendKind::Native(Box::new(backend)),
                    Err(err) => {
                        warn!("audio backend unavailable; falling back to null backend: {err}");
                        AudioBackendKind::Null(NullAudioBackend)
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                AudioBackendKind::Null(NullAudioBackend)
            }
        };
        Self {
            backend,
            logged_errors: std::collections::HashSet::new(),
        }
    }
}

impl AudioBackendResource {
    pub(super) fn sync_graph(&mut self, catalog: &AudioCatalogState, settings: &AudioSettings) {
        #[cfg(not(target_arch = "wasm32"))]
        if let AudioBackendKind::Native(backend) = &mut self.backend
            && let Err(err) = backend.sync_graph(catalog, settings)
        {
            self.log_once(format!("audio graph sync failed: {err}"));
        }
    }

    pub(super) fn sync_listener(&mut self, position: Vec3, rotation: Quat) {
        #[cfg(not(target_arch = "wasm32"))]
        if let AudioBackendKind::Native(backend) = &mut self.backend {
            backend.sync_listener(position, rotation);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn ensure_music(
        &mut self,
        profile_id: &str,
        cue_id: &str,
        catalog: &AudioCatalogState,
        settings: &AudioSettings,
        resolver: &AudioAssetResolver<'_>,
        demand_asset_ids: &mut HashSet<String>,
        critical_asset_ids: &mut HashSet<String>,
    ) {
        let AudioBackendKind::Native(backend) = &mut self.backend else {
            return;
        };
        if let Err(err) = backend.sync_graph(catalog, settings) {
            self.log_once(format!("audio graph sync failed: {err}"));
            return;
        }
        backend.sync_settings(settings);
        let Some(asset_id) = load_clip_asset_id(catalog, profile_id, cue_id) else {
            self.log_once(format!(
                "audio music cue missing profile_id={profile_id} cue_id={cue_id}"
            ));
            return;
        };
        demand_asset_ids.insert(asset_id.clone());
        critical_asset_ids.insert(asset_id.clone());
        if let Err(err) = backend.ensure_music(profile_id, cue_id, catalog, resolver) {
            self.log_once(format!(
                "audio music playback failed profile_id={profile_id} cue_id={cue_id}: {err}"
            ));
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn play_one_shot(
        &mut self,
        request: OneShotRequest<'_>,
        resolver: &AudioAssetResolver<'_>,
        catalog: &AudioCatalogState,
        settings: &AudioSettings,
        demand_asset_ids: &mut HashSet<String>,
    ) {
        let profile_id = request.profile_id.to_string();
        let cue_id = request.cue_id.to_string();
        let AudioBackendKind::Native(backend) = &mut self.backend else {
            return;
        };
        if let Err(err) = backend.sync_graph(catalog, settings) {
            self.log_once(format!("audio graph sync failed: {err}"));
            return;
        }
        let Some(asset_id) = load_clip_asset_id(catalog, request.profile_id, request.cue_id) else {
            self.log_once(format!(
                "audio one-shot cue missing profile_id={} cue_id={}",
                request.profile_id, request.cue_id
            ));
            return;
        };
        demand_asset_ids.insert(asset_id);
        if let Err(err) = backend.play_one_shot(request, catalog, resolver) {
            self.log_once(format!(
                "audio one-shot playback failed profile_id={} cue_id={}: {err}",
                profile_id, cue_id
            ));
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn trigger_loop_emitter(
        &mut self,
        request: LoopEmitterRequest<'_>,
        resolver: &AudioAssetResolver<'_>,
        catalog: &AudioCatalogState,
        settings: &AudioSettings,
        demand_asset_ids: &mut HashSet<String>,
    ) {
        let profile_id = request.profile_id.to_string();
        let cue_id = request.cue_id.to_string();
        let AudioBackendKind::Native(backend) = &mut self.backend else {
            return;
        };
        if let Err(err) = backend.sync_graph(catalog, settings) {
            self.log_once(format!("audio graph sync failed: {err}"));
            return;
        }
        let Some(asset_id) = load_clip_asset_id(catalog, request.profile_id, request.cue_id) else {
            self.log_once(format!(
                "audio loop cue missing profile_id={} cue_id={}",
                request.profile_id, request.cue_id
            ));
            return;
        };
        demand_asset_ids.insert(asset_id);
        if let Err(err) = backend.trigger_loop_emitter(request, catalog, resolver) {
            self.log_once(format!(
                "audio loop playback failed profile_id={} cue_id={}: {err}",
                profile_id, cue_id
            ));
        }
    }

    pub(super) fn tick(&mut self, now_s: f64, settings: &AudioSettings) {
        #[cfg(not(target_arch = "wasm32"))]
        if let AudioBackendKind::Native(backend) = &mut self.backend {
            backend.sync_settings(settings);
            backend.tick(now_s);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn play_debug_probe(
        &mut self,
        mode: DebugProbeMode,
        profile_id: &str,
        cue_id: &str,
        catalog: &AudioCatalogState,
        resolver: &AudioAssetResolver<'_>,
    ) {
        let AudioBackendKind::Native(backend) = &mut self.backend else {
            return;
        };
        if let Err(err) = backend.play_debug_probe(mode, profile_id, cue_id, catalog, resolver) {
            self.log_once(format!(
                "audio debug probe failed profile_id={profile_id} cue_id={cue_id}: {err}"
            ));
        }
    }

    fn log_once(&mut self, message: String) {
        if self.logged_errors.insert(message.clone()) {
            warn!("{message}");
        }
    }
}
