//! Gateway auth API, Lightyear auth/session-ready messages, headless session config.

use super::app_state::*;
use super::assets::{LocalAssetManager, LocalAssetRecord, RuntimeAssetCatalogRecord};
use super::resources::AssetRootPath;
use super::resources::{
    AssetCacheAdapter, ClientAuthSyncState, GatewayHttpAdapter, HeadlessAccountSwitchPlan,
    HeadlessTransportMode, LogoutCleanupRequested, PendingDisconnectNotify,
    SessionReadyWatchdogConfig, SessionReadyWatchdogState,
};
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task, futures_lite::future};
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
    request_state.pending = Some(IoTaskPool::get().spawn(async move {
        let result: Result<(Option<AuthTokens>, Option<String>), String> = match selected_action {
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

        match result {
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
                        Err(err) => GatewayRequestResult::Auth(AuthRequestResult::Error(format!(
                            "Auth OK but character lookup failed: {err}"
                        ))),
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
        }
    }));
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

    request_state.pending = Some(IoTaskPool::get().spawn(async move {
        match enter_world_request(gateway_http, &gateway_url, &access_token, &requested_player)
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
            Err(err) => GatewayRequestResult::EnterWorld(EnterWorldRequestResult::Error(format!(
                "Enter World failed: {err}"
            ))),
        }
    }));
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

    request_state.pending = Some(IoTaskPool::get().spawn(async move {
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
                let bytes = (gateway_http.fetch_asset_bytes)(url, access_token.clone()).await?;
                let payload_sha = sha256_hex(&bytes);
                if payload_sha != required.sha256_hex {
                    return Err(format!(
                        "asset checksum mismatch asset_id={} expected={} got={}",
                        required.asset_id, required.sha256_hex, payload_sha
                    ));
                }
                (cache_adapter.write_asset)(
                    asset_root.clone(),
                    required.relative_cache_path.clone(),
                    bytes,
                )
                .await?;
                info!(
                    "asset bootstrap cache write complete: asset_id={} relative_cache_path={} bytes={}",
                    required.asset_id, required.relative_cache_path, required.byte_len
                );
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
    gateway_http: Res<'_, GatewayHttpAdapter>,
    cache_adapter: Res<'_, AssetCacheAdapter>,
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
            session.status = "World entry accepted. Waiting for replication bind...".to_string();
            submit_asset_bootstrap_request(
                session.as_mut(),
                asset_bootstrap_state.as_mut(),
                *gateway_http,
                *cache_adapter,
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
        (
            Entity,
            &mut MessageSender<ClientAuthMessage>,
            Option<&Transport>,
            Has<lightyear::prelude::client::Connected>,
        ),
        With<lightyear::prelude::client::Client>,
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
            !session_ready_for_player && now_s - last_sent_at_s >= 0.5;
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
