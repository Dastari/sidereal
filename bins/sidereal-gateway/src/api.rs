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
use sidereal_asset_runtime::{
    RuntimeAssetCatalogEntry, build_runtime_asset_catalog, materialize_runtime_asset,
};
use sidereal_core::gateway_dtos::{
    AdminSpawnEntityRequest, AdminSpawnEntityResponse, AssetBootstrapManifestEntry,
    AssetBootstrapManifestResponse, AuthTokens, CharacterSummary, CharactersResponse,
    DiscardScriptDraftResponse, EnterWorldRequest, EnterWorldResponse, ListScriptsResponse,
    LoginRequest, MeResponse, PasswordResetConfirmRequest, PasswordResetConfirmResponse,
    PasswordResetRequest, PasswordResetResponse, PublishScriptResponse, RefreshRequest,
    RegisterRequest, ReloadScriptsFromDiskResponse, SaveScriptDraftRequest,
    SaveScriptDraftResponse, ScriptCatalogDocumentDetailDto,
};
use sidereal_scripting::{load_asset_registry_from_root, resolve_scripts_root};
use std::path::PathBuf;
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
        .route("/admin/scripts", get(list_scripts))
        .route(
            "/admin/scripts/reload-from-disk",
            post(reload_scripts_from_disk),
        )
        .route("/admin/scripts/detail/{*script_path}", get(get_script))
        .route(
            "/admin/scripts/draft/{*script_path}",
            post(save_script_draft),
        )
        .route(
            "/admin/scripts/draft/{*script_path}",
            axum::routing::delete(discard_script_draft),
        )
        .route(
            "/admin/scripts/publish/{*script_path}",
            post(publish_script_draft),
        )
        .route("/assets/bootstrap-manifest", get(asset_bootstrap_manifest))
        .route("/assets/{asset_guid}", get(fetch_asset_by_guid))
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

async fn list_scripts(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
) -> Result<Json<ListScriptsResponse>, ApiError> {
    let access_token = extract_bearer_token(&headers)?;
    let scripts = service.list_scripts(access_token).await?;
    Ok(Json(ListScriptsResponse { scripts }))
}

async fn get_script(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
    Path(script_path): Path<String>,
) -> Result<Json<ScriptCatalogDocumentDetailDto>, ApiError> {
    let access_token = extract_bearer_token(&headers)?;
    let Some(script) = service.get_script(access_token, &script_path).await? else {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "unknown script_path"));
    };
    Ok(Json(script))
}

async fn save_script_draft(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
    Path(script_path): Path<String>,
    Json(req): Json<SaveScriptDraftRequest>,
) -> Result<Json<SaveScriptDraftResponse>, ApiError> {
    let access_token = extract_bearer_token(&headers)?;
    service
        .save_script_draft(
            access_token,
            &script_path,
            &req.source,
            req.origin.as_deref(),
            req.family.as_deref(),
        )
        .await?;
    Ok(Json(SaveScriptDraftResponse {
        ok: true,
        script_path,
    }))
}

async fn publish_script_draft(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
    Path(script_path): Path<String>,
) -> Result<Json<PublishScriptResponse>, ApiError> {
    let access_token = extract_bearer_token(&headers)?;
    let Some(published_revision) = service
        .publish_script_draft(access_token, &script_path)
        .await?
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "no draft exists for script_path",
        ));
    };
    Ok(Json(PublishScriptResponse {
        ok: true,
        script_path,
        published_revision,
    }))
}

async fn discard_script_draft(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
    Path(script_path): Path<String>,
) -> Result<Json<DiscardScriptDraftResponse>, ApiError> {
    let access_token = extract_bearer_token(&headers)?;
    let discarded = service
        .discard_script_draft(access_token, &script_path)
        .await?;
    Ok(Json(DiscardScriptDraftResponse {
        ok: true,
        script_path,
        discarded,
    }))
}

async fn reload_scripts_from_disk(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
) -> Result<Json<ReloadScriptsFromDiskResponse>, ApiError> {
    let access_token = extract_bearer_token(&headers)?;
    let script_count = service.reload_scripts_from_disk(access_token).await?;
    Ok(Json(ReloadScriptsFromDiskResponse {
        ok: true,
        script_count,
    }))
}

async fn asset_bootstrap_manifest(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
) -> Result<Json<AssetBootstrapManifestResponse>, ApiError> {
    let access_token = extract_bearer_token(&headers)?;
    let _ = service.me(access_token).await?;
    let catalog = load_runtime_asset_catalog()?;
    let required_ids = catalog
        .iter()
        .filter(|entry| entry.bootstrap_required)
        .map(|entry| entry.asset_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut catalog_entries = catalog
        .iter()
        .map(|entry| AssetBootstrapManifestEntry {
            asset_id: entry.asset_id.clone(),
            asset_guid: entry.asset_guid.clone(),
            sha256_hex: entry.sha256_hex.clone(),
            relative_cache_path: entry.relative_cache_path.clone(),
            content_type: entry.content_type.clone(),
            byte_len: entry.byte_len,
            url: format!("/assets/{}", entry.asset_guid),
        })
        .collect::<Vec<_>>();
    catalog_entries.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    let required_assets = catalog_entries
        .iter()
        .filter(|entry| required_ids.contains(entry.asset_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    Ok(Json(AssetBootstrapManifestResponse {
        catalog_version: "lua-registry-v1".to_string(),
        required_assets,
        catalog: catalog_entries,
    }))
}

async fn fetch_asset_by_guid(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
    Path(asset_guid): Path<String>,
) -> Result<Response, ApiError> {
    let access_token = extract_bearer_token(&headers)?;
    let _ = service.me(access_token).await?;

    let catalog = load_runtime_asset_catalog()?;
    let Some(entry) = catalog.iter().find(|entry| entry.asset_guid == asset_guid) else {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "unknown asset_guid"));
    };
    stream_asset_file(entry).await
}

async fn stream_asset_file(entry: &RuntimeAssetCatalogEntry) -> Result<Response, ApiError> {
    let root = asset_root_dir();
    let materialized = materialize_runtime_asset(root.as_path(), entry).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            ApiError::new(
                StatusCode::NOT_FOUND,
                format!("asset missing on gateway: {}", entry.asset_id),
            )
        } else {
            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
        }
    })?;
    let full_path = materialized.full_path;
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
    Ok((
        [(header::CONTENT_TYPE, materialized.content_type.as_str())],
        body,
    )
        .into_response())
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

fn scripts_root_dir() -> PathBuf {
    resolve_scripts_root(env!("CARGO_MANIFEST_DIR"))
}

fn load_runtime_asset_catalog() -> Result<Vec<RuntimeAssetCatalogEntry>, ApiError> {
    let registry_assets = load_registry_assets()?;
    let asset_root = asset_root_dir();
    build_runtime_asset_catalog(asset_root.as_path(), &registry_assets).map_err(|err| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "failed building runtime asset catalog from {}: {}",
                asset_root.display(),
                err
            ),
        )
    })
}

fn load_registry_assets() -> Result<Vec<sidereal_scripting::ScriptAssetRegistryEntry>, ApiError> {
    let scripts_root = scripts_root_dir();
    let registry = load_asset_registry_from_root(&scripts_root).map_err(|err| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "failed to load lua asset registry from {}: {}",
                scripts_root.display(),
                err
            ),
        )
    })?;
    Ok(registry.assets)
}
