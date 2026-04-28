use sidereal_ui::{UiSemanticTone, UiThemeId, theme_definition};

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

#[test]
fn warning_tone_uses_white_foreground_and_accent_chrome() {
    let theme = theme_definition(UiThemeId::Tron);

    let foreground = UiSemanticTone::Warning.foreground_color(theme).to_srgba();
    let chrome = UiSemanticTone::Warning.chrome_color(theme).to_srgba();
    let accent = UiSemanticTone::Warning.accent_color(theme).to_srgba();

    assert!((foreground.red - 1.0).abs() < f32::EPSILON);
    assert!((foreground.green - 1.0).abs() < f32::EPSILON);
    assert!((foreground.blue - 1.0).abs() < f32::EPSILON);
    assert!((chrome.red - accent.red).abs() < f32::EPSILON);
    assert!((chrome.green - accent.green).abs() < f32::EPSILON);
    assert!((chrome.blue - accent.blue).abs() < f32::EPSILON);
}
