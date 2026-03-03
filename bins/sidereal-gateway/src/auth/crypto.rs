use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::auth::error::AuthError;

const MIN_PASSWORD_LEN: usize = 12;

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

pub fn generate_opaque_token() -> String {
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

pub fn now_epoch_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs()
}
