use crate::auth::{
    AuthConfig, AuthError, AuthService, InMemoryAuthStore, NoopBootstrapDispatcher,
    NoopStarterWorldPersister, ScriptCatalogResource, current_script_catalog, scripts_root_dir,
};
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::Method;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use sidereal_asset_runtime::{
    RuntimeAssetCatalogEntry, build_runtime_asset_catalog, catalog_version, expand_required_assets,
    hot_reload_poll_interval, materialize_runtime_asset,
};
use sidereal_audio::{AudioRegistry, audio_registry_version};
use sidereal_core::gateway_dtos::{
    AdminSpawnEntityRequest, AdminSpawnEntityResponse, AssetBootstrapManifestEntry,
    AssetBootstrapManifestResponse, AuthTokens, CharacterSummary, CharactersResponse,
    DiscardScriptDraftResponse, EnterWorldRequest, EnterWorldResponse, ListScriptsResponse,
    LoginRequest, MeResponse, PasswordResetConfirmRequest, PasswordResetConfirmResponse,
    PasswordResetRequest, PasswordResetResponse, PublishScriptResponse, RefreshRequest,
    RegisterRequest, ReloadScriptsFromDiskResponse, ReplicationTransportConfig,
    SaveScriptDraftRequest, SaveScriptDraftResponse, ScriptCatalogDocumentDetailDto,
};
use sidereal_scripting::{load_asset_registry_from_source, load_audio_registry_from_source};
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tokio_util::io::ReaderStream;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{error, info, warn};

pub type SharedAuthService = Arc<AuthService>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct RuntimeAssetCatalogCacheState {
    scripts_root: String,
    asset_root: String,
    asset_registry_revision: u64,
    catalog: Vec<RuntimeAssetCatalogEntry>,
    built_at: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct RuntimeAudioCatalogCacheState {
    scripts_root: String,
    audio_registry_revision: u64,
    version: String,
    registry: AudioRegistry,
    built_at: Option<Instant>,
}

static RUNTIME_ASSET_CATALOG_CACHE: OnceLock<Mutex<RuntimeAssetCatalogCacheState>> =
    OnceLock::new();
static RUNTIME_AUDIO_CATALOG_CACHE: OnceLock<Mutex<RuntimeAudioCatalogCacheState>> =
    OnceLock::new();

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
        .layer(browser_cors_layer())
        .with_state(service)
}

fn browser_cors_layer() -> CorsLayer {
    let allowed_origins = allowed_browser_origins();
    if allowed_origins.is_empty() {
        return CorsLayer::new();
    }

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
}

fn allowed_browser_origins() -> Vec<HeaderValue> {
    let configured = std::env::var("GATEWAY_ALLOWED_ORIGINS")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            vec![
                "http://localhost:3000".to_string(),
                "http://127.0.0.1:3000".to_string(),
            ]
        });

    configured
        .into_iter()
        .filter_map(|origin| match HeaderValue::from_str(&origin) {
            Ok(value) => Some(value),
            Err(err) => {
                warn!(
                    "ignoring invalid GATEWAY_ALLOWED_ORIGINS entry origin={} err={}",
                    origin, err
                );
                None
            }
        })
        .collect()
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
    Ok(Json(EnterWorldResponse {
        accepted: true,
        replication_transport: replication_transport_config_from_env(),
    }))
}

fn replication_transport_config_from_env() -> ReplicationTransportConfig {
    let udp_addr = std::env::var("REPLICATION_UDP_PUBLIC_ADDR")
        .ok()
        .or_else(|| std::env::var("REPLICATION_UDP_BIND").ok());
    let webtransport_addr = std::env::var("REPLICATION_WEBTRANSPORT_PUBLIC_ADDR")
        .ok()
        .or_else(|| std::env::var("REPLICATION_WEBTRANSPORT_BIND").ok());
    let webtransport_certificate_sha256 =
        std::env::var("REPLICATION_WEBTRANSPORT_CERT_SHA256").ok();
    ReplicationTransportConfig {
        udp_addr,
        webtransport_addr,
        webtransport_certificate_sha256,
    }
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
    let catalog = load_runtime_asset_catalog_async().await?;
    let (audio_catalog_version, audio_catalog) = load_runtime_audio_catalog_async().await?;
    let required_ids = catalog
        .iter()
        .filter(|entry| entry.bootstrap_required)
        .map(|entry| entry.asset_id.clone())
        .collect::<std::collections::HashSet<_>>();
    let dependencies_by_asset_id = catalog
        .iter()
        .map(|entry| (entry.asset_id.clone(), entry.dependencies.clone()))
        .collect::<std::collections::HashMap<_, _>>();
    let required_ids = expand_required_assets(&required_ids, &dependencies_by_asset_id);
    let mut catalog_entries = catalog
        .iter()
        .map(|entry| AssetBootstrapManifestEntry {
            asset_id: entry.asset_id.clone(),
            asset_guid: entry.asset_guid.clone(),
            shader_family: entry.shader_family.clone(),
            dependencies: entry.dependencies.clone(),
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
        .filter(|entry| required_ids.contains(&entry.asset_id))
        .cloned()
        .collect::<Vec<_>>();
    Ok(Json(AssetBootstrapManifestResponse {
        catalog_version: catalog_version(&catalog),
        audio_catalog_version,
        required_assets,
        catalog: catalog_entries,
        audio_catalog,
    }))
}

async fn fetch_asset_by_guid(
    State(service): State<SharedAuthService>,
    headers: HeaderMap,
    Path(asset_guid): Path<String>,
) -> Result<Response, ApiError> {
    let access_token = extract_bearer_token(&headers)?;
    let _ = service.me(access_token).await?;

    let catalog = load_runtime_asset_catalog_async().await?;
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

async fn load_runtime_asset_catalog_async() -> Result<Vec<RuntimeAssetCatalogEntry>, ApiError> {
    tokio::task::spawn_blocking(load_runtime_asset_catalog)
        .await
        .map_err(|err| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("runtime asset catalog task failed: {err}"),
            )
        })?
}

async fn load_runtime_audio_catalog_async() -> Result<(String, AudioRegistry), ApiError> {
    tokio::task::spawn_blocking(load_runtime_audio_catalog)
        .await
        .map_err(|err| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("runtime audio catalog task failed: {err}"),
            )
        })?
}

fn load_runtime_asset_catalog() -> Result<Vec<RuntimeAssetCatalogEntry>, ApiError> {
    let asset_root = asset_root_dir();
    let scripts_root = scripts_root_dir();
    let script_catalog = current_script_catalog(&scripts_root)?;
    let cache = RUNTIME_ASSET_CATALOG_CACHE
        .get_or_init(|| Mutex::new(RuntimeAssetCatalogCacheState::default()));
    let mut guard = cache.lock().map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "runtime asset catalog cache lock poisoned",
        )
    })?;
    load_runtime_asset_catalog_from_catalog(&script_catalog, asset_root.as_path(), &mut guard)
}

fn load_runtime_audio_catalog() -> Result<(String, AudioRegistry), ApiError> {
    let scripts_root = scripts_root_dir();
    let script_catalog = current_script_catalog(&scripts_root)?;
    let cache = RUNTIME_AUDIO_CATALOG_CACHE
        .get_or_init(|| Mutex::new(RuntimeAudioCatalogCacheState::default()));
    let mut guard = cache.lock().map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "runtime audio catalog cache lock poisoned",
        )
    })?;
    load_runtime_audio_catalog_from_catalog(&script_catalog, &mut guard)
}

fn load_runtime_asset_catalog_from_catalog(
    script_catalog: &ScriptCatalogResource,
    asset_root: &FsPath,
    cache_state: &mut RuntimeAssetCatalogCacheState,
) -> Result<Vec<RuntimeAssetCatalogEntry>, ApiError> {
    let registry_entry = script_catalog
        .entries
        .iter()
        .find(|entry| entry.script_path == "assets/registry.lua")
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "script catalog missing assets/registry.lua for root {}",
                    script_catalog.root_dir
                ),
            )
        })?;
    let scripts_root = script_catalog.root_dir.clone();
    let asset_root_display = asset_root.display().to_string();
    if cache_state.scripts_root == scripts_root
        && cache_state.asset_root == asset_root_display
        && cache_state.asset_registry_revision == registry_entry.revision
        && cache_state
            .built_at
            .is_some_and(|built_at| built_at.elapsed() < hot_reload_poll_interval())
    {
        return Ok(cache_state.catalog.clone());
    }

    let runtime_catalog =
        build_runtime_asset_catalog_from_registry_source(&registry_entry.source, asset_root)?;
    *cache_state = RuntimeAssetCatalogCacheState {
        scripts_root,
        asset_root: asset_root_display,
        asset_registry_revision: registry_entry.revision,
        catalog: runtime_catalog.clone(),
        built_at: Some(Instant::now()),
    };
    Ok(runtime_catalog)
}

fn build_runtime_asset_catalog_from_registry_source(
    registry_source: &str,
    asset_root: &FsPath,
) -> Result<Vec<RuntimeAssetCatalogEntry>, ApiError> {
    let registry =
        load_asset_registry_from_source(registry_source, FsPath::new("assets/registry.lua"))
            .map_err(|err| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to load active lua asset registry: {err}"),
                )
            })?;
    build_runtime_asset_catalog(asset_root, &registry.assets).map_err(|err| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "failed building runtime asset catalog from {}: {}",
                asset_root.display(),
                err,
            ),
        )
    })
}

fn load_runtime_audio_catalog_from_catalog(
    script_catalog: &ScriptCatalogResource,
    cache_state: &mut RuntimeAudioCatalogCacheState,
) -> Result<(String, AudioRegistry), ApiError> {
    let registry_entry = script_catalog
        .entries
        .iter()
        .find(|entry| entry.script_path == "audio/registry.lua")
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "script catalog missing audio/registry.lua for root {}",
                    script_catalog.root_dir
                ),
            )
        })?;
    let scripts_root = script_catalog.root_dir.clone();
    if cache_state.scripts_root == scripts_root
        && cache_state.audio_registry_revision == registry_entry.revision
        && cache_state
            .built_at
            .is_some_and(|built_at| built_at.elapsed() < hot_reload_poll_interval())
    {
        return Ok((cache_state.version.clone(), cache_state.registry.clone()));
    }

    let registry =
        load_audio_registry_from_source(&registry_entry.source, FsPath::new("audio/registry.lua"))
            .map_err(|err| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to load active lua audio registry: {err}"),
                )
            })?;
    let version = audio_registry_version(&registry).map_err(|err| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to version audio registry: {err}"),
        )
    })?;
    *cache_state = RuntimeAudioCatalogCacheState {
        scripts_root,
        audio_registry_revision: registry_entry.revision,
        version: version.clone(),
        registry: registry.clone(),
        built_at: Some(Instant::now()),
    };
    Ok((version, registry))
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeAssetCatalogCacheState, RuntimeAudioCatalogCacheState,
        load_runtime_asset_catalog_from_catalog, load_runtime_audio_catalog_from_catalog,
        parse_vec3_property,
    };
    use crate::auth::{ScriptCatalogEntry, ScriptCatalogResource};
    use std::path::PathBuf;

    fn temp_asset_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "sidereal_gateway_asset_api_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn test_asset_registry_source() -> String {
        r#"
return {
  schema_version = 1,
  assets = {
    {
      asset_id = "shader.main",
      shader_family = "world_sprite_generic",
      source_path = "shaders/main.wgsl",
      content_type = "text/wgsl",
      dependencies = {},
      bootstrap_required = true,
    },
  },
}
"#
        .to_string()
    }

    fn test_audio_registry_source(bus_volume_db: f32) -> String {
        format!(
            r#"
return {{
  schema_version = 1,
  buses = {{
    {{
      bus_id = "music",
      parent = "master",
      default_volume_db = {bus_volume_db},
    }},
  }},
  sends = {{}},
  environments = {{}},
  concurrency_groups = {{}},
  profiles = {{
    {{
      profile_id = "music.menu.standard",
      kind = "music",
      cues = {{
        main = {{
          playback = {{
            kind = "loop",
            clip_asset_id = "audio.music.menu_loop",
          }},
          route = {{
            bus = "music",
          }},
          spatial = {{
            mode = "screen_nonpositional",
          }},
        }},
      }},
    }},
  }},
}}
"#
        )
    }

    #[test]
    fn parse_vec3_property_returns_zero_for_invalid_inputs() {
        let value = serde_json::json!({ "position": [1.0, 2.0] });
        assert_eq!(parse_vec3_property(&value, "position"), [0.0, 0.0, 0.0]);
        assert_eq!(
            parse_vec3_property(&serde_json::json!({}), "missing"),
            [0.0, 0.0, 0.0]
        );
    }

    #[test]
    fn runtime_asset_catalog_cache_reuses_built_catalog_until_revision_changes() {
        let asset_root = temp_asset_root();
        let shader_path = asset_root.join("shaders/main.wgsl");
        std::fs::create_dir_all(shader_path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&shader_path, "first").expect("write first shader payload");

        let make_catalog = |revision| ScriptCatalogResource {
            entries: vec![ScriptCatalogEntry {
                script_path: "assets/registry.lua".to_string(),
                source: test_asset_registry_source(),
                revision,
                origin: "test".to_string(),
            }],
            revision,
            root_dir: "/tmp/test-scripts".to_string(),
        };

        let mut cache_state = RuntimeAssetCatalogCacheState::default();
        let first_catalog = load_runtime_asset_catalog_from_catalog(
            &make_catalog(1),
            &asset_root,
            &mut cache_state,
        )
        .expect("build initial runtime asset catalog");
        assert_eq!(first_catalog.len(), 1);
        let first_sha = first_catalog[0].sha256_hex.clone();

        std::fs::write(&shader_path, "second").expect("write second shader payload");
        let cached_catalog = load_runtime_asset_catalog_from_catalog(
            &make_catalog(1),
            &asset_root,
            &mut cache_state,
        )
        .expect("reuse cached runtime asset catalog");
        assert_eq!(cached_catalog[0].sha256_hex, first_sha);

        let rebuilt_catalog = load_runtime_asset_catalog_from_catalog(
            &make_catalog(2),
            &asset_root,
            &mut cache_state,
        )
        .expect("rebuild runtime asset catalog after revision change");
        assert_ne!(rebuilt_catalog[0].sha256_hex, first_sha);

        let _ = std::fs::remove_dir_all(&asset_root);
    }

    #[test]
    fn runtime_audio_catalog_cache_reuses_registry_until_revision_changes() {
        let make_catalog = |revision, volume_db| ScriptCatalogResource {
            entries: vec![ScriptCatalogEntry {
                script_path: "audio/registry.lua".to_string(),
                source: test_audio_registry_source(volume_db),
                revision,
                origin: "test".to_string(),
            }],
            revision,
            root_dir: "/tmp/test-scripts".to_string(),
        };

        let mut cache_state = RuntimeAudioCatalogCacheState::default();
        let (first_version, first_registry) =
            load_runtime_audio_catalog_from_catalog(&make_catalog(1, -4.0), &mut cache_state)
                .expect("build initial runtime audio catalog");
        assert_eq!(first_registry.profiles.len(), 1);

        let (cached_version, cached_registry) =
            load_runtime_audio_catalog_from_catalog(&make_catalog(1, -8.0), &mut cache_state)
                .expect("reuse cached runtime audio catalog");
        assert_eq!(cached_version, first_version);
        assert_eq!(cached_registry.buses[0].default_volume_db, Some(-4.0));

        let (rebuilt_version, rebuilt_registry) =
            load_runtime_audio_catalog_from_catalog(&make_catalog(2, -8.0), &mut cache_state)
                .expect("rebuild runtime audio catalog after revision change");
        assert_ne!(rebuilt_version, first_version);
        assert_eq!(rebuilt_registry.buses[0].default_volume_db, Some(-8.0));
    }
}
