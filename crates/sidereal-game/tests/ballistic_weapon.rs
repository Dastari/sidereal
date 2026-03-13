use sidereal_game::BallisticWeapon;

#[test]
fn ballistic_weapon_deserializes_without_audio_profile() {
    let weapon: BallisticWeapon = serde_json::from_value(serde_json::json!({
        "weapon_name": "Test Weapon",
        "rpm": 120.0,
        "damage_per_shot": 4.0,
        "max_range_m": 40.0,
        "projectile_speed_mps": 0.0,
        "spread_rad": 0.0,
        "damage_type": "Ballistic"
    }))
    .expect("weapon should deserialize");

    assert_eq!(weapon.fire_audio_profile_id, None);
}
