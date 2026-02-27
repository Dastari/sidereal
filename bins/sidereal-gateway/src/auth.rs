use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sidereal_persistence::{
    GraphPersistence, ensure_schema_in_transaction, persist_graph_records_in_transaction,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use tokio_postgres::error::SqlState;
use tokio_postgres::{Client, NoTls};
use tracing::error;
use uuid::Uuid;

const MIN_PASSWORD_LEN: usize = 12;
const ACCOUNTS_TABLE: &str = "auth_accounts";
const CHARACTERS_TABLE: &str = "auth_characters";
const REFRESH_TOKENS_TABLE: &str = "auth_refresh_tokens";
const PASSWORD_RESET_TOKENS_TABLE: &str = "auth_password_reset_tokens";

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub access_token_ttl_s: u64,
    pub refresh_token_ttl_s: u64,
    pub reset_token_ttl_s: u64,
}

impl AuthConfig {
    pub fn from_env() -> Result<Self, AuthError> {
        let jwt_secret = std::env::var("GATEWAY_JWT_SECRET")
            .map_err(|_| AuthError::Config("GATEWAY_JWT_SECRET is required".to_string()))?;
        if jwt_secret.len() < 32 {
            return Err(AuthError::Config(
                "GATEWAY_JWT_SECRET must be at least 32 characters".to_string(),
            ));
        }

        let access_token_ttl_s = parse_ttl_env("GATEWAY_ACCESS_TOKEN_TTL_S", 900)?;
        let refresh_token_ttl_s = parse_ttl_env("GATEWAY_REFRESH_TOKEN_TTL_S", 2_592_000)?;
        let reset_token_ttl_s = parse_ttl_env("GATEWAY_RESET_TOKEN_TTL_S", 3_600)?;

        Ok(Self {
            jwt_secret,
            access_token_ttl_s,
            refresh_token_ttl_s,
            reset_token_ttl_s,
        })
    }

    pub fn for_tests() -> Self {
        Self {
            jwt_secret: "0123456789abcdef0123456789abcdef".to_string(),
            access_token_ttl_s: 900,
            refresh_token_ttl_s: 3_600,
            reset_token_ttl_s: 900,
        }
    }
}

fn parse_ttl_env(name: &str, default_value: u64) -> Result<u64, AuthError> {
    match std::env::var(name) {
        Ok(raw) => raw
            .parse::<u64>()
            .map_err(|_| AuthError::Config(format!("{name} must be a positive integer"))),
        Err(_) => Ok(default_value),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub account_id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub player_entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: String,
    pub player_entity_id: String,
    pub iat: u64,
    pub exp: u64,
    pub jti: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMe {
    pub account_id: Uuid,
    pub email: String,
    pub player_entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountCharacter {
    pub player_entity_id: String,
}

#[derive(Debug, Clone)]
pub struct RefreshTokenRecord {
    pub account_id: Uuid,
    pub expires_at_epoch_s: u64,
}

#[derive(Debug, Clone)]
pub struct PasswordResetTokenRecord {
    pub account_id: Uuid,
    pub expires_at_epoch_s: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BootstrapCommand {
    pub account_id: Uuid,
    pub player_entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetRequestResult {
    pub accepted: bool,
    pub reset_token: Option<String>,
}

#[async_trait]
pub trait AuthStore: Send + Sync {
    async fn create_account_atomic(
        &self,
        _email: &str,
        _password_hash: &str,
    ) -> Result<Option<Account>, AuthError> {
        Ok(None)
    }
    async fn create_account(&self, email: &str, password_hash: &str) -> Result<Account, AuthError>;
    async fn get_account_by_email(&self, email: &str) -> Result<Option<Account>, AuthError>;
    async fn get_account_by_id(&self, account_id: Uuid) -> Result<Option<Account>, AuthError>;
    async fn insert_refresh_token(
        &self,
        token_hash: &str,
        account_id: Uuid,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError>;
    async fn consume_refresh_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<RefreshTokenRecord>, AuthError>;
    async fn insert_password_reset_token(
        &self,
        token_hash: &str,
        account_id: Uuid,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError>;
    async fn consume_password_reset_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<PasswordResetTokenRecord>, AuthError>;
    async fn update_password_hash(
        &self,
        account_id: Uuid,
        new_password_hash: &str,
    ) -> Result<(), AuthError>;
    async fn list_account_characters(
        &self,
        account_id: Uuid,
    ) -> Result<Vec<AccountCharacter>, AuthError>;
    async fn account_owns_player_entity(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
    ) -> Result<bool, AuthError>;
}

#[async_trait]
pub trait BootstrapDispatcher: Send + Sync {
    async fn dispatch(&self, command: &BootstrapCommand) -> Result<(), AuthError>;
}

#[async_trait]
pub trait StarterWorldPersister: Send + Sync {
    async fn persist_starter_world(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
    ) -> Result<(), AuthError>;
}

pub struct GraphStarterWorldPersister;

#[async_trait]
impl StarterWorldPersister for GraphStarterWorldPersister {
    async fn persist_starter_world(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
    ) -> Result<(), AuthError> {
        let player_entity_id = player_entity_id.to_string();
        tokio::task::spawn_blocking(move || {
            persist_starter_world_for_new_account(account_id, &player_entity_id)
        })
        .await
        .map_err(|err| {
            AuthError::Internal(format!("starter world persistence task failed: {err}"))
        })?
    }
}

pub struct NoopStarterWorldPersister;

#[async_trait]
impl StarterWorldPersister for NoopStarterWorldPersister {
    async fn persist_starter_world(
        &self,
        _account_id: Uuid,
        _player_entity_id: &str,
    ) -> Result<(), AuthError> {
        Ok(())
    }
}

pub struct AuthService {
    config: AuthConfig,
    store: Arc<dyn AuthStore>,
    bootstrap_dispatcher: Arc<dyn BootstrapDispatcher>,
    starter_world_persister: Arc<dyn StarterWorldPersister>,
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
            account
        } else {
            let account = self
                .store
                .create_account(&normalized_email, &password_hash)
                .await?;
            self.starter_world_persister
                .persist_starter_world(account.account_id, &account.player_entity_id)
                .await?;
            account
        };

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
        if player_entity_id.trim().is_empty() {
            return Err(AuthError::Validation(
                "player_entity_id is required".to_string(),
            ));
        }
        let claims = self.decode_access_token(access_token)?;
        let account_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AuthError::Unauthorized("invalid access token subject".to_string()))?;
        let owns = self
            .store
            .account_owns_player_entity(account_id, player_entity_id)
            .await?;
        if !owns {
            return Err(AuthError::Unauthorized(
                "player_entity_id is not owned by authenticated account".to_string(),
            ));
        }
        self.bootstrap_dispatcher
            .dispatch(&BootstrapCommand {
                account_id,
                player_entity_id: player_entity_id.to_string(),
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
        Ok(token.claims)
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
            player_entity_id: account.player_entity_id,
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

#[derive(Debug)]
pub struct PostgresAuthStore {
    client: Client,
    database_url: String,
}

impl PostgresAuthStore {
    pub async fn connect(database_url: &str) -> Result<Self, AuthError> {
        let (client, connection) = tokio_postgres::connect(database_url, NoTls)
            .await
            .map_err(|err| AuthError::Config(format!("postgres connect failed: {err}")))?;
        tokio::spawn(async move {
            if let Err(err) = connection.await {
                error!("gateway postgres connection ended: {}", err);
            }
        });
        Ok(Self {
            client,
            database_url: database_url.to_string(),
        })
    }

    pub async fn ensure_schema(&self) -> Result<(), AuthError> {
        let schema = format!(
            "
                CREATE TABLE IF NOT EXISTS {ACCOUNTS_TABLE} (
                    account_id UUID PRIMARY KEY,
                    email TEXT NOT NULL UNIQUE,
                    password_hash TEXT NOT NULL,
                    player_entity_id TEXT NOT NULL,
                    created_at_epoch_s BIGINT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS {CHARACTERS_TABLE} (
                    account_id UUID NOT NULL REFERENCES {ACCOUNTS_TABLE}(account_id) ON DELETE CASCADE,
                    player_entity_id TEXT PRIMARY KEY,
                    created_at_epoch_s BIGINT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS {REFRESH_TOKENS_TABLE} (
                    token_hash TEXT PRIMARY KEY,
                    account_id UUID NOT NULL REFERENCES {ACCOUNTS_TABLE}(account_id) ON DELETE CASCADE,
                    expires_at_epoch_s BIGINT NOT NULL,
                    created_at_epoch_s BIGINT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS {PASSWORD_RESET_TOKENS_TABLE} (
                    token_hash TEXT PRIMARY KEY,
                    account_id UUID NOT NULL REFERENCES {ACCOUNTS_TABLE}(account_id) ON DELETE CASCADE,
                    expires_at_epoch_s BIGINT NOT NULL,
                    created_at_epoch_s BIGINT NOT NULL
                );
                "
        );
        self.client
            .batch_execute(&schema)
            .await
            .map_err(|err| AuthError::Internal(format!("schema ensure failed: {err}")))
    }
}

#[async_trait]
impl AuthStore for PostgresAuthStore {
    async fn create_account_atomic(
        &self,
        email: &str,
        password_hash: &str,
    ) -> Result<Option<Account>, AuthError> {
        let email = email.to_string();
        let password_hash = password_hash.to_string();
        let database_url = self.database_url.clone();
        let account = tokio::task::spawn_blocking(move || {
            let mut client = postgres::Client::connect(&database_url, postgres::NoTls)
                .map_err(|err| AuthError::Internal(format!("postgres connect failed: {err}")))?;
            let mut tx = client
                .transaction()
                .map_err(|err| AuthError::Internal(format!("transaction begin failed: {err}")))?;

            let now = now_epoch_s() as i64;
            let account_id = Uuid::new_v4();
            let player_entity_id = format!("player:{account_id}");
            let row = tx.query_one(
                &format!(
                    "
                    WITH inserted_account AS (
                        INSERT INTO {ACCOUNTS_TABLE} (account_id, email, password_hash, player_entity_id, created_at_epoch_s)
                        VALUES ($1, $2, $3, $4, $5)
                        RETURNING account_id, email, password_hash, player_entity_id
                    ),
                    inserted_character AS (
                        INSERT INTO {CHARACTERS_TABLE} (account_id, player_entity_id, created_at_epoch_s)
                        SELECT account_id, player_entity_id, $5 FROM inserted_account
                    )
                    SELECT account_id, email, password_hash, player_entity_id FROM inserted_account
                    "
                ),
                &[&account_id, &email, &password_hash, &player_entity_id, &now],
            );
            let row = match row {
                Ok(row) => row,
                Err(err) if err.code() == Some(&postgres::error::SqlState::UNIQUE_VIOLATION) => {
                    return Err(AuthError::Conflict("account already exists".to_string()));
                }
                Err(err) => {
                    return Err(AuthError::Internal(format!("create account failed: {err}")));
                }
            };

            ensure_schema_in_transaction(&mut tx, "sidereal").map_err(|err| {
                AuthError::Internal(format!("starter world schema ensure failed: {err}"))
            })?;
            let position = sidereal_game::corvette_random_spawn_position(account_id);
            let graph_records = sidereal_runtime_sync::entity_templates::corvette_starter_graph_records(
                account_id,
                &player_entity_id,
                position,
            );
            persist_graph_records_in_transaction(&mut tx, "sidereal", &graph_records, 0).map_err(
                |err| AuthError::Internal(format!("persist starter world failed: {err}")),
            )?;

            tx.commit()
                .map_err(|err| AuthError::Internal(format!("transaction commit failed: {err}")))?;

            Ok(Account {
                account_id: row.get(0),
                email: row.get(1),
                password_hash: row.get(2),
                player_entity_id: row.get(3),
            })
        })
        .await
        .map_err(|err| AuthError::Internal(format!("register task failed: {err}")))??;
        Ok(Some(account))
    }

    async fn create_account(&self, email: &str, password_hash: &str) -> Result<Account, AuthError> {
        let now = now_epoch_s() as i64;
        let account_id = Uuid::new_v4();
        let player_entity_id = format!("player:{account_id}");
        let row = self
            .client
            .query_one(
                &format!(
                    "
                WITH inserted_account AS (
                    INSERT INTO {ACCOUNTS_TABLE} (account_id, email, password_hash, player_entity_id, created_at_epoch_s)
                    VALUES ($1, $2, $3, $4, $5)
                    RETURNING account_id, email, password_hash, player_entity_id
                ),
                inserted_character AS (
                    INSERT INTO {CHARACTERS_TABLE} (account_id, player_entity_id, created_at_epoch_s)
                    SELECT account_id, player_entity_id, $5 FROM inserted_account
                )
                SELECT account_id, email, password_hash, player_entity_id FROM inserted_account
                "
                ),
                &[&account_id, &email, &password_hash, &player_entity_id, &now],
            )
            .await;

        match row {
            Ok(row) => Ok(Account {
                account_id: row.get(0),
                email: row.get(1),
                password_hash: row.get(2),
                player_entity_id: row.get(3),
            }),
            Err(err) if err.code() == Some(&SqlState::UNIQUE_VIOLATION) => {
                Err(AuthError::Conflict("account already exists".to_string()))
            }
            Err(err) => Err(AuthError::Internal(format!("create account failed: {err}"))),
        }
    }

    async fn get_account_by_email(&self, email: &str) -> Result<Option<Account>, AuthError> {
        let row = self
            .client
            .query_opt(
                &format!(
                    "SELECT account_id, email, password_hash, player_entity_id FROM {ACCOUNTS_TABLE} WHERE email = $1"
                ),
                &[&email],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("get account by email failed: {err}")))?;

        Ok(row.map(|row| Account {
            account_id: row.get(0),
            email: row.get(1),
            password_hash: row.get(2),
            player_entity_id: row.get(3),
        }))
    }

    async fn get_account_by_id(&self, account_id: Uuid) -> Result<Option<Account>, AuthError> {
        let row = self
            .client
            .query_opt(
                &format!(
                    "SELECT account_id, email, password_hash, player_entity_id FROM {ACCOUNTS_TABLE} WHERE account_id = $1"
                ),
                &[&account_id],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("get account by id failed: {err}")))?;

        Ok(row.map(|row| Account {
            account_id: row.get(0),
            email: row.get(1),
            password_hash: row.get(2),
            player_entity_id: row.get(3),
        }))
    }

    async fn insert_refresh_token(
        &self,
        token_hash: &str,
        account_id: Uuid,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError> {
        let now = now_epoch_s() as i64;
        self.client
            .execute(
                &format!(
                    "INSERT INTO {REFRESH_TOKENS_TABLE} (token_hash, account_id, expires_at_epoch_s, created_at_epoch_s) VALUES ($1, $2, $3, $4)"
                ),
                &[&token_hash, &account_id, &(expires_at_epoch_s as i64), &now],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("insert refresh token failed: {err}")))?;
        Ok(())
    }

    async fn consume_refresh_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<RefreshTokenRecord>, AuthError> {
        let row = self
            .client
            .query_opt(
                &format!(
                    "DELETE FROM {REFRESH_TOKENS_TABLE} WHERE token_hash = $1 RETURNING account_id, expires_at_epoch_s"
                ),
                &[&token_hash],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("consume refresh token failed: {err}")))?;

        Ok(row.map(|row| RefreshTokenRecord {
            account_id: row.get(0),
            expires_at_epoch_s: row.get::<usize, i64>(1) as u64,
        }))
    }

    async fn insert_password_reset_token(
        &self,
        token_hash: &str,
        account_id: Uuid,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError> {
        let now = now_epoch_s() as i64;
        self.client
            .execute(
                &format!(
                    "INSERT INTO {PASSWORD_RESET_TOKENS_TABLE} (token_hash, account_id, expires_at_epoch_s, created_at_epoch_s) VALUES ($1, $2, $3, $4)"
                ),
                &[&token_hash, &account_id, &(expires_at_epoch_s as i64), &now],
            )
            .await
            .map_err(|err| {
                AuthError::Internal(format!("insert password reset token failed: {err}"))
            })?;
        Ok(())
    }

    async fn consume_password_reset_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<PasswordResetTokenRecord>, AuthError> {
        let row = self
            .client
            .query_opt(
                &format!(
                    "DELETE FROM {PASSWORD_RESET_TOKENS_TABLE} WHERE token_hash = $1 RETURNING account_id, expires_at_epoch_s"
                ),
                &[&token_hash],
            )
            .await
            .map_err(|err| {
                AuthError::Internal(format!("consume password reset token failed: {err}"))
            })?;

        Ok(row.map(|row| PasswordResetTokenRecord {
            account_id: row.get(0),
            expires_at_epoch_s: row.get::<usize, i64>(1) as u64,
        }))
    }

    async fn update_password_hash(
        &self,
        account_id: Uuid,
        new_password_hash: &str,
    ) -> Result<(), AuthError> {
        let updated = self
            .client
            .execute(
                &format!("UPDATE {ACCOUNTS_TABLE} SET password_hash = $2 WHERE account_id = $1"),
                &[&account_id, &new_password_hash],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("update password hash failed: {err}")))?;
        if updated == 0 {
            return Err(AuthError::Unauthorized("unknown account".to_string()));
        }
        Ok(())
    }

    async fn list_account_characters(
        &self,
        account_id: Uuid,
    ) -> Result<Vec<AccountCharacter>, AuthError> {
        let rows = self
            .client
            .query(
                &format!(
                    "SELECT player_entity_id FROM {CHARACTERS_TABLE} WHERE account_id = $1 ORDER BY created_at_epoch_s ASC"
                ),
                &[&account_id],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("list account characters failed: {err}")))?;
        Ok(rows
            .into_iter()
            .map(|row| AccountCharacter {
                player_entity_id: row.get(0),
            })
            .collect())
    }

    async fn account_owns_player_entity(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
    ) -> Result<bool, AuthError> {
        let row = self
            .client
            .query_opt(
                &format!(
                    "SELECT 1 FROM {CHARACTERS_TABLE} WHERE account_id = $1 AND player_entity_id = $2"
                ),
                &[&account_id, &player_entity_id],
            )
            .await
            .map_err(|err| {
                AuthError::Internal(format!("character ownership lookup failed: {err}"))
            })?;
        Ok(row.is_some())
    }
}

#[derive(Debug)]
pub struct UdpBootstrapDispatcher {
    socket: UdpSocket,
    target: SocketAddr,
}

impl UdpBootstrapDispatcher {
    /// Constructor for tests or when socket and target are already available.
    pub fn new(socket: UdpSocket, target: SocketAddr) -> Self {
        Self { socket, target }
    }

    pub async fn from_env() -> Result<Self, AuthError> {
        let target_raw = std::env::var("REPLICATION_CONTROL_UDP_ADDR").map_err(|_| {
            AuthError::Config(
                "REPLICATION_CONTROL_UDP_ADDR is required for bootstrap handoff".to_string(),
            )
        })?;
        let target: SocketAddr = target_raw
            .parse()
            .map_err(|_| AuthError::Config("invalid REPLICATION_CONTROL_UDP_ADDR".to_string()))?;

        let bind = std::env::var("GATEWAY_REPLICATION_CONTROL_UDP_BIND")
            .unwrap_or_else(|_| "0.0.0.0:0".to_string());
        let socket = UdpSocket::bind(&bind)
            .await
            .map_err(|err| AuthError::Config(format!("udp bind failed: {err}")))?;

        Ok(Self { socket, target })
    }
}

#[derive(Debug, Clone)]
pub struct DirectBootstrapDispatcher {
    pub database_url: String,
}

impl DirectBootstrapDispatcher {
    pub fn from_env() -> Self {
        let database_url = std::env::var("GATEWAY_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());
        Self { database_url }
    }
}

#[derive(Debug, Serialize)]
struct BootstrapWireMessage {
    kind: &'static str,
    account_id: Uuid,
    player_entity_id: String,
}

#[async_trait]
impl BootstrapDispatcher for UdpBootstrapDispatcher {
    async fn dispatch(&self, command: &BootstrapCommand) -> Result<(), AuthError> {
        let payload = BootstrapWireMessage {
            kind: "bootstrap_player",
            account_id: command.account_id,
            player_entity_id: command.player_entity_id.clone(),
        };
        let bytes = serde_json::to_vec(&payload)
            .map_err(|err| AuthError::Internal(format!("bootstrap serialize failed: {err}")))?;
        self.socket
            .send_to(&bytes, self.target)
            .await
            .map_err(|err| AuthError::Internal(format!("bootstrap send failed: {err}")))?;
        Ok(())
    }
}

#[async_trait]
impl BootstrapDispatcher for DirectBootstrapDispatcher {
    async fn dispatch(&self, command: &BootstrapCommand) -> Result<(), AuthError> {
        let database_url = self.database_url.clone();
        let command = command.clone();
        tokio::task::spawn_blocking(move || {
            let mut persistence = GraphPersistence::connect(&database_url)
                .map_err(|err| AuthError::Internal(format!("persistence connect failed: {err}")))?;
            persistence.ensure_schema().map_err(|err| {
                AuthError::Internal(format!("persistence ensure schema failed: {err}"))
            })?;
            // Registration is responsible for starter-world provisioning.
            // Direct bootstrap mode is a runtime handoff fallback and must remain idempotent.
            let records = persistence
                .load_graph_records()
                .map_err(|err| AuthError::Internal(format!("load graph records failed: {err}")))?;
            if !records
                .iter()
                .any(|record| record.entity_id == command.player_entity_id)
            {
                return Err(AuthError::Internal(format!(
                    "bootstrap rejected: player entity {} not found in graph persistence",
                    command.player_entity_id
                )));
            }
            Ok::<_, AuthError>(())
        })
        .await
        .map_err(|err| AuthError::Internal(format!("bootstrap dispatch task failed: {err}")))?
    }
}

#[derive(Debug, Default)]
pub struct NoopBootstrapDispatcher;

#[async_trait]
impl BootstrapDispatcher for NoopBootstrapDispatcher {
    async fn dispatch(&self, _command: &BootstrapCommand) -> Result<(), AuthError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct RecordingBootstrapDispatcher {
    commands: Mutex<Vec<BootstrapCommand>>,
}

impl RecordingBootstrapDispatcher {
    pub async fn commands(&self) -> Vec<BootstrapCommand> {
        self.commands.lock().await.clone()
    }
}

#[async_trait]
impl BootstrapDispatcher for RecordingBootstrapDispatcher {
    async fn dispatch(&self, command: &BootstrapCommand) -> Result<(), AuthError> {
        self.commands.lock().await.push(command.clone());
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct InMemoryAuthStore {
    state: RwLock<InMemoryAuthState>,
}

#[derive(Debug, Default)]
struct InMemoryAuthState {
    accounts_by_email: HashMap<String, Account>,
    accounts_by_id: HashMap<Uuid, Account>,
    refresh_tokens_by_hash: HashMap<String, RefreshTokenRecord>,
    password_reset_tokens_by_hash: HashMap<String, PasswordResetTokenRecord>,
}

#[async_trait]
impl AuthStore for InMemoryAuthStore {
    async fn create_account(&self, email: &str, password_hash: &str) -> Result<Account, AuthError> {
        let mut state = self.state.write().await;
        if state.accounts_by_email.contains_key(email) {
            return Err(AuthError::Conflict("account already exists".to_string()));
        }
        let account_id = Uuid::new_v4();
        let account = Account {
            account_id,
            email: email.to_string(),
            password_hash: password_hash.to_string(),
            player_entity_id: format!("player:{account_id}"),
        };
        state
            .accounts_by_email
            .insert(email.to_string(), account.clone());
        state
            .accounts_by_id
            .insert(account.account_id, account.clone());
        Ok(account)
    }

    async fn get_account_by_email(&self, email: &str) -> Result<Option<Account>, AuthError> {
        let state = self.state.read().await;
        Ok(state.accounts_by_email.get(email).cloned())
    }

    async fn get_account_by_id(&self, account_id: Uuid) -> Result<Option<Account>, AuthError> {
        let state = self.state.read().await;
        Ok(state.accounts_by_id.get(&account_id).cloned())
    }

    async fn insert_refresh_token(
        &self,
        token_hash: &str,
        account_id: Uuid,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError> {
        let mut state = self.state.write().await;
        state.refresh_tokens_by_hash.insert(
            token_hash.to_string(),
            RefreshTokenRecord {
                account_id,
                expires_at_epoch_s,
            },
        );
        Ok(())
    }

    async fn consume_refresh_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<RefreshTokenRecord>, AuthError> {
        let mut state = self.state.write().await;
        Ok(state.refresh_tokens_by_hash.remove(token_hash))
    }

    async fn insert_password_reset_token(
        &self,
        token_hash: &str,
        account_id: Uuid,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError> {
        let mut state = self.state.write().await;
        state.password_reset_tokens_by_hash.insert(
            token_hash.to_string(),
            PasswordResetTokenRecord {
                account_id,
                expires_at_epoch_s,
            },
        );
        Ok(())
    }

    async fn consume_password_reset_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<PasswordResetTokenRecord>, AuthError> {
        let mut state = self.state.write().await;
        Ok(state.password_reset_tokens_by_hash.remove(token_hash))
    }

    async fn update_password_hash(
        &self,
        account_id: Uuid,
        new_password_hash: &str,
    ) -> Result<(), AuthError> {
        let mut state = self.state.write().await;
        let account = state
            .accounts_by_id
            .get_mut(&account_id)
            .ok_or_else(|| AuthError::Unauthorized("unknown account".to_string()))?;
        account.password_hash = new_password_hash.to_string();
        let updated = account.clone();
        state
            .accounts_by_email
            .insert(updated.email.clone(), updated);
        Ok(())
    }

    async fn list_account_characters(
        &self,
        account_id: Uuid,
    ) -> Result<Vec<AccountCharacter>, AuthError> {
        let state = self.state.read().await;
        let Some(account) = state.accounts_by_id.get(&account_id) else {
            return Ok(Vec::new());
        };
        Ok(vec![AccountCharacter {
            player_entity_id: account.player_entity_id.clone(),
        }])
    }

    async fn account_owns_player_entity(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
    ) -> Result<bool, AuthError> {
        let state = self.state.read().await;
        Ok(state
            .accounts_by_id
            .get(&account_id)
            .is_some_and(|account| account.player_entity_id == player_entity_id))
    }
}

fn persist_starter_world_for_new_account(
    account_id: Uuid,
    player_entity_id: &str,
) -> Result<(), AuthError> {
    let database_url = std::env::var("GATEWAY_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());
    let mut persistence = GraphPersistence::connect(&database_url)
        .map_err(|err| AuthError::Internal(format!("persistence connect failed: {err}")))?;
    persistence
        .ensure_schema()
        .map_err(|err| AuthError::Internal(format!("persistence ensure schema failed: {err}")))?;
    let records = persistence
        .load_graph_records()
        .map_err(|err| AuthError::Internal(format!("load graph records failed: {err}")))?;
    if records
        .iter()
        .any(|record| record.entity_id == player_entity_id)
    {
        return Err(AuthError::Internal(format!(
            "register invariant violation: player entity {player_entity_id} already exists in graph persistence"
        )));
    }
    let position = sidereal_game::corvette_random_spawn_position(account_id);
    let graph_records = sidereal_runtime_sync::entity_templates::corvette_starter_graph_records(
        account_id,
        player_entity_id,
        position,
    );
    persistence
        .persist_graph_records(&graph_records, 0)
        .map_err(|err| AuthError::Internal(format!("persist starter world failed: {err}")))?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Config(String),
    #[error("{0}")]
    Internal(String),
}

pub fn hash_password(password: &str) -> Result<String, AuthError> {
    validate_password(password)?;
    let mut salt_bytes = [0_u8; 16];
    let mut rng = rand::rng();
    rng.fill_bytes(&mut salt_bytes);
    let salt = SaltString::encode_b64(&salt_bytes)
        .map_err(|_| AuthError::Internal("password salt generation failed".to_string()))?;
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| AuthError::Internal("password hash failed".to_string()))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<(), AuthError> {
    let parsed = PasswordHash::new(hash)
        .map_err(|_| AuthError::Unauthorized("invalid credentials".to_string()))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| AuthError::Unauthorized("invalid credentials".to_string()))
}

pub fn normalize_email(email: &str) -> Result<String, AuthError> {
    let normalized = email.trim().to_ascii_lowercase();
    validate_email(&normalized)?;
    Ok(normalized)
}

pub fn validate_email(email: &str) -> Result<(), AuthError> {
    if email.len() < 3 || email.len() > 254 {
        return Err(AuthError::Validation(
            "email must be between 3 and 254 chars".to_string(),
        ));
    }
    let mut parts = email.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || local.is_empty()
        || domain.is_empty()
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return Err(AuthError::Validation("email format is invalid".to_string()));
    }
    Ok(())
}

pub fn validate_password(password: &str) -> Result<(), AuthError> {
    if password.len() < MIN_PASSWORD_LEN {
        return Err(AuthError::Validation(format!(
            "password must be at least {MIN_PASSWORD_LEN} chars"
        )));
    }
    if password.len() > 128 {
        return Err(AuthError::Validation(
            "password must be <= 128 chars".to_string(),
        ));
    }
    Ok(())
}

pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    bytes_to_hex(&digest)
}

fn generate_opaque_token() -> String {
    let mut bytes = [0_u8; 32];
    let mut rng = rand::rng();
    rng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn now_epoch_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs()
}
