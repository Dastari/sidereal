use sidereal_game::{Cost, Inventory, SiderealComponentMetadata, VisibilityScope};

#[test]
fn inventory_metadata_defaults_owner_visibility() {
    let meta = <Inventory as SiderealComponentMetadata>::META;
    assert_eq!(meta.kind, "inventory");
    assert!(meta.persist);
    assert!(meta.replicate);
    assert_eq!(meta.visibility, &[VisibilityScope::OwnerOnly]);
}

#[test]
fn cost_metadata_supports_visibility_array() {
    let meta = <Cost as SiderealComponentMetadata>::META;
    assert_eq!(meta.kind, "cost");
    assert!(meta.persist);
    assert!(meta.replicate);
    assert_eq!(
        meta.visibility,
        &[VisibilityScope::OwnerOnly, VisibilityScope::Public]
    );
}
