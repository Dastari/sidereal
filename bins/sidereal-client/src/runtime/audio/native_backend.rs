#![cfg(not(target_arch = "wasm32"))]

use super::catalog::AudioCatalogState;
use super::settings::{AudioBusSettings, AudioSettings};
use crate::runtime::assets::LocalAssetManager;
use crate::runtime::resources::AssetCacheAdapter;
use bevy::log::info;
use bevy::prelude::{Quat, Vec2, Vec3};
use kira::effect::EffectBuilder;
use kira::effect::distortion::{DistortionBuilder, DistortionKind};
use kira::effect::filter::{FilterBuilder, FilterHandle, FilterMode};
use kira::effect::reverb::ReverbBuilder;
use kira::listener::ListenerHandle;
use kira::sound::PlaybackState;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::track::{
    MainTrackBuilder, SendTrackBuilder, SendTrackHandle, SpatialTrackBuilder, SpatialTrackHandle,
    TrackBuilder, TrackHandle,
};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Mix, Tween};
use sidereal_audio::{
    AudioCueDefinition, AudioEffectDefinition, AudioPlaybackDefinition, AudioSendDefinition,
};
use std::collections::HashMap;
use std::io::Cursor;
use std::time::Duration;

pub(super) struct AudioAssetResolver<'a> {
    pub asset_root: &'a str,
    pub asset_manager: &'a LocalAssetManager,
    pub cache_adapter: AssetCacheAdapter,
}

impl<'a> AudioAssetResolver<'a> {
    fn bytes_for_asset(&self, asset_id: &str) -> Option<Vec<u8>> {
        let catalog = self.asset_manager.catalog_by_asset_id.get(asset_id)?;
        (self.cache_adapter.read_valid_asset_sync)(
            self.asset_root,
            &catalog.relative_cache_path,
            &catalog.sha256_hex,
        )
    }
}

pub(super) struct OneShotRequest<'a> {
    pub profile_id: &'a str,
    pub cue_id: &'a str,
    pub position: Option<Vec2>,
}

pub(super) struct LoopEmitterRequest<'a> {
    pub key: String,
    pub profile_id: &'a str,
    pub cue_id: &'a str,
    pub position: Vec2,
    pub release_timeout_s: f64,
    pub now_s: f64,
}

pub(super) enum DebugProbeMode {
    Nonspatial,
    SpatialAtListener,
    SpatialOffsetRight,
}

struct CachedClip {
    sha256_hex: String,
    sound: StaticSoundData,
}

struct ActiveMusicPlayback {
    profile_id: String,
    cue_id: String,
    _track: TrackHandle,
    sound: StaticSoundHandle,
}

struct ActiveLoopEmitter {
    track: ActiveLoopTrack,
    sound: StaticSoundHandle,
    last_trigger_at_s: f64,
    release_timeout_s: f64,
    outro_start_s: Option<f64>,
    released: bool,
}

enum ActiveLoopTrack {
    Spatial(SpatialTrackHandle),
    Nonspatial { _track: TrackHandle },
}

pub(super) struct NativeAudioBackend {
    manager: AudioManager<DefaultBackend>,
    listener: ListenerHandle,
    listener_position: Vec3,
    main_filter: FilterHandle,
    bus_tracks: HashMap<String, TrackHandle>,
    send_tracks: HashMap<String, SendTrackHandle>,
    clip_cache: HashMap<String, CachedClip>,
    active_music: Option<ActiveMusicPlayback>,
    active_loops: HashMap<String, ActiveLoopEmitter>,
    graph_catalog_version: Option<String>,
}

impl NativeAudioBackend {
    pub fn new() -> Result<Self, String> {
        let mut main_track_builder = MainTrackBuilder::new();
        let main_filter = main_track_builder.add_effect(
            FilterBuilder::new()
                .mode(FilterMode::LowPass)
                .cutoff(20_000.0)
                .resonance(0.0)
                .mix(Mix::WET),
        );
        let mut manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings {
            main_track_builder,
            ..AudioManagerSettings::default()
        })
        .map_err(|err| err.to_string())?;
        let listener = manager
            .add_listener(mint_vec3(Vec3::ZERO), mint_quat(Quat::IDENTITY))
            .map_err(|err| err.to_string())?;
        Ok(Self {
            manager,
            listener,
            listener_position: Vec3::ZERO,
            main_filter,
            bus_tracks: HashMap::new(),
            send_tracks: HashMap::new(),
            clip_cache: HashMap::new(),
            active_music: None,
            active_loops: HashMap::new(),
            graph_catalog_version: None,
        })
    }

    pub fn sync_graph(
        &mut self,
        catalog: &AudioCatalogState,
        settings: &AudioSettings,
    ) -> Result<(), String> {
        let Some(version) = catalog.version.as_ref() else {
            return Ok(());
        };
        if self.graph_catalog_version.as_deref() == Some(version.as_str()) {
            return Ok(());
        }
        let Some(registry) = catalog.registry() else {
            return Ok(());
        };

        self.stop_music();
        self.active_loops.clear();
        self.bus_tracks.clear();
        self.send_tracks.clear();

        for send in &registry.sends {
            let track = self
                .manager
                .add_send_track(build_send_track(send))
                .map_err(|err| err.to_string())?;
            self.send_tracks.insert(send.send_id.clone(), track);
        }

        for bus in &registry.buses {
            let base_volume_db = bus.default_volume_db.unwrap_or(0.0)
                + bus_settings_for(settings, bus.bus_id.as_str()).volume_db;
            let track = self
                .manager
                .add_sub_track(TrackBuilder::new().volume(base_volume_db))
                .map_err(|err| err.to_string())?;
            self.bus_tracks.insert(bus.bus_id.clone(), track);
        }

        self.graph_catalog_version = Some(version.clone());
        self.sync_settings(settings);
        Ok(())
    }

    pub fn sync_listener(&mut self, position: Vec3, rotation: Quat) {
        self.listener_position = position;
        self.listener
            .set_position(mint_vec3(position), Tween::default());
        self.listener
            .set_orientation(mint_quat(rotation), Tween::default());
    }

    pub fn sync_settings(&mut self, settings: &AudioSettings) {
        let master_db = if settings.master_muted {
            Decibels::SILENCE
        } else {
            Decibels::from(settings.master_volume_db)
        };
        self.manager
            .main_track()
            .set_volume(master_db, Tween::default());
        let cutoff_hz = settings.master_low_pass_hz.unwrap_or(20_000.0);
        self.main_filter.set_cutoff(cutoff_hz, Tween::default());
        for (bus_id, track) in &mut self.bus_tracks {
            let bus_settings = bus_settings_for(settings, bus_id);
            let bus_db = if bus_settings.muted {
                Decibels::SILENCE
            } else {
                Decibels::from(bus_settings.volume_db)
            };
            track.set_volume(bus_db, Tween::default());
        }
    }

    pub fn ensure_music(
        &mut self,
        profile_id: &str,
        cue_id: &str,
        catalog: &AudioCatalogState,
        resolver: &AudioAssetResolver<'_>,
    ) -> Result<(), String> {
        if self
            .active_music
            .as_ref()
            .is_some_and(|active| active.profile_id == profile_id && active.cue_id == cue_id)
        {
            return Ok(());
        }

        self.stop_music();
        let cue = catalog
            .cue(profile_id, cue_id)
            .ok_or_else(|| format!("missing audio cue profile_id={profile_id} cue_id={cue_id}"))?;
        let mut track = self
            .bus_tracks
            .get_mut(cue.route.bus.as_str())
            .ok_or_else(|| format!("unknown audio bus {}", cue.route.bus))?
            .add_sub_track(build_nonspatial_track(&self.send_tracks, cue, true))
            .map_err(|err| err.to_string())?;
        let asset_id = clip_asset_id_for_playback(&cue.playback)
            .ok_or_else(|| "audio cue does not reference a clip_asset_id".to_string())?;
        let sound_data = self.load_sound_data(cue, resolver)?;
        let sound = track.play(sound_data).map_err(|err| err.to_string())?;
        info!(
            profile_id,
            cue_id,
            bus = cue.route.bus.as_str(),
            asset_id,
            "audio music started"
        );
        self.active_music = Some(ActiveMusicPlayback {
            profile_id: profile_id.to_string(),
            cue_id: cue_id.to_string(),
            _track: track,
            sound,
        });
        Ok(())
    }

    pub fn stop_music(&mut self) {
        if let Some(mut active_music) = self.active_music.take() {
            active_music.sound.stop(Tween {
                duration: Duration::from_millis(120),
                ..Tween::default()
            });
        }
    }

    pub fn play_one_shot(
        &mut self,
        request: OneShotRequest<'_>,
        catalog: &AudioCatalogState,
        resolver: &AudioAssetResolver<'_>,
    ) -> Result<(), String> {
        let cue = catalog
            .cue(request.profile_id, request.cue_id)
            .ok_or_else(|| {
                format!(
                    "missing audio cue profile_id={} cue_id={}",
                    request.profile_id, request.cue_id
                )
            })?;
        let asset_id = clip_asset_id_for_playback(&cue.playback)
            .ok_or_else(|| "audio cue does not reference a clip_asset_id".to_string())?;
        match cue.spatial.mode.as_str() {
            "world_2d" => {
                let requested_position = request.position.unwrap_or(Vec2::ZERO);
                let effective_position = debug_effective_position(
                    request.profile_id,
                    requested_position,
                    self.listener_position,
                );
                let listener_xy = self.listener_position.truncate();
                let distance_m = effective_position.distance(listener_xy);
                let sound_data = self.load_sound_data(cue, resolver)?;
                let force_nonspatial_track = debug_force_nonspatial_track(request.profile_id);
                if force_nonspatial_track {
                    let mut track = self
                        .bus_tracks
                        .get_mut(cue.route.bus.as_str())
                        .ok_or_else(|| format!("unknown audio bus {}", cue.route.bus))?
                        .add_sub_track(build_nonspatial_track(&self.send_tracks, cue, true))
                        .map_err(|err| err.to_string())?;
                    let _ = track.play(sound_data).map_err(|err| err.to_string())?;
                } else {
                    let mut track = self
                        .bus_tracks
                        .get_mut(cue.route.bus.as_str())
                        .ok_or_else(|| format!("unknown audio bus {}", cue.route.bus))?
                        .add_spatial_sub_track(
                            self.listener.id(),
                            mint_vec3(Vec3::new(effective_position.x, effective_position.y, 0.0)),
                            build_spatial_track(&self.send_tracks, cue, true),
                        )
                        .map_err(|err| err.to_string())?;
                    let _ = track.play(sound_data).map_err(|err| err.to_string())?;
                }
                info!(
                    profile_id = request.profile_id,
                    cue_id = request.cue_id,
                    bus = cue.route.bus.as_str(),
                    asset_id,
                    spatial_mode = cue.spatial.mode.as_str(),
                    requested_position_x = requested_position.x,
                    requested_position_y = requested_position.y,
                    effective_position_x = effective_position.x,
                    effective_position_y = effective_position.y,
                    listener_x = listener_xy.x,
                    listener_y = listener_xy.y,
                    distance_m,
                    force_listener_position = debug_force_listener_position(request.profile_id),
                    force_nonspatial_track,
                    "audio one-shot started"
                );
            }
            _ => {
                let mut track = self
                    .bus_tracks
                    .get_mut(cue.route.bus.as_str())
                    .ok_or_else(|| format!("unknown audio bus {}", cue.route.bus))?
                    .add_sub_track(build_nonspatial_track(&self.send_tracks, cue, true))
                    .map_err(|err| err.to_string())?;
                let sound_data = self.load_sound_data(cue, resolver)?;
                let _ = track.play(sound_data).map_err(|err| err.to_string())?;
                info!(
                    profile_id = request.profile_id,
                    cue_id = request.cue_id,
                    bus = cue.route.bus.as_str(),
                    asset_id,
                    spatial_mode = cue.spatial.mode.as_str(),
                    "audio one-shot started"
                );
            }
        }
        Ok(())
    }

    pub fn trigger_loop_emitter(
        &mut self,
        request: LoopEmitterRequest<'_>,
        catalog: &AudioCatalogState,
        resolver: &AudioAssetResolver<'_>,
    ) -> Result<(), String> {
        let cue = catalog
            .cue(request.profile_id, request.cue_id)
            .ok_or_else(|| {
                format!(
                    "missing audio cue profile_id={} cue_id={}",
                    request.profile_id, request.cue_id
                )
            })?;

        if let Some(active) = self.active_loops.get_mut(&request.key)
            && !active.released
        {
            if let ActiveLoopTrack::Spatial(track) = &mut active.track {
                track.set_position(
                    mint_vec3(Vec3::new(request.position.x, request.position.y, 0.0)),
                    Tween::default(),
                );
            }
            active.last_trigger_at_s = request.now_s;
            active.release_timeout_s = request.release_timeout_s;
            return Ok(());
        }
        if let Some(mut finished_tail) = self.active_loops.remove(&request.key) {
            finished_tail.sound.stop(Tween {
                duration: Duration::from_millis(40),
                ..Tween::default()
            });
        }

        let effective_position =
            debug_effective_position(request.profile_id, request.position, self.listener_position);
        let listener_xy = self.listener_position.truncate();
        let distance_m = effective_position.distance(listener_xy);
        let asset_id = clip_asset_id_for_playback(&cue.playback)
            .ok_or_else(|| "audio cue does not reference a clip_asset_id".to_string())?;
        let sound_data = self.load_sound_data(cue, resolver)?;
        let force_nonspatial_track = debug_force_nonspatial_track(request.profile_id);
        let (track, sound) = if force_nonspatial_track {
            let mut track = self
                .bus_tracks
                .get_mut(cue.route.bus.as_str())
                .ok_or_else(|| format!("unknown audio bus {}", cue.route.bus))?
                .add_sub_track(build_nonspatial_track(&self.send_tracks, cue, false))
                .map_err(|err| err.to_string())?;
            let sound = track.play(sound_data).map_err(|err| err.to_string())?;
            (ActiveLoopTrack::Nonspatial { _track: track }, sound)
        } else {
            let mut track = self
                .bus_tracks
                .get_mut(cue.route.bus.as_str())
                .ok_or_else(|| format!("unknown audio bus {}", cue.route.bus))?
                .add_spatial_sub_track(
                    self.listener.id(),
                    mint_vec3(Vec3::new(effective_position.x, effective_position.y, 0.0)),
                    build_spatial_track(&self.send_tracks, cue, false),
                )
                .map_err(|err| err.to_string())?;
            let sound = track.play(sound_data).map_err(|err| err.to_string())?;
            (ActiveLoopTrack::Spatial(track), sound)
        };
        info!(
            emitter_key = request.key.as_str(),
            profile_id = request.profile_id,
            cue_id = request.cue_id,
            bus = cue.route.bus.as_str(),
            asset_id,
            requested_position_x = request.position.x,
            requested_position_y = request.position.y,
            effective_position_x = effective_position.x,
            effective_position_y = effective_position.y,
            listener_x = listener_xy.x,
            listener_y = listener_xy.y,
            distance_m,
            force_listener_position = debug_force_listener_position(request.profile_id),
            force_nonspatial_track,
            release_timeout_s = request.release_timeout_s,
            "audio loop emitter started"
        );
        self.active_loops.insert(
            request.key,
            ActiveLoopEmitter {
                track,
                sound,
                last_trigger_at_s: request.now_s,
                release_timeout_s: request.release_timeout_s,
                outro_start_s: segmented_outro_start_s(&cue.playback),
                released: false,
            },
        );
        Ok(())
    }

    pub fn tick(&mut self, now_s: f64) {
        let mut finished_keys = Vec::new();
        for (key, active) in &mut self.active_loops {
            if !active.released && now_s - active.last_trigger_at_s >= active.release_timeout_s {
                active.sound.set_loop_region(None);
                if let Some(outro_start_s) = active.outro_start_s {
                    active.sound.seek_to(outro_start_s);
                }
                active.released = true;
            }
            if active.released && active.sound.state() == PlaybackState::Stopped {
                finished_keys.push(key.clone());
            }
        }
        for key in finished_keys {
            self.active_loops.remove(&key);
        }
        let stop_music = self
            .active_music
            .as_ref()
            .is_some_and(|active| active.sound.state() == PlaybackState::Stopped);
        if stop_music {
            self.active_music = None;
        }
    }

    pub fn play_debug_probe(
        &mut self,
        mode: DebugProbeMode,
        profile_id: &str,
        cue_id: &str,
        catalog: &AudioCatalogState,
        resolver: &AudioAssetResolver<'_>,
    ) -> Result<(), String> {
        let cue = catalog
            .cue(profile_id, cue_id)
            .ok_or_else(|| format!("missing audio cue profile_id={profile_id} cue_id={cue_id}"))?;
        let asset_id = clip_asset_id_for_playback(&cue.playback)
            .ok_or_else(|| "audio cue does not reference a clip_asset_id".to_string())?;
        let sound_data = self.load_sound_data(cue, resolver)?;

        match mode {
            DebugProbeMode::Nonspatial => {
                let _ = self
                    .manager
                    .play(sound_data)
                    .map_err(|err| err.to_string())?;
                info!(
                    profile_id,
                    cue_id, asset_id, "audio debug probe played root nonspatial"
                );
            }
            DebugProbeMode::SpatialAtListener | DebugProbeMode::SpatialOffsetRight => {
                let position = match mode {
                    DebugProbeMode::SpatialAtListener => self.listener_position,
                    DebugProbeMode::SpatialOffsetRight => {
                        self.listener_position + Vec3::new(25.0, 0.0, 0.0)
                    }
                    DebugProbeMode::Nonspatial => Vec3::ZERO,
                };
                let mut track = self
                    .manager
                    .add_spatial_sub_track(
                        self.listener.id(),
                        mint_vec3(position),
                        SpatialTrackBuilder::new()
                            .persist_until_sounds_finish(true)
                            .distances((1.0, 300.0))
                            .spatialization_strength(1.0),
                    )
                    .map_err(|err| err.to_string())?;
                let _ = track.play(sound_data).map_err(|err| err.to_string())?;
                info!(
                    profile_id,
                    cue_id,
                    asset_id,
                    position_x = position.x,
                    position_y = position.y,
                    listener_x = self.listener_position.x,
                    listener_y = self.listener_position.y,
                    "audio debug probe played root spatial"
                );
            }
        }
        Ok(())
    }

    fn load_sound_data(
        &mut self,
        cue: &AudioCueDefinition,
        resolver: &AudioAssetResolver<'_>,
    ) -> Result<StaticSoundData, String> {
        let asset_id = clip_asset_id_for_playback(&cue.playback)
            .ok_or_else(|| "audio cue does not reference a clip_asset_id".to_string())?;
        let expected_sha = resolver
            .asset_manager
            .catalog_by_asset_id
            .get(asset_id.as_str())
            .map(|entry| entry.sha256_hex.clone())
            .ok_or_else(|| format!("missing runtime asset catalog entry for {asset_id}"))?;
        if let Some(cached) = self.clip_cache.get(asset_id.as_str())
            && cached.sha256_hex == expected_sha
        {
            return Ok(apply_playback_config(&cached.sound, &cue.playback));
        }

        let bytes = resolver
            .bytes_for_asset(asset_id.as_str())
            .ok_or_else(|| format!("audio asset not cached yet: {asset_id}"))?;
        let decoded =
            StaticSoundData::from_cursor(Cursor::new(bytes)).map_err(|err| err.to_string())?;
        self.clip_cache.insert(
            asset_id.clone(),
            CachedClip {
                sha256_hex: expected_sha,
                sound: decoded.clone(),
            },
        );
        Ok(apply_playback_config(&decoded, &cue.playback))
    }
}

pub(super) fn load_clip_asset_id(
    catalog: &AudioCatalogState,
    profile_id: &str,
    cue_id: &str,
) -> Option<String> {
    let cue = catalog.cue(profile_id, cue_id)?;
    clip_asset_id_for_playback(&cue.playback)
}

fn clip_asset_id_for_playback(playback: &AudioPlaybackDefinition) -> Option<String> {
    playback.clip_asset_id.clone().or_else(|| {
        playback
            .variants
            .first()
            .map(|variant| variant.clip_asset_id.clone())
    })
}

fn apply_playback_config(
    sound: &StaticSoundData,
    playback: &AudioPlaybackDefinition,
) -> StaticSoundData {
    let mut configured = sound.clone();
    let slice_start_s = playback_slice_start_s(playback);
    if let Some(end_s) = playback.clip_end_s
        && end_s > slice_start_s
    {
        configured = configured.slice(f64::from(slice_start_s)..f64::from(end_s));
    } else if let Some(end_s) = playback.clip_end_s {
        configured = configured.slice(..f64::from(end_s));
    }

    match playback.kind.as_str() {
        "loop" => configured.loop_region(..),
        "segmented_loop" => {
            let loop_start = playback.loop_start_s.or_else(|| {
                playback
                    .loop_region
                    .as_ref()
                    .map(|loop_region| loop_region.start_s)
            });
            let loop_end = playback.loop_end_s.or_else(|| {
                playback
                    .loop_region
                    .as_ref()
                    .map(|loop_region| loop_region.end_s)
            });
            match (loop_start, loop_end) {
                (Some(start_s), Some(end_s)) if end_s > start_s => {
                    let relative_start_s = start_s - slice_start_s;
                    let relative_end_s = end_s - slice_start_s;
                    if relative_end_s > relative_start_s && relative_start_s >= 0.0 {
                        configured
                            .loop_region(f64::from(relative_start_s)..f64::from(relative_end_s))
                    } else {
                        configured
                    }
                }
                _ => configured,
            }
        }
        _ => configured,
    }
}

fn build_send_track(send: &AudioSendDefinition) -> SendTrackBuilder {
    let mut builder = SendTrackBuilder::new();
    for effect in &send.effects {
        if let Some(effect) = build_effect(effect) {
            builder.add_built_effect(effect);
        }
    }
    builder
}

fn build_nonspatial_track(
    send_tracks: &HashMap<String, SendTrackHandle>,
    cue: &AudioCueDefinition,
    persist_until_sounds_finish: bool,
) -> TrackBuilder {
    let mut builder = TrackBuilder::new().persist_until_sounds_finish(persist_until_sounds_finish);
    for send in &cue.route.sends {
        if let Some(send_track) = send_tracks.get(send.send_id.as_str()) {
            builder = builder.with_send(send_track, send.level_db);
        }
    }
    builder
}

fn build_spatial_track(
    send_tracks: &HashMap<String, SendTrackHandle>,
    cue: &AudioCueDefinition,
    persist_until_sounds_finish: bool,
) -> SpatialTrackBuilder {
    let mut builder = SpatialTrackBuilder::new()
        .persist_until_sounds_finish(persist_until_sounds_finish)
        .distances((
            cue.spatial.min_distance_m.unwrap_or(1.0),
            cue.spatial.max_distance_m.unwrap_or(150.0),
        ))
        .spatialization_strength(cue.spatial.pan_strength.unwrap_or(1.0));
    builder = match cue.spatial.rolloff.as_deref() {
        Some("none") => builder.attenuation_function(None::<kira::Easing>),
        Some("logarithmic") => builder.attenuation_function(Some(kira::Easing::InPowf(2.0))),
        _ => builder.attenuation_function(Some(kira::Easing::Linear)),
    };
    if let Some(lowpass) = cue.spatial.distance_lowpass.as_ref()
        && lowpass.enabled
    {
        let cutoff_hz = lowpass.far_hz.or(lowpass.near_hz).unwrap_or(12_000.0) as f64;
        builder = builder.with_effect(
            FilterBuilder::new()
                .mode(FilterMode::LowPass)
                .cutoff(cutoff_hz)
                .resonance(0.0)
                .mix(Mix::WET),
        );
    }
    for send in &cue.route.sends {
        if let Some(send_track) = send_tracks.get(send.send_id.as_str()) {
            builder = builder.with_send(send_track, send.level_db);
        }
    }
    builder
}

fn build_effect(effect: &AudioEffectDefinition) -> Option<Box<dyn kira::effect::Effect>> {
    match effect.kind.as_str() {
        "reverb" => Some(
            ReverbBuilder::new()
                .feedback(json_f64(effect, "room_size").unwrap_or(0.9))
                .damping(json_f64(effect, "damping").unwrap_or(0.15))
                .mix(Mix::WET)
                .build()
                .0,
        ),
        "filter" => {
            let mode = match json_string(effect, "mode").as_deref() {
                Some("high_pass") => FilterMode::HighPass,
                Some("band_pass") => FilterMode::BandPass,
                Some("notch") => FilterMode::Notch,
                _ => FilterMode::LowPass,
            };
            Some(
                FilterBuilder::new()
                    .mode(mode)
                    .cutoff(json_f64(effect, "cutoff_hz").unwrap_or(10_000.0))
                    .resonance(json_f64(effect, "q").unwrap_or(0.0))
                    .mix(Mix::WET)
                    .build()
                    .0,
            )
        }
        "distortion" => {
            let kind = match json_string(effect, "mode").as_deref() {
                Some("soft_clip") => DistortionKind::SoftClip,
                _ => DistortionKind::HardClip,
            };
            Some(
                DistortionBuilder::new()
                    .kind(kind)
                    .drive(json_f64(effect, "drive").unwrap_or(0.0) as f32)
                    .mix(Mix::WET)
                    .build()
                    .0,
            )
        }
        _ => None,
    }
}

fn json_f64(effect: &AudioEffectDefinition, key: &str) -> Option<f64> {
    effect.params.get(key).and_then(|value| value.as_f64())
}

fn json_string(effect: &AudioEffectDefinition, key: &str) -> Option<String> {
    effect
        .params
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn bus_settings_for(settings: &AudioSettings, bus_id: &str) -> AudioBusSettings {
    settings.buses.get(bus_id).cloned().unwrap_or_default()
}

fn mint_vec3(value: Vec3) -> mint::Vector3<f32> {
    mint::Vector3 {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

fn mint_quat(value: Quat) -> mint::Quaternion<f32> {
    mint::Quaternion {
        v: mint::Vector3 {
            x: value.x,
            y: value.y,
            z: value.z,
        },
        s: value.w,
    }
}

fn debug_effective_position(
    profile_id: &str,
    requested_position: Vec2,
    listener_position: Vec3,
) -> Vec2 {
    if debug_force_listener_position(profile_id) {
        listener_position.truncate()
    } else {
        requested_position
    }
}

fn debug_force_listener_position(profile_id: &str) -> bool {
    debug_force_nonspatial_track(profile_id)
}

fn debug_force_nonspatial_track(profile_id: &str) -> bool {
    matches!(
        profile_id,
        "weapon.ballistic_gatling" | "destruction.asteroid.default" | "explosion_burst"
    )
}

fn playback_slice_start_s(playback: &AudioPlaybackDefinition) -> f32 {
    playback.intro_start_s.unwrap_or(0.0)
}

fn segmented_outro_start_s(playback: &AudioPlaybackDefinition) -> Option<f64> {
    let outro_start_s = playback.outro_start_s?;
    let slice_start_s = playback_slice_start_s(playback);
    (outro_start_s >= slice_start_s).then_some(f64::from(outro_start_s - slice_start_s))
}
