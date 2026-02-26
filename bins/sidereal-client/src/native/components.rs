//! ECS component markers and data used by native client systems.

use bevy::prelude::*;

#[derive(Component)]
pub(crate) struct WorldEntity;

#[derive(Component)]
pub(crate) struct HudText;

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
pub(crate) struct OwnedShipsPanelRoot;

#[derive(Component)]
pub(crate) struct OwnedShipsPanelButton {
    pub action: OwnedShipsPanelAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OwnedShipsPanelAction {
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

#[derive(Component, Clone)]
pub(crate) struct StreamedSpriteShaderAssetId(pub String);

#[derive(Component)]
pub(crate) struct SuppressedPredictedDuplicateVisual;

#[derive(Component, Debug, Clone)]
pub(crate) struct InterpolatedVisualSmoothing {
    pub from_pos: Vec2,
    pub to_pos: Vec2,
    pub from_rot: Quat,
    pub to_rot: Quat,
    pub elapsed_s: f32,
    pub duration_s: f32,
    pub last_snapshot_at_s: f64,
}

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
