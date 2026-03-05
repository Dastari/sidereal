use sidereal_game::entities::ship::corvette::default_corvette_mass_kg;

#[test]
fn corvette_total_mass() {
    let hull_mass = default_corvette_mass_kg();
    let total = hull_mass + 50.0 + 500.0 + 2.0 * 1100.0 + 120.0;
    assert_eq!(total, 17_870.0);
}
