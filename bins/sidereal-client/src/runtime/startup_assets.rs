use super::app_state::{ClientAppState, ClientSession};
use super::assets::{LocalAssetManager, LocalAssetRecord, RuntimeAssetCatalogRecord};
use super::audio::AudioCatalogState;
use super::dialog_ui::DialogQueue;
use super::resources::{AssetCacheAdapter, AssetRootPath, GatewayHttpAdapter};
use async_channel::{Receiver, TryRecvError, bounded};
use bevy::log::info;
use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use sidereal_asset_runtime::{AssetCacheIndexRecord, asset_version_from_sha256_hex, sha256_hex};
use sidereal_core::gateway_dtos::{AssetBootstrapManifestEntry, StartupAssetManifestResponse};

const MAX_PARALLEL_STARTUP_FETCHES: usize = 4;

#[derive(Resource, Default)]
pub(crate) struct StartupAssetRequestState {
    pending: Option<StartupAssetRequestTask>,
    pub submitted: bool,
    pub completed: bool,
    pub failed: bool,
}

struct StartupAssetRequestTask {
    receiver: Receiver<Result<StartupAssetRequestResult, String>>,
}

#[derive(Debug)]
struct StartupAssetRequestResult {
    manifest: StartupAssetManifestResponse,
    records: Vec<StartupAssetRecord>,
    cache_index: sidereal_asset_runtime::AssetCacheIndex,
    startup_total_bytes: u64,
    startup_ready_bytes: u64,
}

#[derive(Debug, Clone)]
struct StartupAssetRecord {
    asset_id: String,
    relative_cache_path: String,
    content_type: String,
    byte_len: u64,
    asset_version: u64,
    sha256_hex: String,
    ready: bool,
}

#[derive(Debug)]
struct StartupFetchedAsset {
    asset_id: String,
    relative_cache_path: String,
    byte_len: u64,
    sha256_hex: String,
    payload: Vec<u8>,
}

fn try_recv_pending_result<T>(receiver: &Receiver<T>) -> Option<T> {
    match receiver.try_recv() {
        Ok(result) => Some(result),
        Err(TryRecvError::Empty) | Err(TryRecvError::Closed) => None,
    }
}

pub(crate) fn init_startup_asset_request_state(app: &mut App) {
    app.insert_resource(StartupAssetRequestState::default());
}

pub(crate) fn submit_startup_asset_request_system(
    mut session: ResMut<'_, ClientSession>,
    mut request_state: ResMut<'_, StartupAssetRequestState>,
    gateway_http: Res<'_, GatewayHttpAdapter>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
    asset_root: Res<'_, AssetRootPath>,
) {
    submit_startup_asset_request(
        session.as_mut(),
        request_state.as_mut(),
        *gateway_http,
        *cache_adapter,
        &asset_root.0,
    );
}

fn submit_startup_asset_request(
    session: &mut ClientSession,
    request_state: &mut StartupAssetRequestState,
    gateway_http: GatewayHttpAdapter,
    cache_adapter: AssetCacheAdapter,
    asset_root: &str,
) {
    if request_state.pending.is_some() || request_state.submitted {
        return;
    }
    let gateway_url = session.gateway_url.clone();
    let asset_root = asset_root.to_string();
    request_state.submitted = true;
    request_state.completed = false;
    request_state.failed = false;
    session.status = "Fetching startup asset manifest...".to_string();
    session.ui_dirty = true;
    info!(
        "startup asset request submitted: gateway_url={} asset_root={}",
        gateway_url, asset_root
    );

    let (sender, receiver) = bounded(1);
    IoTaskPool::get()
        .spawn(async move {
            let result = async move {
                info!("startup asset task starting");
                (cache_adapter.prepare_root)(asset_root.clone()).await?;
                let manifest = (gateway_http.fetch_startup_manifest)(gateway_url.clone()).await?;
                info!(
                    "startup asset manifest fetched: required_assets={} catalog_assets={}",
                    manifest.required_assets.len(),
                    manifest.catalog.len()
                );
                let mut cache_index = (cache_adapter.load_index)(asset_root.clone()).await?;
                let mut records = Vec::<StartupAssetRecord>::new();
                let mut startup_total_bytes = 0u64;
                let mut startup_ready_bytes = 0u64;

                for entry in &manifest.catalog {
                    let ready = (cache_adapter.read_valid_asset)(
                        asset_root.clone(),
                        entry.relative_cache_path.clone(),
                        entry.sha256_hex.clone(),
                    )
                    .await?
                    .is_some();
                    records.push(StartupAssetRecord {
                        asset_id: entry.asset_id.clone(),
                        relative_cache_path: entry.relative_cache_path.clone(),
                        content_type: entry.content_type.clone(),
                        byte_len: entry.byte_len,
                        asset_version: asset_version_from_sha256_hex(&entry.sha256_hex),
                        sha256_hex: entry.sha256_hex.clone(),
                        ready,
                    });
                }

                let mut missing_required_assets = Vec::<AssetBootstrapManifestEntry>::new();
                for required in &manifest.required_assets {
                    startup_total_bytes = startup_total_bytes.saturating_add(required.byte_len);
                    let satisfied = (cache_adapter.read_valid_asset)(
                        asset_root.clone(),
                        required.relative_cache_path.clone(),
                        required.sha256_hex.clone(),
                    )
                    .await?
                    .is_some();
                    if !satisfied {
                        missing_required_assets.push(required.clone());
                    } else {
                        startup_ready_bytes =
                            startup_ready_bytes.saturating_add(required.byte_len);
                    }
                }

                let mut pending_fetches = Vec::new();
                for required in missing_required_assets {
                    let gateway_http = gateway_http;
                    let gateway_url = gateway_url.clone();
                    pending_fetches.push(IoTaskPool::get().spawn(async move {
                        let url = if required.url.starts_with("http://")
                            || required.url.starts_with("https://")
                        {
                            required.url.clone()
                        } else {
                            format!("{gateway_url}{}", required.url)
                        };
                        info!(
                            "startup asset download starting: asset_id={} relative_cache_path={} bytes={}",
                            required.asset_id, required.relative_cache_path, required.byte_len
                        );
                        let payload = (gateway_http.fetch_public_asset_bytes)(url).await?;
                        let payload_sha = sha256_hex(&payload);
                        if payload_sha != required.sha256_hex {
                            return Err(format!(
                                "startup asset checksum mismatch asset_id={} expected={} got={}",
                                required.asset_id, required.sha256_hex, payload_sha
                            ));
                        }
                        Ok(StartupFetchedAsset {
                            asset_id: required.asset_id,
                            relative_cache_path: required.relative_cache_path,
                            byte_len: required.byte_len,
                            sha256_hex: required.sha256_hex,
                            payload,
                        })
                    }));

                    if pending_fetches.len() >= MAX_PARALLEL_STARTUP_FETCHES {
                        let fetched = pending_fetches.remove(0).await?;
                        (cache_adapter.write_asset)(
                            asset_root.clone(),
                            fetched.relative_cache_path.clone(),
                            fetched.payload,
                        )
                        .await?;
                        info!(
                            "startup asset cache write complete: asset_id={} relative_cache_path={} bytes={}",
                            fetched.asset_id, fetched.relative_cache_path, fetched.byte_len
                        );
                        startup_ready_bytes =
                            startup_ready_bytes.saturating_add(fetched.byte_len);
                        cache_index.by_asset_id.insert(
                            fetched.asset_id,
                            AssetCacheIndexRecord {
                                asset_version: asset_version_from_sha256_hex(&fetched.sha256_hex),
                                sha256_hex: fetched.sha256_hex,
                            },
                        );
                    }
                }

                for pending in pending_fetches {
                    let fetched = pending.await?;
                    (cache_adapter.write_asset)(
                        asset_root.clone(),
                        fetched.relative_cache_path.clone(),
                        fetched.payload,
                    )
                    .await?;
                    info!(
                        "startup asset cache write complete: asset_id={} relative_cache_path={} bytes={}",
                        fetched.asset_id, fetched.relative_cache_path, fetched.byte_len
                    );
                    startup_ready_bytes = startup_ready_bytes.saturating_add(fetched.byte_len);
                    cache_index.by_asset_id.insert(
                        fetched.asset_id,
                        AssetCacheIndexRecord {
                            asset_version: asset_version_from_sha256_hex(&fetched.sha256_hex),
                            sha256_hex: fetched.sha256_hex,
                        },
                    );
                }

                for required in &manifest.required_assets {
                    cache_index.by_asset_id.insert(
                        required.asset_id.clone(),
                        AssetCacheIndexRecord {
                            asset_version: asset_version_from_sha256_hex(&required.sha256_hex),
                            sha256_hex: required.sha256_hex.clone(),
                        },
                    );
                }
                (cache_adapter.save_index)(asset_root.clone(), cache_index.clone()).await?;

                for record in &mut records {
                    if manifest
                        .required_assets
                        .iter()
                        .any(|required| required.asset_id == record.asset_id)
                    {
                        record.ready = true;
                    }
                }

                Ok(StartupAssetRequestResult {
                    manifest,
                    records,
                    cache_index,
                    startup_total_bytes,
                    startup_ready_bytes,
                })
            }
            .await;
            let _ = sender.send(result).await;
        })
        .detach();
    request_state.pending = Some(StartupAssetRequestTask { receiver });
}

pub(crate) fn poll_startup_asset_request_results(
    mut request_state: ResMut<'_, StartupAssetRequestState>,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
    mut session: ResMut<'_, ClientSession>,
    mut asset_manager: ResMut<'_, LocalAssetManager>,
    mut audio_catalog: ResMut<'_, AudioCatalogState>,
    mut dialog_queue: ResMut<'_, DialogQueue>,
) {
    let Some(task) = request_state.pending.as_ref() else {
        return;
    };
    let Some(result) = try_recv_pending_result(&task.receiver) else {
        return;
    };
    request_state.pending = None;

    match result {
        Ok(payload) => {
            request_state.completed = true;
            request_state.failed = false;
            asset_manager.startup_manifest_seen = true;
            asset_manager.startup_phase_complete = true;
            asset_manager.startup_total_bytes = payload.startup_total_bytes;
            asset_manager.startup_ready_bytes = payload.startup_ready_bytes;
            asset_manager.cache_index = payload.cache_index;
            asset_manager.cache_index_loaded = true;
            asset_manager.catalog_version = Some(payload.manifest.catalog_version.clone());
            asset_manager.catalog_by_asset_id.clear();
            for entry in &payload.manifest.catalog {
                asset_manager.catalog_by_asset_id.insert(
                    entry.asset_id.clone(),
                    RuntimeAssetCatalogRecord {
                        _asset_guid: entry.asset_guid.clone(),
                        shader_family: entry.shader_family.clone(),
                        dependencies: entry.dependencies.clone(),
                        url: entry.url.clone(),
                        relative_cache_path: entry.relative_cache_path.clone(),
                        content_type: entry.content_type.clone(),
                        _byte_len: entry.byte_len,
                        sha256_hex: entry.sha256_hex.clone(),
                    },
                );
            }
            asset_manager.records_by_asset_id.clear();
            for record in payload.records {
                asset_manager.records_by_asset_id.insert(
                    record.asset_id,
                    LocalAssetRecord {
                        relative_cache_path: record.relative_cache_path,
                        _content_type: record.content_type,
                        _byte_len: record.byte_len,
                        _chunk_count: 1,
                        _asset_version: record.asset_version,
                        _sha256_hex: record.sha256_hex,
                        ready: record.ready,
                    },
                );
            }
            audio_catalog.apply_registry(
                payload.manifest.audio_catalog_version,
                payload.manifest.audio_catalog,
            );
            session.status = format!(
                "Startup preload complete ({} required assets).",
                payload.manifest.required_assets.len()
            );
            session.ui_dirty = true;
        }
        Err(err) => {
            request_state.completed = false;
            request_state.failed = true;
            asset_manager.startup_manifest_seen = true;
            asset_manager.startup_phase_complete = false;
            session.status = format!("Startup preload failed: {err}");
            session.ui_dirty = true;
            dialog_queue.push_error("Startup Asset Preload Failed", err);
        }
    }

    next_state.set(ClientAppState::Auth);
}

#[cfg(test)]
mod tests {
    use super::{StartupAssetRequestState, poll_startup_asset_request_results};
    use crate::runtime::app_state::{ClientAppState, ClientSession};
    use crate::runtime::assets::LocalAssetManager;
    use crate::runtime::audio::AudioCatalogState;
    use crate::runtime::dialog_ui::DialogQueue;
    use async_channel::bounded;
    use bevy::prelude::*;
    use bevy::tasks::{IoTaskPool, TaskPool};
    use sidereal_core::gateway_dtos::{AssetBootstrapManifestEntry, StartupAssetManifestResponse};

    #[test]
    fn startup_success_applies_catalog_and_advances_to_auth() {
        IoTaskPool::get_or_init(TaskPool::new);

        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<ClientAppState>();
        app.insert_resource(ClientSession::default());
        app.insert_resource(LocalAssetManager::default());
        app.insert_resource(AudioCatalogState::default());
        app.insert_resource(DialogQueue::default());
        let (sender, receiver) = bounded(1);
        IoTaskPool::get()
            .spawn(async move {
                let _ = sender
                    .send(Ok(super::StartupAssetRequestResult {
                        manifest: StartupAssetManifestResponse {
                            catalog_version: "startup-v1".to_string(),
                            audio_catalog_version: "audio-v1".to_string(),
                            required_assets: vec![AssetBootstrapManifestEntry {
                                asset_id: "audio.music.menu_loop".to_string(),
                                asset_guid: "guid-1".to_string(),
                                shader_family: None,
                                dependencies: Vec::new(),
                                startup_required: true,
                                sha256_hex: "abc".to_string(),
                                relative_cache_path: "audio/menu-loop.ogg".to_string(),
                                content_type: "audio/ogg".to_string(),
                                byte_len: 123,
                                url: "/startup-assets/guid-1".to_string(),
                            }],
                            catalog: vec![AssetBootstrapManifestEntry {
                                asset_id: "audio.music.menu_loop".to_string(),
                                asset_guid: "guid-1".to_string(),
                                shader_family: None,
                                dependencies: Vec::new(),
                                startup_required: true,
                                sha256_hex: "abc".to_string(),
                                relative_cache_path: "audio/menu-loop.ogg".to_string(),
                                content_type: "audio/ogg".to_string(),
                                byte_len: 123,
                                url: "/startup-assets/guid-1".to_string(),
                            }],
                            audio_catalog: sidereal_audio::AudioRegistry {
                                schema_version: 1,
                                buses: vec![sidereal_audio::AudioBusDefinition {
                                    bus_id: "music".to_string(),
                                    parent: Some("master".to_string()),
                                    default_volume_db: Some(0.0),
                                    muted: Some(false),
                                }],
                                sends: Vec::new(),
                                environments: Vec::new(),
                                concurrency_groups: Vec::new(),
                                clips: Vec::new(),
                                profiles: Vec::new(),
                            },
                        },
                        records: vec![super::StartupAssetRecord {
                            asset_id: "audio.music.menu_loop".to_string(),
                            relative_cache_path: "audio/menu-loop.ogg".to_string(),
                            content_type: "audio/ogg".to_string(),
                            byte_len: 123,
                            asset_version: 1,
                            sha256_hex: "abc".to_string(),
                            ready: true,
                        }],
                        cache_index: Default::default(),
                        startup_total_bytes: 123,
                        startup_ready_bytes: 123,
                    }))
                    .await;
            })
            .detach();
        app.insert_resource(StartupAssetRequestState {
            pending: Some(super::StartupAssetRequestTask { receiver }),
            submitted: true,
            completed: false,
            failed: false,
        });
        app.add_systems(Update, poll_startup_asset_request_results);

        app.update();
        app.update();

        let request_state = app.world().resource::<StartupAssetRequestState>();
        let session = app.world().resource::<ClientSession>();
        let asset_manager = app.world().resource::<LocalAssetManager>();
        let audio_catalog = app.world().resource::<AudioCatalogState>();
        let state = app.world().resource::<State<ClientAppState>>();

        assert!(request_state.pending.is_none());
        assert!(request_state.completed);
        assert!(!request_state.failed);
        assert!(asset_manager.startup_manifest_seen);
        assert!(asset_manager.startup_phase_complete);
        assert_eq!(asset_manager.startup_total_bytes, 123);
        assert_eq!(
            asset_manager
                .catalog_by_asset_id
                .get("audio.music.menu_loop")
                .expect("startup catalog entry")
                .relative_cache_path,
            "audio/menu-loop.ogg"
        );
        assert_eq!(audio_catalog.version.as_deref(), Some("audio-v1"));
        assert_eq!(state.get(), &ClientAppState::Auth);
        assert_eq!(
            session.status,
            "Startup preload complete (1 required assets)."
        );
    }
}
