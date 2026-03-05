use async_trait::async_trait;
use sidereal_core::bootstrap_wire::AUTH_CHARACTERS_TABLE;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio_postgres::{Client, NoTls};
use tracing::error;
use uuid::Uuid;

use crate::auth::crypto::now_epoch_s;
use crate::auth::error::AuthError;
use crate::auth::types::{Account, AccountCharacter, PasswordResetTokenRecord, RefreshTokenRecord};

const ACCOUNTS_TABLE: &str = "auth_accounts";
const REFRESH_TOKENS_TABLE: &str = "auth_refresh_tokens";
const PASSWORD_RESET_TOKENS_TABLE: &str = "auth_password_reset_tokens";

#[async_trait]
pub trait AuthStore: Send + Sync {
    async fn create_account_atomic(
        &self,
        email: &str,
        password_hash: &str,
    ) -> Result<Account, AuthError>;
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

                CREATE TABLE IF NOT EXISTS {AUTH_CHARACTERS_TABLE} (
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
    ) -> Result<Account, AuthError> {
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
            let player_entity_id = account_id.to_string();
            let row = tx.query_one(
                &format!(
                    "
                    WITH inserted_account AS (
                        INSERT INTO {ACCOUNTS_TABLE} (account_id, email, password_hash, player_entity_id, created_at_epoch_s)
                        VALUES ($1, $2, $3, $4, $5)
                        RETURNING account_id, email, password_hash, player_entity_id
                    ),
                    inserted_character AS (
                        INSERT INTO {AUTH_CHARACTERS_TABLE} (account_id, player_entity_id, created_at_epoch_s)
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
        Ok(account)
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
                    "SELECT player_entity_id FROM {AUTH_CHARACTERS_TABLE} WHERE account_id = $1 ORDER BY created_at_epoch_s ASC"
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
                    "SELECT 1 FROM {AUTH_CHARACTERS_TABLE} WHERE account_id = $1 AND player_entity_id = $2"
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
    async fn create_account_atomic(
        &self,
        email: &str,
        password_hash: &str,
    ) -> Result<Account, AuthError> {
        let mut state = self.state.write().await;
        if state.accounts_by_email.contains_key(email) {
            return Err(AuthError::Conflict("account already exists".to_string()));
        }
        let account_id = Uuid::new_v4();
        let account = Account {
            account_id,
            email: email.to_string(),
            password_hash: password_hash.to_string(),
            player_entity_id: account_id.to_string(),
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
