use bevy::log::LogPlugin;
use bevy::log::info;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::window::{Window, WindowPlugin};
use sidereal_asset_runtime::AssetCacheIndex;
use sidereal_core::gateway_dtos::{
    AssetBootstrapManifestResponse, AuthTokens, CharactersResponse, EnterWorldRequest,
    EnterWorldResponse, LoginRequest, MeResponse, PasswordResetConfirmRequest,
    PasswordResetConfirmResponse, PasswordResetRequest, PasswordResetResponse, RegisterRequest,
};
use std::cell::RefCell;
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen_futures::JsFuture;

use crate::runtime::{self, AssetCacheAdapter, CacheFuture, GatewayFuture, GatewayHttpAdapter};

const WASM_CACHE_DB_NAME: &str = "sidereal_asset_cache";
const WASM_CACHE_DB_VERSION: u32 = 1;
const WASM_CACHE_META_STORE: &str = "meta";
const WASM_CACHE_ASSET_STORE: &str = "assets";
const WASM_CACHE_INDEX_KEY: &str = "index";

#[derive(Default, Clone)]
struct WasmAssetCacheMirror {
    hydrated: bool,
    cache_index: AssetCacheIndex,
    assets_by_path: HashMap<String, Vec<u8>>,
}

thread_local! {
    static WASM_ASSET_CACHE_MIRROR: RefCell<WasmAssetCacheMirror> =
        RefCell::new(WasmAssetCacheMirror::default());
}

pub(crate) fn run() {
    let mut app = runtime::build_windowed_client_app(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    canvas: Some("#sidereal-game-client-canvas".to_string()),
                    fit_canvas_to_parent: true,
                    ..Default::default()
                }),
                ..Default::default()
            })
            .set(LogPlugin {
                custom_layer: runtime::build_log_capture_layer,
                ..Default::default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    backends: Some(preferred_backends()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        ".".to_string(),
        wasm_gateway_http_adapter(),
        wasm_asset_cache_adapter(),
    );
    if let Some(mut session) = app.world_mut().get_resource_mut::<runtime::ClientSession>() {
        session.gateway_url = browser_gateway_url();
    }
    app.add_systems(Startup, || {
        info!("sidereal-client wasm runtime booted with shared client configuration");
    });
    app.run();
}

fn preferred_backends() -> Backends {
    Backends::from_env().unwrap_or(Backends::BROWSER_WEBGPU | Backends::GL)
}

fn browser_gateway_url() -> String {
    let Some(window) = web_sys::window() else {
        return "http://127.0.0.1:8080".to_string();
    };
    let value = js_sys::Reflect::get(
        window.as_ref(),
        &wasm_bindgen::JsValue::from_str("__SIDEREAL_GATEWAY_URL"),
    )
    .ok()
    .and_then(|value| value.as_string())
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty());
    value.unwrap_or_else(|| "http://127.0.0.1:8080".to_string())
}

async fn decode_api_json<T: serde::de::DeserializeOwned>(
    response: web_sys::Response,
) -> Result<T, String> {
    let status = response.status();
    let body = response_text(response).await?;
    if !(200..300).contains(&status) {
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

fn js_error_to_string(value: wasm_bindgen::JsValue) -> String {
    if let Some(text) = value.as_string() {
        return text;
    }
    if let Some(error) = value.dyn_ref::<js_sys::Error>() {
        return error.message().into();
    }
    format!("{value:?}")
}

async fn response_text(response: web_sys::Response) -> Result<String, String> {
    let promise = response.text().map_err(js_error_to_string)?;
    let value = JsFuture::from(promise).await.map_err(js_error_to_string)?;
    value
        .as_string()
        .ok_or_else(|| "Response body was not a string.".to_string())
}

async fn response_bytes(response: web_sys::Response) -> Result<Vec<u8>, String> {
    let promise = response.array_buffer().map_err(js_error_to_string)?;
    let value = JsFuture::from(promise).await.map_err(js_error_to_string)?;
    let buffer = value
        .dyn_into::<js_sys::ArrayBuffer>()
        .map_err(js_error_to_string)?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

async fn send_request(
    method: &str,
    url: String,
    bearer_token: Option<String>,
    json_body: Option<String>,
) -> Result<web_sys::Response, String> {
    let Some(window) = web_sys::window() else {
        return Err("Browser window is unavailable.".to_string());
    };

    let headers = web_sys::Headers::new().map_err(js_error_to_string)?;
    if let Some(token) = bearer_token.as_ref() {
        headers
            .set("Authorization", &format!("Bearer {token}"))
            .map_err(js_error_to_string)?;
    }
    if json_body.is_some() {
        headers
            .set("Content-Type", "application/json")
            .map_err(js_error_to_string)?;
    }

    let init = web_sys::RequestInit::new();
    init.set_method(method);
    init.set_headers(&headers);
    if let Some(body) = json_body.as_ref() {
        init.set_body(&wasm_bindgen::JsValue::from_str(body));
    }

    let request =
        web_sys::Request::new_with_str_and_init(&url, &init).map_err(js_error_to_string)?;
    let value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(js_error_to_string)?;
    value
        .dyn_into::<web_sys::Response>()
        .map_err(js_error_to_string)
}

async fn get_json<T: serde::de::DeserializeOwned>(
    url: String,
    bearer_token: Option<String>,
) -> Result<T, String> {
    let response = send_request("GET", url, bearer_token, None).await?;
    decode_api_json(response).await
}

async fn post_json<Request, ResponseBody>(
    url: String,
    bearer_token: Option<String>,
    payload: Request,
) -> Result<ResponseBody, String>
where
    Request: serde::Serialize,
    ResponseBody: serde::de::DeserializeOwned,
{
    let json_body = serde_json::to_string(&payload).map_err(|err| err.to_string())?;
    let response = send_request("POST", url, bearer_token, Some(json_body)).await?;
    decode_api_json(response).await
}

async fn get_bytes(url: String, bearer_token: Option<String>) -> Result<Vec<u8>, String> {
    let response = send_request("GET", url, bearer_token, None).await?;
    let status = response.status();
    if !(200..300).contains(&status) {
        let body = response_text(response).await.unwrap_or_default();
        if body.trim().is_empty() {
            return Err(status.to_string());
        }
        return Err(format!("{status}: {body}"));
    }
    response_bytes(response).await
}

fn wasm_login(gateway_url: String, payload: LoginRequest) -> GatewayFuture<AuthTokens> {
    Box::pin(async move { post_json(format!("{gateway_url}/auth/login"), None, payload).await })
}

fn wasm_register(gateway_url: String, payload: RegisterRequest) -> GatewayFuture<AuthTokens> {
    Box::pin(async move { post_json(format!("{gateway_url}/auth/register"), None, payload).await })
}

fn wasm_request_password_reset(
    gateway_url: String,
    payload: PasswordResetRequest,
) -> GatewayFuture<PasswordResetResponse> {
    Box::pin(async move {
        post_json(
            format!("{gateway_url}/auth/password-reset/request"),
            None,
            payload,
        )
        .await
    })
}

fn wasm_confirm_password_reset(
    gateway_url: String,
    payload: PasswordResetConfirmRequest,
) -> GatewayFuture<()> {
    Box::pin(async move {
        let _: PasswordResetConfirmResponse = post_json(
            format!("{gateway_url}/auth/password-reset/confirm"),
            None,
            payload,
        )
        .await?;
        Ok(())
    })
}

fn wasm_fetch_me(gateway_url: String, access_token: String) -> GatewayFuture<MeResponse> {
    Box::pin(async move { get_json(format!("{gateway_url}/auth/me"), Some(access_token)).await })
}

fn wasm_fetch_characters(
    gateway_url: String,
    access_token: String,
) -> GatewayFuture<CharactersResponse> {
    Box::pin(
        async move { get_json(format!("{gateway_url}/auth/characters"), Some(access_token)).await },
    )
}

fn wasm_enter_world(
    gateway_url: String,
    access_token: String,
    payload: EnterWorldRequest,
) -> GatewayFuture<EnterWorldResponse> {
    Box::pin(async move {
        post_json(
            format!("{gateway_url}/world/enter"),
            Some(access_token),
            payload,
        )
        .await
    })
}

fn wasm_fetch_bootstrap_manifest(
    gateway_url: String,
    access_token: String,
) -> GatewayFuture<AssetBootstrapManifestResponse> {
    Box::pin(async move {
        get_json(
            format!("{gateway_url}/assets/bootstrap-manifest"),
            Some(access_token),
        )
        .await
    })
}

fn wasm_fetch_asset_bytes(url: String, access_token: String) -> GatewayFuture<Vec<u8>> {
    Box::pin(async move { get_bytes(url, Some(access_token)).await })
}

fn wasm_gateway_http_adapter() -> GatewayHttpAdapter {
    GatewayHttpAdapter {
        login: wasm_login,
        register: wasm_register,
        request_password_reset: wasm_request_password_reset,
        confirm_password_reset: wasm_confirm_password_reset,
        fetch_me: wasm_fetch_me,
        fetch_characters: wasm_fetch_characters,
        enter_world: wasm_enter_world,
        fetch_bootstrap_manifest: wasm_fetch_bootstrap_manifest,
        fetch_asset_bytes: wasm_fetch_asset_bytes,
    }
}

fn with_cache_mirror<R>(f: impl FnOnce(&WasmAssetCacheMirror) -> R) -> R {
    WASM_ASSET_CACHE_MIRROR.with(|mirror| f(&mirror.borrow()))
}

fn mutate_cache_mirror<R>(f: impl FnOnce(&mut WasmAssetCacheMirror) -> R) -> R {
    WASM_ASSET_CACHE_MIRROR.with(|mirror| f(&mut mirror.borrow_mut()))
}

fn indexed_db_factory() -> Result<web_sys::IdbFactory, String> {
    let Some(window) = web_sys::window() else {
        return Err("Browser window is unavailable.".to_string());
    };
    window
        .indexed_db()
        .map_err(js_error_to_string)?
        .ok_or_else(|| "IndexedDB is unavailable in this browser context.".to_string())
}

fn idb_request_promise(request: &web_sys::IdbRequest) -> js_sys::Promise {
    let request = request.clone();
    js_sys::Promise::new(&mut |resolve, reject| {
        let success_request = request.clone();
        let resolve_fn = resolve.clone();
        let reject_fn = reject.clone();
        let success = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_| {
            match success_request.result() {
                Ok(result) => {
                    let _ = resolve_fn.call1(&wasm_bindgen::JsValue::UNDEFINED, &result);
                }
                Err(err) => {
                    let _ = reject_fn.call1(&wasm_bindgen::JsValue::UNDEFINED, &err);
                }
            }
        }));
        let error_request = request.clone();
        let error = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_| {
            let value = error_request
                .error()
                .ok()
                .flatten()
                .map(wasm_bindgen::JsValue::from)
                .unwrap_or_else(|| wasm_bindgen::JsValue::from_str("IndexedDB request failed."));
            let _ = reject.call1(&wasm_bindgen::JsValue::UNDEFINED, &value);
        }));
        request.set_onsuccess(Some(success.as_ref().unchecked_ref()));
        request.set_onerror(Some(error.as_ref().unchecked_ref()));
        success.forget();
        error.forget();
    })
}

async fn await_idb_request(request: &web_sys::IdbRequest) -> Result<wasm_bindgen::JsValue, String> {
    JsFuture::from(idb_request_promise(request))
        .await
        .map_err(js_error_to_string)
}

fn idb_transaction_promise(transaction: &web_sys::IdbTransaction) -> js_sys::Promise {
    let transaction = transaction.clone();
    js_sys::Promise::new(&mut |resolve, reject| {
        let resolve_fn = resolve.clone();
        let complete = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_| {
            let _ = resolve_fn.call0(&wasm_bindgen::JsValue::UNDEFINED);
        }));
        let error_tx = transaction.clone();
        let error = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_| {
            let value = error_tx
                .error()
                .map(wasm_bindgen::JsValue::from)
                .unwrap_or_else(|| {
                    wasm_bindgen::JsValue::from_str("IndexedDB transaction failed.")
                });
            let _ = reject.call1(&wasm_bindgen::JsValue::UNDEFINED, &value);
        }));
        transaction.set_oncomplete(Some(complete.as_ref().unchecked_ref()));
        transaction.set_onabort(Some(error.as_ref().unchecked_ref()));
        transaction.set_onerror(Some(error.as_ref().unchecked_ref()));
        complete.forget();
        error.forget();
    })
}

async fn await_idb_transaction(transaction: &web_sys::IdbTransaction) -> Result<(), String> {
    JsFuture::from(idb_transaction_promise(transaction))
        .await
        .map_err(js_error_to_string)?;
    Ok(())
}

async fn open_cache_db() -> Result<web_sys::IdbDatabase, String> {
    info!(
        "wasm asset cache opening IndexedDB database name={} version={}",
        WASM_CACHE_DB_NAME, WASM_CACHE_DB_VERSION
    );
    let factory = indexed_db_factory()?;
    let request = factory
        .open_with_u32(WASM_CACHE_DB_NAME, WASM_CACHE_DB_VERSION)
        .map_err(js_error_to_string)?;
    let upgrade =
        Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |event: web_sys::Event| {
            let Some(target) = event.target() else {
                return;
            };
            let Ok(open_request) = target.dyn_into::<web_sys::IdbOpenDbRequest>() else {
                return;
            };
            let Ok(result) = open_request.result() else {
                return;
            };
            let Ok(db) = result.dyn_into::<web_sys::IdbDatabase>() else {
                return;
            };
            let store_names = db.object_store_names();
            if !store_names.contains(WASM_CACHE_META_STORE) {
                let _ = db.create_object_store(WASM_CACHE_META_STORE);
            }
            if !store_names.contains(WASM_CACHE_ASSET_STORE) {
                let _ = db.create_object_store(WASM_CACHE_ASSET_STORE);
            }
        }));
    request.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
    let result = await_idb_request(request.unchecked_ref()).await?;
    request.set_onupgradeneeded(None);
    drop(upgrade);
    result
        .dyn_into::<web_sys::IdbDatabase>()
        .map_err(js_error_to_string)
}

async fn load_index_from_db(db: &web_sys::IdbDatabase) -> Result<AssetCacheIndex, String> {
    let transaction = db
        .transaction_with_str_and_mode(WASM_CACHE_META_STORE, web_sys::IdbTransactionMode::Readonly)
        .map_err(js_error_to_string)?;
    let store = transaction
        .object_store(WASM_CACHE_META_STORE)
        .map_err(js_error_to_string)?;
    let request = store
        .get(&wasm_bindgen::JsValue::from_str(WASM_CACHE_INDEX_KEY))
        .map_err(js_error_to_string)?;
    let value = await_idb_request(&request).await?;
    await_idb_transaction(&transaction).await?;
    if value.is_undefined() || value.is_null() {
        return Ok(AssetCacheIndex::default());
    }
    let json = value
        .as_string()
        .ok_or_else(|| "IndexedDB cache index record was not a string.".to_string())?;
    serde_json::from_str::<AssetCacheIndex>(&json).map_err(|err| err.to_string())
}

async fn save_index_to_db(
    db: &web_sys::IdbDatabase,
    cache_index: &AssetCacheIndex,
) -> Result<(), String> {
    let transaction = db
        .transaction_with_str_and_mode(
            WASM_CACHE_META_STORE,
            web_sys::IdbTransactionMode::Readwrite,
        )
        .map_err(js_error_to_string)?;
    let store = transaction
        .object_store(WASM_CACHE_META_STORE)
        .map_err(js_error_to_string)?;
    let json = serde_json::to_string(cache_index).map_err(|err| err.to_string())?;
    let request = store
        .put_with_key(
            &wasm_bindgen::JsValue::from_str(&json),
            &wasm_bindgen::JsValue::from_str(WASM_CACHE_INDEX_KEY),
        )
        .map_err(js_error_to_string)?;
    let _ = await_idb_request(&request).await?;
    await_idb_transaction(&transaction).await
}

fn js_value_to_bytes(value: wasm_bindgen::JsValue) -> Result<Vec<u8>, String> {
    if value.is_undefined() || value.is_null() {
        return Err("IndexedDB asset payload was empty.".to_string());
    }
    Ok(js_sys::Uint8Array::new(&value).to_vec())
}

async fn load_assets_from_db(
    db: &web_sys::IdbDatabase,
) -> Result<HashMap<String, Vec<u8>>, String> {
    let transaction = db
        .transaction_with_str_and_mode(
            WASM_CACHE_ASSET_STORE,
            web_sys::IdbTransactionMode::Readonly,
        )
        .map_err(js_error_to_string)?;
    let store = transaction
        .object_store(WASM_CACHE_ASSET_STORE)
        .map_err(js_error_to_string)?;
    let keys = js_sys::Array::from(
        &await_idb_request(&store.get_all_keys().map_err(js_error_to_string)?).await?,
    );
    let values = js_sys::Array::from(
        &await_idb_request(&store.get_all().map_err(js_error_to_string)?).await?,
    );
    await_idb_transaction(&transaction).await?;
    let mut assets_by_path = HashMap::new();
    for index in 0..keys.length() {
        let Some(relative_cache_path) = keys.get(index).as_string() else {
            continue;
        };
        let bytes = js_value_to_bytes(values.get(index))?;
        assets_by_path.insert(relative_cache_path, bytes);
    }
    Ok(assets_by_path)
}

async fn save_asset_to_db(
    db: &web_sys::IdbDatabase,
    relative_cache_path: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let transaction = db
        .transaction_with_str_and_mode(
            WASM_CACHE_ASSET_STORE,
            web_sys::IdbTransactionMode::Readwrite,
        )
        .map_err(js_error_to_string)?;
    let store = transaction
        .object_store(WASM_CACHE_ASSET_STORE)
        .map_err(js_error_to_string)?;
    let request = store
        .put_with_key(
            &js_sys::Uint8Array::from(bytes).into(),
            &wasm_bindgen::JsValue::from_str(relative_cache_path),
        )
        .map_err(js_error_to_string)?;
    let _ = await_idb_request(&request).await?;
    await_idb_transaction(&transaction).await
}

async fn ensure_cache_mirror_loaded() -> Result<(), String> {
    if with_cache_mirror(|mirror| mirror.hydrated) {
        return Ok(());
    }
    info!("wasm asset cache hydrating IndexedDB mirror");
    let db = open_cache_db().await?;
    let cache_index = load_index_from_db(&db).await?;
    let assets_by_path = load_assets_from_db(&db).await?;
    db.close();
    mutate_cache_mirror(|mirror| {
        mirror.hydrated = true;
        mirror.cache_index = cache_index;
        mirror.assets_by_path = assets_by_path;
    });
    info!(
        "wasm asset cache mirror hydrated: indexed_assets={}",
        with_cache_mirror(|mirror| mirror.assets_by_path.len())
    );
    Ok(())
}

fn wasm_read_valid_asset_sync(
    _: &str,
    relative_cache_path: &str,
    expected_sha256: &str,
) -> Option<Vec<u8>> {
    with_cache_mirror(|mirror| {
        let bytes = mirror.assets_by_path.get(relative_cache_path)?.clone();
        (sidereal_asset_runtime::sha256_hex(&bytes) == expected_sha256).then_some(bytes)
    })
}

fn wasm_prepare_root(_: String) -> CacheFuture<()> {
    Box::pin(async move { ensure_cache_mirror_loaded().await })
}

fn wasm_load_index(_: String) -> CacheFuture<AssetCacheIndex> {
    Box::pin(async move {
        ensure_cache_mirror_loaded().await?;
        Ok(with_cache_mirror(|mirror| mirror.cache_index.clone()))
    })
}

fn wasm_save_index(_: String, cache_index: AssetCacheIndex) -> CacheFuture<()> {
    Box::pin(async move {
        ensure_cache_mirror_loaded().await?;
        let db = open_cache_db().await?;
        save_index_to_db(&db, &cache_index).await?;
        db.close();
        mutate_cache_mirror(|mirror| {
            mirror.cache_index = cache_index;
        });
        Ok(())
    })
}

fn wasm_read_valid_asset(
    _: String,
    relative_cache_path: String,
    expected_sha256: String,
) -> CacheFuture<Option<Vec<u8>>> {
    Box::pin(async move {
        ensure_cache_mirror_loaded().await?;
        Ok(wasm_read_valid_asset_sync(
            "",
            &relative_cache_path,
            &expected_sha256,
        ))
    })
}

fn wasm_write_asset(_: String, relative_cache_path: String, bytes: Vec<u8>) -> CacheFuture<()> {
    Box::pin(async move {
        ensure_cache_mirror_loaded().await?;
        let db = open_cache_db().await?;
        save_asset_to_db(&db, &relative_cache_path, &bytes).await?;
        db.close();
        let logged_relative_cache_path = relative_cache_path.clone();
        mutate_cache_mirror(|mirror| {
            mirror.assets_by_path.insert(relative_cache_path, bytes);
        });
        info!(
            "wasm asset cache stored payload in IndexedDB: relative_cache_path={}",
            logged_relative_cache_path
        );
        Ok(())
    })
}

fn wasm_asset_cache_adapter() -> AssetCacheAdapter {
    AssetCacheAdapter {
        prepare_root: wasm_prepare_root,
        load_index: wasm_load_index,
        save_index: wasm_save_index,
        read_valid_asset: wasm_read_valid_asset,
        write_asset: wasm_write_asset,
        read_valid_asset_sync: wasm_read_valid_asset_sync,
    }
}
