use sidereal_game::{
    AsteroidField, AsteroidFieldAmbient, AsteroidFieldDamageState, AsteroidFieldLayout,
    AsteroidFieldMember, AsteroidFieldPopulation, AsteroidFractureProfile, AsteroidResourceProfile,
    CollisionOutlineM, CollisionProfile, ContactResolutionM, Cost, Destructible, Inventory,
    ScannerComponent, SiderealComponentMetadata, SignalSignature, ThrusterPlumeShaderSettings,
    VisibilityScope,
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
fn signal_signature_metadata_is_public_and_persisted() {
    let meta = <SignalSignature as SiderealComponentMetadata>::META;
    assert_eq!(meta.kind, "signal_signature");
    assert!(meta.persist);
    assert!(meta.replicate);
    assert_eq!(meta.visibility, &[VisibilityScope::Public]);
}

#[test]
fn contact_resolution_metadata_is_owner_only_and_persisted() {
    let meta = <ContactResolutionM as SiderealComponentMetadata>::META;
    assert_eq!(meta.kind, "contact_resolution_m");
    assert!(meta.persist);
    assert!(meta.replicate);
    assert_eq!(meta.visibility, &[VisibilityScope::OwnerOnly]);
}

#[test]
fn scanner_component_metadata_is_owner_only_and_persisted() {
    let meta = <ScannerComponent as SiderealComponentMetadata>::META;
    assert_eq!(meta.kind, "scanner_component");
    assert!(meta.persist);
    assert!(meta.replicate);
    assert_eq!(meta.visibility, &[VisibilityScope::OwnerOnly]);
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

#[test]
fn asteroid_field_v2_public_metadata_is_persisted_and_replicated() {
    for (kind, persist, replicate, visibility) in [
        (
            <AsteroidField as SiderealComponentMetadata>::META.kind,
            <AsteroidField as SiderealComponentMetadata>::META.persist,
            <AsteroidField as SiderealComponentMetadata>::META.replicate,
            <AsteroidField as SiderealComponentMetadata>::META.visibility,
        ),
        (
            <AsteroidFieldLayout as SiderealComponentMetadata>::META.kind,
            <AsteroidFieldLayout as SiderealComponentMetadata>::META.persist,
            <AsteroidFieldLayout as SiderealComponentMetadata>::META.replicate,
            <AsteroidFieldLayout as SiderealComponentMetadata>::META.visibility,
        ),
        (
            <AsteroidFieldPopulation as SiderealComponentMetadata>::META.kind,
            <AsteroidFieldPopulation as SiderealComponentMetadata>::META.persist,
            <AsteroidFieldPopulation as SiderealComponentMetadata>::META.replicate,
            <AsteroidFieldPopulation as SiderealComponentMetadata>::META.visibility,
        ),
        (
            <AsteroidFieldMember as SiderealComponentMetadata>::META.kind,
            <AsteroidFieldMember as SiderealComponentMetadata>::META.persist,
            <AsteroidFieldMember as SiderealComponentMetadata>::META.replicate,
            <AsteroidFieldMember as SiderealComponentMetadata>::META.visibility,
        ),
        (
            <AsteroidFieldAmbient as SiderealComponentMetadata>::META.kind,
            <AsteroidFieldAmbient as SiderealComponentMetadata>::META.persist,
            <AsteroidFieldAmbient as SiderealComponentMetadata>::META.replicate,
            <AsteroidFieldAmbient as SiderealComponentMetadata>::META.visibility,
        ),
    ] {
        assert!(kind.starts_with("asteroid_"));
        assert!(persist);
        assert!(replicate);
        assert_eq!(visibility, &[VisibilityScope::Public]);
    }
}

#[test]
fn asteroid_field_v2_server_owned_metadata_is_not_replicated() {
    for (kind, persist, replicate, visibility) in [
        (
            <AsteroidFieldDamageState as SiderealComponentMetadata>::META.kind,
            <AsteroidFieldDamageState as SiderealComponentMetadata>::META.persist,
            <AsteroidFieldDamageState as SiderealComponentMetadata>::META.replicate,
            <AsteroidFieldDamageState as SiderealComponentMetadata>::META.visibility,
        ),
        (
            <AsteroidFractureProfile as SiderealComponentMetadata>::META.kind,
            <AsteroidFractureProfile as SiderealComponentMetadata>::META.persist,
            <AsteroidFractureProfile as SiderealComponentMetadata>::META.replicate,
            <AsteroidFractureProfile as SiderealComponentMetadata>::META.visibility,
        ),
        (
            <AsteroidResourceProfile as SiderealComponentMetadata>::META.kind,
            <AsteroidResourceProfile as SiderealComponentMetadata>::META.persist,
            <AsteroidResourceProfile as SiderealComponentMetadata>::META.replicate,
            <AsteroidResourceProfile as SiderealComponentMetadata>::META.visibility,
        ),
    ] {
        assert!(kind.starts_with("asteroid_"));
        assert!(persist);
        assert!(!replicate);
        assert_eq!(visibility, &[VisibilityScope::OwnerOnly]);
    }
}
