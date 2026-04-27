use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use qrcode::QrCode;
use qrcode::render::svg;
use rand::RngCore;
use sha1::Sha1;

use crate::auth::error::AuthError;

type HmacSha1 = Hmac<Sha1>;

pub fn generate_totp_secret() -> Vec<u8> {
    let mut secret = vec![0_u8; 20];
    rand::rng().fill_bytes(&mut secret);
    secret
}

pub fn manual_secret(secret: &[u8]) -> String {
    BASE32_NOPAD.encode(secret)
}

pub fn provisioning_uri(
    issuer: &str,
    email: &str,
    secret: &[u8],
    digits: u32,
    step_s: u64,
) -> String {
    let issuer = issuer.trim();
    let label = format!("{issuer}:{}", email.trim());
    format!(
        "otpauth://totp/{}?secret={}&issuer={}&algorithm=SHA1&digits={digits}&period={step_s}",
        percent_encode(&label),
        manual_secret(secret),
        percent_encode(issuer)
    )
}

pub fn qr_svg(provisioning_uri: &str) -> Result<String, AuthError> {
    let code = QrCode::new(provisioning_uri.as_bytes())
        .map_err(|err| AuthError::Internal(format!("totp QR generation failed: {err}")))?;
    Ok(code
        .render::<svg::Color<'_>>()
        .min_dimensions(256, 256)
        .build())
}

pub fn encrypt_secret(secret: &[u8], key_bytes: &[u8; 32]) -> Result<String, AuthError> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key_bytes));
    let mut nonce_bytes = [0_u8; 12];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), secret)
        .map_err(|_| AuthError::Internal("totp secret encryption failed".to_string()))?;
    let mut payload = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
    payload.extend_from_slice(&nonce_bytes);
    payload.extend_from_slice(&ciphertext);
    Ok(URL_SAFE_NO_PAD.encode(payload))
}

pub fn decrypt_secret(encrypted_secret: &str, key_bytes: &[u8; 32]) -> Result<Vec<u8>, AuthError> {
    let payload = URL_SAFE_NO_PAD
        .decode(encrypted_secret)
        .map_err(|_| AuthError::Internal("totp secret payload is invalid".to_string()))?;
    if payload.len() <= 12 {
        return Err(AuthError::Internal(
            "totp secret payload is too short".to_string(),
        ));
    }
    let (nonce, ciphertext) = payload.split_at(12);
    ChaCha20Poly1305::new(Key::from_slice(key_bytes))
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| AuthError::Internal("totp secret decryption failed".to_string()))
}

pub fn verify_totp_code(
    secret: &[u8],
    code: &str,
    now_epoch_s: u64,
    step_s: u64,
    digits: u32,
    allowed_drift_steps: i64,
) -> Result<bool, AuthError> {
    let code = code.trim();
    if code.len() != digits as usize || !code.chars().all(|ch| ch.is_ascii_digit()) {
        return Ok(false);
    }
    let current_step = (now_epoch_s / step_s) as i64;
    for offset in -allowed_drift_steps..=allowed_drift_steps {
        let step = current_step + offset;
        if step < 0 {
            continue;
        }
        if totp_code(secret, step as u64, digits)? == code {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn totp_code(secret: &[u8], counter: u64, digits: u32) -> Result<String, AuthError> {
    let mut mac = <HmacSha1 as Mac>::new_from_slice(secret)
        .map_err(|_| AuthError::Internal("totp hmac key invalid".to_string()))?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = ((u32::from(digest[offset]) & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    let modulus = 10_u32.pow(digits);
    Ok(format!(
        "{:0width$}",
        binary % modulus,
        width = digits as usize
    ))
}

fn percent_encode(raw: &str) -> String {
    let mut encoded = String::new();
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
