use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sidereal_core::auth::AuthClaims;
use sidereal_core::bootstrap_wire::BootstrapCommand;
use sidereal_core::gateway_dtos::AuthTokens;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use crate::auth::bootstrap_dispatch::BootstrapDispatcher;
use crate::auth::config::AuthConfig;
use crate::auth::crypto::{
    generate_opaque_token, hash_password, hash_token, normalize_email, now_epoch_s,
    validate_password, verify_password,
};
use crate::auth::error::AuthError;
use crate::auth::starter_world::{GraphStarterWorldPersister, StarterWorldPersister};
use crate::auth::store::AuthStore;
use crate::auth::types::{AccountCharacter, AuthMe, PasswordResetRequestResult};

pub struct AuthService {
    config: AuthConfig,
    store: Arc<dyn AuthStore>,
    bootstrap_dispatcher: Arc<dyn BootstrapDispatcher>,
    starter_world_persister: Arc<dyn StarterWorldPersister>,
}

fn parse_player_entity_uuid(raw: &str) -> Option<Uuid> {
    Uuid::parse_str(raw).ok().or_else(|| {
        raw.split(':')
            .nth(1)
            .and_then(|candidate| Uuid::parse_str(candidate).ok())
    })
}

fn canonical_player_entity_id(raw: &str) -> Option<String> {
    parse_player_entity_uuid(raw).map(|uuid| uuid.to_string())
}

fn bare_player_entity_id(raw: &str) -> Option<String> {
    parse_player_entity_uuid(raw).map(|uuid| uuid.to_string())
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
        Self {
            config,
            store,
            bootstrap_dispatcher,
            starter_world_persister,
        }
    }

    pub async fn register(&self, email: &str, password: &str) -> Result<AuthTokens, AuthError> {
        let normalized_email = normalize_email(email)?;
        validate_password(password)?;

        let password_hash = hash_password(password)?;
        let account = if let Some(account) = self
            .store
            .create_account_atomic(&normalized_email, &password_hash)
            .await?
        {
            info!(
                "gateway register used atomic account creation path account_id={} player_entity_id={}",
                account.account_id, account.player_entity_id
            );
            account
        } else {
            let account = self
                .store
                .create_account(&normalized_email, &password_hash)
                .await?;
            info!(
                "gateway register used fallback account creation path account_id={} player_entity_id={}",
                account.account_id, account.player_entity_id
            );
            account
        };
        self.starter_world_persister
            .persist_starter_world(
                account.account_id,
                &account.player_entity_id,
                &normalized_email,
            )
            .await?;

        self.issue_tokens(account.account_id).await
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<AuthTokens, AuthError> {
        let normalized_email = normalize_email(email)?;
        let account = self
            .store
            .get_account_by_email(&normalized_email)
            .await?
            .ok_or_else(|| AuthError::Unauthorized("invalid credentials".to_string()))?;
        verify_password(password, &account.password_hash)?;
        self.issue_tokens(account.account_id).await
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
        let claims = self.decode_access_token(access_token)?;
        let account_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AuthError::Unauthorized("invalid access token subject".to_string()))?;
        self.store.list_account_characters(account_id).await
    }

    pub async fn enter_world(
        &self,
        access_token: &str,
        player_entity_id: &str,
    ) -> Result<(), AuthError> {
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
                player_entity_id: normalized_player_entity_id,
            })
            .await
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

    async fn issue_tokens(&self, account_id: Uuid) -> Result<AuthTokens, AuthError> {
        let account = self
            .store
            .get_account_by_id(account_id)
            .await?
            .ok_or_else(|| AuthError::Internal("account missing".to_string()))?;
        let iat = now_epoch_s();
        let exp = iat + self.config.access_token_ttl_s;
        let claims = AuthClaims {
            sub: account.account_id.to_string(),
            player_entity_id: canonical_player_entity_id(&account.player_entity_id)
                .unwrap_or(account.player_entity_id),
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
}
