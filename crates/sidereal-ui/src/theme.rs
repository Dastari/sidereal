use bevy::color::Oklcha;
use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiThemeId {
    Tron,
    Ares,
    Clu,
    Athena,
    Aphrodite,
    Poseidon,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveUiTheme(pub UiThemeId);

impl Default for ActiveUiTheme {
    fn default() -> Self {
        Self(UiThemeId::Tron)
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct UiVisualSettings {
    pub glow_intensity: f32,
}

impl UiVisualSettings {
    pub fn glow_intensity(self) -> f32 {
        self.glow_intensity.max(0.0)
    }
}

impl Default for UiVisualSettings {
    fn default() -> Self {
        Self {
            glow_intensity: 0.3,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UiTheme {
    pub id: UiThemeId,
    pub name: &'static str,
    pub colors: UiThemeColors,
    pub metrics: UiMetrics,
}

#[derive(Debug, Clone, Copy)]
pub struct UiMetrics {
    pub panel_padding_px: f32,
    pub panel_radius_px: f32,
    pub control_radius_px: f32,
    pub input_radius_px: f32,
    pub panel_border_px: f32,
    pub control_border_px: f32,
    pub row_gap_px: f32,
    pub panel_shadow_blur_px: f32,
    pub panel_shadow_spread_px: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct UiThemeColors {
    pub background: Oklcha,
    pub foreground: Oklcha,
    pub panel: Oklcha,
    pub panel_foreground: Oklcha,
    pub card: Oklcha,
    pub card_foreground: Oklcha,
    pub popover: Oklcha,
    pub popover_foreground: Oklcha,
    pub primary: Oklcha,
    pub primary_foreground: Oklcha,
    pub secondary: Oklcha,
    pub secondary_foreground: Oklcha,
    pub accent: Oklcha,
    pub accent_foreground: Oklcha,
    pub muted: Oklcha,
    pub muted_foreground: Oklcha,
    pub border: Oklcha,
    pub input: Oklcha,
    pub ring: Oklcha,
    pub destructive: Oklcha,
    pub warning: Oklcha,
    pub success: Oklcha,
    pub info: Oklcha,
    pub glow: Oklcha,
    pub glow_muted: Oklcha,
    pub overlay: Oklcha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiSemanticTone {
    Info,
    Success,
    Warning,
    Danger,
}

impl UiThemeColors {
    pub fn background_color(self) -> Color {
        self.background.into()
    }

    pub fn foreground_color(self) -> Color {
        self.foreground.into()
    }

    pub fn panel_color(self) -> Color {
        self.panel.into()
    }

    pub fn panel_foreground_color(self) -> Color {
        self.panel_foreground.into()
    }

    pub fn primary_color(self) -> Color {
        self.primary.into()
    }

    pub fn primary_foreground_color(self) -> Color {
        self.primary_foreground.into()
    }

    pub fn secondary_color(self) -> Color {
        self.secondary.into()
    }

    pub fn secondary_foreground_color(self) -> Color {
        self.secondary_foreground.into()
    }

    pub fn accent_color(self) -> Color {
        self.accent.into()
    }

    pub fn accent_foreground_color(self) -> Color {
        self.accent_foreground.into()
    }

    pub fn muted_color(self) -> Color {
        self.muted.into()
    }

    pub fn muted_foreground_color(self) -> Color {
        self.muted_foreground.into()
    }

    pub fn border_color(self) -> Color {
        self.border.into()
    }

    pub fn input_color(self) -> Color {
        self.input.into()
    }

    pub fn ring_color(self) -> Color {
        self.ring.into()
    }

    pub fn destructive_color(self) -> Color {
        self.destructive.into()
    }

    pub fn warning_color(self) -> Color {
        self.warning.into()
    }

    pub fn success_color(self) -> Color {
        self.success.into()
    }

    pub fn info_color(self) -> Color {
        self.info.into()
    }

    pub fn glow_color(self) -> Color {
        self.glow.into()
    }

    pub fn glow_muted_color(self) -> Color {
        self.glow_muted.into()
    }

    pub fn overlay_color(self) -> Color {
        self.overlay.into()
    }
}

impl UiSemanticTone {
    pub fn accent_token(self, theme: UiTheme) -> Oklcha {
        match self {
            Self::Info => theme.colors.info,
            Self::Success => theme.colors.success,
            Self::Warning => theme.colors.warning,
            Self::Danger => theme.colors.destructive,
        }
    }

    pub fn accent_color(self, theme: UiTheme) -> Color {
        self.accent_token(theme).into()
    }

    pub fn foreground_color(self, theme: UiTheme) -> Color {
        match self {
            Self::Info => theme.colors.foreground_color(),
            Self::Warning | Self::Danger => Color::WHITE,
            Self::Success => Color::srgb(0.04, 0.04, 0.04),
        }
    }

    pub fn chrome_color(self, theme: UiTheme) -> Color {
        self.accent_color(theme)
    }
}

pub fn color(token: Oklcha) -> Color {
    token.into()
}

pub fn with_alpha(token: Oklcha, alpha: f32) -> Oklcha {
    token.with_alpha(alpha)
}

pub fn theme_definition(theme_id: UiThemeId) -> UiTheme {
    let shared = shared_colors();
    let metrics = UiMetrics {
        panel_padding_px: 28.0,
        panel_radius_px: 0.0,
        control_radius_px: 0.0,
        input_radius_px: 4.0,
        panel_border_px: 1.0,
        control_border_px: 1.0,
        row_gap_px: 14.0,
        panel_shadow_blur_px: 28.0,
        panel_shadow_spread_px: 6.0,
    };

    match theme_id {
        UiThemeId::Tron => UiTheme {
            id: theme_id,
            name: "tron",
            colors: UiThemeColors {
                primary: oklch(0.75, 0.18, 195.0),
                primary_foreground: oklch(0.98, 0.0, 0.0),
                secondary: oklch(0.18, 0.05, 200.0),
                secondary_foreground: oklch(0.95, 0.0, 0.0),
                accent: oklch(0.7, 0.15, 195.0),
                accent_foreground: oklch(0.1, 0.0, 0.0),
                muted: oklch(0.15, 0.03, 200.0),
                muted_foreground: oklch(0.65, 0.0, 0.0),
                border: oklch(0.3, 0.1, 195.0),
                input: oklch(0.2, 0.06, 195.0),
                ring: oklch(0.75, 0.18, 195.0),
                glow: oklch(0.75, 0.18, 195.0),
                glow_muted: oklch(0.5, 0.12, 195.0),
                ..shared
            },
            metrics,
        },
        UiThemeId::Ares => UiTheme {
            id: theme_id,
            name: "ares",
            colors: UiThemeColors {
                primary: oklch(0.6, 0.25, 25.0),
                primary_foreground: oklch(0.98, 0.0, 0.0),
                secondary: oklch(0.65, 0.18, 55.0),
                secondary_foreground: oklch(0.1, 0.0, 0.0),
                accent: oklch(0.7, 0.2, 50.0),
                accent_foreground: oklch(0.1, 0.0, 0.0),
                muted: oklch(0.15, 0.02, 250.0),
                muted_foreground: oklch(0.6, 0.04, 220.0),
                border: oklch(0.3, 0.12, 25.0),
                input: oklch(0.15, 0.04, 250.0),
                ring: oklch(0.6, 0.25, 25.0),
                glow: oklch(0.6, 0.25, 25.0),
                glow_muted: oklch(0.4, 0.15, 25.0),
                ..shared
            },
            metrics,
        },
        UiThemeId::Clu => UiTheme {
            id: theme_id,
            name: "clu",
            colors: UiThemeColors {
                primary: oklch(0.75, 0.2, 55.0),
                primary_foreground: oklch(0.98, 0.0, 0.0),
                secondary: oklch(0.18, 0.06, 50.0),
                secondary_foreground: oklch(0.95, 0.0, 0.0),
                accent: oklch(0.7, 0.18, 55.0),
                accent_foreground: oklch(0.1, 0.0, 0.0),
                muted: oklch(0.15, 0.04, 50.0),
                muted_foreground: oklch(0.65, 0.0, 0.0),
                border: oklch(0.3, 0.1, 55.0),
                input: oklch(0.2, 0.06, 55.0),
                ring: oklch(0.75, 0.2, 55.0),
                glow: oklch(0.75, 0.2, 55.0),
                glow_muted: oklch(0.5, 0.12, 55.0),
                ..shared
            },
            metrics,
        },
        UiThemeId::Athena => UiTheme {
            id: theme_id,
            name: "athena",
            colors: UiThemeColors {
                primary: oklch(0.85, 0.18, 90.0),
                primary_foreground: oklch(0.98, 0.0, 0.0),
                secondary: oklch(0.18, 0.05, 85.0),
                secondary_foreground: oklch(0.95, 0.0, 0.0),
                accent: oklch(0.8, 0.15, 90.0),
                accent_foreground: oklch(0.1, 0.0, 0.0),
                muted: oklch(0.15, 0.03, 85.0),
                muted_foreground: oklch(0.65, 0.0, 0.0),
                border: oklch(0.35, 0.1, 90.0),
                input: oklch(0.2, 0.06, 90.0),
                ring: oklch(0.85, 0.18, 90.0),
                glow: oklch(0.85, 0.18, 90.0),
                glow_muted: oklch(0.55, 0.12, 90.0),
                ..shared
            },
            metrics,
        },
        UiThemeId::Aphrodite => UiTheme {
            id: theme_id,
            name: "aphrodite",
            colors: UiThemeColors {
                primary: oklch(0.7, 0.22, 340.0),
                primary_foreground: oklch(0.98, 0.0, 0.0),
                secondary: oklch(0.18, 0.06, 340.0),
                secondary_foreground: oklch(0.95, 0.0, 0.0),
                accent: oklch(0.65, 0.2, 340.0),
                accent_foreground: oklch(0.98, 0.0, 0.0),
                muted: oklch(0.15, 0.04, 340.0),
                muted_foreground: oklch(0.65, 0.0, 0.0),
                border: oklch(0.3, 0.1, 340.0),
                input: oklch(0.2, 0.06, 340.0),
                ring: oklch(0.7, 0.22, 340.0),
                glow: oklch(0.7, 0.22, 340.0),
                glow_muted: oklch(0.5, 0.15, 340.0),
                ..shared
            },
            metrics,
        },
        UiThemeId::Poseidon => UiTheme {
            id: theme_id,
            name: "poseidon",
            colors: UiThemeColors {
                primary: oklch(0.6, 0.2, 250.0),
                primary_foreground: oklch(0.98, 0.0, 0.0),
                secondary: oklch(0.18, 0.06, 250.0),
                secondary_foreground: oklch(0.95, 0.0, 0.0),
                accent: oklch(0.55, 0.18, 250.0),
                accent_foreground: oklch(0.98, 0.0, 0.0),
                muted: oklch(0.15, 0.04, 250.0),
                muted_foreground: oklch(0.65, 0.0, 0.0),
                border: oklch(0.28, 0.1, 250.0),
                input: oklch(0.2, 0.06, 250.0),
                ring: oklch(0.6, 0.2, 250.0),
                glow: oklch(0.6, 0.2, 250.0),
                glow_muted: oklch(0.4, 0.12, 250.0),
                ..shared
            },
            metrics,
        },
    }
}

const fn shared_colors() -> UiThemeColors {
    UiThemeColors {
        background: oklch(0.06, 0.02, 250.0),
        foreground: oklch(0.95, 0.02, 220.0),
        panel: oklcha(0.1, 0.02, 250.0, 0.92),
        panel_foreground: oklch(0.92, 0.02, 220.0),
        card: oklcha(0.1, 0.02, 250.0, 0.92),
        card_foreground: oklch(0.92, 0.02, 220.0),
        popover: oklcha(0.1, 0.02, 250.0, 0.96),
        popover_foreground: oklch(0.92, 0.02, 220.0),
        primary: oklch(0.75, 0.18, 195.0),
        primary_foreground: oklch(0.98, 0.0, 0.0),
        secondary: oklch(0.18, 0.05, 200.0),
        secondary_foreground: oklch(0.95, 0.0, 0.0),
        accent: oklch(0.7, 0.15, 195.0),
        accent_foreground: oklch(0.1, 0.0, 0.0),
        muted: oklch(0.15, 0.03, 200.0),
        muted_foreground: oklch(0.65, 0.0, 0.0),
        border: oklch(0.3, 0.1, 195.0),
        input: oklcha(0.2, 0.06, 195.0, 0.95),
        ring: oklch(0.75, 0.18, 195.0),
        destructive: oklch(0.55, 0.25, 30.0),
        warning: oklch(0.82, 0.18, 90.0),
        success: oklch(0.78, 0.18, 155.0),
        info: oklch(0.72, 0.15, 235.0),
        glow: oklch(0.75, 0.18, 195.0),
        glow_muted: oklch(0.5, 0.12, 195.0),
        overlay: oklcha(0.0, 0.0, 0.0, 0.78),
    }
}

const fn oklcha(lightness: f32, chroma: f32, hue: f32, alpha: f32) -> Oklcha {
    Oklcha::new(lightness, chroma, hue, alpha)
}

const fn oklch(lightness: f32, chroma: f32, hue: f32) -> Oklcha {
    Oklcha::lch(lightness, chroma, hue)
}
