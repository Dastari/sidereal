use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::Value;
use sidereal_core::auth::{AuthClaims, AuthSessionContext};
use sidereal_gateway::api::app_with_service;
use sidereal_gateway::auth::{
    AuthConfig, AuthService, InMemoryAuthStore, NoopStarterWorldPersister,
    RecordingBootstrapDispatcher, RecordingEmailDelivery, now_epoch_s, totp_code,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;
use uuid::Uuid;

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
        .clone()
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
    assert_eq!(
        me_json["player_entity_id"]
            .as_str()
            .expect("legacy player entity id"),
        "",
        "registration should not create a default character"
    );

    let empty_characters = app
        .clone()
        .oneshot(json_request(
            Method::GET,
            "/auth/v1/characters",
            "",
            Some(&access_token),
        ))
        .await
        .expect("empty characters response");
    assert_eq!(empty_characters.status(), StatusCode::OK);
    let empty_json = response_json(empty_characters).await;
    assert_eq!(
        empty_json["characters"]
            .as_array()
            .expect("characters array")
            .len(),
        0
    );

    let create_character = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/v1/characters",
            r#"{"display_name":"Talanah"}"#,
            Some(&access_token),
        ))
        .await
        .expect("create character response");
    assert_eq!(create_character.status(), StatusCode::OK);
    let create_json = response_json(create_character).await;
    assert_eq!(
        create_json["display_name"].as_str().expect("display_name"),
        "Talanah"
    );
    let player_entity_id = create_json["player_entity_id"]
        .as_str()
        .expect("player entity id");
    assert!(
        uuid::Uuid::parse_str(player_entity_id).is_ok(),
        "player_entity_id should be a valid UUID, got: {player_entity_id}"
    );

    let characters = app
        .oneshot(json_request(
            Method::GET,
            "/auth/v1/characters",
            "",
            Some(&access_token),
        ))
        .await
        .expect("characters response");
    assert_eq!(characters.status(), StatusCode::OK);
    let characters_json = response_json(characters).await;
    assert_eq!(
        characters_json["characters"][0]["player_entity_id"]
            .as_str()
            .expect("listed character id"),
        player_entity_id
    );
}

#[tokio::test]
async fn login_route_answers_cors_preflight_for_dashboard_origin() {
    let service = Arc::new(AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
    ));
    let app = app_with_service(service);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/auth/login")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
                .body(Body::empty())
                .expect("preflight request"),
        )
        .await
        .expect("preflight response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .expect("allow origin header"),
        "http://localhost:3000"
    );
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
    let email_delivery = Arc::new(RecordingEmailDelivery::default());
    let service = Arc::new(AuthService::new_with_dependencies(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
        email_delivery.clone(),
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

    let legacy_request_reset = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/password-reset/request",
            r#"{"email":"pilot@example.com"}"#,
            None,
        ))
        .await
        .expect("legacy password reset request");
    assert_eq!(legacy_request_reset.status(), StatusCode::NOT_FOUND);

    let request_reset = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/v1/password-reset/request",
            r#"{"email":"pilot@example.com"}"#,
            None,
        ))
        .await
        .expect("password reset request");
    assert_eq!(request_reset.status(), StatusCode::OK);
    let messages = email_delivery.messages().await;
    assert_eq!(messages.len(), 1);
    let reset_token = extract_email_line(&messages[0].body_text, "Reset token: ");

    let confirm_body =
        format!(r#"{{"reset_token":"{reset_token}","new_password":"new-very-strong-password"}}"#);
    let legacy_confirm = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/password-reset/confirm",
            &confirm_body,
            None,
        ))
        .await
        .expect("legacy password reset confirm");
    assert_eq!(legacy_confirm.status(), StatusCode::NOT_FOUND);

    let confirm = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/v1/password-reset/confirm",
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
async fn v1_password_reset_request_sends_email_without_returning_token() {
    let email_delivery = Arc::new(RecordingEmailDelivery::default());
    let service = Arc::new(AuthService::new_with_dependencies(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
        email_delivery.clone(),
    ));
    let app = app_with_service(service.clone());

    let _ = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/v1/register",
            r#"{"email":"pilot@example.com","password":"very-strong-password"}"#,
            None,
        ))
        .await
        .expect("register response");

    let request_reset = app
        .oneshot(json_request(
            Method::POST,
            "/auth/v1/password-reset/request",
            r#"{"email":"pilot@example.com"}"#,
            None,
        ))
        .await
        .expect("password reset request");

    assert_eq!(request_reset.status(), StatusCode::OK);
    let reset_json = response_json(request_reset).await;
    assert_eq!(reset_json["accepted"].as_bool(), Some(true));
    assert!(reset_json["reset_token"].is_null());
    let messages = email_delivery.messages().await;
    assert_eq!(messages.len(), 1);
    assert!(
        messages[0].body_text.contains("Reset token: "),
        "test delivery should receive reset token in body"
    );
}

#[tokio::test]
async fn v1_email_login_request_and_verify_issues_tokens() {
    let email_delivery = Arc::new(RecordingEmailDelivery::default());
    let service = Arc::new(AuthService::new_with_dependencies(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
        email_delivery.clone(),
    ));
    let app = app_with_service(service.clone());

    let _ = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/v1/register",
            r#"{"email":"pilot@example.com","password":"very-strong-password"}"#,
            None,
        ))
        .await
        .expect("register response");

    let request_login = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/v1/login/email/request",
            r#"{"email":"pilot@example.com"}"#,
            None,
        ))
        .await
        .expect("email login request");
    assert_eq!(request_login.status(), StatusCode::OK);
    let request_json = response_json(request_login).await;
    assert_eq!(request_json["accepted"].as_bool(), Some(true));

    let messages = email_delivery.messages().await;
    assert_eq!(messages.len(), 1);
    let challenge_id = extract_email_line(&messages[0].body_text, "Challenge ID: ");
    let code = extract_email_line(&messages[0].body_text, "Code: ");
    let verify_body = format!(r#"{{"challenge_id":"{challenge_id}","code":"{code}"}}"#);
    let verify = app
        .oneshot(json_request(
            Method::POST,
            "/auth/v1/login/email/verify",
            &verify_body,
            None,
        ))
        .await
        .expect("email login verify");

    assert_eq!(verify.status(), StatusCode::OK);
    let verify_json = response_json(verify).await;
    assert_eq!(verify_json["token_type"].as_str(), Some("bearer"));
    assert!(verify_json["access_token"].as_str().is_some());
}

#[tokio::test]
async fn v1_totp_enroll_and_verify_routes_enable_mfa() {
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
            "/auth/v1/register",
            r#"{"email":"pilot@example.com","password":"very-strong-password"}"#,
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

    let enroll_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/v1/mfa/totp/enroll",
            "",
            Some(&access_token),
        ))
        .await
        .expect("totp enroll response");
    assert_eq!(enroll_response.status(), StatusCode::OK);
    let enroll_json = response_json(enroll_response).await;
    assert!(
        enroll_json["provisioning_uri"]
            .as_str()
            .expect("provisioning uri")
            .starts_with("otpauth://totp/")
    );
    assert!(
        enroll_json["qr_svg"]
            .as_str()
            .expect("qr svg")
            .contains("<svg")
    );

    let manual_secret = enroll_json["manual_secret"]
        .as_str()
        .expect("manual secret");
    let secret = data_encoding::BASE32_NOPAD
        .decode(manual_secret.as_bytes())
        .expect("manual secret decode");
    let code = totp_code(
        &secret,
        now_epoch_s() / AuthConfig::for_tests().totp_step_s,
        6,
    )
    .expect("totp code");
    let verify_body = format!(
        r#"{{"enrollment_id":"{}","code":"{}"}}"#,
        enroll_json["enrollment_id"]
            .as_str()
            .expect("enrollment id"),
        code
    );
    let verify_response = app
        .oneshot(json_request(
            Method::POST,
            "/auth/v1/mfa/totp/verify",
            &verify_body,
            Some(&access_token),
        ))
        .await
        .expect("totp verify response");
    assert_eq!(verify_response.status(), StatusCode::OK);
    let verify_json = response_json(verify_response).await;
    assert_eq!(verify_json["accepted"].as_bool(), Some(true));
}

#[tokio::test]
async fn v1_password_login_returns_totp_challenge_for_mfa_account() {
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
            "/auth/v1/register",
            r#"{"email":"pilot@example.com","password":"very-strong-password"}"#,
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

    let enroll_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/v1/mfa/totp/enroll",
            "",
            Some(&access_token),
        ))
        .await
        .expect("totp enroll response");
    let enroll_json = response_json(enroll_response).await;
    let secret = data_encoding::BASE32_NOPAD
        .decode(
            enroll_json["manual_secret"]
                .as_str()
                .expect("manual secret")
                .as_bytes(),
        )
        .expect("manual secret decode");
    let code = totp_code(
        &secret,
        now_epoch_s() / AuthConfig::for_tests().totp_step_s,
        6,
    )
    .expect("totp code");
    let verify_body = format!(
        r#"{{"enrollment_id":"{}","code":"{}"}}"#,
        enroll_json["enrollment_id"]
            .as_str()
            .expect("enrollment id"),
        code
    );
    let verify_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/v1/mfa/totp/verify",
            &verify_body,
            Some(&access_token),
        ))
        .await
        .expect("totp verify response");
    assert_eq!(verify_response.status(), StatusCode::OK);
    let verify_json = response_json(verify_response).await;
    assert_eq!(verify_json["accepted"].as_bool(), Some(true));
    let enrollment_access_token = verify_json["tokens"]["access_token"]
        .as_str()
        .expect("enrollment access token");
    let enrollment_claims = service
        .decode_access_token(enrollment_access_token)
        .expect("decode enrollment access token");
    assert!(enrollment_claims.session_context.mfa_verified);
    assert_eq!(
        enrollment_claims.session_context.auth_method,
        "totp_enrollment"
    );

    let legacy_login_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/login",
            r#"{"email":"pilot@example.com","password":"very-strong-password"}"#,
            None,
        ))
        .await
        .expect("legacy password login response");
    assert_eq!(legacy_login_response.status(), StatusCode::UNAUTHORIZED);

    let login_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/auth/v1/login/password",
            r#"{"email":"pilot@example.com","password":"very-strong-password"}"#,
            None,
        ))
        .await
        .expect("password login response");
    assert_eq!(login_response.status(), StatusCode::OK);
    let login_json = response_json(login_response).await;
    assert_eq!(login_json["status"].as_str(), Some("mfa_required"));
    assert_eq!(login_json["challenge_type"].as_str(), Some("totp"));
    assert!(login_json["tokens"].is_null());

    let code = totp_code(
        &secret,
        now_epoch_s() / AuthConfig::for_tests().totp_step_s,
        6,
    )
    .expect("totp code");
    let challenge_body = format!(
        r#"{{"challenge_id":"{}","code":"{}"}}"#,
        login_json["challenge_id"].as_str().expect("challenge id"),
        code
    );
    let challenge_response = app
        .oneshot(json_request(
            Method::POST,
            "/auth/v1/login/challenge/totp",
            &challenge_body,
            None,
        ))
        .await
        .expect("totp challenge response");
    assert_eq!(challenge_response.status(), StatusCode::OK);
    let challenge_json = response_json(challenge_response).await;
    let access_token = challenge_json["access_token"]
        .as_str()
        .expect("access token");
    let claims = service
        .decode_access_token(access_token)
        .expect("decode access token");
    assert!(claims.session_context.mfa_verified);
    assert_eq!(claims.session_context.auth_method, "password_totp");
}

#[tokio::test]
async fn admin_spawn_entity_rejects_non_admin_caller() {
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

    let response = app
        .oneshot(json_request(
            Method::POST,
            "/admin/spawn-entity",
            r#"{"player_entity_id":"11111111-1111-1111-1111-111111111111","bundle_id":"corvette","overrides":{"display_name":"Test Corvette"}}"#,
            Some(&access_token),
        ))
        .await
        .expect("admin spawn response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_spawn_entity_rejects_invalid_player_id() {
    let dispatcher = Arc::new(RecordingBootstrapDispatcher::default());
    let service = Arc::new(AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        dispatcher,
        Arc::new(NoopStarterWorldPersister),
    ));
    let app = app_with_service(service.clone());
    let admin_token = signed_token_with_roles(
        &AuthConfig::for_tests().jwt_secret,
        Uuid::new_v4(),
        Uuid::new_v4(),
        vec!["admin".to_string()],
        vec!["admin:spawn".to_string()],
        true,
    );

    let response = app
        .oneshot(json_request(
            Method::POST,
            "/admin/spawn-entity",
            r#"{"player_entity_id":"not-a-uuid","bundle_id":"corvette","overrides":{"display_name":"Test Corvette"}}"#,
            Some(&admin_token),
        ))
        .await
        .expect("admin spawn response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn admin_spawn_entity_dispatches_for_admin_role() {
    let dispatcher = Arc::new(RecordingBootstrapDispatcher::default());
    let service = Arc::new(AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        dispatcher.clone(),
        Arc::new(NoopStarterWorldPersister),
    ));
    let app = app_with_service(service.clone());
    let admin_token = signed_token_with_roles(
        &AuthConfig::for_tests().jwt_secret,
        Uuid::new_v4(),
        Uuid::new_v4(),
        vec!["dev_tool".to_string()],
        vec!["admin:spawn".to_string()],
        true,
    );
    let target_player_id = Uuid::new_v4().to_string();

    let response = app
        .oneshot(json_request(
            Method::POST,
            "/admin/spawn-entity",
            &format!(
                r#"{{"player_entity_id":"{target_player_id}","bundle_id":"corvette","overrides":{{"display_name":"Dashboard Corvette"}}}}"#
            ),
            Some(&admin_token),
        ))
        .await
        .expect("admin spawn response");
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let spawned_entity_id = json["spawned_entity_id"]
        .as_str()
        .expect("spawned_entity_id");
    assert!(Uuid::parse_str(spawned_entity_id).is_ok());
    assert_eq!(
        json["owner_player_entity_id"]
            .as_str()
            .expect("owner_player_entity_id"),
        target_player_id
    );

    let spawn_commands = dispatcher.spawn_commands().await;
    assert_eq!(spawn_commands.len(), 1);
    assert_eq!(spawn_commands[0].bundle_id, "corvette");
    assert_eq!(spawn_commands[0].player_entity_id, target_player_id);
    assert_eq!(spawn_commands[0].requested_entity_id, spawned_entity_id);
}

#[tokio::test]
async fn admin_spawn_entity_returns_nondeterministic_entity_ids() {
    let dispatcher = Arc::new(RecordingBootstrapDispatcher::default());
    let service = Arc::new(AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        dispatcher,
        Arc::new(NoopStarterWorldPersister),
    ));
    let app = app_with_service(service.clone());
    let admin_token = signed_token_with_roles(
        &AuthConfig::for_tests().jwt_secret,
        Uuid::new_v4(),
        Uuid::new_v4(),
        vec!["admin".to_string()],
        vec!["admin:spawn".to_string()],
        true,
    );
    let target_player_id = Uuid::new_v4().to_string();
    let request_body = format!(
        r#"{{"player_entity_id":"{target_player_id}","bundle_id":"corvette","overrides":{{}}}}"#
    );

    let first = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/admin/spawn-entity",
            &request_body,
            Some(&admin_token),
        ))
        .await
        .expect("first spawn");
    let second = app
        .oneshot(json_request(
            Method::POST,
            "/admin/spawn-entity",
            &request_body,
            Some(&admin_token),
        ))
        .await
        .expect("second spawn");
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);
    let first_json = response_json(first).await;
    let second_json = response_json(second).await;
    let first_id = first_json["spawned_entity_id"]
        .as_str()
        .expect("first spawned_entity_id");
    let second_id = second_json["spawned_entity_id"]
        .as_str()
        .expect("second spawned_entity_id");
    assert_ne!(first_id, second_id);
}

#[tokio::test]
async fn admin_spawn_entity_requires_mfa_verified_token() {
    let service = Arc::new(AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
    ));
    let app = app_with_service(service.clone());
    let admin_token = signed_token_with_roles(
        &AuthConfig::for_tests().jwt_secret,
        Uuid::new_v4(),
        Uuid::new_v4(),
        vec!["admin".to_string()],
        vec!["admin:spawn".to_string()],
        false,
    );
    let target_player_id = Uuid::new_v4().to_string();

    let response = app
        .oneshot(json_request(
            Method::POST,
            "/admin/spawn-entity",
            &format!(
                r#"{{"player_entity_id":"{target_player_id}","bundle_id":"corvette","overrides":{{}}}}"#
            ),
            Some(&admin_token),
        ))
        .await
        .expect("admin spawn response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_spawn_entity_requires_route_scope() {
    let service = Arc::new(AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
    ));
    let app = app_with_service(service.clone());
    let admin_token = signed_token_with_roles(
        &AuthConfig::for_tests().jwt_secret,
        Uuid::new_v4(),
        Uuid::new_v4(),
        vec!["admin".to_string()],
        vec!["scripts:read".to_string()],
        true,
    );
    let target_player_id = Uuid::new_v4().to_string();

    let response = app
        .oneshot(json_request(
            Method::POST,
            "/admin/spawn-entity",
            &format!(
                r#"{{"player_entity_id":"{target_player_id}","bundle_id":"corvette","overrides":{{}}}}"#
            ),
            Some(&admin_token),
        ))
        .await
        .expect("admin spawn response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_scripts_routes_require_authentication() {
    let service = Arc::new(AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
    ));
    let app = app_with_service(service);

    let response = app
        .clone()
        .oneshot(json_request(Method::GET, "/admin/scripts", "", None))
        .await
        .expect("scripts response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let draft_response = app
        .oneshot(json_request(
            Method::POST,
            "/admin/scripts/draft/world/world_init.lua",
            r#"{"source":"return {}"}"#,
            None,
        ))
        .await
        .expect("draft response");
    assert_eq!(draft_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_scripts_routes_reject_non_admin_caller() {
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

    let response = app
        .clone()
        .oneshot(json_request(
            Method::GET,
            "/admin/scripts",
            "",
            Some(&access_token),
        ))
        .await
        .expect("scripts response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let publish_response = app
        .oneshot(json_request(
            Method::POST,
            "/admin/scripts/publish/world/world_init.lua",
            "",
            Some(&access_token),
        ))
        .await
        .expect("publish response");
    assert_eq!(publish_response.status(), StatusCode::UNAUTHORIZED);
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

fn extract_email_line(body: &str, prefix: &str) -> String {
    body.lines()
        .find_map(|line| line.strip_prefix(prefix))
        .map(str::to_string)
        .unwrap_or_else(|| panic!("missing {prefix} line in email body: {body}"))
}

fn signed_token_with_roles(
    jwt_secret: &str,
    account_id: Uuid,
    player_entity_id: Uuid,
    roles: Vec<String>,
    scopes: Vec<String>,
    mfa_verified: bool,
) -> String {
    let now_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_secs();
    let claims = AuthClaims {
        sub: account_id.to_string(),
        player_entity_id: player_entity_id.to_string(),
        roles,
        scope: scopes.join(" "),
        session_context: AuthSessionContext {
            auth_method: if mfa_verified {
                "password_totp".to_string()
            } else {
                "password".to_string()
            },
            mfa_verified,
            mfa_methods: if mfa_verified {
                vec!["totp".to_string()]
            } else {
                Vec::new()
            },
            active_scope: scopes,
            active_character_id: None,
        },
        iat: now_s,
        exp: now_s + 3600,
        jti: Uuid::new_v4().to_string(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .expect("token encode")
}
