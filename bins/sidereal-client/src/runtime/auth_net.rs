//! Gateway auth API, Lightyear auth/session-ready messages, headless session config.

use super::app_state::*;
use super::assets::{
    AssetCatalogHotReloadState, LocalAssetManager, LocalAssetRecord, RuntimeAssetCatalogRecord,
};
use super::audio::AudioCatalogState;
use super::resources::AssetRootPath;
use super::resources::{
    AssetCacheAdapter, ClientAuthSyncState, GatewayHttpAdapter, HeadlessAccountSwitchPlan,
    HeadlessTransportMode, LogoutCleanupRequested, PendingDisconnectNotify,
    SessionReadyWatchdogConfig, SessionReadyWatchdogState,
};
use async_channel::{Receiver, TryRecvError, bounded};
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};
use lightyear::prelude::{MessageReceiver, MessageSender, Transport};
use sidereal_asset_runtime::{AssetCacheIndexRecord, asset_version_from_sha256_hex, sha256_hex};
use sidereal_core::gateway_dtos::{
    AssetBootstrapManifestResponse, AuthTokens, CharactersResponse, EnterWorldRequest,
    EnterWorldResponse, LoginRequest, MeResponse, PasswordResetConfirmRequest,
    PasswordResetRequest, RegisterRequest,
};
use sidereal_net::{
    ClientAuthMessage, ControlChannel, LIGHTYEAR_PROTOCOL_VERSION, PlayerEntityId,
    ServerSessionDeniedMessage, ServerSessionReadyMessage,
};

fn canonicalize_player_entity_id(raw: &str) -> String {
    PlayerEntityId::parse(raw)
        .map(PlayerEntityId::canonical_wire_id)
        .unwrap_or_else(|| raw.to_string())
}

#[cfg(target_arch = "wasm32")]
fn validate_world_entry_transport(
    transport: &sidereal_core::gateway_dtos::ReplicationTransportConfig,
) -> Result<(), String> {
    if transport
        .webtransport_addr
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err(
            "Gateway world-entry response did not include a WebTransport address. Configure replication/gateway WebTransport env before using the browser client.".to_string(),
        );
    }
    if transport
        .webtransport_certificate_sha256
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err(
            "Gateway world-entry response did not include a WebTransport certificate digest. Configure replication/gateway WebTransport env before using the browser client.".to_string(),
        );
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_world_entry_transport(
    _transport: &sidereal_core::gateway_dtos::ReplicationTransportConfig,
) -> Result<(), String> {
    Ok(())
}

#[derive(Resource, Default)]
pub struct GatewayRequestState {
    pending: Option<GatewayRequestTask>,
}

struct GatewayRequestTask {
    receiver: Receiver<GatewayRequestResult>,
}

#[derive(Debug)]
enum GatewayRequestResult {
    Auth(AuthRequestResult),
    EnterWorld(EnterWorldRequestResult),
}

#[derive(Debug)]
enum AuthRequestResult {
    LoginOrRegister {
        tokens: AuthTokens,
        me: MeResponse,
        characters: CharactersResponse,
    },
    PasswordResetRequested {
        reset_token: Option<String>,
    },
    PasswordResetConfirmed,
    Error(String),
}

#[derive(Debug)]
enum EnterWorldRequestResult {
    Accepted {
        player_entity_id: String,
        replication_transport: sidereal_core::gateway_dtos::ReplicationTransportConfig,
    },
    Rejected {
        reason: String,
    },
    Error(String),
}

#[derive(Resource, Default)]
pub struct AssetBootstrapRequestState {
    pending: Option<AssetBootstrapRequestTask>,
    pub submitted: bool,
    pub completed: bool,
    pub failed: bool,
}

struct AssetBootstrapRequestTask {
    receiver: Receiver<Result<AssetBootstrapRequestResult, String>>,
}

const MAX_PARALLEL_BOOTSTRAP_FETCHES: usize = 4;

#[derive(Debug)]
struct AssetBootstrapRequestResult {
    manifest: AssetBootstrapManifestResponse,
    records: Vec<AssetBootstrapRecord>,
    cache_index: sidereal_asset_runtime::AssetCacheIndex,
    bootstrap_total_bytes: u64,
    bootstrap_ready_bytes: u64,
}

#[derive(Debug, Clone)]
struct AssetBootstrapRecord {
    asset_id: String,
    relative_cache_path: String,
    content_type: String,
    byte_len: u64,
    asset_version: u64,
    sha256_hex: String,
    ready: bool,
}

#[derive(Debug)]
struct AssetBootstrapFetchedAsset {
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

pub fn init_gateway_request_state(app: &mut App) {
    app.insert_resource(GatewayRequestState::default());
    app.insert_resource(AssetBootstrapRequestState::default());
}

pub fn submit_auth_request(
    session: &mut ClientSession,
    request_state: &mut GatewayRequestState,
    gateway_http: GatewayHttpAdapter,
) {
    if request_state.pending.is_some() {
        session.status = "Another gateway request is already in progress.".to_string();
        session.ui_dirty = true;
        return;
    }

    let gateway_url = session.gateway_url.clone();
    let selected_action = session.selected_action;
    let email = session.email.clone();
    let password = session.password.clone();
    let reset_token = session.reset_token.clone();
    let new_password = session.new_password.clone();
    session.status = "Submitting request...".to_string();
    session.ui_dirty = true;
    let (sender, receiver) = bounded(1);
    IoTaskPool::get()
        .spawn(async move {
            let auth_result: Result<(Option<AuthTokens>, Option<String>), String> =
                match selected_action {
                    AuthAction::Login => (gateway_http.login)(
                        gateway_url.clone(),
                        LoginRequest {
                            email: email.clone(),
                            password: password.clone(),
                        },
                    )
                    .await
                    .map(|tokens| (Some(tokens), None::<String>)),
                    AuthAction::Register => (gateway_http.register)(
                        gateway_url.clone(),
                        RegisterRequest {
                            email: email.clone(),
                            password: password.clone(),
                        },
                    )
                    .await
                    .map(|tokens| (Some(tokens), None::<String>)),
                    AuthAction::ForgotRequest => (gateway_http.request_password_reset)(
                        gateway_url.clone(),
                        PasswordResetRequest {
                            email: email.clone(),
                        },
                    )
                    .await
                    .map(|resp| (None, resp.reset_token)),
                    AuthAction::ForgotConfirm => (gateway_http.confirm_password_reset)(
                        gateway_url.clone(),
                        PasswordResetConfirmRequest {
                            reset_token: reset_token.clone(),
                            new_password: new_password.clone(),
                        },
                    )
                    .await
                    .map(|()| (None, None::<String>)),
                };

            let request_result = match auth_result {
                Ok((Some(tokens), _)) => {
                    match fetch_auth_me(gateway_http, &gateway_url, &tokens.access_token).await {
                        Ok(me) => match fetch_auth_characters(
                            gateway_http,
                            &gateway_url,
                            &tokens.access_token,
                        )
                        .await
                        {
                            Ok(characters) => {
                                GatewayRequestResult::Auth(AuthRequestResult::LoginOrRegister {
                                    tokens,
                                    me,
                                    characters,
                                })
                            }
                            Err(err) => GatewayRequestResult::Auth(AuthRequestResult::Error(
                                format!("Auth OK but character lookup failed: {err}"),
                            )),
                        },
                        Err(err) => GatewayRequestResult::Auth(AuthRequestResult::Error(format!(
                            "Auth OK but profile lookup failed: {err}"
                        ))),
                    }
                }
                Ok((None, reset_token)) => {
                    if selected_action == AuthAction::ForgotRequest {
                        GatewayRequestResult::Auth(AuthRequestResult::PasswordResetRequested {
                            reset_token,
                        })
                    } else {
                        GatewayRequestResult::Auth(AuthRequestResult::PasswordResetConfirmed)
                    }
                }
                Err(err) => GatewayRequestResult::Auth(AuthRequestResult::Error(format!(
                    "Request failed: {err}"
                ))),
            };
            let _ = sender.send(request_result).await;
        })
        .detach();
    request_state.pending = Some(GatewayRequestTask { receiver });
}

fn fetch_auth_me(
    gateway_http: GatewayHttpAdapter,
    gateway_url: &str,
    access_token: &str,
) -> super::resources::GatewayFuture<MeResponse> {
    (gateway_http.fetch_me)(gateway_url.to_string(), access_token.to_string())
}

fn fetch_auth_characters(
    gateway_http: GatewayHttpAdapter,
    gateway_url: &str,
    access_token: &str,
) -> super::resources::GatewayFuture<CharactersResponse> {
    (gateway_http.fetch_characters)(gateway_url.to_string(), access_token.to_string())
}

fn enter_world_request(
    gateway_http: GatewayHttpAdapter,
    gateway_url: &str,
    access_token: &str,
    player_entity_id: &str,
) -> super::resources::GatewayFuture<EnterWorldResponse> {
    (gateway_http.enter_world)(
        gateway_url.to_string(),
        access_token.to_string(),
        EnterWorldRequest {
            player_entity_id: player_entity_id.to_string(),
        },
    )
}

pub fn submit_enter_world_request(
    session: &mut ClientSession,
    request_state: &mut GatewayRequestState,
    gateway_http: GatewayHttpAdapter,
    player_entity_id: String,
) {
    if request_state.pending.is_some() {
        session.status = "Another gateway request is already in progress.".to_string();
        session.ui_dirty = true;
        return;
    }
    let Some(access_token) = session.access_token.clone() else {
        session.status = "No access token; please log in again.".to_string();
        session.ui_dirty = true;
        return;
    };
    let gateway_url = session.gateway_url.clone();
    let requested_player = player_entity_id;
    session.status = "Submitting Enter World request...".to_string();
    session.ui_dirty = true;

    let (sender, receiver) = bounded(1);
    IoTaskPool::get()
        .spawn(async move {
            let result = match enter_world_request(
                gateway_http,
                &gateway_url,
                &access_token,
                &requested_player,
            )
            .await
            {
                Ok(response) if response.accepted => {
                    GatewayRequestResult::EnterWorld(EnterWorldRequestResult::Accepted {
                        player_entity_id: requested_player,
                        replication_transport: response.replication_transport,
                    })
                }
                Ok(_) => GatewayRequestResult::EnterWorld(EnterWorldRequestResult::Rejected {
                    reason: "Enter World request rejected by gateway.".to_string(),
                }),
                Err(err) => GatewayRequestResult::EnterWorld(EnterWorldRequestResult::Error(
                    format!("Enter World failed: {err}"),
                )),
            };
            let _ = sender.send(result).await;
        })
        .detach();
    request_state.pending = Some(GatewayRequestTask { receiver });
}

pub fn submit_asset_bootstrap_request(
    session: &mut ClientSession,
    request_state: &mut AssetBootstrapRequestState,
    gateway_http: GatewayHttpAdapter,
    cache_adapter: AssetCacheAdapter,
    asset_root: &str,
) {
    if request_state.pending.is_some() {
        info!("asset bootstrap request already pending; skipping duplicate submit");
        return;
    }
    let Some(access_token) = session.access_token.clone() else {
        session.status = "Missing access token for asset bootstrap.".to_string();
        session.ui_dirty = true;
        warn!("asset bootstrap request skipped: missing access token");
        return;
    };
    let gateway_url = session.gateway_url.clone();
    let asset_root = asset_root.to_string();
    request_state.submitted = true;
    request_state.completed = false;
    request_state.failed = false;
    session.status = "Fetching asset bootstrap manifest...".to_string();
    session.ui_dirty = true;
    info!(
        "asset bootstrap request submitted: gateway_url={} asset_root={}",
        gateway_url, asset_root
    );

    let (sender, receiver) = bounded(1);
    IoTaskPool::get()
        .spawn(async move {
            let result = async move {
                info!("asset bootstrap task starting");
                info!("asset bootstrap preparing cache root");
                (cache_adapter.prepare_root)(asset_root.clone()).await?;
                info!("asset bootstrap cache root prepared");
                info!("asset bootstrap requesting manifest from gateway");
                let manifest =
                    (gateway_http.fetch_bootstrap_manifest)(gateway_url.clone(), access_token.clone())
                        .await?;
                info!(
                    "asset bootstrap manifest fetched: required_assets={} catalog_assets={}",
                    manifest.required_assets.len(),
                    manifest.catalog.len()
                );
                let mut cache_index = (cache_adapter.load_index)(asset_root.clone()).await?;
                let mut records = Vec::<AssetBootstrapRecord>::new();
                let mut bootstrap_total_bytes = 0u64;
                let mut bootstrap_ready_bytes = 0u64;

                for entry in &manifest.catalog {
                    let ready = (cache_adapter.read_valid_asset)(
                        asset_root.clone(),
                        entry.relative_cache_path.clone(),
                        entry.sha256_hex.clone(),
                    )
                    .await?
                    .is_some();
                    records.push(AssetBootstrapRecord {
                        asset_id: entry.asset_id.clone(),
                        relative_cache_path: entry.relative_cache_path.clone(),
                        content_type: entry.content_type.clone(),
                        byte_len: entry.byte_len,
                        asset_version: asset_version_from_sha256_hex(&entry.sha256_hex),
                        sha256_hex: entry.sha256_hex.clone(),
                        ready,
                    });
                }

                let mut missing_required_assets = Vec::new();
                for required in &manifest.required_assets {
                    bootstrap_total_bytes = bootstrap_total_bytes.saturating_add(required.byte_len);
                    let satisfied = (cache_adapter.read_valid_asset)(
                        asset_root.clone(),
                        required.relative_cache_path.clone(),
                        required.sha256_hex.clone(),
                    )
                    .await?
                    .is_some();
                    if !satisfied {
                        missing_required_assets.push(required.clone());
                    }
                    bootstrap_ready_bytes = bootstrap_ready_bytes.saturating_add(required.byte_len);
                }

                let mut pending_fetches = Vec::<Task<Result<AssetBootstrapFetchedAsset, String>>>::new();
                for required in missing_required_assets {
                    let gateway_http = gateway_http;
                    let gateway_url = gateway_url.clone();
                    let access_token = access_token.clone();
                    pending_fetches.push(IoTaskPool::get().spawn(async move {
                        let url = if required.url.starts_with("http://")
                            || required.url.starts_with("https://")
                        {
                            required.url.clone()
                        } else {
                            format!("{gateway_url}{}", required.url)
                        };
                        info!(
                            "asset bootstrap download starting: asset_id={} relative_cache_path={} bytes={}",
                            required.asset_id, required.relative_cache_path, required.byte_len
                        );
                        let payload = (gateway_http.fetch_asset_bytes)(url, access_token).await?;
                        let payload_sha = sha256_hex(&payload);
                        if payload_sha != required.sha256_hex {
                            return Err(format!(
                                "asset checksum mismatch asset_id={} expected={} got={}",
                                required.asset_id, required.sha256_hex, payload_sha
                            ));
                        }
                        Ok(AssetBootstrapFetchedAsset {
                            asset_id: required.asset_id,
                            relative_cache_path: required.relative_cache_path,
                            byte_len: required.byte_len,
                            sha256_hex: required.sha256_hex,
                            payload,
                        })
                    }));
                    if pending_fetches.len() >= MAX_PARALLEL_BOOTSTRAP_FETCHES {
                        let fetched = pending_fetches.remove(0).await?;
                        (cache_adapter.write_asset)(
                            asset_root.clone(),
                            fetched.relative_cache_path.clone(),
                            fetched.payload,
                        )
                        .await?;
                        info!(
                            "asset bootstrap cache write complete: asset_id={} relative_cache_path={} bytes={}",
                            fetched.asset_id, fetched.relative_cache_path, fetched.byte_len
                        );
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
                        "asset bootstrap cache write complete: asset_id={} relative_cache_path={} bytes={}",
                        fetched.asset_id, fetched.relative_cache_path, fetched.byte_len
                    );
                    cache_index.by_asset_id.insert(
                        fetched.asset_id,
                        AssetCacheIndexRecord {
                            asset_version: asset_version_from_sha256_hex(&fetched.sha256_hex),
                            sha256_hex: fetched.sha256_hex,
                        },
                    );
                }

                for required in &manifest.required_assets {
                    let version = asset_version_from_sha256_hex(&required.sha256_hex);
                    cache_index.by_asset_id.insert(
                        required.asset_id.clone(),
                        AssetCacheIndexRecord {
                            asset_version: version,
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

                Ok(AssetBootstrapRequestResult {
                    manifest,
                    records,
                    cache_index,
                    bootstrap_total_bytes,
                    bootstrap_ready_bytes,
                })
            }
            .await;
            let _ = sender.send(result).await;
        })
        .detach();
    request_state.pending = Some(AssetBootstrapRequestTask { receiver });
}

fn session_matches_ready_player(
    session: &ClientSession,
    session_ready: &SessionReadyState,
) -> bool {
    match (
        session
            .player_entity_id
            .as_deref()
            .and_then(PlayerEntityId::parse),
        session_ready
            .ready_player_entity_id
            .as_deref()
            .and_then(PlayerEntityId::parse),
    ) {
        (Some(local), Some(ready)) => local == ready,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn poll_gateway_request_results(
    mut request_state: ResMut<'_, GatewayRequestState>,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
    mut session: ResMut<'_, ClientSession>,
    mut character_selection: ResMut<'_, CharacterSelectionState>,
    mut session_ready: ResMut<'_, SessionReadyState>,
    mut auth_sync: ResMut<'_, super::resources::ClientAuthSyncState>,
    mut dialog_queue: ResMut<'_, super::dialog_ui::DialogQueue>,
    mut asset_bootstrap_state: ResMut<'_, AssetBootstrapRequestState>,
) {
    let Some(task) = request_state.pending.as_ref() else {
        return;
    };
    let Some(payload) = try_recv_pending_result(&task.receiver) else {
        return;
    };
    request_state.pending = None;

    match payload {
        GatewayRequestResult::Auth(AuthRequestResult::LoginOrRegister {
            tokens,
            me,
            characters,
        }) => {
            session.access_token = Some(tokens.access_token.clone());
            session.refresh_token = Some(tokens.refresh_token);
            session.account_id = Some(me.account_id.clone());
            character_selection.characters = characters
                .characters
                .into_iter()
                .map(|c| c.player_entity_id)
                .collect();
            if character_selection.characters.is_empty() {
                session.status = "Authenticated but no characters are available.".to_string();
                dialog_queue.push_error(
                    "No Characters",
                    "This account has no characters. Character creation UI is not implemented yet."
                        .to_string(),
                );
            } else {
                character_selection.selected_player_entity_id =
                    character_selection.characters.first().cloned();
                session.player_entity_id = None;
                session_ready.ready_player_entity_id = None;
                session.status =
                    "Authenticated. Select a character and press Enter World.".to_string();
                next_state.set(ClientAppState::CharacterSelect);
            }
        }
        GatewayRequestResult::Auth(AuthRequestResult::PasswordResetRequested { reset_token }) => {
            if let Some(token) = reset_token {
                session.reset_token = token;
            }
            session.status = "Password reset token requested. Use F4 to confirm reset.".to_string();
        }
        GatewayRequestResult::Auth(AuthRequestResult::PasswordResetConfirmed) => {
            session.status = "Password reset confirmed. Switch to Login (F1).".to_string();
        }
        GatewayRequestResult::EnterWorld(EnterWorldRequestResult::Accepted {
            player_entity_id,
            replication_transport,
        }) => {
            if let Err(err) = validate_world_entry_transport(&replication_transport) {
                session.status = err.clone();
                dialog_queue.push_error("Browser Transport Unavailable", err);
                session.ui_dirty = true;
                return;
            }
            session.player_entity_id = Some(canonicalize_player_entity_id(&player_entity_id));
            session.replication_transport = replication_transport;
            auth_sync.sent_for_client_entities.clear();
            auth_sync.last_player_entity_id = None;
            session_ready.ready_player_entity_id = None;
            asset_bootstrap_state.submitted = false;
            asset_bootstrap_state.completed = false;
            asset_bootstrap_state.failed = false;
            asset_bootstrap_state.pending = None;
            session.status = "World entry accepted. Waiting for replication bind...".to_string();
            next_state.set(ClientAppState::WorldLoading);
        }
        GatewayRequestResult::Auth(AuthRequestResult::Error(err))
        | GatewayRequestResult::EnterWorld(EnterWorldRequestResult::Error(err)) => {
            session.status = err.clone();
            dialog_queue.push_error("Gateway Request Failed", err);
        }
        GatewayRequestResult::EnterWorld(EnterWorldRequestResult::Rejected { reason }) => {
            session.status = reason;
        }
    }
    session.ui_dirty = true;
}

pub fn poll_asset_bootstrap_request_results(
    mut request_state: ResMut<'_, AssetBootstrapRequestState>,
    mut session: ResMut<'_, ClientSession>,
    mut asset_manager: ResMut<'_, LocalAssetManager>,
    mut audio_catalog: ResMut<'_, AudioCatalogState>,
    mut hot_reload: ResMut<'_, AssetCatalogHotReloadState>,
    mut dialog_queue: ResMut<'_, super::dialog_ui::DialogQueue>,
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
            let previous_catalog_version = asset_manager.catalog_version.clone();
            let previous_catalog = asset_manager.catalog_by_asset_id.clone();
            let next_catalog_version = payload.manifest.catalog_version.clone();
            let next_catalog_ids = payload
                .manifest
                .catalog
                .iter()
                .map(|entry| entry.asset_id.clone())
                .collect::<std::collections::HashSet<_>>();
            request_state.completed = true;
            request_state.failed = false;
            asset_manager.bootstrap_manifest_seen = true;
            audio_catalog.apply_registry(
                payload.manifest.audio_catalog_version.clone(),
                payload.manifest.audio_catalog.clone(),
            );
            asset_manager.bootstrap_total_bytes = payload.bootstrap_total_bytes;
            asset_manager.bootstrap_ready_bytes = payload.bootstrap_ready_bytes;
            asset_manager.bootstrap_phase_complete = true;
            asset_manager.cache_index = payload.cache_index;
            asset_manager
                .cache_index
                .by_asset_id
                .retain(|asset_id, _| next_catalog_ids.contains(asset_id));
            asset_manager.cache_index_loaded = true;
            asset_manager.catalog_version = Some(next_catalog_version.clone());
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
            asset_manager
                .records_by_asset_id
                .retain(|asset_id, _| next_catalog_ids.contains(asset_id));
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
            let mut changed_asset_ids = asset_manager
                .catalog_by_asset_id
                .iter()
                .filter_map(|(asset_id, next)| match previous_catalog.get(asset_id) {
                    Some(previous)
                        if previous.sha256_hex == next.sha256_hex
                            && previous.relative_cache_path == next.relative_cache_path =>
                    {
                        None
                    }
                    _ => Some(asset_id.clone()),
                })
                .collect::<std::collections::HashSet<_>>();
            if previous_catalog_version.as_deref() != Some(next_catalog_version.as_str()) {
                asset_manager.reload_generation = asset_manager.reload_generation.saturating_add(1);
            }
            hot_reload
                .forced_asset_ids
                .retain(|asset_id| asset_manager.catalog_by_asset_id.contains_key(asset_id));
            hot_reload
                .forced_asset_ids
                .extend(changed_asset_ids.drain());
            if hot_reload.pending_catalog_version.as_deref() == Some(next_catalog_version.as_str())
            {
                hot_reload.pending_catalog_version = None;
            }
            session.status = format!(
                "Asset bootstrap complete ({} required assets).",
                payload.manifest.required_assets.len()
            );
            session.ui_dirty = true;
        }
        Err(err) => {
            request_state.completed = false;
            request_state.failed = true;
            asset_manager.bootstrap_manifest_seen = true;
            asset_manager.bootstrap_phase_complete = false;
            session.status = format!("Asset bootstrap failed: {err}");
            session.ui_dirty = true;
            dialog_queue.push_error("Asset Bootstrap Failed", err);
        }
    }
}

pub fn submit_asset_bootstrap_after_session_ready(
    mut session: ResMut<'_, ClientSession>,
    session_ready: Res<'_, SessionReadyState>,
    mut request_state: ResMut<'_, AssetBootstrapRequestState>,
    asset_root: Res<'_, AssetRootPath>,
    gateway_http: Res<'_, GatewayHttpAdapter>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
) {
    if request_state.submitted || request_state.pending.is_some() {
        return;
    }
    if !session_matches_ready_player(&session, &session_ready) {
        return;
    }

    submit_asset_bootstrap_request(
        session.as_mut(),
        request_state.as_mut(),
        *gateway_http,
        *cache_adapter,
        &asset_root.0,
    );
}

pub fn trigger_asset_catalog_refresh_requests(
    mut session: ResMut<'_, ClientSession>,
    mut request_state: ResMut<'_, AssetBootstrapRequestState>,
    hot_reload: Res<'_, AssetCatalogHotReloadState>,
    asset_root: Res<'_, AssetRootPath>,
    gateway_http: Res<'_, GatewayHttpAdapter>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
) {
    let Some(_pending_catalog_version) = hot_reload.pending_catalog_version.as_ref() else {
        return;
    };
    if request_state.pending.is_some() {
        return;
    }
    submit_asset_bootstrap_request(
        session.as_mut(),
        request_state.as_mut(),
        *gateway_http,
        *cache_adapter,
        &asset_root.0,
    );
}

pub fn configure_headless_session_from_env(
    mut commands: Commands<'_, '_>,
    mut session: ResMut<'_, ClientSession>,
) {
    if let Ok(player_entity_id) = std::env::var("SIDEREAL_CLIENT_HEADLESS_PLAYER_ENTITY_ID") {
        session.player_entity_id = Some(canonicalize_player_entity_id(&player_entity_id));
    }
    if let Ok(access_token) = std::env::var("SIDEREAL_CLIENT_HEADLESS_ACCESS_TOKEN") {
        session.access_token = Some(access_token);
    }
    let next_player = std::env::var("SIDEREAL_CLIENT_HEADLESS_SWITCH_PLAYER_ENTITY_ID").ok();
    let next_token = std::env::var("SIDEREAL_CLIENT_HEADLESS_SWITCH_ACCESS_TOKEN").ok();
    if let (Some(next_player_entity_id), Some(next_access_token)) = (next_player, next_token) {
        let switch_after_s = std::env::var("SIDEREAL_CLIENT_HEADLESS_SWITCH_AFTER_S")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(1.0)
            .max(0.0);
        commands.insert_resource(HeadlessAccountSwitchPlan {
            switch_after_s,
            switched: false,
            next_player_entity_id,
            next_access_token,
        });
    }
}

pub fn apply_headless_account_switch_system(
    time: Res<'_, Time>,
    mut session: ResMut<'_, ClientSession>,
    plan: Option<ResMut<'_, HeadlessAccountSwitchPlan>>,
) {
    let Some(mut plan) = plan else {
        return;
    };
    if plan.switched || time.elapsed_secs_f64() < plan.switch_after_s {
        return;
    }
    session.player_entity_id = Some(canonicalize_player_entity_id(&plan.next_player_entity_id));
    session.access_token = Some(plan.next_access_token.clone());
    plan.switched = true;
    info!(
        "headless account switch applied player_entity_id={}",
        plan.next_player_entity_id
    );
}

#[allow(clippy::type_complexity)]
pub fn send_lightyear_auth_messages(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    headless_mode: Res<'_, HeadlessTransportMode>,
    time: Res<'_, Time>,
    session: Res<'_, ClientSession>,
    session_ready: Res<'_, SessionReadyState>,
    mut auth_state: ResMut<'_, ClientAuthSyncState>,
    mut senders: Query<
        '_,
        '_,
        (
            Entity,
            &mut MessageSender<ClientAuthMessage>,
            Option<&Transport>,
            Has<lightyear::prelude::client::Connected>,
        ),
        With<lightyear::prelude::client::Client>,
    >,
) {
    const AUTH_RESEND_INTERVAL_S: f64 = 2.0;
    let active_world_state = is_active_world_state(&app_state, &headless_mode);
    if !active_world_state {
        return;
    }
    let Some(access_token) = session.access_token.as_ref() else {
        return;
    };
    let Some(player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let Some(canonical_player_entity_id) =
        PlayerEntityId::parse(player_entity_id.as_str()).map(PlayerEntityId::canonical_wire_id)
    else {
        return;
    };
    if auth_state.last_player_entity_id.as_deref() != Some(canonical_player_entity_id.as_str()) {
        auth_state.sent_for_client_entities.clear();
        auth_state.last_sent_at_s_by_client_entity.clear();
        auth_state.last_player_entity_id = Some(canonical_player_entity_id.clone());
    }
    let now_s = time.elapsed_secs_f64();
    let session_ready_for_player = session_ready
        .ready_player_entity_id
        .as_deref()
        .and_then(PlayerEntityId::parse)
        .is_some_and(|ready_id| ready_id.canonical_wire_id() == canonical_player_entity_id);

    for (client_entity, mut sender, transport, connected) in &mut senders {
        if !connected {
            continue;
        }
        let sent_before = auth_state.sent_for_client_entities.contains(&client_entity);
        if sent_before && session_ready_for_player {
            continue;
        }
        let last_sent_at_s = auth_state
            .last_sent_at_s_by_client_entity
            .get(&client_entity)
            .copied()
            .unwrap_or(0.0);
        let should_resend_while_unbound =
            !session_ready_for_player && now_s - last_sent_at_s >= AUTH_RESEND_INTERVAL_S;
        if sent_before && !should_resend_while_unbound {
            continue;
        }
        let has_control_sender =
            transport.is_some_and(|transport| transport.has_sender::<ControlChannel>());
        if !has_control_sender {
            warn!(
                "client auth bind skipped: ControlChannel sender missing for connected client_entity={:?}",
                client_entity
            );
            continue;
        }
        let auth_message = ClientAuthMessage {
            player_entity_id: canonical_player_entity_id.clone(),
            access_token: access_token.clone(),
        };
        sender.send::<ControlChannel>(auth_message);
        info!(
            "client auth bind message sent for player_entity_id={} client_entity={:?} has_control_sender={}",
            canonical_player_entity_id, client_entity, has_control_sender
        );
        auth_state.sent_for_client_entities.insert(client_entity);
        auth_state
            .last_sent_at_s_by_client_entity
            .insert(client_entity, now_s);
    }
}

pub fn receive_lightyear_session_ready_messages(
    mut receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<ServerSessionReadyMessage>,
        With<lightyear::prelude::client::Client>,
    >,
    session: Res<'_, ClientSession>,
    mut session_ready: ResMut<'_, SessionReadyState>,
    mut cleanup_requested: ResMut<'_, LogoutCleanupRequested>,
    mut dialog_queue: ResMut<'_, super::dialog_ui::DialogQueue>,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let Some(local_player_id) = PlayerEntityId::parse(local_player_entity_id.as_str()) else {
        return;
    };
    for mut receiver in &mut receivers {
        for message in receiver.receive() {
            let Some(message_player_id) = PlayerEntityId::parse(message.player_entity_id.as_str())
            else {
                continue;
            };
            if message_player_id != local_player_id {
                continue;
            }
            if message.protocol_version != LIGHTYEAR_PROTOCOL_VERSION {
                warn!(
                    "session ready protocol mismatch: server={} client={}",
                    message.protocol_version, LIGHTYEAR_PROTOCOL_VERSION
                );
                dialog_queue.push_error(
                    "Protocol Mismatch",
                    format!(
                        "Replication protocol mismatch.\n\nServer protocol: {}\nClient protocol: {}\n\nRestart client and server with the same build.",
                        message.protocol_version, LIGHTYEAR_PROTOCOL_VERSION
                    ),
                );
                cleanup_requested.0 = true;
                continue;
            }
            info!(
                "client session ready received for player_entity_id={}",
                message.player_entity_id
            );
            session_ready.ready_player_entity_id = Some(message.player_entity_id);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn watch_session_ready_timeout_system(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    headless_mode: Res<'_, HeadlessTransportMode>,
    time: Res<'_, Time>,
    session: Res<'_, ClientSession>,
    session_ready: Res<'_, SessionReadyState>,
    clients: Query<'_, '_, Entity, With<lightyear::prelude::client::Client>>,
    cfg: Res<'_, SessionReadyWatchdogConfig>,
    mut watchdog: ResMut<'_, SessionReadyWatchdogState>,
    pending_disconnect: Res<'_, PendingDisconnectNotify>,
    mut cleanup_requested: ResMut<'_, LogoutCleanupRequested>,
    mut dialog_queue: ResMut<'_, super::dialog_ui::DialogQueue>,
) {
    let active_world_state = is_active_world_state(&app_state, &headless_mode);
    if !active_world_state
        || cleanup_requested.0
        || pending_disconnect.0.is_some()
        || session.access_token.is_none()
        || session.player_entity_id.is_none()
        || clients.is_empty()
    {
        watchdog.started_at_s = None;
        return;
    }

    let session_ready_for_player = session_matches_ready_player(&session, &session_ready);
    if session_ready_for_player {
        watchdog.started_at_s = None;
        return;
    }

    let now_s = time.elapsed_secs_f64();
    let started_at_s = *watchdog.started_at_s.get_or_insert(now_s);
    if now_s - started_at_s < cfg.timeout_s {
        return;
    }

    warn!(
        "session bind timeout after {:.1}s without ServerSessionReady; forcing disconnect (likely protocol/build mismatch)",
        cfg.timeout_s
    );
    dialog_queue.push_error(
        "Replication Session Failed",
        "The client could not bind to the replication session in time.\n\nThis usually means the client and replication server are running different builds/protocols.\n\nRestart both and try again.",
    );
    cleanup_requested.0 = true;
    watchdog.started_at_s = None;
}

pub fn receive_lightyear_session_denied_messages(
    mut receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<ServerSessionDeniedMessage>,
        With<lightyear::prelude::client::Client>,
    >,
    session: Res<'_, ClientSession>,
    mut dialog_queue: ResMut<'_, super::dialog_ui::DialogQueue>,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let Some(local_player_id) = PlayerEntityId::parse(local_player_entity_id.as_str()) else {
        return;
    };
    for mut receiver in &mut receivers {
        for message in receiver.receive() {
            let player_matches = PlayerEntityId::parse(message.player_entity_id.as_str())
                .map(|message_player_id| message_player_id == local_player_id)
                .unwrap_or_else(|| message.player_entity_id == *local_player_entity_id);
            if !player_matches {
                continue;
            }
            warn!(
                "session denied for player {}: {}",
                message.player_entity_id, message.reason
            );
            dialog_queue.push_error(
                "Session Denied",
                format!(
                    "The server denied your session.\n\nReason: {}\n\n\
                     Returning to character select.",
                    message.reason
                ),
            );
            next_state.set(ClientAppState::CharacterSelect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AssetBootstrapRequestState, poll_asset_bootstrap_request_results,
        submit_asset_bootstrap_after_session_ready,
    };
    use crate::runtime::app_state::{ClientSession, SessionReadyState};
    use crate::runtime::assets::{AssetCatalogHotReloadState, LocalAssetManager};
    use crate::runtime::audio::AudioCatalogState;
    use crate::runtime::dialog_ui::DialogQueue;
    use crate::runtime::resources::{AssetCacheAdapter, AssetRootPath, GatewayHttpAdapter};
    use async_channel::bounded;
    use bevy::prelude::*;
    use bevy::tasks::{IoTaskPool, TaskPool};
    use sidereal_asset_runtime::AssetCacheIndex;
    use sidereal_core::gateway_dtos::AssetBootstrapManifestResponse;

    fn ok_manifest(
        _: String,
        _: String,
    ) -> crate::runtime::resources::GatewayFuture<AssetBootstrapManifestResponse> {
        Box::pin(async {
            Ok(AssetBootstrapManifestResponse {
                catalog_version: "test-catalog".to_string(),
                audio_catalog_version: "test-audio".to_string(),
                required_assets: Vec::new(),
                catalog: Vec::new(),
                audio_catalog: Default::default(),
            })
        })
    }

    fn ok_prepare_root(_: String) -> crate::runtime::resources::CacheFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn ok_load_index(_: String) -> crate::runtime::resources::CacheFuture<AssetCacheIndex> {
        Box::pin(async { Ok(AssetCacheIndex::default()) })
    }

    fn ok_save_index(_: String, _: AssetCacheIndex) -> crate::runtime::resources::CacheFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn ok_read_valid_asset(
        _: String,
        _: String,
        _: String,
    ) -> crate::runtime::resources::CacheFuture<Option<Vec<u8>>> {
        Box::pin(async { Ok(None) })
    }

    fn ok_write_asset(
        _: String,
        _: String,
        _: Vec<u8>,
    ) -> crate::runtime::resources::CacheFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn gateway_http_adapter() -> GatewayHttpAdapter {
        GatewayHttpAdapter {
            login: |_, _| Box::pin(async { Err("unused".to_string()) }),
            register: |_, _| Box::pin(async { Err("unused".to_string()) }),
            request_password_reset: |_, _| Box::pin(async { Err("unused".to_string()) }),
            confirm_password_reset: |_, _| Box::pin(async { Err("unused".to_string()) }),
            fetch_me: |_, _| Box::pin(async { Err("unused".to_string()) }),
            fetch_characters: |_, _| Box::pin(async { Err("unused".to_string()) }),
            enter_world: |_, _, _| Box::pin(async { Err("unused".to_string()) }),
            fetch_startup_manifest: |_| Box::pin(async { Err("unused".to_string()) }),
            fetch_bootstrap_manifest: ok_manifest,
            fetch_public_asset_bytes: |_| Box::pin(async { Err("unused".to_string()) }),
            fetch_asset_bytes: |_, _| Box::pin(async { Err("unused".to_string()) }),
        }
    }

    fn asset_cache_adapter() -> AssetCacheAdapter {
        AssetCacheAdapter {
            prepare_root: ok_prepare_root,
            load_index: ok_load_index,
            save_index: ok_save_index,
            read_valid_asset: ok_read_valid_asset,
            write_asset: ok_write_asset,
            read_valid_asset_sync: |_, _, _| None,
        }
    }

    #[test]
    fn bootstrap_failure_marks_request_failed_and_preserves_fail_closed_state() {
        IoTaskPool::get_or_init(TaskPool::new);

        let mut app = App::new();
        app.insert_resource(ClientSession::default());
        app.insert_resource(LocalAssetManager {
            bootstrap_phase_complete: true,
            ..Default::default()
        });
        app.insert_resource(AudioCatalogState::default());
        app.insert_resource(AssetCatalogHotReloadState::default());
        app.insert_resource(DialogQueue::default());
        let (sender, receiver) = bounded(1);
        IoTaskPool::get()
            .spawn(async move {
                let _ = sender
                    .send(Err("required asset download failed".to_string()))
                    .await;
            })
            .detach();
        app.insert_resource(AssetBootstrapRequestState {
            pending: Some(super::AssetBootstrapRequestTask { receiver }),
            submitted: true,
            completed: false,
            failed: false,
        });
        app.add_systems(Update, poll_asset_bootstrap_request_results);

        app.update();
        app.update();

        let request_state = app.world().resource::<AssetBootstrapRequestState>();
        let session = app.world().resource::<ClientSession>();
        let asset_manager = app.world().resource::<LocalAssetManager>();

        assert!(request_state.pending.is_none());
        assert!(request_state.submitted);
        assert!(!request_state.completed);
        assert!(request_state.failed);
        assert!(asset_manager.bootstrap_manifest_seen);
        assert!(!asset_manager.bootstrap_phase_complete);
        assert_eq!(
            session.status,
            "Asset bootstrap failed: required asset download failed"
        );
        assert!(session.ui_dirty);
    }

    #[test]
    fn asset_bootstrap_waits_for_session_ready() {
        IoTaskPool::get_or_init(TaskPool::new);

        let mut app = App::new();
        app.insert_resource(ClientSession {
            access_token: Some("test-token".to_string()),
            player_entity_id: Some("11111111-1111-1111-1111-111111111111".to_string()),
            ..Default::default()
        });
        app.insert_resource(SessionReadyState::default());
        app.insert_resource(AssetBootstrapRequestState::default());
        app.insert_resource(AssetRootPath("test-assets".to_string()));
        app.insert_resource(gateway_http_adapter());
        app.insert_resource(asset_cache_adapter());
        app.add_systems(Update, submit_asset_bootstrap_after_session_ready);

        app.update();
        assert!(
            !app.world()
                .resource::<AssetBootstrapRequestState>()
                .submitted,
            "bootstrap should not start before session ready"
        );

        app.world_mut()
            .resource_mut::<SessionReadyState>()
            .ready_player_entity_id = Some("11111111-1111-1111-1111-111111111111".to_string());

        app.update();

        let request_state = app.world().resource::<AssetBootstrapRequestState>();
        assert!(request_state.submitted);
        assert!(request_state.pending.is_some());
    }
}
