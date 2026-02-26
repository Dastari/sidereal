//! Gateway auth API, Lightyear auth/session-ready messages, headless session config.

use bevy::log::info;
use bevy::prelude::*;
use lightyear::prelude::{MessageReceiver, MessageSender};
use sidereal_net::{
    ClientAuthMessage, ControlChannel, ServerSessionReadyMessage,
};

use super::resources::{
    BootstrapWatchdogState, ClientAuthSyncState, HeadlessAccountSwitchPlan, HeadlessTransportMode,
};
use super::state::*;

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

pub fn submit_auth_request(
    session: &mut ClientSession,
    character_selection: &mut CharacterSelectionState,
    session_ready: &mut SessionReadyState,
    next_state: &mut NextState<ClientAppState>,
    dialog_queue: &mut super::dialog_ui::DialogQueue,
    _asset_root: &super::resources::AssetRootPath,
) {
    let client = reqwest::blocking::Client::new();
    let gateway_url = session.gateway_url.clone();
    let result = match session.selected_action {
        AuthAction::Login => (|| -> Result<(Option<AuthTokens>, Option<String>), String> {
            let response = client
                .post(format!("{gateway_url}/auth/login"))
                .json(&LoginRequest {
                    email: session.email.clone(),
                    password: session.password.clone(),
                })
                .send()
                .map_err(|err| err.to_string())?;
            let tokens = decode_api_json::<AuthTokens>(response)?;
            session.status = "Login succeeded. Fetching world snapshot...".to_string();
            Ok((Some(tokens), None::<String>))
        })(),
        AuthAction::Register => (|| -> Result<(Option<AuthTokens>, Option<String>), String> {
            let response = client
                .post(format!("{gateway_url}/auth/register"))
                .json(&RegisterRequest {
                    email: session.email.clone(),
                    password: session.password.clone(),
                })
                .send()
                .map_err(|err| err.to_string())?;
            let tokens = decode_api_json::<AuthTokens>(response)?;
            session.status = "Registration succeeded. Fetching world snapshot...".to_string();
            Ok((Some(tokens), None::<String>))
        })(),
        AuthAction::ForgotRequest => {
            (|| -> Result<(Option<AuthTokens>, Option<String>), String> {
                let response = client
                    .post(format!("{gateway_url}/auth/password-reset/request"))
                    .json(&ForgotRequest {
                        email: session.email.clone(),
                    })
                    .send()
                    .map_err(|err| err.to_string())?;
                let resp = decode_api_json::<ForgotResponse>(response)?;
                if let Some(token) = resp.reset_token {
                    session.reset_token = token;
                }
                session.status =
                    "Password reset token requested. Use F4 to confirm reset.".to_string();
                Ok((None, None::<String>))
            })()
        }
        AuthAction::ForgotConfirm => {
            (|| -> Result<(Option<AuthTokens>, Option<String>), String> {
                let response = client
                    .post(format!("{gateway_url}/auth/password-reset/confirm"))
                    .json(&ForgotConfirmRequest {
                        reset_token: session.reset_token.clone(),
                        new_password: session.new_password.clone(),
                    })
                    .send()
                    .map_err(|err| err.to_string())?;
                let _ = decode_api_json::<ForgotConfirmResponse>(response)?;
                session.status = "Password reset confirmed. Switch to Login (F1).".to_string();
                Ok((None, None::<String>))
            })()
        }
    };

    match result {
        Ok((Some(tokens), _)) => {
            session.access_token = Some(tokens.access_token.clone());
            session.refresh_token = Some(tokens.refresh_token);
            match fetch_auth_me(&client, &gateway_url, &tokens.access_token) {
                Ok(me) => {
                    session.account_id = Some(me.account_id.clone());
                    match fetch_auth_characters(&client, &gateway_url, &tokens.access_token) {
                        Ok(characters) => {
                            character_selection.characters = characters
                                .characters
                                .into_iter()
                                .map(|c| c.player_entity_id)
                                .collect();
                            if character_selection.characters.is_empty() {
                                session.status =
                                    "Authenticated but no characters are available.".to_string();
                                dialog_queue.push_error(
                                    "No Characters",
                                    "This account has no characters. Character creation UI is not implemented yet."
                                        .to_string(),
                                );
                                return;
                            }
                            character_selection.selected_player_entity_id =
                                character_selection.characters.first().cloned();
                            session.player_entity_id = None;
                            session_ready.ready_player_entity_id = None;
                            session.status =
                                "Authenticated. Select a character and press Enter World."
                                    .to_string();
                            next_state.set(ClientAppState::CharacterSelect);
                        }
                        Err(err) => {
                            session.status = format!("Auth OK but character lookup failed: {err}");
                            dialog_queue.push_error(
                                "Character Lookup Failed",
                                format!(
                                    "Authentication succeeded, but failed to fetch /auth/characters.\n\nDetails: {err}"
                                ),
                            );
                        }
                    }
                }
                Err(err) => {
                    session.status = format!("Auth OK but profile lookup failed: {err}");
                    dialog_queue.push_error(
                        "Profile Lookup Failed",
                        format!(
                            "Authentication succeeded, but failed to fetch /auth/me.\n\n\
                             Details: {err}\n\n\
                             This usually means:\n\
                             • Backend server needs to be restarted/recompiled\n\
                             • Protocol version mismatch between client and server\n\
                             • Network connectivity issue"
                        ),
                    );
                }
            }
        }
        Ok((None, _)) => {}
        Err(err) => {
            session.status = format!("Request failed: {err}");
            dialog_queue.push_error(
                "Authentication Failed",
                format!("Failed to connect or authenticate.\n\nDetails: {err}"),
            );
        }
    }
    session.ui_dirty = true;
}

fn fetch_auth_me(
    client: &reqwest::blocking::Client,
    gateway_url: &str,
    access_token: &str,
) -> Result<AuthMeResponse, String> {
    client
        .get(format!("{gateway_url}/auth/me"))
        .bearer_auth(access_token)
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .json::<AuthMeResponse>()
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

pub fn enter_world_request(
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

pub fn configure_headless_session_from_env(
    mut commands: Commands<'_, '_>,
    mut session: ResMut<'_, ClientSession>,
) {
    if let Ok(player_entity_id) = std::env::var("SIDEREAL_CLIENT_HEADLESS_PLAYER_ENTITY_ID") {
        session.player_entity_id = Some(player_entity_id);
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
    session.player_entity_id = Some(plan.next_player_entity_id.clone());
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
    watchdog: Res<'_, BootstrapWatchdogState>,
    session: Res<'_, ClientSession>,
    mut auth_state: ResMut<'_, ClientAuthSyncState>,
    mut senders: Query<
        '_,
        '_,
        (Entity, &mut MessageSender<ClientAuthMessage>),
        (With<lightyear::prelude::client::Client>, With<lightyear::prelude::client::Connected>),
    >,
) {
    let active_world_state = app_state.as_ref().is_some_and(|state| {
        matches!(
            state.get(),
            ClientAppState::InWorld | ClientAppState::WorldLoading
        )
    }) || headless_mode.0;
    if !active_world_state {
        return;
    }
    let Some(access_token) = session.access_token.as_ref() else {
        return;
    };
    let Some(player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    if auth_state.last_player_entity_id.as_deref() != Some(player_entity_id.as_str()) {
        auth_state.sent_for_client_entities.clear();
        auth_state.last_sent_at_s_by_client_entity.clear();
        auth_state.last_player_entity_id = Some(player_entity_id.clone());
    }
    let now_s = time.elapsed_secs_f64();

    for (client_entity, mut sender) in &mut senders {
        let sent_before = auth_state.sent_for_client_entities.contains(&client_entity);
        let last_sent_at_s = auth_state
            .last_sent_at_s_by_client_entity
            .get(&client_entity)
            .copied()
            .unwrap_or(0.0);
        let should_resend_while_unbound =
            !watchdog.replication_state_seen && now_s - last_sent_at_s >= 0.5;
        if sent_before && !should_resend_while_unbound {
            continue;
        }
        let auth_message = ClientAuthMessage {
            player_entity_id: player_entity_id.clone(),
            access_token: access_token.clone(),
        };
        sender.send::<ControlChannel>(auth_message);
        info!(
            "client auth bind message sent for player_entity_id={} client_entity={:?}",
            player_entity_id, client_entity
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
        (With<lightyear::prelude::client::Client>, With<lightyear::prelude::client::Connected>),
    >,
    session: Res<'_, ClientSession>,
    mut session_ready: ResMut<'_, SessionReadyState>,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    for mut receiver in &mut receivers {
        for message in receiver.receive() {
            if message.player_entity_id != *local_player_entity_id {
                continue;
            }
            session_ready.ready_player_entity_id = Some(message.player_entity_id);
        }
    }
}
