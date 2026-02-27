use crate::replication::view::guid_from_entity_id_like;

#[test]
fn parses_prefixed_or_raw_guid() {
    let guid = uuid::Uuid::new_v4();
    assert_eq!(
        guid_from_entity_id_like(&format!("ship:{guid}")),
        Some(guid.to_string())
    );
    assert_eq!(
        guid_from_entity_id_like(&guid.to_string()),
        Some(guid.to_string())
    );
}

#[test]
fn rejects_invalid_identifier() {
    assert_eq!(guid_from_entity_id_like("ship:not-a-guid"), None);
    assert_eq!(guid_from_entity_id_like("definitely-not-a-guid"), None);
}
