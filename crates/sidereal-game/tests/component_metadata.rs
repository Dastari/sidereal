use sidereal_game::{
    CollisionOutlineM, CollisionProfile, Cost, Destructible, Inventory, SiderealComponentMetadata,
    ThrusterPlumeShaderSettings, VisibilityScope,
};

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

#[test]
fn collision_profile_metadata_is_public_and_persisted() {
    let meta = <CollisionProfile as SiderealComponentMetadata>::META;
    assert_eq!(meta.kind, "collision_profile");
    assert!(meta.persist);
    assert!(meta.replicate);
    assert_eq!(meta.visibility, &[VisibilityScope::Public]);
}

#[test]
fn collision_outline_metadata_is_public_and_persisted() {
    let meta = <CollisionOutlineM as SiderealComponentMetadata>::META;
    assert_eq!(meta.kind, "collision_outline_m");
    assert!(meta.persist);
    assert!(meta.replicate);
    assert_eq!(meta.visibility, &[VisibilityScope::Public]);
}

#[test]
fn destructible_metadata_is_persisted_without_replication() {
    let meta = <Destructible as SiderealComponentMetadata>::META;
    assert_eq!(meta.kind, "destructible");
    assert!(meta.persist);
    assert!(!meta.replicate);
    assert_eq!(meta.visibility, &[VisibilityScope::OwnerOnly]);
}

#[test]
fn thruster_plume_settings_metadata_is_public() {
    let meta = <ThrusterPlumeShaderSettings as SiderealComponentMetadata>::META;
    assert_eq!(meta.kind, "thruster_plume_shader_settings");
    assert!(meta.persist);
    assert!(meta.replicate);
    assert_eq!(meta.visibility, &[VisibilityScope::Public]);
}
