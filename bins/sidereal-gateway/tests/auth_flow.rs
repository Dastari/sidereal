use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use serde_json::Value;
use sidereal_gateway::api::app_with_service;
use sidereal_gateway::auth::{
    AuthConfig, AuthService, InMemoryAuthStore, NoopBootstrapDispatcher, NoopStarterWorldPersister,
    RecordingBootstrapDispatcher,
};
use sidereal_persistence::GraphPersistence;
use std::sync::Arc;
use tower::ServiceExt;

#[tokio::test]
async fn register_login_refresh_me_happy_path() {
    let service = Arc::new(AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
    ));
    let app = app_with_service(service.clone());

    let register_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/register",
            r#"{"email":"pilot@example.com","password":"very-strong-password"}"#,
            None,
        ))
        .await
        .expect("register response");
    assert_eq!(register_response.status(), StatusCode::OK);
    let register_json = response_json(register_response).await;
    let access_token = register_json["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();
    let refresh_token = register_json["refresh_token"]
        .as_str()
        .expect("refresh_token")
        .to_string();

    let login_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/login",
            r#"{"email":"pilot@example.com","password":"very-strong-password"}"#,
            None,
        ))
        .await
        .expect("login response");
    assert_eq!(login_response.status(), StatusCode::OK);

    let refresh_body = format!(r#"{{"refresh_token":"{refresh_token}"}}"#);
    let refresh_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/refresh",
            &refresh_body,
            None,
        ))
        .await
        .expect("refresh response");
    assert_eq!(refresh_response.status(), StatusCode::OK);
    let refresh_json = response_json(refresh_response).await;
    assert_ne!(
        refresh_json["refresh_token"].as_str().expect("new refresh"),
        refresh_token
    );

    let me_response = app
        .oneshot(json_request(
            Method::GET,
            "/auth/me",
            "",
            Some(&access_token),
        ))
        .await
        .expect("me response");
    assert_eq!(me_response.status(), StatusCode::OK);
    let me_json = response_json(me_response).await;
    assert_eq!(
        me_json["email"].as_str().expect("email"),
        "pilot@example.com"
    );
    assert!(
        me_json["player_entity_id"]
            .as_str()
            .expect("player entity id")
            .starts_with("player:")
    );
}

#[tokio::test]
async fn login_does_not_dispatch_bootstrap_command() {
    let dispatcher = Arc::new(RecordingBootstrapDispatcher::default());
    let service = Arc::new(AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        dispatcher.clone(),
        Arc::new(NoopStarterWorldPersister),
    ));
    let app = app_with_service(service.clone());

    let _ = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/register",
            r#"{"email":"pilot@example.com","password":"very-strong-password"}"#,
            None,
        ))
        .await
        .expect("register response");
    let dispatch_after_register = dispatcher.commands().await.len();

    let _ = app
        .oneshot(json_request(
            Method::POST,
            "/auth/login",
            r#"{"email":"pilot@example.com","password":"very-strong-password"}"#,
            None,
        ))
        .await
        .expect("login response");

    let dispatch_after_login = dispatcher.commands().await.len();
    assert_eq!(dispatch_after_register, dispatch_after_login);
}

#[tokio::test]
async fn register_conflict_does_not_dispatch_bootstrap() {
    let dispatcher = Arc::new(RecordingBootstrapDispatcher::default());
    let service = Arc::new(AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        dispatcher.clone(),
        Arc::new(NoopStarterWorldPersister),
    ));
    let app = app_with_service(service.clone());

    let first = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/register",
            r#"{"email":"pilot@example.com","password":"very-strong-password"}"#,
            None,
        ))
        .await
        .expect("register first");
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .oneshot(json_request(
            Method::POST,
            "/auth/register",
            r#"{"email":"pilot@example.com","password":"very-strong-password"}"#,
            None,
        ))
        .await
        .expect("register second");
    assert_eq!(second.status(), StatusCode::CONFLICT);

    let dispatch_count = dispatcher.commands().await.len();
    assert_eq!(dispatch_count, 0);
}

#[tokio::test]
async fn password_reset_request_confirm_allows_new_login() {
    let service = Arc::new(AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
    ));
    let app = app_with_service(service.clone());

    let _ = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/register",
            r#"{"email":"pilot@example.com","password":"very-strong-password"}"#,
            None,
        ))
        .await
        .expect("register response");

    let request_reset = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/password-reset/request",
            r#"{"email":"pilot@example.com"}"#,
            None,
        ))
        .await
        .expect("password reset request");
    assert_eq!(request_reset.status(), StatusCode::OK);
    let reset_json = response_json(request_reset).await;
    let reset_token = reset_json["reset_token"]
        .as_str()
        .expect("reset token")
        .to_string();

    let confirm_body =
        format!(r#"{{"reset_token":"{reset_token}","new_password":"new-very-strong-password"}}"#);
    let confirm = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/password-reset/confirm",
            &confirm_body,
            None,
        ))
        .await
        .expect("password reset confirm");
    assert_eq!(confirm.status(), StatusCode::OK);

    let old_login = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/login",
            r#"{"email":"pilot@example.com","password":"very-strong-password"}"#,
            None,
        ))
        .await
        .expect("old login response");
    assert_eq!(old_login.status(), StatusCode::UNAUTHORIZED);

    let new_login = app
        .oneshot(json_request(
            Method::POST,
            "/auth/login",
            r#"{"email":"pilot@example.com","password":"new-very-strong-password"}"#,
            None,
        ))
        .await
        .expect("new login response");
    assert_eq!(new_login.status(), StatusCode::OK);
}

#[tokio::test]
async fn register_then_world_me_returns_starter_ship_and_assets() {
    let database_url = test_database_url();
    let db_available = std::thread::spawn({
        let database_url = database_url.clone();
        move || GraphPersistence::connect(&database_url).is_ok()
    })
    .join()
    .unwrap_or(false);
    if !db_available {
        eprintln!("skipping world_me bootstrap lifecycle test; postgres unavailable");
        return;
    }

    let service = Arc::new(AuthService::new(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(NoopBootstrapDispatcher),
    ));
    let app = app_with_service(service);

    let register_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/register",
            r#"{"email":"pilot-world@example.com","password":"very-strong-password"}"#,
            None,
        ))
        .await
        .expect("register response");
    assert_eq!(register_response.status(), StatusCode::OK);
    let register_json = response_json(register_response).await;
    let access_token = register_json["access_token"]
        .as_str()
        .expect("access token")
        .to_string();

    let world_me_response = app
        .oneshot(json_request(
            Method::GET,
            "/world/me",
            "",
            Some(&access_token),
        ))
        .await
        .expect("world/me response");
    assert_eq!(world_me_response.status(), StatusCode::OK);
    let world_me_json = response_json(world_me_response).await;

    assert_eq!(world_me_json["model_asset_id"], "corvette_01");
    assert!(
        world_me_json["ship_entity_id"]
            .as_str()
            .expect("ship id")
            .starts_with("ship:")
    );
    let assets = world_me_json["assets"].as_array().expect("assets array");
    assert!(
        assets
            .iter()
            .any(|asset| asset["asset_id"] == "corvette_01_gltf")
    );
    assert!(
        assets
            .iter()
            .any(|asset| asset["asset_id"] == "starfield_wgsl")
    );
}

fn test_database_url() -> String {
    std::env::var("SIDEREAL_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("GATEWAY_DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string())
}

fn json_request(
    method: Method,
    uri: &str,
    body: &str,
    bearer_token: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer_token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    builder
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request should build")
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    serde_json::from_slice(&bytes).expect("json body")
}
