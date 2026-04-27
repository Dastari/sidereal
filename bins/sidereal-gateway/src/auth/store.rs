use async_trait::async_trait;
use sidereal_core::bootstrap_wire::AUTH_CHARACTERS_TABLE;
use sidereal_persistence::script_catalog_schema_sql;
use std::collections::HashMap;
use std::future::poll_fn;
use tokio::sync::RwLock;
use tokio_postgres::{AsyncMessage, Client, NoTls};
use tracing::{debug, error};
use uuid::Uuid;

use crate::auth::crypto::now_epoch_s;
use crate::auth::error::AuthError;
use crate::auth::types::{
    Account, AccountCharacter, EmailLoginChallengeRecord, PasswordResetTokenRecord,
    RefreshTokenRecord, TotpEnrollmentRecord, TotpLoginChallengeRecord,
};

const ACCOUNTS_TABLE: &str = "auth_accounts";
const REFRESH_TOKENS_TABLE: &str = "auth_refresh_tokens";
const PASSWORD_RESET_TOKENS_TABLE: &str = "auth_password_reset_tokens";
const EMAIL_LOGIN_CHALLENGES_TABLE: &str = "auth_email_login_challenges";
const EMAIL_DELIVERY_EVENTS_TABLE: &str = "auth_email_delivery_events";
const TOTP_ENROLLMENTS_TABLE: &str = "auth_totp_enrollments";
const TOTP_SECRETS_TABLE: &str = "auth_totp_secrets";
const TOTP_LOGIN_CHALLENGES_TABLE: &str = "auth_totp_login_challenges";
const ACCOUNT_ROLES_TABLE: &str = "auth_account_roles";
const ACCOUNT_SCOPES_TABLE: &str = "auth_account_scopes";
const BOOTSTRAP_STATE_TABLE: &str = "auth_bootstrap_state";

fn validate_auth_label(kind: &str, value: &str) -> Result<String, AuthError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
        return Err(AuthError::Validation(format!(
            "{kind} must be non-empty and contain no whitespace"
        )));
    }
    Ok(trimmed.to_string())
}

#[async_trait]
pub trait AuthStore: Send + Sync {
    async fn create_account_atomic(
        &self,
        email: &str,
        password_hash: &str,
    ) -> Result<Account, AuthError>;
    async fn get_account_by_email(&self, email: &str) -> Result<Option<Account>, AuthError>;
    async fn get_account_by_id(&self, account_id: Uuid) -> Result<Option<Account>, AuthError>;
    async fn list_account_roles(&self, account_id: Uuid) -> Result<Vec<String>, AuthError>;
    async fn list_account_scopes(&self, account_id: Uuid) -> Result<Vec<String>, AuthError>;
    async fn add_account_role(&self, account_id: Uuid, role: &str) -> Result<(), AuthError>;
    async fn add_account_scope(&self, account_id: Uuid, scope: &str) -> Result<(), AuthError>;
    async fn admin_bootstrap_required(&self) -> Result<bool, AuthError>;
    async fn create_first_admin_account_atomic(
        &self,
        email: &str,
        password_hash: &str,
        roles: Vec<String>,
        scopes: Vec<String>,
    ) -> Result<Account, AuthError>;
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
    async fn count_email_delivery_events(
        &self,
        target_hash: &str,
        purpose: &str,
        since_epoch_s: u64,
    ) -> Result<u64, AuthError>;
    async fn insert_email_delivery_event(
        &self,
        target_hash: &str,
        purpose: &str,
        created_at_epoch_s: u64,
    ) -> Result<(), AuthError>;
    async fn insert_totp_enrollment(
        &self,
        enrollment_id: Uuid,
        account_id: Uuid,
        encrypted_secret: &str,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError>;
    async fn get_totp_enrollment(
        &self,
        enrollment_id: Uuid,
    ) -> Result<Option<TotpEnrollmentRecord>, AuthError>;
    async fn activate_totp_enrollment(
        &self,
        enrollment_id: Uuid,
        account_id: Uuid,
        verified_at_epoch_s: u64,
    ) -> Result<bool, AuthError>;
    async fn account_has_verified_totp(&self, account_id: Uuid) -> Result<bool, AuthError>;
    async fn get_verified_totp_secret(&self, account_id: Uuid)
    -> Result<Option<String>, AuthError>;
    async fn insert_totp_login_challenge(
        &self,
        challenge_id: Uuid,
        account_id: Uuid,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError>;
    async fn get_totp_login_challenge(
        &self,
        challenge_id: Uuid,
    ) -> Result<Option<TotpLoginChallengeRecord>, AuthError>;
    async fn consume_totp_login_challenge(
        &self,
        challenge_id: Uuid,
        account_id: Uuid,
        consumed_at_epoch_s: u64,
    ) -> Result<bool, AuthError>;
    async fn insert_email_login_challenge(
        &self,
        challenge_id: Uuid,
        account_id: Uuid,
        code_hash: &str,
        token_hash: &str,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError>;
    async fn consume_email_login_challenge_by_code(
        &self,
        challenge_id: Uuid,
        code_hash: &str,
    ) -> Result<Option<EmailLoginChallengeRecord>, AuthError>;
    async fn consume_email_login_challenge_by_token(
        &self,
        challenge_id: Uuid,
        token_hash: &str,
    ) -> Result<Option<EmailLoginChallengeRecord>, AuthError>;
    async fn list_account_characters(
        &self,
        account_id: Uuid,
    ) -> Result<Vec<AccountCharacter>, AuthError>;
    async fn insert_account_character(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
        display_name: &str,
        created_at_epoch_s: u64,
    ) -> Result<AccountCharacter, AuthError>;
    async fn delete_account_character(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
    ) -> Result<(), AuthError>;
    async fn account_owns_player_entity(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
    ) -> Result<bool, AuthError>;
}

#[derive(Debug)]
pub struct PostgresAuthStore {
    client: Client,
}

impl PostgresAuthStore {
    pub async fn connect(database_url: &str) -> Result<Self, AuthError> {
        let (client, mut connection) = tokio_postgres::connect(database_url, NoTls)
            .await
            .map_err(|err| AuthError::Config(format!("postgres connect failed: {err}")))?;
        tokio::spawn(async move {
            loop {
                match poll_fn(|cx| connection.poll_message(cx)).await {
                    Some(Ok(AsyncMessage::Notice(notice))) => {
                        debug!(
                            target: "tokio_postgres::connection",
                            "{}: {}",
                            notice.severity(),
                            notice.message()
                        );
                    }
                    Some(Ok(_)) => {}
                    Some(Err(err)) => {
                        error!("gateway postgres connection ended: {}", err);
                        break;
                    }
                    None => break,
                }
            }
        });
        Ok(Self { client })
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
                    created_at_epoch_s BIGINT NOT NULL,
                    display_name TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL DEFAULT 'active',
                    updated_at_epoch_s BIGINT NOT NULL DEFAULT 0
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

                CREATE TABLE IF NOT EXISTS {EMAIL_LOGIN_CHALLENGES_TABLE} (
                    challenge_id UUID PRIMARY KEY,
                    account_id UUID NOT NULL REFERENCES {ACCOUNTS_TABLE}(account_id) ON DELETE CASCADE,
                    code_hash TEXT NOT NULL,
                    token_hash TEXT NOT NULL,
                    expires_at_epoch_s BIGINT NOT NULL,
                    created_at_epoch_s BIGINT NOT NULL,
                    consumed_at_epoch_s BIGINT NULL
                );

                CREATE TABLE IF NOT EXISTS {EMAIL_DELIVERY_EVENTS_TABLE} (
                    event_id UUID PRIMARY KEY,
                    target_hash TEXT NOT NULL,
                    purpose TEXT NOT NULL,
                    created_at_epoch_s BIGINT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS {TOTP_ENROLLMENTS_TABLE} (
                    enrollment_id UUID PRIMARY KEY,
                    account_id UUID NOT NULL REFERENCES {ACCOUNTS_TABLE}(account_id) ON DELETE CASCADE,
                    encrypted_secret TEXT NOT NULL,
                    created_at_epoch_s BIGINT NOT NULL,
                    expires_at_epoch_s BIGINT NOT NULL,
                    consumed_at_epoch_s BIGINT NULL
                );

                CREATE TABLE IF NOT EXISTS {TOTP_SECRETS_TABLE} (
                    account_id UUID PRIMARY KEY REFERENCES {ACCOUNTS_TABLE}(account_id) ON DELETE CASCADE,
                    encrypted_secret TEXT NOT NULL,
                    created_at_epoch_s BIGINT NOT NULL,
                    verified_at_epoch_s BIGINT NOT NULL,
                    disabled_at_epoch_s BIGINT NULL
                );

                CREATE TABLE IF NOT EXISTS {TOTP_LOGIN_CHALLENGES_TABLE} (
                    challenge_id UUID PRIMARY KEY,
                    account_id UUID NOT NULL REFERENCES {ACCOUNTS_TABLE}(account_id) ON DELETE CASCADE,
                    expires_at_epoch_s BIGINT NOT NULL,
                    created_at_epoch_s BIGINT NOT NULL,
                    consumed_at_epoch_s BIGINT NULL
                );

                CREATE TABLE IF NOT EXISTS {ACCOUNT_ROLES_TABLE} (
                    account_id UUID NOT NULL REFERENCES {ACCOUNTS_TABLE}(account_id) ON DELETE CASCADE,
                    role TEXT NOT NULL,
                    created_at_epoch_s BIGINT NOT NULL,
                    PRIMARY KEY (account_id, role)
                );

                CREATE TABLE IF NOT EXISTS {ACCOUNT_SCOPES_TABLE} (
                    account_id UUID NOT NULL REFERENCES {ACCOUNTS_TABLE}(account_id) ON DELETE CASCADE,
                    scope TEXT NOT NULL,
                    created_at_epoch_s BIGINT NOT NULL,
                    PRIMARY KEY (account_id, scope)
                );

                CREATE TABLE IF NOT EXISTS {BOOTSTRAP_STATE_TABLE} (
                    id SMALLINT PRIMARY KEY CHECK (id = 1),
                    completed_by_account_id UUID NOT NULL REFERENCES {ACCOUNTS_TABLE}(account_id) ON DELETE CASCADE,
                    completed_at_epoch_s BIGINT NOT NULL
                );
                "
        );
        self.client
            .batch_execute(&schema)
            .await
            .map_err(|err| AuthError::Internal(format!("schema ensure failed: {err}")))?;
        self.client
            .batch_execute(&format!(
                "
                ALTER TABLE {AUTH_CHARACTERS_TABLE}
                    ADD COLUMN IF NOT EXISTS display_name TEXT NOT NULL DEFAULT '';
                ALTER TABLE {AUTH_CHARACTERS_TABLE}
                    ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'active';
                ALTER TABLE {AUTH_CHARACTERS_TABLE}
                    ADD COLUMN IF NOT EXISTS updated_at_epoch_s BIGINT NOT NULL DEFAULT 0;
                "
            ))
            .await
            .map_err(|err| AuthError::Internal(format!("character schema update failed: {err}")))?;
        self.client
            .batch_execute(script_catalog_schema_sql())
            .await
            .map_err(|err| {
                AuthError::Internal(format!("script catalog schema ensure failed: {err}"))
            })
    }
}

#[async_trait]
impl AuthStore for PostgresAuthStore {
    async fn create_account_atomic(
        &self,
        email: &str,
        password_hash: &str,
    ) -> Result<Account, AuthError> {
        let now = now_epoch_s() as i64;
        let account_id = Uuid::new_v4();
        let row = self
            .client
            .query_one(
                &format!(
                    "
                    INSERT INTO {ACCOUNTS_TABLE} (account_id, email, password_hash, player_entity_id, created_at_epoch_s)
                    VALUES ($1, $2, $3, $4, $5)
                    RETURNING account_id, email, password_hash, player_entity_id
                    "
                ),
                &[&account_id, &email, &password_hash, &"", &now],
            )
            .await
            .map_err(|err| {
                if err.code() == Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION) {
                    AuthError::Conflict("account already exists".to_string())
                } else {
                    AuthError::Internal(format!("create account failed: {err}"))
                }
            })?;

        Ok(Account {
            account_id: row.get(0),
            email: row.get(1),
            password_hash: row.get(2),
            player_entity_id: row.get(3),
        })
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

    async fn list_account_roles(&self, account_id: Uuid) -> Result<Vec<String>, AuthError> {
        let rows = self
            .client
            .query(
                &format!(
                    "SELECT role FROM {ACCOUNT_ROLES_TABLE} WHERE account_id = $1 ORDER BY role ASC"
                ),
                &[&account_id],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("list account roles failed: {err}")))?;
        Ok(rows.into_iter().map(|row| row.get(0)).collect())
    }

    async fn list_account_scopes(&self, account_id: Uuid) -> Result<Vec<String>, AuthError> {
        let rows = self
            .client
            .query(
                &format!("SELECT scope FROM {ACCOUNT_SCOPES_TABLE} WHERE account_id = $1 ORDER BY scope ASC"),
                &[&account_id],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("list account scopes failed: {err}")))?;
        Ok(rows.into_iter().map(|row| row.get(0)).collect())
    }

    async fn add_account_role(&self, account_id: Uuid, role: &str) -> Result<(), AuthError> {
        let role = validate_auth_label("role", role)?;
        let now = now_epoch_s() as i64;
        self.client
            .execute(
                &format!(
                    "INSERT INTO {ACCOUNT_ROLES_TABLE} (account_id, role, created_at_epoch_s) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"
                ),
                &[&account_id, &role.as_str(), &now],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("add account role failed: {err}")))?;
        Ok(())
    }

    async fn add_account_scope(&self, account_id: Uuid, scope: &str) -> Result<(), AuthError> {
        let scope = validate_auth_label("scope", scope)?;
        let now = now_epoch_s() as i64;
        self.client
            .execute(
                &format!(
                    "INSERT INTO {ACCOUNT_SCOPES_TABLE} (account_id, scope, created_at_epoch_s) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"
                ),
                &[&account_id, &scope.as_str(), &now],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("add account scope failed: {err}")))?;
        Ok(())
    }

    async fn admin_bootstrap_required(&self) -> Result<bool, AuthError> {
        let row = self
            .client
            .query_one(
                &format!(
                    "
                    SELECT
                        NOT EXISTS (SELECT 1 FROM {BOOTSTRAP_STATE_TABLE} WHERE id = 1)
                        AND NOT EXISTS (
                            SELECT 1 FROM {ACCOUNT_ROLES_TABLE}
                            WHERE lower(role) IN ('admin', 'dev_tool', 'developer')
                        )
                    "
                ),
                &[],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("bootstrap status failed: {err}")))?;
        Ok(row.get(0))
    }

    async fn create_first_admin_account_atomic(
        &self,
        email: &str,
        password_hash: &str,
        roles: Vec<String>,
        scopes: Vec<String>,
    ) -> Result<Account, AuthError> {
        let roles = roles
            .iter()
            .map(|role| validate_auth_label("role", role))
            .collect::<Result<Vec<_>, _>>()?;
        let scopes = scopes
            .iter()
            .map(|scope| validate_auth_label("scope", scope))
            .collect::<Result<Vec<_>, _>>()?;
        let account_id = Uuid::new_v4();
        let now = now_epoch_s() as i64;
        let row = self
            .client
            .query_opt(
                &format!(
                    "
                    WITH lock AS (
                        SELECT pg_advisory_xact_lock(732573190001)
                    ),
                    eligible AS (
                        SELECT 1 FROM lock
                        WHERE NOT EXISTS (SELECT 1 FROM {BOOTSTRAP_STATE_TABLE} WHERE id = 1)
                          AND NOT EXISTS (
                              SELECT 1 FROM {ACCOUNT_ROLES_TABLE}
                              WHERE lower(role) IN ('admin', 'dev_tool', 'developer')
                          )
                    ),
                    created AS (
                        INSERT INTO {ACCOUNTS_TABLE} (account_id, email, password_hash, player_entity_id, created_at_epoch_s)
                        SELECT $1, $2, $3, '', $4 FROM eligible
                        ON CONFLICT DO NOTHING
                        RETURNING account_id, email, password_hash, player_entity_id
                    ),
                    role_insert AS (
                        INSERT INTO {ACCOUNT_ROLES_TABLE} (account_id, role, created_at_epoch_s)
                        SELECT created.account_id, role_value, $4
                        FROM created CROSS JOIN unnest($5::text[]) AS role_values(role_value)
                        ON CONFLICT DO NOTHING
                        RETURNING 1
                    ),
                    scope_insert AS (
                        INSERT INTO {ACCOUNT_SCOPES_TABLE} (account_id, scope, created_at_epoch_s)
                        SELECT created.account_id, scope_value, $4
                        FROM created CROSS JOIN unnest($6::text[]) AS scope_values(scope_value)
                        ON CONFLICT DO NOTHING
                        RETURNING 1
                    ),
                    bootstrap_insert AS (
                        INSERT INTO {BOOTSTRAP_STATE_TABLE} (id, completed_by_account_id, completed_at_epoch_s)
                        SELECT 1, created.account_id, $4 FROM created
                        ON CONFLICT DO NOTHING
                        RETURNING 1
                    )
                    SELECT account_id, email, password_hash, player_entity_id FROM created
                    "
                ),
                &[&account_id, &email, &password_hash, &now, &roles, &scopes],
            )
            .await
            .map_err(|err| {
                if err.code() == Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION) {
                    AuthError::Conflict("account already exists".to_string())
                } else {
                    AuthError::Internal(format!("bootstrap admin create failed: {err}"))
                }
            })?;
        let Some(row) = row else {
            return Err(AuthError::Conflict(
                "administrator bootstrap is already complete".to_string(),
            ));
        };
        Ok(Account {
            account_id: row.get(0),
            email: row.get(1),
            password_hash: row.get(2),
            player_entity_id: row.get(3),
        })
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

    async fn count_email_delivery_events(
        &self,
        target_hash: &str,
        purpose: &str,
        since_epoch_s: u64,
    ) -> Result<u64, AuthError> {
        let row = self
            .client
            .query_one(
                &format!(
                    "SELECT COUNT(*) FROM {EMAIL_DELIVERY_EVENTS_TABLE} WHERE target_hash = $1 AND purpose = $2 AND created_at_epoch_s >= $3"
                ),
                &[&target_hash, &purpose, &(since_epoch_s as i64)],
            )
            .await
            .map_err(|err| {
                AuthError::Internal(format!("count email delivery events failed: {err}"))
            })?;
        let count = row.get::<usize, i64>(0);
        Ok(count as u64)
    }

    async fn insert_email_delivery_event(
        &self,
        target_hash: &str,
        purpose: &str,
        created_at_epoch_s: u64,
    ) -> Result<(), AuthError> {
        let event_id = Uuid::new_v4();
        self.client
            .execute(
                &format!(
                    "INSERT INTO {EMAIL_DELIVERY_EVENTS_TABLE} (event_id, target_hash, purpose, created_at_epoch_s) VALUES ($1, $2, $3, $4)"
                ),
                &[
                    &event_id,
                    &target_hash,
                    &purpose,
                    &(created_at_epoch_s as i64),
                ],
            )
            .await
            .map_err(|err| {
                AuthError::Internal(format!("insert email delivery event failed: {err}"))
            })?;
        Ok(())
    }

    async fn insert_totp_enrollment(
        &self,
        enrollment_id: Uuid,
        account_id: Uuid,
        encrypted_secret: &str,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError> {
        let now = now_epoch_s() as i64;
        self.client
            .execute(
                &format!(
                    "INSERT INTO {TOTP_ENROLLMENTS_TABLE} (enrollment_id, account_id, encrypted_secret, created_at_epoch_s, expires_at_epoch_s) VALUES ($1, $2, $3, $4, $5)"
                ),
                &[
                    &enrollment_id,
                    &account_id,
                    &encrypted_secret,
                    &now,
                    &(expires_at_epoch_s as i64),
                ],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("insert totp enrollment failed: {err}")))?;
        Ok(())
    }

    async fn get_totp_enrollment(
        &self,
        enrollment_id: Uuid,
    ) -> Result<Option<TotpEnrollmentRecord>, AuthError> {
        let row = self
            .client
            .query_opt(
                &format!(
                    "SELECT enrollment_id, account_id, encrypted_secret, expires_at_epoch_s FROM {TOTP_ENROLLMENTS_TABLE} WHERE enrollment_id = $1 AND consumed_at_epoch_s IS NULL"
                ),
                &[&enrollment_id],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("get totp enrollment failed: {err}")))?;
        Ok(row.map(|row| TotpEnrollmentRecord {
            enrollment_id: row.get(0),
            account_id: row.get(1),
            encrypted_secret: row.get(2),
            expires_at_epoch_s: row.get::<usize, i64>(3) as u64,
        }))
    }

    async fn activate_totp_enrollment(
        &self,
        enrollment_id: Uuid,
        account_id: Uuid,
        verified_at_epoch_s: u64,
    ) -> Result<bool, AuthError> {
        let row = self
            .client
            .query_opt(
                &format!(
                    "UPDATE {TOTP_ENROLLMENTS_TABLE} SET consumed_at_epoch_s = $3 WHERE enrollment_id = $1 AND account_id = $2 AND consumed_at_epoch_s IS NULL RETURNING encrypted_secret"
                ),
                &[&enrollment_id, &account_id, &(verified_at_epoch_s as i64)],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("activate totp enrollment failed: {err}")))?;
        let Some(row) = row else {
            return Ok(false);
        };
        let encrypted_secret: String = row.get(0);
        self.client
            .execute(
                &format!(
                    "INSERT INTO {TOTP_SECRETS_TABLE} (account_id, encrypted_secret, created_at_epoch_s, verified_at_epoch_s) VALUES ($1, $2, $3, $3) ON CONFLICT (account_id) DO UPDATE SET encrypted_secret = EXCLUDED.encrypted_secret, verified_at_epoch_s = EXCLUDED.verified_at_epoch_s, disabled_at_epoch_s = NULL"
                ),
                &[&account_id, &encrypted_secret, &(verified_at_epoch_s as i64)],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("upsert totp secret failed: {err}")))?;
        Ok(true)
    }

    async fn account_has_verified_totp(&self, account_id: Uuid) -> Result<bool, AuthError> {
        let row = self
            .client
            .query_opt(
                &format!(
                    "SELECT 1 FROM {TOTP_SECRETS_TABLE} WHERE account_id = $1 AND disabled_at_epoch_s IS NULL"
                ),
                &[&account_id],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("totp lookup failed: {err}")))?;
        Ok(row.is_some())
    }

    async fn get_verified_totp_secret(
        &self,
        account_id: Uuid,
    ) -> Result<Option<String>, AuthError> {
        let row = self
            .client
            .query_opt(
                &format!(
                    "SELECT encrypted_secret FROM {TOTP_SECRETS_TABLE} WHERE account_id = $1 AND disabled_at_epoch_s IS NULL"
                ),
                &[&account_id],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("totp secret lookup failed: {err}")))?;
        Ok(row.map(|row| row.get(0)))
    }

    async fn insert_totp_login_challenge(
        &self,
        challenge_id: Uuid,
        account_id: Uuid,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError> {
        let now = now_epoch_s() as i64;
        self.client
            .execute(
                &format!(
                    "INSERT INTO {TOTP_LOGIN_CHALLENGES_TABLE} (challenge_id, account_id, expires_at_epoch_s, created_at_epoch_s) VALUES ($1, $2, $3, $4)"
                ),
                &[&challenge_id, &account_id, &(expires_at_epoch_s as i64), &now],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("insert totp login challenge failed: {err}")))?;
        Ok(())
    }

    async fn get_totp_login_challenge(
        &self,
        challenge_id: Uuid,
    ) -> Result<Option<TotpLoginChallengeRecord>, AuthError> {
        let row = self
            .client
            .query_opt(
                &format!(
                    "SELECT challenge_id, account_id, expires_at_epoch_s FROM {TOTP_LOGIN_CHALLENGES_TABLE} WHERE challenge_id = $1 AND consumed_at_epoch_s IS NULL"
                ),
                &[&challenge_id],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("get totp login challenge failed: {err}")))?;
        Ok(row.map(|row| TotpLoginChallengeRecord {
            challenge_id: row.get(0),
            account_id: row.get(1),
            expires_at_epoch_s: row.get::<usize, i64>(2) as u64,
        }))
    }

    async fn consume_totp_login_challenge(
        &self,
        challenge_id: Uuid,
        account_id: Uuid,
        consumed_at_epoch_s: u64,
    ) -> Result<bool, AuthError> {
        let updated = self
            .client
            .execute(
                &format!(
                    "UPDATE {TOTP_LOGIN_CHALLENGES_TABLE} SET consumed_at_epoch_s = $3 WHERE challenge_id = $1 AND account_id = $2 AND consumed_at_epoch_s IS NULL"
                ),
                &[&challenge_id, &account_id, &(consumed_at_epoch_s as i64)],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("consume totp login challenge failed: {err}")))?;
        Ok(updated > 0)
    }

    async fn insert_email_login_challenge(
        &self,
        challenge_id: Uuid,
        account_id: Uuid,
        code_hash: &str,
        token_hash: &str,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError> {
        let now = now_epoch_s() as i64;
        self.client
            .execute(
                &format!(
                    "INSERT INTO {EMAIL_LOGIN_CHALLENGES_TABLE} (challenge_id, account_id, code_hash, token_hash, expires_at_epoch_s, created_at_epoch_s) VALUES ($1, $2, $3, $4, $5, $6)"
                ),
                &[
                    &challenge_id,
                    &account_id,
                    &code_hash,
                    &token_hash,
                    &(expires_at_epoch_s as i64),
                    &now,
                ],
            )
            .await
            .map_err(|err| {
                AuthError::Internal(format!("insert email login challenge failed: {err}"))
            })?;
        Ok(())
    }

    async fn consume_email_login_challenge_by_code(
        &self,
        challenge_id: Uuid,
        code_hash: &str,
    ) -> Result<Option<EmailLoginChallengeRecord>, AuthError> {
        let now = now_epoch_s() as i64;
        let row = self
            .client
            .query_opt(
                &format!(
                    "UPDATE {EMAIL_LOGIN_CHALLENGES_TABLE} SET consumed_at_epoch_s = $3 WHERE challenge_id = $1 AND code_hash = $2 AND consumed_at_epoch_s IS NULL RETURNING challenge_id, account_id, expires_at_epoch_s"
                ),
                &[&challenge_id, &code_hash, &now],
            )
            .await
            .map_err(|err| {
                AuthError::Internal(format!("consume email login challenge failed: {err}"))
            })?;
        Ok(row.map(|row| EmailLoginChallengeRecord {
            challenge_id: row.get(0),
            account_id: row.get(1),
            expires_at_epoch_s: row.get::<usize, i64>(2) as u64,
        }))
    }

    async fn consume_email_login_challenge_by_token(
        &self,
        challenge_id: Uuid,
        token_hash: &str,
    ) -> Result<Option<EmailLoginChallengeRecord>, AuthError> {
        let now = now_epoch_s() as i64;
        let row = self
            .client
            .query_opt(
                &format!(
                    "UPDATE {EMAIL_LOGIN_CHALLENGES_TABLE} SET consumed_at_epoch_s = $3 WHERE challenge_id = $1 AND token_hash = $2 AND consumed_at_epoch_s IS NULL RETURNING challenge_id, account_id, expires_at_epoch_s"
                ),
                &[&challenge_id, &token_hash, &now],
            )
            .await
            .map_err(|err| {
                AuthError::Internal(format!("consume email login challenge failed: {err}"))
            })?;
        Ok(row.map(|row| EmailLoginChallengeRecord {
            challenge_id: row.get(0),
            account_id: row.get(1),
            expires_at_epoch_s: row.get::<usize, i64>(2) as u64,
        }))
    }

    async fn list_account_characters(
        &self,
        account_id: Uuid,
    ) -> Result<Vec<AccountCharacter>, AuthError> {
        let rows = self
            .client
            .query(
                &format!(
                    "SELECT player_entity_id, display_name, created_at_epoch_s, status FROM {AUTH_CHARACTERS_TABLE} WHERE account_id = $1 AND status = 'active' ORDER BY created_at_epoch_s ASC"
                ),
                &[&account_id],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("list account characters failed: {err}")))?;
        Ok(rows
            .into_iter()
            .map(|row| AccountCharacter {
                player_entity_id: row.get(0),
                display_name: row.get(1),
                created_at_epoch_s: row.get::<usize, i64>(2) as u64,
                status: row.get(3),
            })
            .collect())
    }

    async fn insert_account_character(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
        display_name: &str,
        created_at_epoch_s: u64,
    ) -> Result<AccountCharacter, AuthError> {
        let created_at = created_at_epoch_s as i64;
        self.client
            .execute(
                &format!(
                    "INSERT INTO {AUTH_CHARACTERS_TABLE} (account_id, player_entity_id, created_at_epoch_s, display_name, status, updated_at_epoch_s) VALUES ($1, $2, $3, $4, 'active', $3)"
                ),
                &[&account_id, &player_entity_id, &created_at, &display_name],
            )
            .await
            .map_err(|err| {
                if err.code() == Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION) {
                    AuthError::Conflict("character already exists".to_string())
                } else {
                    AuthError::Internal(format!("insert character failed: {err}"))
                }
            })?;
        Ok(AccountCharacter {
            player_entity_id: player_entity_id.to_string(),
            display_name: display_name.to_string(),
            created_at_epoch_s,
            status: "active".to_string(),
        })
    }

    async fn delete_account_character(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
    ) -> Result<(), AuthError> {
        self.client
            .execute(
                &format!(
                    "DELETE FROM {AUTH_CHARACTERS_TABLE} WHERE account_id = $1 AND player_entity_id = $2"
                ),
                &[&account_id, &player_entity_id],
            )
            .await
            .map_err(|err| AuthError::Internal(format!("delete character failed: {err}")))?;
        Ok(())
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
                    "SELECT 1 FROM {AUTH_CHARACTERS_TABLE} WHERE account_id = $1 AND player_entity_id = $2 AND status = 'active'"
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
    roles_by_account_id: HashMap<Uuid, Vec<String>>,
    scopes_by_account_id: HashMap<Uuid, Vec<String>>,
    characters_by_account_id: HashMap<Uuid, Vec<AccountCharacter>>,
    email_login_challenges_by_id: HashMap<Uuid, InMemoryEmailLoginChallenge>,
    email_delivery_events: Vec<InMemoryEmailDeliveryEvent>,
    totp_enrollments_by_id: HashMap<Uuid, InMemoryTotpEnrollment>,
    totp_secrets_by_account_id: HashMap<Uuid, InMemoryTotpSecret>,
    totp_login_challenges_by_id: HashMap<Uuid, InMemoryTotpLoginChallenge>,
    refresh_tokens_by_hash: HashMap<String, RefreshTokenRecord>,
    password_reset_tokens_by_hash: HashMap<String, PasswordResetTokenRecord>,
    bootstrap_completed: bool,
}

#[derive(Debug, Clone)]
struct InMemoryEmailLoginChallenge {
    account_id: Uuid,
    code_hash: String,
    token_hash: String,
    expires_at_epoch_s: u64,
    consumed: bool,
}

#[derive(Debug, Clone)]
struct InMemoryEmailDeliveryEvent {
    target_hash: String,
    purpose: String,
    created_at_epoch_s: u64,
}

#[derive(Debug, Clone)]
struct InMemoryTotpEnrollment {
    account_id: Uuid,
    encrypted_secret: String,
    expires_at_epoch_s: u64,
    consumed: bool,
}

#[derive(Debug, Clone)]
struct InMemoryTotpSecret {
    encrypted_secret: String,
    verified_at_epoch_s: u64,
    disabled: bool,
}

#[derive(Debug, Clone)]
struct InMemoryTotpLoginChallenge {
    account_id: Uuid,
    expires_at_epoch_s: u64,
    consumed: bool,
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
            player_entity_id: String::new(),
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

    async fn list_account_roles(&self, account_id: Uuid) -> Result<Vec<String>, AuthError> {
        let state = self.state.read().await;
        let mut roles = state
            .roles_by_account_id
            .get(&account_id)
            .cloned()
            .unwrap_or_default();
        roles.sort();
        Ok(roles)
    }

    async fn list_account_scopes(&self, account_id: Uuid) -> Result<Vec<String>, AuthError> {
        let state = self.state.read().await;
        let mut scopes = state
            .scopes_by_account_id
            .get(&account_id)
            .cloned()
            .unwrap_or_default();
        scopes.sort();
        Ok(scopes)
    }

    async fn add_account_role(&self, account_id: Uuid, role: &str) -> Result<(), AuthError> {
        let role = validate_auth_label("role", role)?;
        let mut state = self.state.write().await;
        if !state.accounts_by_id.contains_key(&account_id) {
            return Err(AuthError::Unauthorized("unknown account".to_string()));
        }
        let roles = state.roles_by_account_id.entry(account_id).or_default();
        if !roles.iter().any(|existing| existing == &role) {
            roles.push(role);
        }
        Ok(())
    }

    async fn add_account_scope(&self, account_id: Uuid, scope: &str) -> Result<(), AuthError> {
        let scope = validate_auth_label("scope", scope)?;
        let mut state = self.state.write().await;
        if !state.accounts_by_id.contains_key(&account_id) {
            return Err(AuthError::Unauthorized("unknown account".to_string()));
        }
        let scopes = state.scopes_by_account_id.entry(account_id).or_default();
        if !scopes.iter().any(|existing| existing == &scope) {
            scopes.push(scope);
        }
        Ok(())
    }

    async fn admin_bootstrap_required(&self) -> Result<bool, AuthError> {
        let state = self.state.read().await;
        let has_admin = state.roles_by_account_id.values().any(|roles| {
            roles.iter().any(|role| {
                matches!(
                    role.to_lowercase().as_str(),
                    "admin" | "dev_tool" | "developer"
                )
            })
        });
        Ok(!state.bootstrap_completed && !has_admin)
    }

    async fn create_first_admin_account_atomic(
        &self,
        email: &str,
        password_hash: &str,
        roles: Vec<String>,
        scopes: Vec<String>,
    ) -> Result<Account, AuthError> {
        let roles = roles
            .iter()
            .map(|role| validate_auth_label("role", role))
            .collect::<Result<Vec<_>, _>>()?;
        let scopes = scopes
            .iter()
            .map(|scope| validate_auth_label("scope", scope))
            .collect::<Result<Vec<_>, _>>()?;
        let mut state = self.state.write().await;
        let has_admin = state.roles_by_account_id.values().any(|roles| {
            roles.iter().any(|role| {
                matches!(
                    role.to_lowercase().as_str(),
                    "admin" | "dev_tool" | "developer"
                )
            })
        });
        if state.bootstrap_completed || has_admin {
            return Err(AuthError::Conflict(
                "administrator bootstrap is already complete".to_string(),
            ));
        }
        if state.accounts_by_email.contains_key(email) {
            return Err(AuthError::Conflict("account already exists".to_string()));
        }
        let account_id = Uuid::new_v4();
        let account = Account {
            account_id,
            email: email.to_string(),
            password_hash: password_hash.to_string(),
            player_entity_id: String::new(),
        };
        state
            .accounts_by_email
            .insert(email.to_string(), account.clone());
        state
            .accounts_by_id
            .insert(account.account_id, account.clone());
        state.roles_by_account_id.insert(account_id, roles);
        state.scopes_by_account_id.insert(account_id, scopes);
        state.bootstrap_completed = true;
        Ok(account)
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

    async fn count_email_delivery_events(
        &self,
        target_hash: &str,
        purpose: &str,
        since_epoch_s: u64,
    ) -> Result<u64, AuthError> {
        let state = self.state.read().await;
        Ok(state
            .email_delivery_events
            .iter()
            .filter(|event| {
                event.target_hash == target_hash
                    && event.purpose == purpose
                    && event.created_at_epoch_s >= since_epoch_s
            })
            .count() as u64)
    }

    async fn insert_email_delivery_event(
        &self,
        target_hash: &str,
        purpose: &str,
        created_at_epoch_s: u64,
    ) -> Result<(), AuthError> {
        let mut state = self.state.write().await;
        state
            .email_delivery_events
            .push(InMemoryEmailDeliveryEvent {
                target_hash: target_hash.to_string(),
                purpose: purpose.to_string(),
                created_at_epoch_s,
            });
        Ok(())
    }

    async fn insert_totp_enrollment(
        &self,
        enrollment_id: Uuid,
        account_id: Uuid,
        encrypted_secret: &str,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError> {
        let mut state = self.state.write().await;
        if !state.accounts_by_id.contains_key(&account_id) {
            return Err(AuthError::Unauthorized("unknown account".to_string()));
        }
        state.totp_enrollments_by_id.insert(
            enrollment_id,
            InMemoryTotpEnrollment {
                account_id,
                encrypted_secret: encrypted_secret.to_string(),
                expires_at_epoch_s,
                consumed: false,
            },
        );
        Ok(())
    }

    async fn get_totp_enrollment(
        &self,
        enrollment_id: Uuid,
    ) -> Result<Option<TotpEnrollmentRecord>, AuthError> {
        let state = self.state.read().await;
        let Some(enrollment) = state.totp_enrollments_by_id.get(&enrollment_id) else {
            return Ok(None);
        };
        if enrollment.consumed {
            return Ok(None);
        }
        Ok(Some(TotpEnrollmentRecord {
            enrollment_id,
            account_id: enrollment.account_id,
            encrypted_secret: enrollment.encrypted_secret.clone(),
            expires_at_epoch_s: enrollment.expires_at_epoch_s,
        }))
    }

    async fn activate_totp_enrollment(
        &self,
        enrollment_id: Uuid,
        account_id: Uuid,
        verified_at_epoch_s: u64,
    ) -> Result<bool, AuthError> {
        let mut state = self.state.write().await;
        let Some(enrollment) = state.totp_enrollments_by_id.get_mut(&enrollment_id) else {
            return Ok(false);
        };
        if enrollment.consumed || enrollment.account_id != account_id {
            return Ok(false);
        }
        enrollment.consumed = true;
        let encrypted_secret = enrollment.encrypted_secret.clone();
        state.totp_secrets_by_account_id.insert(
            account_id,
            InMemoryTotpSecret {
                encrypted_secret,
                verified_at_epoch_s,
                disabled: false,
            },
        );
        Ok(true)
    }

    async fn account_has_verified_totp(&self, account_id: Uuid) -> Result<bool, AuthError> {
        let state = self.state.read().await;
        Ok(state
            .totp_secrets_by_account_id
            .get(&account_id)
            .is_some_and(|secret| {
                !secret.disabled
                    && secret.verified_at_epoch_s > 0
                    && !secret.encrypted_secret.is_empty()
            }))
    }

    async fn get_verified_totp_secret(
        &self,
        account_id: Uuid,
    ) -> Result<Option<String>, AuthError> {
        let state = self.state.read().await;
        Ok(state
            .totp_secrets_by_account_id
            .get(&account_id)
            .filter(|secret| !secret.disabled && secret.verified_at_epoch_s > 0)
            .map(|secret| secret.encrypted_secret.clone()))
    }

    async fn insert_totp_login_challenge(
        &self,
        challenge_id: Uuid,
        account_id: Uuid,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError> {
        let mut state = self.state.write().await;
        if !state.accounts_by_id.contains_key(&account_id) {
            return Err(AuthError::Unauthorized("unknown account".to_string()));
        }
        state.totp_login_challenges_by_id.insert(
            challenge_id,
            InMemoryTotpLoginChallenge {
                account_id,
                expires_at_epoch_s,
                consumed: false,
            },
        );
        Ok(())
    }

    async fn get_totp_login_challenge(
        &self,
        challenge_id: Uuid,
    ) -> Result<Option<TotpLoginChallengeRecord>, AuthError> {
        let state = self.state.read().await;
        let Some(challenge) = state.totp_login_challenges_by_id.get(&challenge_id) else {
            return Ok(None);
        };
        if challenge.consumed {
            return Ok(None);
        }
        Ok(Some(TotpLoginChallengeRecord {
            challenge_id,
            account_id: challenge.account_id,
            expires_at_epoch_s: challenge.expires_at_epoch_s,
        }))
    }

    async fn consume_totp_login_challenge(
        &self,
        challenge_id: Uuid,
        account_id: Uuid,
        _consumed_at_epoch_s: u64,
    ) -> Result<bool, AuthError> {
        let mut state = self.state.write().await;
        let Some(challenge) = state.totp_login_challenges_by_id.get_mut(&challenge_id) else {
            return Ok(false);
        };
        if challenge.consumed || challenge.account_id != account_id {
            return Ok(false);
        }
        challenge.consumed = true;
        Ok(true)
    }

    async fn insert_email_login_challenge(
        &self,
        challenge_id: Uuid,
        account_id: Uuid,
        code_hash: &str,
        token_hash: &str,
        expires_at_epoch_s: u64,
    ) -> Result<(), AuthError> {
        let mut state = self.state.write().await;
        state.email_login_challenges_by_id.insert(
            challenge_id,
            InMemoryEmailLoginChallenge {
                account_id,
                code_hash: code_hash.to_string(),
                token_hash: token_hash.to_string(),
                expires_at_epoch_s,
                consumed: false,
            },
        );
        Ok(())
    }

    async fn consume_email_login_challenge_by_code(
        &self,
        challenge_id: Uuid,
        code_hash: &str,
    ) -> Result<Option<EmailLoginChallengeRecord>, AuthError> {
        let mut state = self.state.write().await;
        let Some(challenge) = state.email_login_challenges_by_id.get_mut(&challenge_id) else {
            return Ok(None);
        };
        if challenge.consumed || challenge.code_hash != code_hash {
            return Ok(None);
        }
        challenge.consumed = true;
        Ok(Some(EmailLoginChallengeRecord {
            challenge_id,
            account_id: challenge.account_id,
            expires_at_epoch_s: challenge.expires_at_epoch_s,
        }))
    }

    async fn consume_email_login_challenge_by_token(
        &self,
        challenge_id: Uuid,
        token_hash: &str,
    ) -> Result<Option<EmailLoginChallengeRecord>, AuthError> {
        let mut state = self.state.write().await;
        let Some(challenge) = state.email_login_challenges_by_id.get_mut(&challenge_id) else {
            return Ok(None);
        };
        if challenge.consumed || challenge.token_hash != token_hash {
            return Ok(None);
        }
        challenge.consumed = true;
        Ok(Some(EmailLoginChallengeRecord {
            challenge_id,
            account_id: challenge.account_id,
            expires_at_epoch_s: challenge.expires_at_epoch_s,
        }))
    }

    async fn list_account_characters(
        &self,
        account_id: Uuid,
    ) -> Result<Vec<AccountCharacter>, AuthError> {
        let state = self.state.read().await;
        Ok(state
            .characters_by_account_id
            .get(&account_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|character| character.status == "active")
            .collect())
    }

    async fn insert_account_character(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
        display_name: &str,
        created_at_epoch_s: u64,
    ) -> Result<AccountCharacter, AuthError> {
        let mut state = self.state.write().await;
        if !state.accounts_by_id.contains_key(&account_id) {
            return Err(AuthError::Unauthorized("unknown account".to_string()));
        }
        let characters = state
            .characters_by_account_id
            .entry(account_id)
            .or_default();
        if characters
            .iter()
            .any(|character| character.player_entity_id == player_entity_id)
        {
            return Err(AuthError::Conflict("character already exists".to_string()));
        }
        let character = AccountCharacter {
            player_entity_id: player_entity_id.to_string(),
            display_name: display_name.to_string(),
            created_at_epoch_s,
            status: "active".to_string(),
        };
        characters.push(character.clone());
        Ok(character)
    }

    async fn delete_account_character(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
    ) -> Result<(), AuthError> {
        let mut state = self.state.write().await;
        if let Some(characters) = state.characters_by_account_id.get_mut(&account_id) {
            characters.retain(|character| character.player_entity_id != player_entity_id);
        }
        Ok(())
    }

    async fn account_owns_player_entity(
        &self,
        account_id: Uuid,
        player_entity_id: &str,
    ) -> Result<bool, AuthError> {
        let state = self.state.read().await;
        Ok(state
            .characters_by_account_id
            .get(&account_id)
            .is_some_and(|characters| {
                characters.iter().any(|character| {
                    character.player_entity_id == player_entity_id && character.status == "active"
                })
            }))
    }
}
