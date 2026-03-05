use crate::auth::{
    AuthConfig, AuthError, AuthService, InMemoryAuthStore, NoopBootstrapDispatcher,
    NoopStarterWorldPersister,
};
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use sidereal_core::gateway_dtos::{
    AdminSpawnEntityRequest, AdminSpawnEntityResponse, AuthTokens, CharacterSummary,
    CharactersResponse, EnterWorldRequest, EnterWorldResponse, LoginRequest, MeResponse,
    PasswordResetConfirmRequest, PasswordResetConfirmResponse, PasswordResetRequest,
    PasswordResetResponse, RefreshRequest, RegisterRequest,
};
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use tokio_util::io::ReaderStream;
use tracing::{error, info, warn};

pub type SharedAuthService = Arc<AuthService>;

pub fn app(config: AuthConfig) -> Router {
    let service = Arc::new(AuthService::new_with_persister(
        config,
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(NoopBootstrapDispatcher),
        Arc::new(NoopStarterWorldPersister),
    ));
    app_with_service(service)
}

pub fn app_with_service(service: SharedAuthService) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        .route("/auth/password-reset/request", post(password_reset_request))
        .route("/auth/password-reset/confirm", post(password_reset_confirm))
        .route("/auth/me", get(me))
        .route("/auth/characters", get(characters))
        .route("/world/enter", post(enter_world))
        .route("/admin/spawn-entity", post(admin_spawn_entity))
        .route("/assets/stream/{asset_id}", get(stream_asset))
        .with_state(service)
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn register(
    State(service): State<SharedAuthService>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthTokens>, ApiError> {
    let tokens = service.register(&req.email, &req.password).await?;
    info!("gateway register succeeded for email={}", req.email);
    Ok(Json(tokens))
}

async fn login(
    State(service): State<SharedAuthService>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthTokens>, ApiError> {
    let tokens = service.login(&req.email, &req.password).await?;
    info!("gateway login succeeded for email={}", req.email);
    Ok(Json(tokens))
}

async fn refresh(
    State(service): State<SharedAuthService>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<AuthTokens>, ApiError> {
    let tokens = service.refresh(&req.refresh_token).await?;
    Ok(Json(tokens))
}

async fn password_reset_request(
    State(service): State<SharedAuthService>,
    Json(req): Json<PasswordResetRequest>,
) -> Result<Json<PasswordResetResponse>, ApiError> {
    let result = service.password_reset_request(&req.email).await?;
    Ok(Json(PasswordResetResponse {
        accepted: result.accepted,
        reset_token: result.reset_token,
    }))
}

async fn password_reset_confirm(
    State(service): State<SharedAuthService>,
    Json(req): Json<PasswordResetConfirmRequest>,
) -> Result<Json<PasswordResetConfirmResponse>, ApiError> {
    service
        .password_reset_confirm(&req.reset_token, &req.new_password)
        .await?;
    Ok(Json(PasswordResetConfirmResponse { accepted: true }))
}

async fn me(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
) -> Result<Json<MeResponse>, ApiError> {
    let access_token = extract_bearer_token(&headers)?;

    let me = service.me(access_token).await?;
    info!(
        "gateway /auth/me resolved player_entity_id={}",
        me.player_entity_id
    );
    Ok(Json(MeResponse {
        account_id: me.account_id.to_string(),
        email: me.email,
        player_entity_id: me.player_entity_id,
    }))
}

async fn characters(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
) -> Result<Json<CharactersResponse>, ApiError> {
    let access_token = extract_bearer_token(&headers)?;
    let characters = service.list_characters(access_token).await?;
    Ok(Json(CharactersResponse {
        characters: characters
            .into_iter()
            .map(|character| CharacterSummary {
                player_entity_id: character.player_entity_id,
            })
            .collect(),
    }))
}

async fn enter_world(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
    Json(req): Json<EnterWorldRequest>,
) -> Result<Json<EnterWorldResponse>, ApiError> {
    let access_token = extract_bearer_token(&headers)?;
    service
        .enter_world(access_token, &req.player_entity_id)
        .await?;
    Ok(Json(EnterWorldResponse { accepted: true }))
}

async fn admin_spawn_entity(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
    Json(req): Json<AdminSpawnEntityRequest>,
) -> Result<Json<AdminSpawnEntityResponse>, ApiError> {
    let access_token = extract_bearer_token(&headers)?;
    let response = service.admin_spawn_entity(access_token, &req).await?;
    Ok(Json(response))
}

async fn stream_asset(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
    Path(asset_id): Path<String>,
) -> Result<Response, ApiError> {
    let access_token = extract_bearer_token(&headers)?;
    let _ = service.me(access_token).await?;

    let root = asset_root_dir();
    let (relative_path, content_type) = resolve_asset_stream_path(&asset_id)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "unknown asset_id"))?;
    let full_path = root.join(relative_path);
    let file = match tokio::fs::File::open(&full_path).await {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(ApiError::new(
                StatusCode::NOT_FOUND,
                format!("asset missing on gateway: {}", full_path.display()),
            ));
        }
        Err(err) => {
            return Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            ));
        }
    };
    let stream = ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);
    Ok(([(header::CONTENT_TYPE, content_type)], body).into_response())
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message)
    }
}

impl From<AuthError> for ApiError {
    fn from(value: AuthError) -> Self {
        match value {
            AuthError::Validation(message) => Self::new(StatusCode::BAD_REQUEST, message),
            AuthError::Unauthorized(message) => Self::new(StatusCode::UNAUTHORIZED, message),
            AuthError::Conflict(message) => Self::new(StatusCode::CONFLICT, message),
            AuthError::Config(message) => Self::new(StatusCode::INTERNAL_SERVER_ERROR, message),
            AuthError::Internal(message) => Self::new(StatusCode::INTERNAL_SERVER_ERROR, message),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            error!(
                "gateway API error status={} message={}",
                self.status.as_u16(),
                self.message
            );
        } else if self.status.is_client_error() {
            warn!(
                "gateway API client failure status={} message={}",
                self.status.as_u16(),
                self.message
            );
        }
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

fn extract_bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .ok_or_else(|| ApiError::unauthorized("missing authorization header"))?;
    let auth_header_str = auth_header
        .to_str()
        .map_err(|_| ApiError::unauthorized("invalid authorization header"))?;
    auth_header_str
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::unauthorized("expected Bearer token"))
}

#[doc(hidden)]
pub fn parse_vec3_property(props: &serde_json::Value, key: &str) -> [f32; 3] {
    let Some(values) = props.get(key).and_then(|v| v.as_array()) else {
        return [0.0, 0.0, 0.0];
    };
    if values.len() != 3 {
        return [0.0, 0.0, 0.0];
    }
    [
        values[0].as_f64().unwrap_or_default() as f32,
        values[1].as_f64().unwrap_or_default() as f32,
        values[2].as_f64().unwrap_or_default() as f32,
    ]
}

fn asset_root_dir() -> PathBuf {
    PathBuf::from(std::env::var("ASSET_ROOT").unwrap_or_else(|_| "./data".to_string()))
}

#[doc(hidden)]
pub fn resolve_asset_stream_path(asset_id: &str) -> Option<(&'static FsPath, &'static str)> {
    match asset_id {
        "corvette_01" => Some((FsPath::new("sprites/ships/corvette.png"), "image/png")),
        "starfield_wgsl" => Some((
            FsPath::new("shaders/starfield.wgsl"),
            "text/plain; charset=utf-8",
        )),
        "space_background_wgsl" => Some((
            FsPath::new("shaders/space_background.wgsl"),
            "text/plain; charset=utf-8",
        )),
        "sprite_pixel_effect_wgsl" => Some((
            FsPath::new("shaders/sprite_pixel_effect.wgsl"),
            "text/plain; charset=utf-8",
        )),
        _ => None,
    }
}
