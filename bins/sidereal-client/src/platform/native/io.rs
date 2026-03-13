//! Native gateway HTTP and cache/filesystem boundary helpers.

use reqwest::blocking::{Client, Response};
use serde::Serialize;
use sidereal_asset_runtime::{
    AssetCacheIndex, cache_index_path, load_cache_index, save_cache_index, sha256_hex,
};
use sidereal_core::gateway_dtos::{
    AssetBootstrapManifestResponse, AuthTokens, CharactersResponse, EnterWorldRequest,
    EnterWorldResponse, LoginRequest, MeResponse, PasswordResetConfirmRequest,
    PasswordResetConfirmResponse, PasswordResetRequest, PasswordResetResponse, RegisterRequest,
};

use crate::runtime::{AssetCacheAdapter, CacheFuture, GatewayFuture, GatewayHttpAdapter};

fn decode_api_json<T: serde::de::DeserializeOwned>(response: Response) -> Result<T, String> {
    let status = response.status();
    let body = response.text().map_err(|err| err.to_string())?;
    if !status.is_success() {
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&body)
            && let Some(message) = error_json.get("error").and_then(|v| v.as_str())
        {
            return Err(format!("{status}: {message}"));
        }
        if body.trim().is_empty() {
            return Err(status.to_string());
        }
        return Err(format!("{status}: {body}"));
    }
    serde_json::from_str::<T>(&body).map_err(|err| err.to_string())
}

fn client() -> Client {
    Client::new()
}

pub(super) fn get_json<T: serde::de::DeserializeOwned>(
    url: String,
    bearer_token: Option<&str>,
) -> Result<T, String> {
    let mut request = client().get(url);
    if let Some(token) = bearer_token {
        request = request.bearer_auth(token);
    }
    let response = request.send().map_err(|err| err.to_string())?;
    decode_api_json(response)
}

pub(super) fn post_json<Request, ResponseBody>(
    url: String,
    bearer_token: Option<&str>,
    payload: &Request,
) -> Result<ResponseBody, String>
where
    Request: Serialize,
    ResponseBody: serde::de::DeserializeOwned,
{
    let mut request = client().post(url).json(payload);
    if let Some(token) = bearer_token {
        request = request.bearer_auth(token);
    }
    let response = request.send().map_err(|err| err.to_string())?;
    decode_api_json(response)
}

pub(super) fn get_bytes(url: String, bearer_token: Option<&str>) -> Result<Vec<u8>, String> {
    let mut request = client().get(url);
    if let Some(token) = bearer_token {
        request = request.bearer_auth(token);
    }
    request
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .bytes()
        .map(|bytes| bytes.to_vec())
        .map_err(|err| err.to_string())
}

pub(super) fn prepare_cache_root(asset_root: &str) -> Result<(), String> {
    let cache_root = std::path::PathBuf::from(asset_root).join("data/cache_stream");
    std::fs::create_dir_all(&cache_root).map_err(|err| {
        format!(
            "Failed to prepare asset cache directory {}: {}",
            cache_root.display(),
            err
        )
    })
}

pub(super) fn load_cache_index_for_asset_root(asset_root: &str) -> AssetCacheIndex {
    let cache_index_file = cache_index_path(asset_root);
    load_cache_index(&cache_index_file).unwrap_or_default()
}

pub(super) fn save_cache_index_for_asset_root(
    asset_root: &str,
    cache_index: &AssetCacheIndex,
) -> Result<(), String> {
    let cache_index_file = cache_index_path(asset_root);
    save_cache_index(&cache_index_file, cache_index).map_err(|err| err.to_string())
}

pub(super) fn read_cached_asset_if_valid(
    asset_root: &str,
    relative_cache_path: &str,
    expected_sha256: &str,
) -> Option<Vec<u8>> {
    let target = std::path::PathBuf::from(asset_root)
        .join("data/cache_stream")
        .join(relative_cache_path);
    if !target.is_file() {
        return None;
    }
    let bytes = std::fs::read(target).ok()?;
    (sha256_hex(&bytes) == expected_sha256).then_some(bytes)
}

pub(super) fn write_cached_asset(
    asset_root: &str,
    relative_cache_path: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let target = std::path::PathBuf::from(asset_root)
        .join("data/cache_stream")
        .join(relative_cache_path);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::write(&target, bytes).map_err(|err| err.to_string())
}

fn native_login(gateway_url: String, payload: LoginRequest) -> Result<AuthTokens, String> {
    post_json(format!("{gateway_url}/auth/login"), None, &payload)
}

fn native_login_async(gateway_url: String, payload: LoginRequest) -> GatewayFuture<AuthTokens> {
    Box::pin(async move { native_login(gateway_url, payload) })
}

fn native_register(gateway_url: String, payload: RegisterRequest) -> Result<AuthTokens, String> {
    post_json(format!("{gateway_url}/auth/register"), None, &payload)
}

fn native_register_async(
    gateway_url: String,
    payload: RegisterRequest,
) -> GatewayFuture<AuthTokens> {
    Box::pin(async move { native_register(gateway_url, payload) })
}

fn native_request_password_reset(
    gateway_url: String,
    payload: PasswordResetRequest,
) -> Result<PasswordResetResponse, String> {
    post_json(
        format!("{gateway_url}/auth/password-reset/request"),
        None,
        &payload,
    )
}

fn native_request_password_reset_async(
    gateway_url: String,
    payload: PasswordResetRequest,
) -> GatewayFuture<PasswordResetResponse> {
    Box::pin(async move { native_request_password_reset(gateway_url, payload) })
}

fn native_confirm_password_reset(
    gateway_url: String,
    payload: PasswordResetConfirmRequest,
) -> Result<(), String> {
    let _: PasswordResetConfirmResponse = post_json(
        format!("{gateway_url}/auth/password-reset/confirm"),
        None,
        &payload,
    )?;
    Ok(())
}

fn native_confirm_password_reset_async(
    gateway_url: String,
    payload: PasswordResetConfirmRequest,
) -> GatewayFuture<()> {
    Box::pin(async move { native_confirm_password_reset(gateway_url, payload) })
}

fn native_fetch_me(gateway_url: String, access_token: String) -> Result<MeResponse, String> {
    get_json(format!("{gateway_url}/auth/me"), Some(&access_token))
}

fn native_fetch_me_async(gateway_url: String, access_token: String) -> GatewayFuture<MeResponse> {
    Box::pin(async move { native_fetch_me(gateway_url, access_token) })
}

fn native_fetch_characters(
    gateway_url: String,
    access_token: String,
) -> Result<CharactersResponse, String> {
    get_json(
        format!("{gateway_url}/auth/characters"),
        Some(&access_token),
    )
}

fn native_fetch_characters_async(
    gateway_url: String,
    access_token: String,
) -> GatewayFuture<CharactersResponse> {
    Box::pin(async move { native_fetch_characters(gateway_url, access_token) })
}

fn native_enter_world(
    gateway_url: String,
    access_token: String,
    payload: EnterWorldRequest,
) -> Result<EnterWorldResponse, String> {
    post_json(
        format!("{gateway_url}/world/enter"),
        Some(&access_token),
        &payload,
    )
}

fn native_enter_world_async(
    gateway_url: String,
    access_token: String,
    payload: EnterWorldRequest,
) -> GatewayFuture<EnterWorldResponse> {
    Box::pin(async move { native_enter_world(gateway_url, access_token, payload) })
}

fn native_fetch_bootstrap_manifest(
    gateway_url: String,
    access_token: String,
) -> Result<AssetBootstrapManifestResponse, String> {
    get_json(
        format!("{gateway_url}/assets/bootstrap-manifest"),
        Some(&access_token),
    )
}

fn native_fetch_bootstrap_manifest_async(
    gateway_url: String,
    access_token: String,
) -> GatewayFuture<AssetBootstrapManifestResponse> {
    Box::pin(async move { native_fetch_bootstrap_manifest(gateway_url, access_token) })
}

fn native_fetch_asset_bytes(url: String, access_token: String) -> Result<Vec<u8>, String> {
    get_bytes(url, Some(&access_token))
}

fn native_fetch_asset_bytes_async(url: String, access_token: String) -> GatewayFuture<Vec<u8>> {
    Box::pin(async move { native_fetch_asset_bytes(url, access_token) })
}

pub(crate) fn native_gateway_http_adapter() -> GatewayHttpAdapter {
    GatewayHttpAdapter {
        login: native_login_async,
        register: native_register_async,
        request_password_reset: native_request_password_reset_async,
        confirm_password_reset: native_confirm_password_reset_async,
        fetch_me: native_fetch_me_async,
        fetch_characters: native_fetch_characters_async,
        enter_world: native_enter_world_async,
        fetch_bootstrap_manifest: native_fetch_bootstrap_manifest_async,
        fetch_asset_bytes: native_fetch_asset_bytes_async,
    }
}

fn native_prepare_cache_root_async(asset_root: String) -> CacheFuture<()> {
    Box::pin(async move { prepare_cache_root(&asset_root) })
}

fn native_load_cache_index_async(asset_root: String) -> CacheFuture<AssetCacheIndex> {
    Box::pin(async move { Ok(load_cache_index_for_asset_root(&asset_root)) })
}

fn native_save_cache_index_async(
    asset_root: String,
    cache_index: AssetCacheIndex,
) -> CacheFuture<()> {
    Box::pin(async move { save_cache_index_for_asset_root(&asset_root, &cache_index) })
}

fn native_read_cached_asset_if_valid_async(
    asset_root: String,
    relative_cache_path: String,
    expected_sha256: String,
) -> CacheFuture<Option<Vec<u8>>> {
    Box::pin(async move {
        Ok(read_cached_asset_if_valid(
            &asset_root,
            &relative_cache_path,
            &expected_sha256,
        ))
    })
}

fn native_write_cached_asset_async(
    asset_root: String,
    relative_cache_path: String,
    bytes: Vec<u8>,
) -> CacheFuture<()> {
    Box::pin(async move { write_cached_asset(&asset_root, &relative_cache_path, &bytes) })
}

pub(crate) fn native_asset_cache_adapter() -> AssetCacheAdapter {
    AssetCacheAdapter {
        prepare_root: native_prepare_cache_root_async,
        load_index: native_load_cache_index_async,
        save_index: native_save_cache_index_async,
        read_valid_asset: native_read_cached_asset_if_valid_async,
        write_asset: native_write_cached_asset_async,
        read_valid_asset_sync: read_cached_asset_if_valid,
    }
}
