use crate::auth::{
    AuthConfig, AuthError, AuthService, AuthTokens, InMemoryAuthStore, NoopBootstrapDispatcher,
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
use serde::{Deserialize, Serialize};
use sidereal_persistence::GraphPersistence;
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
        .route("/world/me", get(world_me))
        .route("/assets/stream/{asset_id}", get(stream_asset))
        .with_state(service)
}

async fn health() -> StatusCode {
    StatusCode::OK
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct PasswordResetRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct PasswordResetConfirmRequest {
    pub reset_token: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct PasswordResetRequestResponse {
    pub accepted: bool,
    pub reset_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PasswordResetConfirmResponse {
    pub accepted: bool,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub account_id: String,
    pub email: String,
    pub player_entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub player_entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharactersResponse {
    pub characters: Vec<CharacterSummary>,
}

#[derive(Debug, Deserialize)]
pub struct EnterWorldRequest {
    pub player_entity_id: String,
}

#[derive(Debug, Serialize)]
pub struct EnterWorldResponse {
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamAssetDescriptor {
    pub asset_id: String,
    pub relative_cache_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldMeResponse {
    pub player_entity_id: String,
    pub ship_entity_id: String,
    pub ship_name: String,
    pub position_m: [f32; 3],
    pub velocity_mps: [f32; 3],
    pub heading_rad: f32,
    pub health: f32,
    pub max_health: f32,
    pub engine_max_accel_mps2: f32,
    pub engine_ramp_to_max_s: f32,
    pub model_asset_id: String,
    pub assets: Vec<StreamAssetDescriptor>,
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
) -> Result<Json<PasswordResetRequestResponse>, ApiError> {
    let result = service.password_reset_request(&req.email).await?;
    Ok(Json(PasswordResetRequestResponse {
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

async fn world_me(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
) -> Result<Json<WorldMeResponse>, ApiError> {
    let access_token = extract_bearer_token(&headers)?;
    let me = service.me(access_token).await?;
    let player_entity_id = me.player_entity_id.clone();
    let account_id = me.account_id;
    let account_id_s = account_id.to_string();
    let database_url = gateway_database_url();

    let world = tokio::task::spawn_blocking(move || {
        let mut persistence = GraphPersistence::connect(&database_url)
            .map_err(|err| AuthError::Internal(format!("persistence connect failed: {err}")))?;
        persistence.ensure_schema().map_err(|err| {
            AuthError::Internal(format!("persistence ensure schema failed: {err}"))
        })?;
        let records = persistence
            .load_graph_records()
            .map_err(|err| AuthError::Internal(format!("load graph records failed: {err}")))?;
        let ship = records
            .iter()
            .find(|record| {
                record.labels.iter().any(|label| label == "Ship")
                    && record
                        .properties
                        .get("owner_account_id")
                        .and_then(|v| v.as_str())
                        == Some(account_id_s.as_str())
            })
            .ok_or_else(|| {
                AuthError::Unauthorized("no starter ship found for account".to_string())
            })?
            .clone();
        Ok::<_, AuthError>((player_entity_id, ship))
    })
    .await
    .map_err(|err| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))??;

    let (player_entity_id, ship) = world;
    let position_m = parse_vec3_property(&ship.properties, "position_m");
    let velocity_mps = parse_vec3_property(&ship.properties, "velocity_mps");
    let model_asset_id = ship
        .properties
        .get("asset_id")
        .and_then(|v| v.as_str())
        .unwrap_or("corvette_01")
        .to_string();
    let engine_max_accel_mps2 = ship
        .properties
        .get("engine_max_accel_mps2")
        .and_then(|v| v.as_f64())
        .unwrap_or(80.0) as f32;
    let engine_ramp_to_max_s = ship
        .properties
        .get("engine_ramp_to_max_s")
        .and_then(|v| v.as_f64())
        .unwrap_or(5.0) as f32;

    let assets = vec![
        StreamAssetDescriptor {
            asset_id: "corvette_01".to_string(),
            relative_cache_path: "sprites/ships/corvette.png".to_string(),
        },
        StreamAssetDescriptor {
            asset_id: "starfield_wgsl".to_string(),
            relative_cache_path: "shaders/starfield.wgsl".to_string(),
        },
        StreamAssetDescriptor {
            asset_id: "space_background_wgsl".to_string(),
            relative_cache_path: "shaders/simple_space_background.wgsl".to_string(),
        },
        StreamAssetDescriptor {
            asset_id: "sprite_pixel_effect_wgsl".to_string(),
            relative_cache_path: "shaders/sprite_pixel_effect.wgsl".to_string(),
        },
    ];

    Ok(Json(WorldMeResponse {
        player_entity_id,
        ship_entity_id: ship.entity_id,
        ship_name: ship
            .properties
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Corvette")
            .to_string(),
        position_m,
        velocity_mps,
        heading_rad: ship
            .properties
            .get("heading_rad")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32,
        health: ship
            .properties
            .get("health")
            .and_then(|v| v.as_f64())
            .unwrap_or(100.0) as f32,
        max_health: ship
            .properties
            .get("max_health")
            .and_then(|v| v.as_f64())
            .unwrap_or(100.0) as f32,
        engine_max_accel_mps2,
        engine_ramp_to_max_s,
        model_asset_id,
        assets,
    }))
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
    if !full_path.exists() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            format!("asset missing on gateway: {}", full_path.display()),
        ));
    }
    let file = tokio::fs::File::open(&full_path)
        .await
        .map_err(|err| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
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

fn gateway_database_url() -> String {
    std::env::var("GATEWAY_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string())
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
            FsPath::new("shaders/simple_space_background.wgsl"),
            "text/plain; charset=utf-8",
        )),
        "sprite_pixel_effect_wgsl" => Some((
            FsPath::new("shaders/sprite_pixel_effect.wgsl"),
            "text/plain; charset=utf-8",
        )),
        _ => None,
    }
}
