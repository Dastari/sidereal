use sidereal_core::bootstrap_wire::BootstrapCommand;
use sidereal_gateway::auth::{
    AuthConfig, AuthService, BootstrapDispatcher, InMemoryAuthStore, NoopStarterWorldPersister,
    RecordingBootstrapDispatcher, UdpBootstrapDispatcher, hash_password, normalize_email,
    validate_password, verify_password,
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
        uuid::Uuid::parse_str(&claims.player_entity_id).is_ok(),
        "player_entity_id should be a valid UUID"
    );
    assert!(claims.exp > claims.iat);
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
async fn validation_rejects_invalid_email_and_short_password() {
    assert!(normalize_email("not-an-email").is_err());
    assert!(validate_password("short").is_err());
}

#[tokio::test]
async fn register_does_not_dispatch_bootstrap() {
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
    let commands = dispatcher.commands().await;
    assert!(
        commands.is_empty(),
        "registration should not implicitly bootstrap runtime world state"
    );
}

#[tokio::test]
async fn login_does_not_dispatch_bootstrap() {
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
