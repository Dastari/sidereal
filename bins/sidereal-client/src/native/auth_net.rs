//! Gateway auth API, Lightyear auth/session-ready messages, headless session config.

use super::app_state::*;
use super::assets::{LocalAssetManager, LocalAssetRecord, RuntimeAssetCatalogRecord};
use super::resources::AssetRootPath;
use super::resources::{
    ClientAuthSyncState, HeadlessAccountSwitchPlan, HeadlessTransportMode, LogoutCleanupRequested,
    PendingDisconnectNotify, SessionReadyWatchdogConfig, SessionReadyWatchdogState,
};
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task, futures_lite::future};
use lightyear::prelude::{MessageReceiver, MessageSender};
use sidereal_asset_runtime::{
    AssetCacheIndexRecord, asset_version_from_sha256_hex, cache_index_path, load_cache_index,
    save_cache_index, sha256_hex,
};
use sidereal_core::gateway_dtos::{
    AssetBootstrapManifestResponse, AuthTokens, CharactersResponse, EnterWorldRequest,
    EnterWorldResponse, LoginRequest, MeResponse, PasswordResetConfirmRequest,
    PasswordResetConfirmResponse, PasswordResetRequest, PasswordResetResponse, RegisterRequest,
};
use sidereal_net::{
    ClientAuthMessage, ControlChannel, LIGHTYEAR_PROTOCOL_VERSION, PlayerEntityId,
    ServerSessionDeniedMessage, ServerSessionReadyMessage,
};

fn decode_api_json<T: serde::de::DeserializeOwned>(
    response: reqwest::blocking::Response,
) -> Result<T, String> {
    let status = response.status();
    let body = response.text().map_err(|err| err.to_string())?;
    if !status.is_success() {
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&body)
            && let Some(message) = error_json.get("error").and_then(|v| v.as_str())
        {
            return Err(format!("{status}: {message}"));
        }
        if body.trim().is_empty() {
            return Err(status.to_string());
        }
        return Err(format!("{status}: {body}"));
    }
    serde_json::from_str::<T>(&body).map_err(|err| err.to_string())
}

fn canonicalize_player_entity_id(raw: &str) -> String {
    PlayerEntityId::parse(raw)
        .map(PlayerEntityId::canonical_wire_id)
        .unwrap_or_else(|| raw.to_string())
}

#[derive(Resource, Default)]
pub struct GatewayRequestState {
    pending: Option<Task<GatewayRequestResult>>,
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
    Accepted { player_entity_id: String },
    Rejected { reason: String },
    Error(String),
}

#[derive(Resource, Default)]
pub struct AssetBootstrapRequestState {
    pending: Option<Task<Result<AssetBootstrapRequestResult, String>>>,
    pub submitted: bool,
    pub completed: bool,
    pub failed: bool,
}

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

pub fn init_gateway_request_state(app: &mut App) {
    app.insert_resource(GatewayRequestState::default());
    app.insert_resource(AssetBootstrapRequestState::default());
}

pub fn submit_auth_request(session: &mut ClientSession, request_state: &mut GatewayRequestState) {
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
    request_state.pending = Some(IoTaskPool::get().spawn(async move {
        let client = reqwest::blocking::Client::new();
        let result = match selected_action {
            AuthAction::Login => (|| -> Result<(Option<AuthTokens>, Option<String>), String> {
                let response = client
                    .post(format!("{gateway_url}/auth/login"))
                    .json(&LoginRequest { email, password })
                    .send()
                    .map_err(|err| err.to_string())?;
                let tokens = decode_api_json::<AuthTokens>(response)?;
                Ok((Some(tokens), None::<String>))
            })(),
            AuthAction::Register => (|| -> Result<(Option<AuthTokens>, Option<String>), String> {
                let response = client
                    .post(format!("{gateway_url}/auth/register"))
                    .json(&RegisterRequest { email, password })
                    .send()
                    .map_err(|err| err.to_string())?;
                let tokens = decode_api_json::<AuthTokens>(response)?;
                Ok((Some(tokens), None::<String>))
            })(),
            AuthAction::ForgotRequest => {
                (|| -> Result<(Option<AuthTokens>, Option<String>), String> {
                    let response = client
                        .post(format!("{gateway_url}/auth/password-reset/request"))
                        .json(&PasswordResetRequest { email })
                        .send()
                        .map_err(|err| err.to_string())?;
                    let resp = decode_api_json::<PasswordResetResponse>(response)?;
                    Ok((None, resp.reset_token))
                })()
            }
            AuthAction::ForgotConfirm => {
                (|| -> Result<(Option<AuthTokens>, Option<String>), String> {
                    let response = client
                        .post(format!("{gateway_url}/auth/password-reset/confirm"))
                        .json(&PasswordResetConfirmRequest {
                            reset_token,
                            new_password,
                        })
                        .send()
                        .map_err(|err| err.to_string())?;
                    let _ = decode_api_json::<PasswordResetConfirmResponse>(response)?;
                    Ok((None, None::<String>))
                })()
            }
        };

        match result {
            Ok((Some(tokens), _)) => {
                match fetch_auth_me(&client, &gateway_url, &tokens.access_token) {
                    Ok(me) => {
                        match fetch_auth_characters(&client, &gateway_url, &tokens.access_token) {
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
                        }
                    }
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
        }
    }));
}

fn fetch_auth_me(
    client: &reqwest::blocking::Client,
    gateway_url: &str,
    access_token: &str,
) -> Result<MeResponse, String> {
    client
        .get(format!("{gateway_url}/auth/me"))
        .bearer_auth(access_token)
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .json::<MeResponse>()
        .map_err(|err| err.to_string())
}

fn fetch_auth_characters(
    client: &reqwest::blocking::Client,
    gateway_url: &str,
    access_token: &str,
) -> Result<CharactersResponse, String> {
    client
        .get(format!("{gateway_url}/auth/characters"))
        .bearer_auth(access_token)
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .json::<CharactersResponse>()
        .map_err(|err| err.to_string())
}

fn enter_world_request(
    client: &reqwest::blocking::Client,
    gateway_url: &str,
    access_token: &str,
    player_entity_id: &str,
) -> Result<EnterWorldResponse, String> {
    client
        .post(format!("{gateway_url}/world/enter"))
        .bearer_auth(access_token)
        .json(&EnterWorldRequest {
            player_entity_id: player_entity_id.to_string(),
        })
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .json::<EnterWorldResponse>()
        .map_err(|err| err.to_string())
}

pub fn submit_enter_world_request(
    session: &mut ClientSession,
    request_state: &mut GatewayRequestState,
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

    request_state.pending = Some(IoTaskPool::get().spawn(async move {
        let client = reqwest::blocking::Client::new();
        match enter_world_request(&client, &gateway_url, &access_token, &requested_player) {
            Ok(response) if response.accepted => {
                GatewayRequestResult::EnterWorld(EnterWorldRequestResult::Accepted {
                    player_entity_id: requested_player,
                })
            }
            Ok(_) => GatewayRequestResult::EnterWorld(EnterWorldRequestResult::Rejected {
                reason: "Enter World request rejected by gateway.".to_string(),
            }),
            Err(err) => GatewayRequestResult::EnterWorld(EnterWorldRequestResult::Error(format!(
                "Enter World failed: {err}"
            ))),
        }
    }));
}

pub fn submit_asset_bootstrap_request(
    session: &mut ClientSession,
    request_state: &mut AssetBootstrapRequestState,
    asset_root: &str,
) {
    if request_state.pending.is_some() {
        return;
    }
    let Some(access_token) = session.access_token.clone() else {
        session.status = "Missing access token for asset bootstrap.".to_string();
        session.ui_dirty = true;
        return;
    };
    let gateway_url = session.gateway_url.clone();
    let asset_root = asset_root.to_string();
    let cache_root = std::path::PathBuf::from(&asset_root).join("data/cache_stream");
    if let Err(err) = std::fs::create_dir_all(&cache_root) {
        session.status = format!(
            "Failed to prepare asset cache directory {}: {}",
            cache_root.display(),
            err
        );
        session.ui_dirty = true;
        request_state.submitted = false;
        request_state.completed = false;
        request_state.failed = true;
        return;
    }
    request_state.submitted = true;
    request_state.completed = false;
    request_state.failed = false;
    session.status = "Fetching asset bootstrap manifest...".to_string();
    session.ui_dirty = true;

    request_state.pending = Some(IoTaskPool::get().spawn(async move {
        (|| -> Result<AssetBootstrapRequestResult, String> {
            let client = reqwest::blocking::Client::new();
            let manifest = client
                .get(format!("{gateway_url}/assets/bootstrap-manifest"))
                .bearer_auth(&access_token)
                .send()
                .map_err(|err| err.to_string())
                .and_then(decode_api_json::<AssetBootstrapManifestResponse>)?;
            let cache_index_file = cache_index_path(&asset_root);
            let mut cache_index = load_cache_index(&cache_index_file).unwrap_or_default();
            let mut records = Vec::<AssetBootstrapRecord>::new();
            let mut bootstrap_total_bytes = 0u64;
            let mut bootstrap_ready_bytes = 0u64;

            for entry in &manifest.catalog {
                let target = std::path::PathBuf::from(&asset_root)
                    .join("data/cache_stream")
                    .join(&entry.relative_cache_path);
                let mut ready = false;
                if target.is_file()
                    && let Ok(bytes) = std::fs::read(&target)
                    && sha256_hex(&bytes) == entry.sha256_hex
                {
                    ready = true;
                }
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

            for required in &manifest.required_assets {
                bootstrap_total_bytes = bootstrap_total_bytes.saturating_add(required.byte_len);
                let target = std::path::PathBuf::from(&asset_root)
                    .join("data/cache_stream")
                    .join(&required.relative_cache_path);
                let mut satisfied = false;
                if target.is_file()
                    && let Ok(bytes) = std::fs::read(&target)
                    && sha256_hex(&bytes) == required.sha256_hex
                {
                    satisfied = true;
                }
                if !satisfied {
                    let url = if required.url.starts_with("http://")
                        || required.url.starts_with("https://")
                    {
                        required.url.clone()
                    } else {
                        format!("{gateway_url}{}", required.url)
                    };
                    let response_bytes = client
                        .get(url)
                        .bearer_auth(&access_token)
                        .send()
                        .map_err(|err| err.to_string())?
                        .error_for_status()
                        .map_err(|err| err.to_string())?
                        .bytes()
                        .map_err(|err| err.to_string())?;
                    let bytes = response_bytes.to_vec();
                    let payload_sha = sha256_hex(&bytes);
                    if payload_sha != required.sha256_hex {
                        return Err(format!(
                            "asset checksum mismatch asset_id={} expected={} got={}",
                            required.asset_id, required.sha256_hex, payload_sha
                        ));
                    }
                    if let Some(parent) = target.parent() {
                        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
                    }
                    std::fs::write(&target, &bytes).map_err(|err| err.to_string())?;
                }
                bootstrap_ready_bytes = bootstrap_ready_bytes.saturating_add(required.byte_len);
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
            save_cache_index(&cache_index_file, &cache_index).map_err(|err| err.to_string())?;

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
        })()
    }));
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
    asset_root: Res<'_, AssetRootPath>,
    mut asset_bootstrap_state: ResMut<'_, AssetBootstrapRequestState>,
) {
    let Some(task) = request_state.pending.as_mut() else {
        return;
    };
    let Some(payload) = bevy::tasks::block_on(future::poll_once(task)) else {
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
        }) => {
            session.player_entity_id = Some(canonicalize_player_entity_id(&player_entity_id));
            auth_sync.sent_for_client_entities.clear();
            auth_sync.last_player_entity_id = None;
            session_ready.ready_player_entity_id = None;
            asset_bootstrap_state.submitted = false;
            asset_bootstrap_state.completed = false;
            asset_bootstrap_state.failed = false;
            session.status = "World entry accepted. Waiting for replication bind...".to_string();
            submit_asset_bootstrap_request(
                session.as_mut(),
                asset_bootstrap_state.as_mut(),
                &asset_root.0,
            );
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
    mut dialog_queue: ResMut<'_, super::dialog_ui::DialogQueue>,
) {
    let Some(task) = request_state.pending.as_mut() else {
        return;
    };
    let Some(result) = bevy::tasks::block_on(future::poll_once(task)) else {
        return;
    };
    request_state.pending = None;

    match result {
        Ok(payload) => {
            request_state.completed = true;
            request_state.failed = false;
            asset_manager.bootstrap_manifest_seen = true;
            asset_manager.bootstrap_total_bytes = payload.bootstrap_total_bytes;
            asset_manager.bootstrap_ready_bytes = payload.bootstrap_ready_bytes;
            asset_manager.bootstrap_phase_complete = true;
            asset_manager.cache_index = payload.cache_index;
            asset_manager.cache_index_loaded = true;
            asset_manager.catalog_by_asset_id.clear();
            for entry in &payload.manifest.catalog {
                asset_manager.catalog_by_asset_id.insert(
                    entry.asset_id.clone(),
                    RuntimeAssetCatalogRecord {
                        _asset_guid: entry.asset_guid.clone(),
                        url: entry.url.clone(),
                        relative_cache_path: entry.relative_cache_path.clone(),
                        content_type: entry.content_type.clone(),
                        _byte_len: entry.byte_len,
                        sha256_hex: entry.sha256_hex.clone(),
                    },
                );
            }
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
        (Entity, &mut MessageSender<ClientAuthMessage>),
        (
            With<lightyear::prelude::client::Client>,
            With<lightyear::prelude::client::Connected>,
        ),
    >,
) {
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

    for (client_entity, mut sender) in &mut senders {
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
            !session_ready_for_player && now_s - last_sent_at_s >= 0.5;
        if sent_before && !should_resend_while_unbound {
            continue;
        }
        let auth_message = ClientAuthMessage {
            player_entity_id: canonical_player_entity_id.clone(),
            access_token: access_token.clone(),
        };
        sender.send::<ControlChannel>(auth_message);
        info!(
            "client auth bind message sent for player_entity_id={} client_entity={:?}",
            canonical_player_entity_id, client_entity
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
        (
            With<lightyear::prelude::client::Client>,
            With<lightyear::prelude::client::Connected>,
        ),
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
    connected_clients: Query<
        '_,
        '_,
        Entity,
        (
            With<lightyear::prelude::client::Client>,
            With<lightyear::prelude::client::Connected>,
        ),
    >,
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
        || connected_clients.is_empty()
    {
        watchdog.started_at_s = None;
        return;
    }

    let session_ready_for_player = session
        .player_entity_id
        .as_deref()
        .and_then(PlayerEntityId::parse)
        .and_then(|local| {
            session_ready
                .ready_player_entity_id
                .as_deref()
                .and_then(PlayerEntityId::parse)
                .map(|ready| ready == local)
        })
        .unwrap_or(false);
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
        (
            With<lightyear::prelude::client::Client>,
            With<lightyear::prelude::client::Connected>,
        ),
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
            let Some(message_player_id) = PlayerEntityId::parse(message.player_entity_id.as_str())
            else {
                continue;
            };
            if message_player_id != local_player_id {
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
