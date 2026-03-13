use sidereal_ui::{UiThemeId, theme_definition};

#[test]
fn tron_theme_uses_expected_name_and_primary_hue() {
    let theme = theme_definition(UiThemeId::Tron);
    assert_eq!(theme.name, "tron");
    assert!((theme.colors.primary.hue - 195.0).abs() < f32::EPSILON);
}

#[test]
fn poseidon_and_aphrodite_are_distinct_palettes() {
    let poseidon = theme_definition(UiThemeId::Poseidon);
    let aphrodite = theme_definition(UiThemeId::Aphrodite);
    assert_ne!(poseidon.colors.primary.hue, aphrodite.colors.primary.hue);
    assert_ne!(poseidon.name, aphrodite.name);
}
