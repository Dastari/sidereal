use sidereal_replication::bootstrap::{BootstrapError, BootstrapProcessor, InMemoryBootstrapStore};
use uuid::Uuid;

fn payload(account_id: Uuid) -> Vec<u8> {
    let raw = format!(
        r#"{{"kind":"bootstrap_player","account_id":"{}","player_entity_id":"player:{}"}}"#,
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

    assert!(first.applied);
    assert!(!second.applied);
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
