use sidereal_core::bootstrap_wire::BootstrapCommand;
use sidereal_gateway::auth::{
    AuthConfig, AuthService, AuthStore, BootstrapDispatcher, InMemoryAuthStore,
    NoopStarterWorldPersister, PasswordLoginResult, RecordingBootstrapDispatcher,
    RecordingEmailDelivery, UdpBootstrapDispatcher, hash_password, normalize_email, now_epoch_s,
    totp_code, validate_password, verify_password,
};
use sidereal_replication::bootstrap::{BootstrapProcessor, InMemoryBootstrapStore};
use std::sync::Arc;
use tokio::net::UdpSocket;
use uuid::Uuid;

#[tokio::test]
async fn password_hash_verify_roundtrip() {
    let hash = hash_password("very-strong-password").expect("hash");
    verify_password("very-strong-password", &hash).expect("verify");
}

#[tokio::test]
async fn jwt_claim_encode_decode_roundtrip() {
    let service = AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
    );
    let tokens = service
        .register("pilot@example.com", "very-strong-password")
        .await
        .expect("register");
    let claims = service
        .decode_access_token(&tokens.access_token)
        .expect("decode");
    assert!(
        claims.player_entity_id.is_empty(),
        "account registration should not mint a default character-bound token"
    );
    assert!(claims.exp > claims.iat);
}

#[tokio::test]
async fn issued_tokens_include_account_roles_and_scopes() {
    let store = Arc::new(InMemoryAuthStore::default());
    let service = AuthService::new_with_persister(
        AuthConfig::for_tests(),
        store.clone(),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
    );
    let tokens = service
        .register("pilot@example.com", "very-strong-password")
        .await
        .expect("register");
    let account_id = Uuid::parse_str(
        &service
            .decode_access_token(&tokens.access_token)
            .expect("decode access token")
            .sub,
    )
    .expect("account id");
    store
        .add_account_role(account_id, "admin")
        .await
        .expect("add role");
    store
        .add_account_scope(account_id, "admin:spawn")
        .await
        .expect("add admin scope");
    store
        .add_account_scope(account_id, "scripts:read")
        .await
        .expect("add script scope");
    assert!(
        store
            .add_account_scope(account_id, "scripts write")
            .await
            .is_err()
    );

    let login = service
        .login_password_v1("pilot@example.com", "very-strong-password")
        .await
        .expect("password login");
    let PasswordLoginResult::Authenticated { tokens } = login else {
        panic!("account without mfa should issue password tokens");
    };
    let claims = service
        .decode_access_token(&tokens.access_token)
        .expect("decode access token");

    assert_eq!(claims.roles, vec!["admin"]);
    assert_eq!(claims.scope, "admin:spawn scripts:read");
    assert_eq!(
        claims.session_context.active_scope,
        vec!["admin:spawn", "scripts:read"]
    );
}

#[tokio::test]
async fn first_admin_bootstrap_is_one_time_and_scoped() {
    let store = Arc::new(InMemoryAuthStore::default());
    let mut config = AuthConfig::for_tests();
    config.bootstrap_token = Some("setup-once".to_string());
    let service = AuthService::new_with_persister(
        config,
        store.clone(),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
    );

    assert!(
        service
            .bootstrap_required()
            .await
            .expect("bootstrap status")
    );
    let tokens = service
        .bootstrap_first_admin("admin@example.com", "very-strong-password", "setup-once")
        .await
        .expect("bootstrap first admin");
    assert!(
        !service
            .bootstrap_required()
            .await
            .expect("bootstrap status")
    );

    let claims = service
        .decode_access_token(&tokens.access_token)
        .expect("decode bootstrap token");
    assert_eq!(claims.roles, vec!["admin"]);
    assert!(claims.scope.contains("dashboard:access"));
    assert!(claims.scope.contains("admin:spawn"));
    assert_eq!(claims.session_context.auth_method, "bootstrap_token");
    assert!(claims.session_context.mfa_verified);

    let second = service
        .bootstrap_first_admin("other@example.com", "very-strong-password", "setup-once")
        .await;
    assert!(second.is_err());
}

#[tokio::test]
async fn refresh_token_rotation_invalidates_old_token() {
    let service = AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
    );
    let tokens = service
        .register("pilot@example.com", "very-strong-password")
        .await
        .expect("register");
    let new_tokens = service
        .refresh(&tokens.refresh_token)
        .await
        .expect("refresh");
    let old_refresh_result = service.refresh(&tokens.refresh_token).await;
    assert!(old_refresh_result.is_err());
    assert_ne!(new_tokens.refresh_token, tokens.refresh_token);
}

#[tokio::test]
async fn explicit_character_creation_adds_owned_character() {
    let service = AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
    );
    let tokens = service
        .register("pilot@example.com", "very-strong-password")
        .await
        .expect("register");

    let before = service
        .list_characters(&tokens.access_token)
        .await
        .expect("characters before creation");
    assert!(before.is_empty());

    let character = service
        .create_character(&tokens.access_token, "Talanah")
        .await
        .expect("create character");
    assert!(Uuid::parse_str(&character.player_entity_id).is_ok());
    assert_eq!(character.display_name, "Talanah");
    assert_eq!(character.status, "active");

    let after = service
        .list_characters(&tokens.access_token)
        .await
        .expect("characters after creation");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].player_entity_id, character.player_entity_id);

    let world_tokens = service
        .enter_world(&tokens.access_token, &character.player_entity_id)
        .await
        .expect("enter world");
    let world_claims = service
        .decode_access_token(&world_tokens.access_token)
        .expect("decode world token");
    assert_eq!(world_claims.player_entity_id, character.player_entity_id);
    assert_eq!(
        world_claims.session_context.active_character_id.as_deref(),
        Some(character.player_entity_id.as_str())
    );
    assert_eq!(world_claims.session_context.auth_method, "world_entry");

    let reset = service
        .reset_character(&tokens.access_token, &character.player_entity_id)
        .await
        .expect("reset character");
    assert_eq!(reset.player_entity_id, character.player_entity_id);
    assert_eq!(reset.display_name, "Talanah");

    service
        .delete_character(&tokens.access_token, &character.player_entity_id)
        .await
        .expect("delete character");
    let deleted = service
        .list_characters(&tokens.access_token)
        .await
        .expect("characters after deletion");
    assert!(deleted.is_empty());
}

#[tokio::test]
async fn validation_rejects_invalid_email_and_short_password() {
    assert!(normalize_email("not-an-email").is_err());
    assert!(validate_password("short").is_err());
}

#[tokio::test]
async fn auth_service_register_and_login_do_not_dispatch_bootstrap() {
    let dispatcher = Arc::new(RecordingBootstrapDispatcher::default());
    let service = AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        dispatcher.clone(),
        Arc::new(NoopStarterWorldPersister),
    );

    let _ = service
        .register("pilot@example.com", "very-strong-password")
        .await
        .expect("register");
    assert!(
        dispatcher.commands().await.is_empty(),
        "registration should not implicitly bootstrap runtime world state"
    );

    let _ = service
        .login("pilot@example.com", "very-strong-password")
        .await
        .expect("login");

    let commands = dispatcher.commands().await;
    assert!(
        commands.is_empty(),
        "login should not implicitly bootstrap runtime world state"
    );
}

#[tokio::test]
async fn email_login_unknown_account_is_accepted_without_delivery() {
    let email_delivery = Arc::new(RecordingEmailDelivery::default());
    let service = AuthService::new_with_dependencies(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
        email_delivery.clone(),
    );

    let result = service
        .request_email_login("unknown@example.com")
        .await
        .expect("email login request");

    assert!(result.accepted);
    assert!(email_delivery.messages().await.is_empty());
}

#[tokio::test]
async fn email_login_code_and_magic_token_issue_tokens_once() {
    let email_delivery = Arc::new(RecordingEmailDelivery::default());
    let mut config = AuthConfig::for_tests();
    config.email_resend_cooldown_s = 0;
    let service = AuthService::new_with_dependencies(
        config,
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
        email_delivery.clone(),
    );
    let _ = service
        .register("pilot@example.com", "very-strong-password")
        .await
        .expect("register");

    let result = service
        .request_email_login("pilot@example.com")
        .await
        .expect("email login request");
    assert!(result.accepted);
    let messages = email_delivery.messages().await;
    assert_eq!(messages.len(), 1);
    let challenge_id = extract_email_line(&messages[0].body_text, "Challenge ID: ");
    let code = extract_email_line(&messages[0].body_text, "Code: ");
    let tokens = service
        .verify_email_login(&challenge_id, Some(&code), None)
        .await
        .expect("email login verify");
    let claims = service
        .decode_access_token(&tokens.access_token)
        .expect("decode access token");
    assert!(!claims.sub.is_empty());

    let replay = service
        .verify_email_login(&challenge_id, Some(&code), None)
        .await;
    assert!(replay.is_err());

    let _ = service
        .request_email_login("pilot@example.com")
        .await
        .expect("second email login request");
    let messages = email_delivery.messages().await;
    assert_eq!(messages.len(), 2);
    let challenge_id = extract_email_line(&messages[1].body_text, "Challenge ID: ");
    let token = extract_email_line(&messages[1].body_text, "Magic token: ");
    let tokens = service
        .verify_email_login(&challenge_id, None, Some(&token))
        .await
        .expect("email login verify by token");
    assert!(
        service
            .decode_access_token(&tokens.access_token)
            .expect("decode token")
            .player_entity_id
            .is_empty()
    );
}

#[tokio::test]
async fn email_login_resend_cooldown_suppresses_duplicate_delivery() {
    let email_delivery = Arc::new(RecordingEmailDelivery::default());
    let service = AuthService::new_with_dependencies(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
        email_delivery.clone(),
    );
    let _ = service
        .register("pilot@example.com", "very-strong-password")
        .await
        .expect("register");

    let first = service
        .request_email_login("pilot@example.com")
        .await
        .expect("first email login request");
    let second = service
        .request_email_login("pilot@example.com")
        .await
        .expect("second email login request");

    assert!(first.accepted);
    assert!(second.accepted);
    assert_eq!(email_delivery.messages().await.len(), 1);
}

#[tokio::test]
async fn totp_enrollment_verification_enables_account_mfa() {
    let store = Arc::new(InMemoryAuthStore::default());
    let service = AuthService::new_with_persister(
        AuthConfig::for_tests(),
        store.clone(),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
    );
    let tokens = service
        .register("pilot@example.com", "very-strong-password")
        .await
        .expect("register");
    let enrollment = service
        .enroll_totp(&tokens.access_token)
        .await
        .expect("totp enroll");

    assert_eq!(enrollment.issuer, "Sidereal");
    assert!(enrollment.provisioning_uri.starts_with("otpauth://totp/"));
    assert!(enrollment.qr_svg.contains("<svg"));

    let secret = data_encoding::BASE32_NOPAD
        .decode(enrollment.manual_secret.as_bytes())
        .expect("manual secret should decode");
    let code = totp_code(
        &secret,
        now_epoch_s() / AuthConfig::for_tests().totp_step_s,
        6,
    )
    .expect("totp code");
    let verified_tokens = service
        .verify_totp_enrollment(
            &tokens.access_token,
            &enrollment.enrollment_id.to_string(),
            &code,
        )
        .await
        .expect("totp verify");

    let verified_claims = service
        .decode_access_token(&verified_tokens.access_token)
        .expect("decode verified tokens");
    assert_eq!(
        verified_claims.session_context.auth_method,
        "totp_enrollment"
    );
    assert!(verified_claims.session_context.mfa_verified);
    assert_eq!(verified_claims.session_context.mfa_methods, vec!["totp"]);
    assert!(
        store
            .account_has_verified_totp(
                Uuid::parse_str(
                    &service
                        .decode_access_token(&tokens.access_token)
                        .expect("decode")
                        .sub,
                )
                .expect("account uuid"),
            )
            .await
            .expect("totp state")
    );
}

#[tokio::test]
async fn password_login_requires_totp_after_mfa_enabled() {
    let service = AuthService::new_with_persister(
        AuthConfig::for_tests(),
        Arc::new(InMemoryAuthStore::default()),
        Arc::new(RecordingBootstrapDispatcher::default()),
        Arc::new(NoopStarterWorldPersister),
    );
    let tokens = service
        .register("pilot@example.com", "very-strong-password")
        .await
        .expect("register");
    let enrollment = service
        .enroll_totp(&tokens.access_token)
        .await
        .expect("totp enroll");
    let secret = data_encoding::BASE32_NOPAD
        .decode(enrollment.manual_secret.as_bytes())
        .expect("manual secret should decode");
    let code = totp_code(
        &secret,
        now_epoch_s() / AuthConfig::for_tests().totp_step_s,
        6,
    )
    .expect("totp code");
    let _ = service
        .verify_totp_enrollment(
            &tokens.access_token,
            &enrollment.enrollment_id.to_string(),
            &code,
        )
        .await
        .expect("totp verify");

    let login = service
        .login_password_v1("pilot@example.com", "very-strong-password")
        .await
        .expect("password login");
    let challenge_id = match login {
        PasswordLoginResult::TotpRequired { challenge_id, .. } => challenge_id,
        PasswordLoginResult::Authenticated { .. } => panic!("expected totp challenge"),
    };
    let code = totp_code(
        &secret,
        now_epoch_s() / AuthConfig::for_tests().totp_step_s,
        6,
    )
    .expect("totp code");
    let mfa_tokens = service
        .verify_totp_login_challenge(&challenge_id.to_string(), &code)
        .await
        .expect("totp login challenge");
    let claims = service
        .decode_access_token(&mfa_tokens.access_token)
        .expect("decode mfa token");

    assert_eq!(claims.session_context.auth_method, "password_totp");
    assert!(claims.session_context.mfa_verified);
    assert_eq!(claims.session_context.mfa_methods, vec!["totp"]);
    assert!(
        service
            .verify_totp_login_challenge(&challenge_id.to_string(), &code)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn udp_bootstrap_dispatcher_sends_bootstrap_player_message() {
    let listener = UdpSocket::bind("127.0.0.1:0").await.expect("bind listener");
    let target = listener.local_addr().expect("local addr");
    let sender = UdpSocket::bind("127.0.0.1:0").await.expect("bind sender");
    let dispatcher = UdpBootstrapDispatcher::new(sender, target);
    let player_entity_id = Uuid::new_v4().to_string();
    let command = BootstrapCommand {
        account_id: Uuid::new_v4(),
        player_entity_id: player_entity_id.clone(),
    };

    dispatcher.dispatch(&command).await.expect("dispatch");
    let mut buf = [0_u8; 2048];
    let (size, _) = listener.recv_from(&mut buf).await.expect("recv");
    let msg: serde_json::Value = serde_json::from_slice(&buf[..size]).expect("json");

    assert_eq!(msg["kind"], "bootstrap_player");
    assert_eq!(msg["account_id"], command.account_id.to_string());
    assert_eq!(msg["player_entity_id"], player_entity_id);
}

fn extract_email_line(body: &str, prefix: &str) -> String {
    body.lines()
        .find_map(|line| line.strip_prefix(prefix))
        .map(str::to_string)
        .unwrap_or_else(|| panic!("missing {prefix} line in email body: {body}"))
}

#[tokio::test]
async fn gateway_udp_bootstrap_message_roundtrips_with_replication_processor() {
    let listener = UdpSocket::bind("127.0.0.1:0").await.expect("bind listener");
    let target = listener.local_addr().expect("local addr");
    let sender = UdpSocket::bind("127.0.0.1:0").await.expect("bind sender");
    let dispatcher = UdpBootstrapDispatcher::new(sender, target);
    let account_id = Uuid::new_v4();
    let command = BootstrapCommand {
        account_id,
        player_entity_id: account_id.to_string(),
    };

    dispatcher.dispatch(&command).await.expect("dispatch");
    let mut buf = [0_u8; 2048];
    let (size, _) = listener.recv_from(&mut buf).await.expect("recv");

    let store = InMemoryBootstrapStore::default();
    let mut processor = BootstrapProcessor::new(store).expect("processor");
    let first = processor
        .handle_payload(&buf[..size])
        .expect("first apply should succeed");
    let second = processor
        .handle_payload(&buf[..size])
        .expect("second apply should succeed");

    match first {
        sidereal_replication::bootstrap::ControlHandleResult::Bootstrap(result) => {
            assert_eq!(result.account_id, account_id);
            assert_eq!(result.player_entity_id, account_id.to_string());
            assert!(result.applied);
        }
        _ => panic!("expected bootstrap result"),
    }
    match second {
        sidereal_replication::bootstrap::ControlHandleResult::Bootstrap(result) => {
            assert!(!result.applied);
        }
        _ => panic!("expected bootstrap result"),
    }
}
