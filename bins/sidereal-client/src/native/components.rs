//! ECS component markers and data used by native client systems.

use bevy::prelude::*;

#[derive(Component)]
pub(crate) struct WorldEntity;

#[derive(Component)]
pub(crate) struct ClientSceneEntity;

#[derive(Component)]
pub(crate) struct HudFpsText;

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
pub(crate) struct ShipNameplateRoot {
    pub target: Entity,
}

#[derive(Component)]
pub(crate) struct ShipNameplateHealthBar {
    pub target: Entity,
}

#[derive(Component)]
pub(crate) struct LoadingOverlayText;

#[derive(Component)]
pub(crate) struct LoadingProgressBarFill;

#[derive(Component)]
pub(crate) struct LoadingOverlayRoot;

#[derive(Component)]
pub(crate) struct RuntimeStreamingIconText;

#[derive(Component)]
pub(crate) struct GameplayCamera;

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

#[derive(Component, Clone)]
pub(crate) struct StreamedVisualAssetId(pub String);

#[derive(Component)]
pub(crate) struct StreamedVisualAttached;

#[derive(Component)]
pub(crate) struct StreamedVisualChild;

#[derive(Component)]
pub(crate) struct ThrusterPlumeAttached;

#[derive(Component)]
pub(crate) struct ThrusterPlumeChild;

#[derive(Component)]
pub(crate) struct WeaponTracerBolt {
    pub excluded_entity: Option<Entity>,
    pub velocity: Vec2,
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

#[derive(Resource, Default)]
pub(crate) struct WeaponTracerPool {
    pub bolts: Vec<Entity>,
    pub next_index: usize,
}

#[derive(Resource, Default)]
pub(crate) struct WeaponTracerCooldowns {
    pub by_weapon_entity: std::collections::HashMap<Entity, f32>,
}

#[derive(Component, Clone)]
pub(crate) struct StreamedSpriteShaderAssetId(pub String);

#[derive(Component)]
pub(crate) struct SuppressedPredictedDuplicateVisual;

#[derive(Component)]
pub(crate) struct ReplicatedAdoptionHandled;

#[derive(Component)]
pub(crate) struct StarfieldBackdrop;

#[derive(Component)]
pub(crate) struct SpaceBackgroundBackdrop;

#[derive(Component)]
pub(crate) struct DebugBlueBackdrop;

#[derive(Component)]
pub(crate) struct SpaceBackdropFallback;

#[derive(Component)]
pub(crate) struct FullscreenLayerRenderable {
    pub layer_kind: String,
    pub layer_order: i32,
}

#[derive(Component)]
pub(crate) struct FallbackFullscreenLayer;

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
