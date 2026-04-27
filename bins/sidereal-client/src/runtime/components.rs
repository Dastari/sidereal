//! ECS component markers and data used by native client systems.

use bevy::prelude::*;
use sidereal_game::RuntimeRenderLayerDefinition;

#[derive(Component)]
pub(crate) struct WorldEntity;

#[derive(Component)]
pub(crate) struct ClientSceneEntity;

#[derive(Component)]
pub(crate) struct HudSpeedValueText;

#[derive(Component)]
pub(crate) struct HudPositionValueText;

#[derive(Component)]
pub(crate) struct HudHealthBarFill;

#[derive(Component)]
pub(crate) struct HudFuelBarFill;

#[derive(Component, Clone, Copy)]
pub(crate) struct SegmentedBarStyle {
    pub segments: u8,
    pub active_color: Color,
    pub inactive_color: Color,
    pub shell_color: Color,
    pub border_color: Color,
    #[allow(dead_code)]
    pub corner_color: Color,
    pub scanline_primary_color: Color,
    pub scanline_secondary_color: Color,
    pub segment_width_px: f32,
    pub segment_gap_px: f32,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct SegmentedBarValue {
    pub ratio: f32,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct SegmentedBarSegment {
    pub index: u8,
}

#[derive(Component)]
pub(crate) struct EntityNameplateRoot {
    pub target: Option<Entity>,
    pub health_fill: Entity,
}

#[derive(Component)]
pub(crate) struct EntityNameplateHealthFill;

#[derive(Component)]
pub(crate) struct ActiveNameplateEntry;

#[derive(Component)]
pub(crate) struct LoadingOverlayText;

#[derive(Component)]
pub(crate) struct DebugOverlayPanelRoot;

#[derive(Component)]
pub(crate) struct DebugOverlayPanelLabelText;

#[derive(Component)]
pub(crate) struct DebugOverlayPanelValueText;

#[derive(Component)]
pub(crate) struct DebugOverlayPanelLabelShadowText;

#[derive(Component)]
pub(crate) struct DebugOverlayPanelValueShadowText;

#[derive(Component)]
pub(crate) struct DebugOverlayPanelSecondaryLabelText;

#[derive(Component)]
pub(crate) struct DebugOverlayPanelSecondaryValueText;

#[derive(Component)]
pub(crate) struct DebugOverlayPanelSecondaryLabelShadowText;

#[derive(Component)]
pub(crate) struct DebugOverlayPanelSecondaryValueShadowText;

#[derive(Component)]
pub(crate) struct LoadingProgressBarFill;

#[derive(Component)]
pub(crate) struct LoadingOverlayRoot;

#[derive(Component)]
pub(crate) struct RuntimeStreamingIconText;

#[derive(Component)]
pub(crate) struct TacticalMapOverlayRoot;

#[derive(Component)]
pub(crate) struct TacticalMapTitle;

#[derive(Component)]
pub(crate) struct TacticalMapCursorText;

#[derive(Component)]
pub(crate) struct TacticalMapMarkerDynamic {
    pub key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeScreenOverlayPassKind {
    TacticalMap,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeScreenOverlayPass {
    pub kind: RuntimeScreenOverlayPassKind,
}

#[derive(Component)]
pub(crate) struct GameplayCamera;

#[derive(Component)]
pub(crate) struct PlanetBodyCamera;

#[derive(Component)]
pub(crate) struct DebugOverlayCamera;

#[derive(Component)]
pub(crate) struct DebugVelocityArrowShaft;

#[derive(Component)]
pub(crate) struct DebugVelocityArrowHeadUpper;

#[derive(Component)]
pub(crate) struct DebugVelocityArrowHeadLower;

#[derive(Component)]
pub(crate) struct BackdropCamera;

#[derive(Component)]
pub(crate) struct FullscreenForegroundCamera;

#[derive(Component)]
pub(crate) struct PostProcessCamera;

#[derive(Component)]
pub(crate) struct GameplayHud;

/// Marker for entities that belong to the screen-space UI overlay (HUD). Used to propagate
/// `RenderLayers::layer(UI_OVERLAY_RENDER_LAYER)` to all descendants so they render on the UI camera.
#[derive(Component)]
pub(crate) struct UiOverlayLayer;

#[derive(Component)]
pub(crate) struct UiOverlayCamera;

#[derive(Component)]
pub(crate) struct CharacterSelectRoot;

#[derive(Component)]
pub(crate) struct CharacterSelectStatusText;

#[derive(Component)]
pub(crate) struct CharacterSelectPreviewNameText;

#[derive(Component)]
pub(crate) struct CharacterSelectPreviewMetaText;

#[derive(Component)]
pub(crate) struct CharacterSelectButton {
    pub player_entity_id: String,
}

#[derive(Component)]
pub(crate) struct CharacterSelectEnterButton;

#[derive(Component)]
pub(crate) struct OwnedEntitiesPanelRoot;

#[derive(Component)]
pub(crate) struct OwnedEntitiesPanelButton {
    pub action: OwnedEntitiesPanelAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OwnedEntitiesPanelAction {
    FreeRoam,
    ControlEntity(String),
}

#[derive(Component)]
pub(crate) struct ControlledEntity {
    pub entity_id: String,
    #[allow(dead_code)]
    pub player_entity_id: String,
}

#[derive(Component)]
pub(crate) struct RemoteVisibleEntity {
    #[allow(dead_code)]
    pub entity_id: String,
}

#[derive(Component)]
pub(crate) struct RemoteEntity;

#[derive(Component)]
pub(crate) struct NearbyCollisionProxy;

#[derive(Component)]
pub(crate) struct CanonicalPresentationEntity;

#[derive(Component, Clone)]
pub(crate) struct StreamedVisualAssetId(pub String);

#[derive(Component)]
pub(crate) struct StreamedVisualAttached;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamedVisualAttachmentKind {
    Plain,
    GenericShader,
    AsteroidShader,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StreamedProceduralSpriteVisualFingerprint(pub u64);

#[derive(Component)]
pub(crate) struct StreamedVisualChild;

#[derive(Component)]
pub(crate) struct BallisticProjectileVisualAttached;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeWorldVisualFamily {
    Planet,
    Thruster,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeWorldVisualPassKind {
    PlanetBody,
    PlanetCloudBack,
    PlanetCloudFront,
    PlanetRingBack,
    PlanetRingFront,
    ThrusterPlume,
}

impl RuntimeWorldVisualPassKind {
    const fn bit(self) -> u32 {
        match self {
            Self::PlanetBody => 1 << 0,
            Self::PlanetCloudBack => 1 << 1,
            Self::PlanetCloudFront => 1 << 2,
            Self::PlanetRingBack => 1 << 3,
            Self::PlanetRingFront => 1 << 4,
            Self::ThrusterPlume => 1 << 5,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeWorldVisualPass {
    pub family: RuntimeWorldVisualFamily,
    pub kind: RuntimeWorldVisualPassKind,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RuntimeWorldVisualPassSet {
    pub mask: u32,
}

impl RuntimeWorldVisualPassSet {
    pub fn contains(&self, kind: RuntimeWorldVisualPassKind) -> bool {
        (self.mask & kind.bit()) != 0
    }

    pub fn insert(&mut self, kind: RuntimeWorldVisualPassKind) {
        self.mask |= kind.bit();
    }

    pub fn remove(&mut self, kind: RuntimeWorldVisualPassKind) {
        self.mask &= !kind.bit();
    }

    pub fn is_empty(&self) -> bool {
        self.mask == 0
    }
}

#[derive(Component)]
pub(crate) struct WeaponTracerBolt {
    pub excluded_entity: Option<Entity>,
    pub velocity: Vec2,
    pub impact_xy: Option<Vec2>,
    pub ttl_s: f32,
    pub lateral_normal: Vec2,
    pub wiggle_phase_rad: f32,
    pub wiggle_freq_hz: f32,
    pub wiggle_amp_mps: f32,
}

#[derive(Component)]
pub(crate) struct WeaponImpactSpark {
    pub ttl_s: f32,
    pub max_ttl_s: f32,
}

#[derive(Component)]
pub(crate) struct WeaponImpactExplosion {
    pub ttl_s: f32,
    pub max_ttl_s: f32,
    pub base_scale: f32,
    pub growth_scale: f32,
    pub intensity_scale: f32,
    pub domain_scale: f32,
    pub screen_distortion_scale: f32,
}

#[derive(Resource, Default)]
pub(crate) struct WeaponTracerPool {
    pub bolts: Vec<Entity>,
    pub next_index: usize,
}

#[derive(Resource, Default)]
pub(crate) struct WeaponTracerCooldowns {
    pub by_weapon_entity: std::collections::HashMap<Entity, f32>,
}

#[derive(Resource, Default)]
pub(crate) struct WeaponImpactSparkPool {
    pub sparks: Vec<Entity>,
    pub next_index: usize,
}

#[derive(Resource, Default)]
pub(crate) struct WeaponImpactExplosionPool {
    pub explosions: Vec<Entity>,
    pub next_index: usize,
}

#[derive(Component, Clone)]
pub(crate) struct StreamedSpriteShaderAssetId(pub String);

#[derive(Component)]
pub(crate) struct SuppressedPredictedDuplicateVisual;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PredictedMotionBootstrapSeed {
    pub generation: u64,
}

#[derive(Component)]
pub(crate) struct ReplicatedAdoptionHandled;

#[derive(Component)]
pub(crate) struct PendingInitialVisualReady;

#[derive(Component)]
pub(crate) struct PendingVisibilityFadeIn {
    pub elapsed_s: f32,
    pub duration_s: f32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeFullscreenMaterialBinding {
    Starfield,
    SpaceBackgroundBase,
    SpaceBackgroundNebula,
}

#[derive(Component)]
pub(crate) struct SpaceBackdropFallback;

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeFullscreenRenderable {
    pub layer_id: Option<String>,
    pub owner_entity: Option<Entity>,
    pub pass_id: Option<String>,
}

#[derive(Component, Debug, Clone, PartialEq)]
pub(crate) struct ResolvedRuntimeRenderLayer {
    pub layer_id: String,
    pub definition: RuntimeRenderLayerDefinition,
}

#[derive(Component)]
pub(crate) struct TopDownCamera {
    pub distance: f32,
    pub target_distance: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub zoom_units_per_wheel: f32,
    pub zoom_smoothness: f32,
    pub look_ahead_offset: Vec2,
    pub filtered_focus_xy: Vec2,
    pub focus_initialized: bool,
}
