use sidereal_core::{EntityId, PROTOCOL_VERSION, RENDER_TARGET_HZ, SIM_TICK_HZ};

#[test]
fn entity_id_new_v4_is_not_nil() {
    let id = EntityId::new_v4();
    assert!(!id.is_nil());
}

#[test]
fn entity_id_new_v4_is_unique() {
    let a = EntityId::new_v4();
    let b = EntityId::new_v4();
    assert_ne!(a, b);
}

#[test]
fn baseline_constants_match_design_defaults() {
    assert_eq!(PROTOCOL_VERSION, 1);
    assert_eq!(SIM_TICK_HZ, 60);
    assert_eq!(RENDER_TARGET_HZ, 60);
}
