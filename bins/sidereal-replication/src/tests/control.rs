use crate::replication::control::guid_from_entity_id_like;

#[test]
fn parses_raw_guid_only() {
    let guid = uuid::Uuid::new_v4();
    assert_eq!(
        guid_from_entity_id_like(&guid.to_string()),
        Some(guid.to_string())
    );
}

#[test]
fn rejects_invalid_identifier() {
    assert_eq!(guid_from_entity_id_like("ship:not-a-guid"), None);
    assert_eq!(
        guid_from_entity_id_like("ship:11111111-1111-1111-1111-111111111111"),
        None
    );
    assert_eq!(guid_from_entity_id_like("definitely-not-a-guid"), None);
}
