use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::RngCore;
use sidereal_core::auth::{AuthClaims, AuthSessionContext};
use sidereal_core::bootstrap_wire::{AdminSpawnEntityCommand, BootstrapCommand};
use sidereal_core::gateway_dtos::{
    AdminSpawnEntityRequest, AdminSpawnEntityResponse, AuthTokens, ScriptCatalogDocumentDetailDto,
    ScriptCatalogDocumentSummaryDto,
};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::auth::bootstrap_dispatch::BootstrapDispatcher;
use crate::auth::config::AuthConfig;
use crate::auth::crypto::{
    generate_opaque_token, hash_password, hash_token, normalize_email, now_epoch_s,
    validate_password, verify_password,
};
use crate::auth::email::{EmailDelivery, EmailMessage, EmailTemplate, NoopEmailDelivery};
use crate::auth::error::AuthError;
use crate::auth::starter_world::{GraphStarterWorldPersister, StarterWorldPersister};
use crate::auth::starter_world_scripts::{
    discard_persisted_script_catalog_draft, list_persisted_script_catalog_documents,
    load_persisted_script_catalog_document, publish_persisted_script_catalog_draft,
    reload_script_catalog_from_disk, save_script_catalog_draft, scripts_root_dir,
};
use crate::auth::store::AuthStore;
use crate::auth::totp::{
    decrypt_secret, encrypt_secret, generate_totp_secret, manual_secret, provisioning_uri, qr_svg,
    verify_totp_code,
};
use crate::auth::types::{
    AccountCharacter, AuthMe, EmailLoginRequestResult, PasswordLoginResult,
    PasswordResetRequestResult, TotpEnrollmentResult,
};

pub struct AuthService {
    config: AuthConfig,
    store: Arc<dyn AuthStore>,
    bootstrap_dispatcher: Arc<dyn BootstrapDispatcher>,
    starter_world_persister: Arc<dyn StarterWorldPersister>,
    email_delivery: Arc<dyn EmailDelivery>,
}

const MIN_CHARACTER_DISPLAY_NAME_LEN: usize = 2;
const MAX_CHARACTER_DISPLAY_NAME_LEN: usize = 64;
const EMAIL_PURPOSE_LOGIN: &str = "email_login";
const EMAIL_PURPOSE_PASSWORD_RESET: &str = "password_reset";
const SCOPE_ADMIN_SPAWN: &str = "admin:spawn";
const SCOPE_SCRIPTS_READ: &str = "scripts:read";
const SCOPE_SCRIPTS_WRITE: &str = "scripts:write";
const FIRST_ADMIN_ROLES: &[&str] = &["admin"];
const FIRST_ADMIN_SCOPES: &[&str] = &[
    "dashboard:access",
    "admin:spawn",
    "scripts:read",
    "scripts:write",
    "dashboard:database:read",
    "dashboard:database:write",
    "dashboard:brp:proxy",
    "admin:accounts:read",
    "admin:accounts:write",
    "characters:read",
    "characters:write",
];

fn parse_player_entity_uuid(raw: &str) -> Option<Uuid> {
    Uuid::parse_str(raw).ok()
}

fn canonical_player_entity_id(raw: &str) -> Option<String> {
    parse_player_entity_uuid(raw).map(|uuid| uuid.to_string())
}

fn bare_player_entity_id(raw: &str) -> Option<String> {
    parse_player_entity_uuid(raw).map(|uuid| uuid.to_string())
}

fn has_admin_or_dev_role(claims: &AuthClaims) -> bool {
    claims.roles.iter().any(|role| {
        role.eq_ignore_ascii_case("admin")
            || role.eq_ignore_ascii_case("dev_tool")
            || role.eq_ignore_ascii_case("developer")
    })
}

fn has_scope(claims: &AuthClaims, required_scope: &str) -> bool {
    claims
        .scope
        .split_whitespace()
        .any(|scope| scope == required_scope)
        || claims
            .session_context
            .active_scope
            .iter()
            .any(|scope| scope == required_scope)
}

impl AuthService {
    pub fn new(
        config: AuthConfig,
        store: Arc<dyn AuthStore>,
        bootstrap_dispatcher: Arc<dyn BootstrapDispatcher>,
    ) -> Self {
        Self::new_with_persister(
            config,
            store,
            bootstrap_dispatcher,
            Arc::new(GraphStarterWorldPersister),
        )
    }

    pub fn new_with_persister(
        config: AuthConfig,
        store: Arc<dyn AuthStore>,
        bootstrap_dispatcher: Arc<dyn BootstrapDispatcher>,
        starter_world_persister: Arc<dyn StarterWorldPersister>,
    ) -> Self {
        Self::new_with_dependencies(
            config,
            store,
            bootstrap_dispatcher,
            starter_world_persister,
            Arc::new(NoopEmailDelivery),
        )
    }

    pub fn new_with_dependencies(
        config: AuthConfig,
        store: Arc<dyn AuthStore>,
        bootstrap_dispatcher: Arc<dyn BootstrapDispatcher>,
        starter_world_persister: Arc<dyn StarterWorldPersister>,
        email_delivery: Arc<dyn EmailDelivery>,
    ) -> Self {
        Self {
            config,
            store,
            bootstrap_dispatcher,
            starter_world_persister,
            email_delivery,
        }
    }

    pub async fn register(&self, email: &str, password: &str) -> Result<AuthTokens, AuthError> {
        let normalized_email = normalize_email(email)?;
        validate_password(password)?;

        let password_hash = hash_password(password)?;
        let account = self
            .store
            .create_account_atomic(&normalized_email, &password_hash)
            .await?;
        info!(
            "gateway register created account without default character account_id={}",
            account.account_id
        );

        self.issue_tokens(account.account_id).await
    }

    pub async fn bootstrap_required(&self) -> Result<bool, AuthError> {
        self.store.admin_bootstrap_required().await
    }

    pub fn bootstrap_configured(&self) -> bool {
        self.config.bootstrap_token.is_some()
    }

    pub async fn bootstrap_first_admin(
        &self,
        email: &str,
        password: &str,
        setup_token: &str,
    ) -> Result<AuthTokens, AuthError> {
        let configured_token = self.config.bootstrap_token.as_deref().ok_or_else(|| {
            AuthError::Config(
                "GATEWAY_BOOTSTRAP_TOKEN is required for first admin setup".to_string(),
            )
        })?;
        if hash_token(setup_token) != hash_token(configured_token) {
            return Err(AuthError::Unauthorized("invalid setup token".to_string()));
        }

        let normalized_email = normalize_email(email)?;
        validate_password(password)?;
        let password_hash = hash_password(password)?;
        let account = self
            .store
            .create_first_admin_account_atomic(
                &normalized_email,
                &password_hash,
                FIRST_ADMIN_ROLES
                    .iter()
                    .map(|role| (*role).to_string())
                    .collect(),
                FIRST_ADMIN_SCOPES
                    .iter()
                    .map(|scope| (*scope).to_string())
                    .collect(),
            )
            .await?;
        info!(
            "gateway bootstrap created first administrator account_id={}",
            account.account_id
        );
        self.issue_tokens_with_context(
            account.account_id,
            "bootstrap_token".to_string(),
            true,
            vec!["bootstrap_token".to_string()],
        )
        .await
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<AuthTokens, AuthError> {
        let normalized_email = normalize_email(email)?;
        let account = self
            .store
            .get_account_by_email(&normalized_email)
            .await?
            .ok_or_else(|| AuthError::Unauthorized("invalid credentials".to_string()))?;
        verify_password(password, &account.password_hash)?;
        if self
            .store
            .account_has_verified_totp(account.account_id)
            .await?
        {
            return Err(AuthError::Unauthorized(
                "MFA required; use /auth/v1/login/password".to_string(),
            ));
        }
        self.issue_tokens(account.account_id).await
    }

    pub async fn login_password_v1(
        &self,
        email: &str,
        password: &str,
    ) -> Result<PasswordLoginResult, AuthError> {
        let normalized_email = normalize_email(email)?;
        let account = self
            .store
            .get_account_by_email(&normalized_email)
            .await?
            .ok_or_else(|| AuthError::Unauthorized("invalid credentials".to_string()))?;
        verify_password(password, &account.password_hash)?;
        if self
            .store
            .account_has_verified_totp(account.account_id)
            .await?
        {
            let challenge_id = Uuid::new_v4();
            self.store
                .insert_totp_login_challenge(
                    challenge_id,
                    account.account_id,
                    now_epoch_s() + self.config.totp_login_challenge_ttl_s,
                )
                .await?;
            return Ok(PasswordLoginResult::TotpRequired {
                challenge_id,
                expires_in_s: self.config.totp_login_challenge_ttl_s,
            });
        }
        Ok(PasswordLoginResult::Authenticated {
            tokens: self
                .issue_tokens_with_context(
                    account.account_id,
                    "password".to_string(),
                    false,
                    Vec::new(),
                )
                .await?,
        })
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<AuthTokens, AuthError> {
        if refresh_token.is_empty() {
            return Err(AuthError::Validation(
                "refresh_token is required".to_string(),
            ));
        }
        let refresh_hash = hash_token(refresh_token);
        let record = self
            .store
            .consume_refresh_token(&refresh_hash)
            .await?
            .ok_or_else(|| AuthError::Unauthorized("invalid refresh token".to_string()))?;
        if now_epoch_s() > record.expires_at_epoch_s {
            return Err(AuthError::Unauthorized("refresh token expired".to_string()));
        }
        if self
            .store
            .account_has_verified_totp(record.account_id)
            .await?
        {
            return self
                .issue_tokens_with_context(
                    record.account_id,
                    "refresh_totp".to_string(),
                    true,
                    vec!["totp".to_string()],
                )
                .await;
        }
        self.issue_tokens(record.account_id).await
    }

    pub async fn me(&self, access_token: &str) -> Result<AuthMe, AuthError> {
        let claims = self.decode_access_token(access_token)?;
        let account_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AuthError::Unauthorized("invalid access token subject".to_string()))?;
        let account = self
            .store
            .get_account_by_id(account_id)
            .await?
            .ok_or_else(|| AuthError::Unauthorized("unknown account".to_string()))?;

        Ok(AuthMe {
            account_id: account.account_id,
            email: account.email,
            player_entity_id: account.player_entity_id,
        })
    }

    pub async fn list_characters(
        &self,
        access_token: &str,
    ) -> Result<Vec<AccountCharacter>, AuthError> {
        let account = self.account_from_access_token(access_token).await?;
        self.store.list_account_characters(account.account_id).await
    }

    pub async fn enroll_totp(&self, access_token: &str) -> Result<TotpEnrollmentResult, AuthError> {
        let claims = self.decode_access_token(access_token)?;
        let account_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AuthError::Unauthorized("invalid access token subject".to_string()))?;
        let account = self
            .store
            .get_account_by_id(account_id)
            .await?
            .ok_or_else(|| AuthError::Unauthorized("unknown account".to_string()))?;
        let secret = generate_totp_secret();
        let enrollment_id = Uuid::new_v4();
        let encrypted_secret = encrypt_secret(&secret, &self.config.auth_secret_key)?;
        self.store
            .insert_totp_enrollment(
                enrollment_id,
                account.account_id,
                &encrypted_secret,
                now_epoch_s() + self.config.totp_enrollment_ttl_s,
            )
            .await?;

        let provisioning_uri = provisioning_uri(
            &self.config.totp_issuer,
            &account.email,
            &secret,
            self.config.totp_digits,
            self.config.totp_step_s,
        );
        let qr_svg = qr_svg(&provisioning_uri)?;
        let account_label = format!("{}:{}", self.config.totp_issuer, account.email);

        Ok(TotpEnrollmentResult {
            enrollment_id,
            issuer: self.config.totp_issuer.clone(),
            account_label,
            provisioning_uri,
            qr_svg,
            manual_secret: manual_secret(&secret),
            expires_in_s: self.config.totp_enrollment_ttl_s,
        })
    }

    pub async fn verify_totp_enrollment(
        &self,
        access_token: &str,
        enrollment_id: &str,
        code: &str,
    ) -> Result<AuthTokens, AuthError> {
        let claims = self.decode_access_token(access_token)?;
        let account_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AuthError::Unauthorized("invalid access token subject".to_string()))?;
        let enrollment_id = Uuid::parse_str(enrollment_id)
            .map_err(|_| AuthError::Validation("enrollment_id is invalid".to_string()))?;
        let enrollment = self
            .store
            .get_totp_enrollment(enrollment_id)
            .await?
            .ok_or_else(|| AuthError::Unauthorized("invalid totp enrollment".to_string()))?;
        if enrollment.account_id != account_id {
            return Err(AuthError::Unauthorized(
                "totp enrollment does not belong to authenticated account".to_string(),
            ));
        }
        let now = now_epoch_s();
        if now > enrollment.expires_at_epoch_s {
            return Err(AuthError::Unauthorized(
                "totp enrollment expired".to_string(),
            ));
        }
        let secret = decrypt_secret(&enrollment.encrypted_secret, &self.config.auth_secret_key)?;
        let verified = verify_totp_code(
            &secret,
            code,
            now,
            self.config.totp_step_s,
            self.config.totp_digits,
            self.config.totp_allowed_drift_steps,
        )?;
        if !verified {
            return Err(AuthError::Unauthorized("invalid totp code".to_string()));
        }
        let activated = self
            .store
            .activate_totp_enrollment(enrollment.enrollment_id, account_id, now)
            .await?;
        if !activated {
            return Err(AuthError::Unauthorized(
                "invalid totp enrollment".to_string(),
            ));
        }
        self.issue_tokens_with_context(
            account_id,
            "totp_enrollment".to_string(),
            true,
            vec!["totp".to_string()],
        )
        .await
    }

    pub async fn verify_totp_login_challenge(
        &self,
        challenge_id: &str,
        code: &str,
    ) -> Result<AuthTokens, AuthError> {
        let challenge_id = Uuid::parse_str(challenge_id)
            .map_err(|_| AuthError::Validation("challenge_id is invalid".to_string()))?;
        let challenge = self
            .store
            .get_totp_login_challenge(challenge_id)
            .await?
            .ok_or_else(|| AuthError::Unauthorized("invalid totp challenge".to_string()))?;
        let now = now_epoch_s();
        if now > challenge.expires_at_epoch_s {
            return Err(AuthError::Unauthorized(
                "totp challenge expired".to_string(),
            ));
        }
        let encrypted_secret = self
            .store
            .get_verified_totp_secret(challenge.account_id)
            .await?
            .ok_or_else(|| AuthError::Unauthorized("totp is not enabled".to_string()))?;
        let secret = decrypt_secret(&encrypted_secret, &self.config.auth_secret_key)?;
        let verified = verify_totp_code(
            &secret,
            code,
            now,
            self.config.totp_step_s,
            self.config.totp_digits,
            self.config.totp_allowed_drift_steps,
        )?;
        if !verified {
            return Err(AuthError::Unauthorized("invalid totp code".to_string()));
        }
        let consumed = self
            .store
            .consume_totp_login_challenge(challenge.challenge_id, challenge.account_id, now)
            .await?;
        if !consumed {
            return Err(AuthError::Unauthorized(
                "invalid totp challenge".to_string(),
            ));
        }
        self.issue_tokens_with_context(
            challenge.account_id,
            "password_totp".to_string(),
            true,
            vec!["totp".to_string()],
        )
        .await
    }

    pub async fn create_character(
        &self,
        access_token: &str,
        display_name: &str,
    ) -> Result<AccountCharacter, AuthError> {
        let display_name = validate_character_display_name(display_name)?;
        let account = self.account_from_access_token(access_token).await?;

        let player_entity_id = Uuid::new_v4().to_string();
        if let Err(err) = self
            .starter_world_persister
            .persist_starter_world(account.account_id, &player_entity_id, &account.email)
            .await
        {
            let _ = self
                .store
                .delete_account_character(account.account_id, &player_entity_id)
                .await;
            return Err(err);
        }

        let character = self
            .store
            .insert_account_character(
                account.account_id,
                &player_entity_id,
                &display_name,
                now_epoch_s(),
            )
            .await?;
        info!(
            "gateway character created account_id={} player_entity_id={} display_name={}",
            account.account_id, character.player_entity_id, character.display_name
        );
        Ok(character)
    }

    pub async fn delete_character(
        &self,
        access_token: &str,
        player_entity_id: &str,
    ) -> Result<(), AuthError> {
        let account = self.account_from_access_token(access_token).await?;
        let player_entity_id = canonical_player_entity_id(player_entity_id)
            .ok_or_else(|| AuthError::Validation("player_entity_id is invalid".to_string()))?;
        let owns = self
            .store
            .account_owns_player_entity(account.account_id, &player_entity_id)
            .await?;
        if !owns {
            return Err(AuthError::Unauthorized(
                "player_entity_id is not owned by authenticated account".to_string(),
            ));
        }
        self.starter_world_persister
            .remove_character_world(account.account_id, &player_entity_id)
            .await?;
        self.store
            .delete_account_character(account.account_id, &player_entity_id)
            .await?;
        info!(
            "gateway character deleted account_id={} player_entity_id={}",
            account.account_id, player_entity_id
        );
        Ok(())
    }

    pub async fn reset_character(
        &self,
        access_token: &str,
        player_entity_id: &str,
    ) -> Result<AccountCharacter, AuthError> {
        let account = self.account_from_access_token(access_token).await?;
        let player_entity_id = canonical_player_entity_id(player_entity_id)
            .ok_or_else(|| AuthError::Validation("player_entity_id is invalid".to_string()))?;
        let characters = self
            .store
            .list_account_characters(account.account_id)
            .await?;
        let character = characters
            .into_iter()
            .find(|character| character.player_entity_id == player_entity_id)
            .ok_or_else(|| {
                AuthError::Unauthorized(
                    "player_entity_id is not owned by authenticated account".to_string(),
                )
            })?;
        self.starter_world_persister
            .reset_character_world(account.account_id, &player_entity_id, &account.email)
            .await?;
        info!(
            "gateway character reset account_id={} player_entity_id={}",
            account.account_id, player_entity_id
        );
        Ok(character)
    }

    pub async fn enter_world(
        &self,
        access_token: &str,
        player_entity_id: &str,
    ) -> Result<AuthTokens, AuthError> {
        let Some(normalized_player_entity_id) = bare_player_entity_id(player_entity_id) else {
            return Err(AuthError::Validation(
                "player_entity_id is required".to_string(),
            ));
        };
        let claims = self.decode_access_token(access_token)?;
        let account_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AuthError::Unauthorized("invalid access token subject".to_string()))?;
        let owns = self
            .store
            .account_owns_player_entity(account_id, &normalized_player_entity_id)
            .await?;
        if !owns {
            return Err(AuthError::Unauthorized(
                "player_entity_id is not owned by authenticated account".to_string(),
            ));
        }
        self.bootstrap_dispatcher
            .dispatch(&BootstrapCommand {
                account_id,
                player_entity_id: normalized_player_entity_id.clone(),
            })
            .await?;
        self.issue_tokens_for_player_with_context(
            account_id,
            Some(normalized_player_entity_id),
            "world_entry".to_string(),
            claims.session_context.mfa_verified,
            claims.session_context.mfa_methods,
        )
        .await
    }

    pub async fn admin_spawn_entity(
        &self,
        access_token: &str,
        req: &AdminSpawnEntityRequest,
    ) -> Result<AdminSpawnEntityResponse, AuthError> {
        let claims = self.require_admin_claims(access_token, "admin spawn", SCOPE_ADMIN_SPAWN)?;
        let Some(actor_player_entity_id) = bare_player_entity_id(&claims.player_entity_id) else {
            return Err(AuthError::Unauthorized(
                "invalid actor player_entity_id in access token".to_string(),
            ));
        };
        let Some(owner_player_entity_id) = bare_player_entity_id(&req.player_entity_id) else {
            return Err(AuthError::Validation(
                "player_entity_id is required".to_string(),
            ));
        };
        if req.bundle_id.trim().is_empty() {
            return Err(AuthError::Validation("bundle_id is required".to_string()));
        }
        let actor_account_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AuthError::Unauthorized("invalid access token subject".to_string()))?;

        let spawned_entity_id = Uuid::new_v4().to_string();
        let request_id = Uuid::new_v4();
        let command = AdminSpawnEntityCommand {
            actor_account_id,
            actor_player_entity_id,
            request_id,
            player_entity_id: owner_player_entity_id.clone(),
            bundle_id: req.bundle_id.trim().to_string(),
            requested_entity_id: spawned_entity_id.clone(),
            overrides: req.overrides.clone(),
        };
        self.bootstrap_dispatcher
            .dispatch_admin_spawn(&command)
            .await?;
        info!(
            "gateway admin spawn enqueued request_id={} actor_account_id={} actor_player_entity_id={} target_player_entity_id={} bundle_id={} requested_entity_id={}",
            request_id,
            command.actor_account_id,
            command.actor_player_entity_id,
            owner_player_entity_id,
            command.bundle_id,
            spawned_entity_id
        );
        Ok(AdminSpawnEntityResponse {
            ok: true,
            spawned_entity_id,
            bundle_id: command.bundle_id,
            owner_player_entity_id,
        })
    }

    pub async fn list_scripts(
        &self,
        access_token: &str,
    ) -> Result<Vec<ScriptCatalogDocumentSummaryDto>, AuthError> {
        let _ = self.require_admin_claims(access_token, "list scripts", SCOPE_SCRIPTS_READ)?;
        let summaries = tokio::task::spawn_blocking(list_persisted_script_catalog_documents)
            .await
            .map_err(|err| AuthError::Internal(format!("list scripts task failed: {err}")))??;
        Ok(summaries
            .into_iter()
            .map(|entry| ScriptCatalogDocumentSummaryDto {
                script_path: entry.script_path,
                family: entry.family,
                active_revision: entry.active_revision,
                has_draft: entry.has_draft,
            })
            .collect())
    }

    pub async fn get_script(
        &self,
        access_token: &str,
        script_path: &str,
    ) -> Result<Option<ScriptCatalogDocumentDetailDto>, AuthError> {
        let _ = self.require_admin_claims(access_token, "get script", SCOPE_SCRIPTS_READ)?;
        let script_path = script_path.to_string();
        let detail = tokio::task::spawn_blocking(move || {
            load_persisted_script_catalog_document(&script_path)
        })
        .await
        .map_err(|err| AuthError::Internal(format!("get script task failed: {err}")))??;
        Ok(detail.map(|entry| ScriptCatalogDocumentDetailDto {
            script_path: entry.script_path,
            family: entry.family,
            active_revision: entry.active_revision,
            active_source: entry.active_source,
            active_origin: entry.active_origin,
            draft_source: entry.draft_source,
            draft_origin: entry.draft_origin,
            draft_updated_at_epoch_s: entry.draft_updated_at_epoch_s,
        }))
    }

    pub async fn save_script_draft(
        &self,
        access_token: &str,
        script_path: &str,
        source: &str,
        origin: Option<&str>,
        family: Option<&str>,
    ) -> Result<(), AuthError> {
        let _ =
            self.require_admin_claims(access_token, "save script draft", SCOPE_SCRIPTS_WRITE)?;
        if script_path.trim().is_empty() {
            return Err(AuthError::Validation("script_path is required".to_string()));
        }
        if source.trim().is_empty() {
            return Err(AuthError::Validation("source is required".to_string()));
        }
        let script_path = script_path.trim().to_string();
        let source = source.to_string();
        let origin = origin.map(str::to_string);
        let family = family.map(str::to_string);
        tokio::task::spawn_blocking(move || {
            save_script_catalog_draft(&script_path, &source, origin.as_deref(), family.as_deref())
        })
        .await
        .map_err(|err| AuthError::Internal(format!("save script draft task failed: {err}")))?
    }

    pub async fn publish_script_draft(
        &self,
        access_token: &str,
        script_path: &str,
    ) -> Result<Option<u64>, AuthError> {
        let _ =
            self.require_admin_claims(access_token, "publish script draft", SCOPE_SCRIPTS_WRITE)?;
        if script_path.trim().is_empty() {
            return Err(AuthError::Validation("script_path is required".to_string()));
        }
        let script_path = script_path.trim().to_string();
        tokio::task::spawn_blocking(move || publish_persisted_script_catalog_draft(&script_path))
            .await
            .map_err(|err| AuthError::Internal(format!("publish script task failed: {err}")))?
    }

    pub async fn discard_script_draft(
        &self,
        access_token: &str,
        script_path: &str,
    ) -> Result<bool, AuthError> {
        let _ =
            self.require_admin_claims(access_token, "discard script draft", SCOPE_SCRIPTS_WRITE)?;
        if script_path.trim().is_empty() {
            return Err(AuthError::Validation("script_path is required".to_string()));
        }
        let script_path = script_path.trim().to_string();
        tokio::task::spawn_blocking(move || discard_persisted_script_catalog_draft(&script_path))
            .await
            .map_err(|err| AuthError::Internal(format!("discard script task failed: {err}")))?
    }

    pub async fn reload_scripts_from_disk(&self, access_token: &str) -> Result<usize, AuthError> {
        let _ = self.require_admin_claims(
            access_token,
            "reload scripts from disk",
            SCOPE_SCRIPTS_WRITE,
        )?;
        let root = scripts_root_dir();
        let catalog = tokio::task::spawn_blocking(move || reload_script_catalog_from_disk(&root))
            .await
            .map_err(|err| AuthError::Internal(format!("reload scripts task failed: {err}")))??;
        Ok(catalog.entries.len())
    }

    pub async fn password_reset_request(
        &self,
        email: &str,
    ) -> Result<PasswordResetRequestResult, AuthError> {
        let normalized_email = normalize_email(email)?;
        let Some(account) = self.store.get_account_by_email(&normalized_email).await? else {
            return Ok(PasswordResetRequestResult {
                accepted: true,
                reset_token: None,
            });
        };

        let reset_token = generate_opaque_token();
        let reset_hash = hash_token(&reset_token);
        self.store
            .insert_password_reset_token(
                &reset_hash,
                account.account_id,
                now_epoch_s() + self.config.reset_token_ttl_s,
            )
            .await?;

        Ok(PasswordResetRequestResult {
            accepted: true,
            reset_token: Some(reset_token),
        })
    }

    pub async fn password_reset_request_public(
        &self,
        email: &str,
    ) -> Result<PasswordResetRequestResult, AuthError> {
        let normalized_email = normalize_email(email)?;
        let Some(account) = self.store.get_account_by_email(&normalized_email).await? else {
            return Ok(PasswordResetRequestResult {
                accepted: true,
                reset_token: None,
            });
        };
        if !self
            .email_delivery_allowed(&normalized_email, EMAIL_PURPOSE_PASSWORD_RESET)
            .await?
        {
            return Ok(PasswordResetRequestResult {
                accepted: true,
                reset_token: None,
            });
        }

        let reset_token = generate_opaque_token();
        let reset_hash = hash_token(&reset_token);
        self.store
            .insert_password_reset_token(
                &reset_hash,
                account.account_id,
                now_epoch_s() + self.config.reset_token_ttl_s,
            )
            .await?;

        if let Err(err) = self
            .email_delivery
            .send(self.password_reset_email(&normalized_email, &reset_token))
            .await
        {
            warn!(
                "gateway password reset delivery failed target_hash={} err={}",
                hash_token(&normalized_email),
                err
            );
            self.record_email_delivery(&normalized_email, EMAIL_PURPOSE_PASSWORD_RESET)
                .await?;
            return Ok(PasswordResetRequestResult {
                accepted: true,
                reset_token: None,
            });
        }
        self.record_email_delivery(&normalized_email, EMAIL_PURPOSE_PASSWORD_RESET)
            .await?;

        Ok(PasswordResetRequestResult {
            accepted: true,
            reset_token: None,
        })
    }

    pub async fn password_reset_confirm(
        &self,
        reset_token: &str,
        new_password: &str,
    ) -> Result<(), AuthError> {
        validate_password(new_password)?;
        if reset_token.is_empty() {
            return Err(AuthError::Validation("reset_token is required".to_string()));
        }

        let reset_hash = hash_token(reset_token);
        let record = self
            .store
            .consume_password_reset_token(&reset_hash)
            .await?
            .ok_or_else(|| AuthError::Unauthorized("invalid reset token".to_string()))?;
        if now_epoch_s() > record.expires_at_epoch_s {
            return Err(AuthError::Unauthorized("reset token expired".to_string()));
        }

        let new_hash = hash_password(new_password)?;
        self.store
            .update_password_hash(record.account_id, &new_hash)
            .await?;
        Ok(())
    }

    pub async fn request_email_login(
        &self,
        email: &str,
    ) -> Result<EmailLoginRequestResult, AuthError> {
        let normalized_email = normalize_email(email)?;
        let Some(account) = self.store.get_account_by_email(&normalized_email).await? else {
            return Ok(EmailLoginRequestResult { accepted: true });
        };
        if !self
            .email_delivery_allowed(&normalized_email, EMAIL_PURPOSE_LOGIN)
            .await?
        {
            return Ok(EmailLoginRequestResult { accepted: true });
        }

        let challenge_id = Uuid::new_v4();
        let code = generate_email_login_code();
        let token = generate_opaque_token();
        self.store
            .insert_email_login_challenge(
                challenge_id,
                account.account_id,
                &hash_token(&code),
                &hash_token(&token),
                now_epoch_s() + self.config.email_challenge_ttl_s,
            )
            .await?;
        if let Err(err) = self
            .email_delivery
            .send(self.email_login_message(&normalized_email, challenge_id, &code, &token))
            .await
        {
            warn!(
                "gateway email login delivery failed target_hash={} err={}",
                hash_token(&normalized_email),
                err
            );
            self.record_email_delivery(&normalized_email, EMAIL_PURPOSE_LOGIN)
                .await?;
            return Ok(EmailLoginRequestResult { accepted: true });
        }
        self.record_email_delivery(&normalized_email, EMAIL_PURPOSE_LOGIN)
            .await?;

        Ok(EmailLoginRequestResult { accepted: true })
    }

    pub async fn verify_email_login(
        &self,
        challenge_id: &str,
        code: Option<&str>,
        token: Option<&str>,
    ) -> Result<AuthTokens, AuthError> {
        let challenge_id = Uuid::parse_str(challenge_id)
            .map_err(|_| AuthError::Validation("challenge_id is invalid".to_string()))?;
        let has_code = code.is_some_and(|value| !value.trim().is_empty());
        let has_token = token.is_some_and(|value| !value.trim().is_empty());
        if has_code == has_token {
            return Err(AuthError::Validation(
                "exactly one of code or token is required".to_string(),
            ));
        }

        let record = if let Some(code) = code.filter(|value| !value.trim().is_empty()) {
            self.store
                .consume_email_login_challenge_by_code(challenge_id, &hash_token(code.trim()))
                .await?
        } else if let Some(token) = token.filter(|value| !value.trim().is_empty()) {
            self.store
                .consume_email_login_challenge_by_token(challenge_id, &hash_token(token.trim()))
                .await?
        } else {
            None
        }
        .ok_or_else(|| AuthError::Unauthorized("invalid email login challenge".to_string()))?;

        if now_epoch_s() > record.expires_at_epoch_s {
            return Err(AuthError::Unauthorized(
                "email login challenge expired".to_string(),
            ));
        }
        self.issue_tokens(record.account_id).await
    }

    pub fn decode_access_token(&self, access_token: &str) -> Result<AuthClaims, AuthError> {
        let token = decode::<AuthClaims>(
            access_token,
            &DecodingKey::from_secret(self.config.jwt_secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        )
        .map_err(|_| AuthError::Unauthorized("invalid access token".to_string()))?;
        let mut claims = token.claims;
        if let Some(canonical_player_entity_id) =
            canonical_player_entity_id(&claims.player_entity_id)
        {
            claims.player_entity_id = canonical_player_entity_id;
        }
        Ok(claims)
    }

    fn require_admin_claims(
        &self,
        access_token: &str,
        action: &str,
        required_scope: &str,
    ) -> Result<AuthClaims, AuthError> {
        let claims = self.decode_access_token(access_token)?;
        if !has_admin_or_dev_role(&claims) {
            return Err(AuthError::Unauthorized(format!(
                "{action} requires admin or dev_tool role"
            )));
        }
        if !claims.session_context.mfa_verified {
            return Err(AuthError::Unauthorized(format!(
                "{action} requires verified MFA"
            )));
        }
        if !has_scope(&claims, required_scope) {
            return Err(AuthError::Unauthorized(format!(
                "{action} requires {required_scope} scope"
            )));
        }
        Ok(claims)
    }

    async fn issue_tokens(&self, account_id: Uuid) -> Result<AuthTokens, AuthError> {
        self.issue_tokens_with_context(account_id, String::new(), false, Vec::new())
            .await
    }

    async fn issue_tokens_with_context(
        &self,
        account_id: Uuid,
        auth_method: String,
        mfa_verified: bool,
        mfa_methods: Vec<String>,
    ) -> Result<AuthTokens, AuthError> {
        self.issue_tokens_for_player_with_context(
            account_id,
            None,
            auth_method,
            mfa_verified,
            mfa_methods,
        )
        .await
    }

    async fn issue_tokens_for_player_with_context(
        &self,
        account_id: Uuid,
        selected_player_entity_id: Option<String>,
        auth_method: String,
        mfa_verified: bool,
        mfa_methods: Vec<String>,
    ) -> Result<AuthTokens, AuthError> {
        let account = self
            .store
            .get_account_by_id(account_id)
            .await?
            .ok_or_else(|| AuthError::Internal("account missing".to_string()))?;
        let roles = self.store.list_account_roles(account_id).await?;
        let active_scope = self.store.list_account_scopes(account_id).await?;
        let token_player_entity_id = selected_player_entity_id
            .as_deref()
            .and_then(canonical_player_entity_id)
            .or_else(|| canonical_player_entity_id(&account.player_entity_id))
            .unwrap_or(account.player_entity_id);
        let active_character_id = selected_player_entity_id
            .as_deref()
            .and_then(canonical_player_entity_id);
        let iat = now_epoch_s();
        let exp = iat + self.config.access_token_ttl_s;
        let claims = AuthClaims {
            sub: account.account_id.to_string(),
            player_entity_id: token_player_entity_id,
            roles,
            scope: active_scope.join(" "),
            session_context: AuthSessionContext {
                auth_method,
                mfa_verified,
                mfa_methods,
                active_scope,
                active_character_id,
            },
            iat,
            exp,
            jti: Uuid::new_v4().to_string(),
        };

        let access_token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )
        .map_err(|_| AuthError::Internal("failed to encode access token".to_string()))?;

        let refresh_token = generate_opaque_token();
        let refresh_hash = hash_token(&refresh_token);
        self.store
            .insert_refresh_token(
                &refresh_hash,
                account_id,
                iat + self.config.refresh_token_ttl_s,
            )
            .await?;

        Ok(AuthTokens {
            access_token,
            refresh_token,
            token_type: "bearer".to_string(),
            expires_in_s: self.config.access_token_ttl_s,
        })
    }

    fn password_reset_email(&self, to: &str, reset_token: &str) -> EmailMessage {
        let reset_url = format!(
            "{}/reset-password?token={}",
            self.config.public_base_url, reset_token
        );
        EmailMessage {
            to: to.to_string(),
            subject: "Reset your Sidereal password".to_string(),
            body_text: format!(
                "Use this password reset link to choose a new password.\nReset link: {reset_url}\nReset token: {reset_token}\nThis link expires in {} seconds.",
                self.config.reset_token_ttl_s
            ),
            template: EmailTemplate::PasswordReset,
        }
    }

    fn email_login_message(
        &self,
        to: &str,
        challenge_id: Uuid,
        code: &str,
        token: &str,
    ) -> EmailMessage {
        let login_url = format!(
            "{}/login/email/complete?challenge_id={}&token={}",
            self.config.public_base_url, challenge_id, token
        );
        EmailMessage {
            to: to.to_string(),
            subject: "Your Sidereal login code".to_string(),
            body_text: format!(
                "Use this code or magic link to sign in.\nChallenge ID: {challenge_id}\nCode: {code}\nMagic link: {login_url}\nMagic token: {token}\nThis login challenge expires in {} seconds.",
                self.config.email_challenge_ttl_s
            ),
            template: EmailTemplate::EmailLogin,
        }
    }

    async fn email_delivery_allowed(
        &self,
        normalized_email: &str,
        purpose: &str,
    ) -> Result<bool, AuthError> {
        let target_hash = hash_token(normalized_email);
        let now = now_epoch_s();
        if self.config.email_resend_cooldown_s > 0 {
            let cooldown_count = self
                .store
                .count_email_delivery_events(
                    &target_hash,
                    purpose,
                    now.saturating_sub(self.config.email_resend_cooldown_s),
                )
                .await?;
            if cooldown_count > 0 {
                return Ok(false);
            }
        }
        let hourly_count = self
            .store
            .count_email_delivery_events(&target_hash, purpose, now.saturating_sub(3_600))
            .await?;
        Ok(hourly_count < self.config.email_max_per_email_per_hour)
    }

    async fn record_email_delivery(
        &self,
        normalized_email: &str,
        purpose: &str,
    ) -> Result<(), AuthError> {
        self.store
            .insert_email_delivery_event(&hash_token(normalized_email), purpose, now_epoch_s())
            .await
    }

    async fn account_from_access_token(
        &self,
        access_token: &str,
    ) -> Result<crate::auth::types::Account, AuthError> {
        let claims = self.decode_access_token(access_token)?;
        let account_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AuthError::Unauthorized("invalid access token subject".to_string()))?;
        self.store
            .get_account_by_id(account_id)
            .await?
            .ok_or_else(|| AuthError::Unauthorized("unknown account".to_string()))
    }
}

fn generate_email_login_code() -> String {
    let mut rng = rand::rng();
    format!("{:06}", rng.next_u32() % 1_000_000)
}

fn validate_character_display_name(raw: &str) -> Result<String, AuthError> {
    let display_name = raw.trim();
    if display_name.len() < MIN_CHARACTER_DISPLAY_NAME_LEN
        || display_name.len() > MAX_CHARACTER_DISPLAY_NAME_LEN
    {
        return Err(AuthError::Validation(format!(
            "display_name must be between {MIN_CHARACTER_DISPLAY_NAME_LEN} and {MAX_CHARACTER_DISPLAY_NAME_LEN} chars"
        )));
    }
    if !display_name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == ' ' || ch == '-' || ch == '_')
    {
        return Err(AuthError::Validation(
            "display_name contains unsupported characters".to_string(),
        ));
    }
    Ok(display_name.to_string())
}
