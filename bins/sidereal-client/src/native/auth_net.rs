//! Gateway auth API, Lightyear auth/session-ready messages, headless session config.

use bevy::log::{info, warn};
use bevy::prelude::*;
use lightyear::prelude::{MessageReceiver, MessageSender};
use sidereal_core::gateway_dtos::{
    AuthTokens, CharactersResponse, EnterWorldRequest, EnterWorldResponse, LoginRequest,
    MeResponse, PasswordResetConfirmRequest, PasswordResetConfirmResponse, PasswordResetRequest,
    PasswordResetResponse, RegisterRequest,
};
use sidereal_net::{
    ClientAuthMessage, ControlChannel, PlayerEntityId, ServerSessionDeniedMessage,
    ServerSessionReadyMessage,
};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver};

use super::app_state::*;
use super::resources::{ClientAuthSyncState, HeadlessAccountSwitchPlan, HeadlessTransportMode};

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
    pending: Mutex<Option<Receiver<GatewayRequestResult>>>,
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

pub fn init_gateway_request_state(app: &mut App) {
    app.insert_resource(GatewayRequestState::default());
}

pub fn submit_auth_request(session: &mut ClientSession, request_state: &mut GatewayRequestState) {
    if request_state
        .pending
        .lock()
        .ok()
        .is_some_and(|pending| pending.is_some())
    {
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
    let (tx, rx) = mpsc::channel();
    if let Ok(mut pending) = request_state.pending.lock() {
        *pending = Some(rx);
    }
    session.status = "Submitting request...".to_string();
    session.ui_dirty = true;

    std::thread::spawn(move || {
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

        let payload = match result {
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
        };
        let _ = tx.send(payload);
    });
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
    if request_state
        .pending
        .lock()
        .ok()
        .is_some_and(|pending| pending.is_some())
    {
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
    let (tx, rx) = mpsc::channel();
    if let Ok(mut pending) = request_state.pending.lock() {
        *pending = Some(rx);
    }
    session.status = "Submitting Enter World request...".to_string();
    session.ui_dirty = true;

    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let payload =
            match enter_world_request(&client, &gateway_url, &access_token, &requested_player) {
                Ok(response) if response.accepted => {
                    GatewayRequestResult::EnterWorld(EnterWorldRequestResult::Accepted {
                        player_entity_id: requested_player,
                    })
                }
                Ok(_) => GatewayRequestResult::EnterWorld(EnterWorldRequestResult::Rejected {
                    reason: "Enter World request rejected by gateway.".to_string(),
                }),
                Err(err) => GatewayRequestResult::EnterWorld(EnterWorldRequestResult::Error(
                    format!("Enter World failed: {err}"),
                )),
            };
        let _ = tx.send(payload);
    });
}

#[allow(clippy::too_many_arguments)]
pub fn poll_gateway_request_results(
    request_state: ResMut<'_, GatewayRequestState>,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
    mut session: ResMut<'_, ClientSession>,
    mut character_selection: ResMut<'_, CharacterSelectionState>,
    mut session_ready: ResMut<'_, SessionReadyState>,
    mut auth_sync: ResMut<'_, super::resources::ClientAuthSyncState>,
    mut dialog_queue: ResMut<'_, super::dialog_ui::DialogQueue>,
) {
    let Ok(mut pending) = request_state.pending.lock() else {
        return;
    };
    let Some(receiver) = pending.as_ref() else {
        return;
    };
    let Ok(payload) = receiver.try_recv() else {
        return;
    };
    *pending = None;

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
            session_ready.ready_player_entity_id = Some(message.player_entity_id);
        }
    }
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
