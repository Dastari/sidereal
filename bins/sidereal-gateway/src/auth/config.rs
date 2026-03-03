use crate::auth::error::AuthError;

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
