use sidereal_replication::bootstrap::{
    BootstrapError, BootstrapProcessor, ControlHandleResult, InMemoryBootstrapStore,
};
use uuid::Uuid;

fn payload(account_id: Uuid) -> Vec<u8> {
    let raw = format!(
        r#"{{"kind":"bootstrap_player","account_id":"{}","player_entity_id":"{}"}}"#,
        account_id, account_id
    );
    raw.into_bytes()
}

#[test]
fn bootstrap_processor_is_idempotent_per_player_entity() {
    let store = InMemoryBootstrapStore::default();
    let mut processor = BootstrapProcessor::new(store).expect("processor");
    let account_id = Uuid::new_v4();

    let first = processor
        .handle_payload(&payload(account_id))
        .expect("first");
    let second = processor
        .handle_payload(&payload(account_id))
        .expect("second");

    match first {
        ControlHandleResult::Bootstrap(result) => assert!(result.applied),
        _ => panic!("expected bootstrap result"),
    }
    match second {
        ControlHandleResult::Bootstrap(result) => assert!(!result.applied),
        _ => panic!("expected bootstrap result"),
    }
}

#[test]
fn bootstrap_processor_rejects_invalid_player_entity_format() {
    let store = InMemoryBootstrapStore::default();
    let mut processor = BootstrapProcessor::new(store).expect("processor");
    let account_id = Uuid::new_v4();
    let bad = format!(
        r#"{{"kind":"bootstrap_player","account_id":"{}","player_entity_id":"wrong"}}"#,
        account_id
    );

    let err = processor
        .handle_payload(bad.as_bytes())
        .expect_err("expected validation error");
    match err {
        BootstrapError::Validation(message) => {
            assert!(message.contains("player_entity_id"));
        }
        _ => panic!("expected validation error"),
    }
}

#[test]
fn bootstrap_processor_accepts_admin_spawn_payload() {
    let store = InMemoryBootstrapStore::default();
    let mut processor = BootstrapProcessor::new(store).expect("processor");
    let actor_account_id = Uuid::new_v4();
    let actor_player_entity_id = Uuid::new_v4();
    let target_player_entity_id = Uuid::new_v4();
    let request_id = Uuid::new_v4();
    let requested_entity_id = Uuid::new_v4();
    let payload = format!(
        r#"{{"kind":"admin_spawn_entity","actor_account_id":"{}","actor_player_entity_id":"{}","request_id":"{}","player_entity_id":"{}","bundle_id":"corvette","requested_entity_id":"{}","overrides":{{"display_name":"Test Corvette"}}}}"#,
        actor_account_id,
        actor_player_entity_id,
        request_id,
        target_player_entity_id,
        requested_entity_id
    );

    let result = processor
        .handle_payload(payload.as_bytes())
        .expect("admin spawn should decode");
    match result {
        ControlHandleResult::AdminSpawn(command) => {
            assert_eq!(command.actor_account_id, actor_account_id);
            assert_eq!(
                command.actor_player_entity_id,
                actor_player_entity_id.to_string()
            );
            assert_eq!(
                command.player_entity_id,
                target_player_entity_id.to_string()
            );
            assert_eq!(command.bundle_id, "corvette");
            assert_eq!(command.requested_entity_id, requested_entity_id.to_string());
        }
        _ => panic!("expected admin spawn command"),
    }
}

#[test]
fn bootstrap_processor_rejects_admin_spawn_with_invalid_player_id() {
    let store = InMemoryBootstrapStore::default();
    let mut processor = BootstrapProcessor::new(store).expect("processor");
    let payload = format!(
        r#"{{"kind":"admin_spawn_entity","actor_account_id":"{}","actor_player_entity_id":"{}","request_id":"{}","player_entity_id":"bad-player-id","bundle_id":"corvette","requested_entity_id":"{}","overrides":{{}}}}"#,
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4()
    );

    let err = processor
        .handle_payload(payload.as_bytes())
        .expect_err("expected invalid player id");
    match err {
        BootstrapError::Validation(message) => {
            assert!(message.contains("player_entity_id must be a valid UUID"));
        }
        _ => panic!("expected validation error"),
    }
}
