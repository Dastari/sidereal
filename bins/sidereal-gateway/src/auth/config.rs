use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};

use crate::auth::error::AuthError;

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub access_token_ttl_s: u64,
    pub refresh_token_ttl_s: u64,
    pub reset_token_ttl_s: u64,
    pub email_challenge_ttl_s: u64,
    pub email_resend_cooldown_s: u64,
    pub email_max_per_email_per_hour: u64,
    pub public_base_url: String,
    pub auth_secret_key: [u8; 32],
    pub totp_issuer: String,
    pub totp_step_s: u64,
    pub totp_digits: u32,
    pub totp_allowed_drift_steps: i64,
    pub totp_enrollment_ttl_s: u64,
    pub totp_login_challenge_ttl_s: u64,
    pub bootstrap_token: Option<String>,
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
        let email_challenge_ttl_s = parse_ttl_env("GATEWAY_EMAIL_CHALLENGE_TTL_S", 600)?;
        let email_resend_cooldown_s = parse_ttl_env("GATEWAY_EMAIL_RESEND_COOLDOWN_S", 60)?;
        let email_max_per_email_per_hour =
            parse_ttl_env("GATEWAY_EMAIL_MAX_PER_EMAIL_PER_HOUR", 5)?;
        let public_base_url = std::env::var("GATEWAY_PUBLIC_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:3000".to_string())
            .trim_end_matches('/')
            .to_string();
        let auth_secret_key = match std::env::var("GATEWAY_AUTH_SECRET_KEY_B64") {
            Ok(raw) => parse_auth_secret_key(&raw)?,
            Err(_) => derive_compat_auth_secret_key(&jwt_secret),
        };
        let totp_issuer =
            std::env::var("GATEWAY_TOTP_ISSUER").unwrap_or_else(|_| "Sidereal".to_string());
        let totp_step_s = parse_ttl_env("GATEWAY_TOTP_STEP_S", 30)?;
        let totp_digits = parse_u32_env("GATEWAY_TOTP_DIGITS", 6)?;
        let totp_allowed_drift_steps = parse_i64_env("GATEWAY_TOTP_ALLOWED_DRIFT_STEPS", 1)?;
        let totp_enrollment_ttl_s = parse_ttl_env("GATEWAY_TOTP_ENROLLMENT_TTL_S", 600)?;
        let totp_login_challenge_ttl_s = parse_ttl_env("GATEWAY_TOTP_LOGIN_CHALLENGE_TTL_S", 300)?;
        let bootstrap_token = std::env::var("GATEWAY_BOOTSTRAP_TOKEN")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        Ok(Self {
            jwt_secret,
            access_token_ttl_s,
            refresh_token_ttl_s,
            reset_token_ttl_s,
            email_challenge_ttl_s,
            email_resend_cooldown_s,
            email_max_per_email_per_hour,
            public_base_url,
            auth_secret_key,
            totp_issuer,
            totp_step_s,
            totp_digits,
            totp_allowed_drift_steps,
            totp_enrollment_ttl_s,
            totp_login_challenge_ttl_s,
            bootstrap_token,
        })
    }

    pub fn for_tests() -> Self {
        Self {
            jwt_secret: "0123456789abcdef0123456789abcdef".to_string(),
            access_token_ttl_s: 900,
            refresh_token_ttl_s: 3_600,
            reset_token_ttl_s: 900,
            email_challenge_ttl_s: 600,
            email_resend_cooldown_s: 60,
            email_max_per_email_per_hour: 5,
            public_base_url: "http://localhost:3000".to_string(),
            auth_secret_key: [7_u8; 32],
            totp_issuer: "Sidereal".to_string(),
            totp_step_s: 30,
            totp_digits: 6,
            totp_allowed_drift_steps: 1,
            totp_enrollment_ttl_s: 600,
            totp_login_challenge_ttl_s: 300,
            bootstrap_token: Some("test-bootstrap-token".to_string()),
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

fn parse_u32_env(name: &str, default_value: u32) -> Result<u32, AuthError> {
    match std::env::var(name) {
        Ok(raw) => raw
            .parse::<u32>()
            .map_err(|_| AuthError::Config(format!("{name} must be a positive integer"))),
        Err(_) => Ok(default_value),
    }
}

fn parse_i64_env(name: &str, default_value: i64) -> Result<i64, AuthError> {
    match std::env::var(name) {
        Ok(raw) => raw
            .parse::<i64>()
            .map_err(|_| AuthError::Config(format!("{name} must be an integer"))),
        Err(_) => Ok(default_value),
    }
}

fn parse_auth_secret_key(raw: &str) -> Result<[u8; 32], AuthError> {
    let bytes = STANDARD.decode(raw.trim()).map_err(|_| {
        AuthError::Config("GATEWAY_AUTH_SECRET_KEY_B64 must be valid base64".to_string())
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        AuthError::Config(format!(
            "GATEWAY_AUTH_SECRET_KEY_B64 must decode to 32 bytes, got {}",
            bytes.len()
        ))
    })
}

fn derive_compat_auth_secret_key(jwt_secret: &str) -> [u8; 32] {
    let digest = Sha256::digest(jwt_secret.as_bytes());
    let mut key = [0_u8; 32];
    key.copy_from_slice(&digest);
    key
}
