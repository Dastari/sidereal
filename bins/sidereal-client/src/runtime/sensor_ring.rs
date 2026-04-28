//! Tactical sensor ring HUD presentation.

use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::window::PrimaryWindow;
use bevy_svg::prelude::{Svg, Svg2d};
use sidereal_game::{
    EntityGuid, MountedOn, ScannerComponent, SizeM, TacticalPresentationDefaults, VisibilityRangeM,
};
use sidereal_runtime_sync::parse_guid_from_entity_id;
use std::collections::{HashMap, HashSet};

use super::app_state::{ClientSession, LocalPlayerViewState};
use super::assets::LocalAssetManager;
use super::components::{ControlledEntity, GameplayCamera, UiOverlayCamera, WorldEntity};
use super::dev_console::{DevConsoleState, is_console_open};
use super::ecs_util::queue_despawn_if_exists;
use super::platform::UI_OVERLAY_RENDER_LAYER;
use super::resources::{
    ActiveScannerProfileCache, AssetCacheAdapter, AssetRootPath, ResolvedScannerProfile,
    TacticalContactsCache, TacticalMapUiState, TacticalSensorRingUiState,
};
use super::ui::{
    TacticalMapIconSvgCache, TacticalMarkerColorRole, resolve_tactical_marker_svg_with_color,
    tactical_icon_centered_translation, tactical_marker_color, tactical_marker_scale_multiplier,
};

const SENSOR_RING_TICK_COUNT: usize = 96;
const SENSOR_RING_DENSITY_SECTORS: usize = 24;
const SENSOR_RING_FALLBACK_RADIUS_RATIO: f32 = 0.18;
const SENSOR_RING_MAX_VIEWPORT_RATIO: f32 = 0.32;
const SENSOR_RING_MIN_RADIUS_PX: f32 = 72.0;
const SENSOR_RING_SHIP_RADIUS_MULTIPLIER: f32 = 3.2;
const SENSOR_RING_SHIP_RADIUS_PADDING_PX: f32 = 34.0;
const SENSOR_RING_CONTACT_BAND_OFFSET_PX: f32 = 18.0;
const SENSOR_RING_CENTER_MARGIN_PX: f32 = 32.0;
const SENSOR_RING_FADE_RATE: f32 = 12.0;
const SENSOR_RING_SIGNAL_SECTORS: usize = 48;

#[derive(Component)]
pub(super) struct TacticalSensorRingElement {
    key: String,
}

#[derive(Default)]
pub(super) struct SensorRingRenderCache {
    quad_mesh: Option<Handle<Mesh>>,
}

#[derive(Clone)]
struct ExistingSensorRingElement {
    entity: Entity,
    material: Option<Handle<ColorMaterial>>,
}

struct ControlledSensorEntity<'a> {
    entity_id: &'a str,
    guid: &'a EntityGuid,
    global_transform: &'a GlobalTransform,
    size_m: Option<&'a SizeM>,
}

#[derive(SystemParam)]
pub(super) struct SensorRingIconAssets<'w, 's> {
    svg_assets: ResMut<'w, Assets<Svg>>,
    asset_root: Res<'w, AssetRootPath>,
    cache_adapter: Res<'w, AssetCacheAdapter>,
    asset_manager: Res<'w, LocalAssetManager>,
    icon_cache: Local<'s, TacticalMapIconSvgCache>,
    tactical_defaults: Query<'w, 's, &'static TacticalPresentationDefaults>,
}

pub(super) type ControlledScannerProfileQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ControlledEntity,
        &'static EntityGuid,
        Option<&'static ScannerComponent>,
        Option<&'static VisibilityRangeM>,
    ),
    With<WorldEntity>,
>;

pub(super) type MountedScannerProfileQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static EntityGuid,
        &'static ScannerComponent,
        Option<&'static VisibilityRangeM>,
        Option<&'static MountedOn>,
    ),
    With<WorldEntity>,
>;

pub(super) type SensorRingElementQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TacticalSensorRingElement,
        Option<&'static MeshMaterial2d<ColorMaterial>>,
        Option<&'static Svg2d>,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    With<TacticalSensorRingElement>,
>;

pub(super) fn update_active_scanner_profile_cache_system(
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    mut cache: ResMut<'_, ActiveScannerProfileCache>,
    controlled_query: ControlledScannerProfileQuery<'_, '_>,
    scanner_query: MountedScannerProfileQuery<'_, '_>,
) {
    let controlled_id = active_scanner_controlled_entity_id(&session, &player_view_state);
    let Some(controlled_id) = controlled_id else {
        cache.controlled_entity_id = None;
        cache.profile = None;
        return;
    };

    let Some((controlled, controlled_guid, root_scanner, root_range)) = controlled_query
        .iter()
        .find(|(controlled, guid, _, _)| controlled_matches(controlled_id, controlled, guid))
    else {
        cache.controlled_entity_id = Some(controlled_id.to_string());
        cache.profile = None;
        return;
    };

    let mut best = root_scanner.map(|scanner| scanner_profile(scanner, root_range));
    for (_guid, scanner, range, mounted_on) in &scanner_query {
        let Some(mounted_on) = mounted_on else {
            continue;
        };
        if mounted_on.parent_entity_id != controlled_guid.0 {
            continue;
        }
        best = best_scanner_profile(best, scanner_profile(scanner, range));
    }

    cache.controlled_entity_id = Some(controlled.entity_id.clone());
    cache.profile = best;
}

pub(super) fn toggle_tactical_sensor_ring_system(
    time: Res<'_, Time>,
    input: Res<'_, ButtonInput<KeyCode>>,
    dev_console_state: Option<Res<'_, DevConsoleState>>,
    scanner_cache: Res<'_, ActiveScannerProfileCache>,
    tactical_map_state: Res<'_, TacticalMapUiState>,
    mut ring_state: ResMut<'_, TacticalSensorRingUiState>,
) {
    if is_console_open(dev_console_state.as_deref()) || !input.just_pressed(KeyCode::Tab) {
        return;
    }
    if tactical_map_state.enabled || scanner_cache.profile.is_none() {
        ring_state.enabled = false;
        ring_state.last_unavailable_notice_at_s = time.elapsed_secs_f64();
        return;
    }
    ring_state.enabled = !ring_state.enabled;
    ring_state.last_controlled_entity_id = scanner_cache.controlled_entity_id.clone();
}

pub(super) fn close_sensor_ring_when_unavailable_system(
    scanner_cache: Res<'_, ActiveScannerProfileCache>,
    tactical_map_state: Res<'_, TacticalMapUiState>,
    mut ring_state: ResMut<'_, TacticalSensorRingUiState>,
) {
    let control_changed = ring_state.last_controlled_entity_id.as_deref()
        != scanner_cache.controlled_entity_id.as_deref();
    if control_changed {
        ring_state.last_controlled_entity_id = scanner_cache.controlled_entity_id.clone();
    }
    if tactical_map_state.enabled || scanner_cache.profile.is_none() || control_changed {
        ring_state.enabled = false;
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(super) fn update_tactical_sensor_ring_overlay_system(
    time: Res<'_, Time>,
    scanner_cache: Res<'_, ActiveScannerProfileCache>,
    contacts_cache: Res<'_, TacticalContactsCache>,
    mut ring_state: ResMut<'_, TacticalSensorRingUiState>,
    windows: Query<'_, '_, &'_ Window, With<PrimaryWindow>>,
    gameplay_camera: Query<
        '_,
        '_,
        (&'_ Camera, &'_ GlobalTransform),
        (With<GameplayCamera>, Without<UiOverlayCamera>),
    >,
    controlled_query: Query<
        '_,
        '_,
        (
            &'_ ControlledEntity,
            &'_ EntityGuid,
            &'_ GlobalTransform,
            Option<&'_ SizeM>,
        ),
        (With<WorldEntity>, Without<TacticalSensorRingElement>),
    >,
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut color_materials: ResMut<'_, Assets<ColorMaterial>>,
    mut icon_assets: SensorRingIconAssets<'_, '_>,
    mut render_cache: Local<'_, SensorRingRenderCache>,
    mut elements: SensorRingElementQuery<'_, '_>,
) {
    if icon_assets.icon_cache.reload_generation != icon_assets.asset_manager.reload_generation {
        *icon_assets.icon_cache = TacticalMapIconSvgCache::default();
        icon_assets.icon_cache.reload_generation = icon_assets.asset_manager.reload_generation;
    }

    let fade_t = 1.0 - (-SENSOR_RING_FADE_RATE * time.delta_secs()).exp();
    ring_state.alpha = if ring_state.enabled {
        ring_state.alpha.lerp(1.0, fade_t)
    } else {
        ring_state.alpha.lerp(0.0, fade_t)
    };
    if !ring_state.enabled && ring_state.alpha < 0.01 {
        ring_state.alpha = 0.0;
        despawn_sensor_ring_elements(&mut commands, &mut elements);
        return;
    }

    let Some(profile) = scanner_cache.profile else {
        ring_state.enabled = false;
        return;
    };
    let Some(controlled_id) = scanner_cache.controlled_entity_id.as_deref() else {
        ring_state.enabled = false;
        return;
    };
    let Some(controlled) = controlled_query
        .iter()
        .find(|(controlled, guid, _, _)| controlled_matches(controlled_id, controlled, guid))
        .map(
            |(controlled, guid, global_transform, size_m)| ControlledSensorEntity {
                entity_id: controlled.entity_id.as_str(),
                guid,
                global_transform,
                size_m,
            },
        )
    else {
        ring_state.enabled = false;
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Some((camera, camera_transform)) =
        gameplay_camera.iter().find(|(camera, _)| camera.is_active)
    else {
        return;
    };
    let center_world = controlled.global_transform.translation();
    let center_world = Vec3::new(center_world.x, center_world.y, 0.0);
    let Ok(controlled_viewport) = camera.world_to_viewport(camera_transform, center_world) else {
        return;
    };
    let viewport_width = window.width().max(1.0);
    let viewport_height = window.height().max(1.0);
    let radius = sensor_ring_radius_px(
        camera,
        camera_transform,
        center_world,
        controlled_viewport,
        controlled.size_m,
        viewport_width,
        viewport_height,
    );
    let center_viewport =
        clamp_ring_center(controlled_viewport, viewport_width, viewport_height, radius);
    let center =
        viewport_to_overlay(center_viewport, viewport_width, viewport_height, -6.0).truncate();
    let quad_mesh = render_cache
        .quad_mesh
        .get_or_insert_with(|| meshes.add(Rectangle::new(1.0, 1.0)))
        .clone();

    let mut existing = HashMap::<String, ExistingSensorRingElement>::new();
    for (entity, element, material, _, _, _) in &mut elements {
        existing.insert(
            element.key.clone(),
            ExistingSensorRingElement {
                entity,
                material: material.map(|material| material.0.clone()),
            },
        );
    }
    let mut seen = HashSet::<String>::new();
    let alpha = ring_state.alpha.clamp(0.0, 1.0);

    draw_ring_ticks(
        &mut commands,
        &mut color_materials,
        &quad_mesh,
        &mut existing,
        &mut seen,
        center,
        radius,
        alpha,
    );
    draw_signal_strength_segments(
        &mut commands,
        &mut color_materials,
        &quad_mesh,
        &mut existing,
        &mut seen,
        center,
        radius,
        alpha,
        camera,
        camera_transform,
        controlled_viewport,
        &contacts_cache,
        &controlled,
    );
    draw_density_segments(
        &mut commands,
        &mut color_materials,
        &quad_mesh,
        &mut existing,
        &mut seen,
        center,
        radius,
        alpha,
        profile,
        &contacts_cache,
        &controlled,
        camera,
        camera_transform,
        controlled_viewport,
    );
    draw_contact_markers(
        &mut commands,
        (
            &icon_assets.asset_manager,
            &icon_assets.asset_root.0,
            *icon_assets.cache_adapter,
        ),
        (&mut *icon_assets.svg_assets, &mut meshes),
        &mut icon_assets.icon_cache,
        &mut existing,
        &mut seen,
        center,
        radius,
        alpha,
        profile,
        &contacts_cache,
        &controlled,
        camera,
        camera_transform,
        controlled_viewport,
        icon_assets.tactical_defaults.iter().next(),
    );

    for (key, stale) in existing {
        if !seen.contains(key.as_str()) {
            queue_despawn_if_exists(&mut commands, stale.entity);
        }
    }
}

fn active_scanner_controlled_entity_id<'a>(
    session: &'a ClientSession,
    player_view_state: &'a LocalPlayerViewState,
) -> Option<&'a str> {
    if player_view_state.detached_free_camera {
        return None;
    }
    let controlled_id = player_view_state.controlled_entity_id.as_deref()?;
    if session
        .player_entity_id
        .as_deref()
        .is_some_and(|player_id| ids_refer_to_same_guid(controlled_id, player_id))
    {
        return None;
    }
    Some(controlled_id)
}

fn controlled_matches(
    controlled_id: &str,
    controlled: &ControlledEntity,
    guid: &EntityGuid,
) -> bool {
    ids_refer_to_same_guid(controlled_id, controlled.entity_id.as_str())
        || ids_refer_to_same_guid(controlled_id, guid.0.to_string().as_str())
}

fn scanner_profile(
    scanner: &ScannerComponent,
    visibility_range: Option<&VisibilityRangeM>,
) -> ResolvedScannerProfile {
    ResolvedScannerProfile {
        detail_tier: scanner.detail_tier,
        level: scanner.level,
        effective_range_m: visibility_range
            .map(|range| range.0)
            .unwrap_or(scanner.base_range_m)
            .max(scanner.base_range_m)
            .max(1.0),
        supports_density: scanner.supports_density,
        supports_directional_awareness: scanner.supports_directional_awareness,
        max_contacts: scanner.max_contacts.max(1),
    }
}

fn best_scanner_profile(
    current: Option<ResolvedScannerProfile>,
    candidate: ResolvedScannerProfile,
) -> Option<ResolvedScannerProfile> {
    let Some(current) = current else {
        return Some(candidate);
    };
    let current_key = (
        current.detail_tier,
        current.level,
        current.effective_range_m,
        current.max_contacts,
    );
    let candidate_key = (
        candidate.detail_tier,
        candidate.level,
        candidate.effective_range_m,
        candidate.max_contacts,
    );
    if candidate_key.0 > current_key.0
        || (candidate_key.0 == current_key.0 && candidate_key.1 > current_key.1)
        || (candidate_key.0 == current_key.0
            && candidate_key.1 == current_key.1
            && candidate_key.2 > current_key.2)
        || (candidate_key.0 == current_key.0
            && candidate_key.1 == current_key.1
            && candidate_key.2 == current_key.2
            && candidate_key.3 > current_key.3)
    {
        Some(candidate)
    } else {
        Some(current)
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_ring_ticks(
    commands: &mut Commands<'_, '_>,
    color_materials: &mut Assets<ColorMaterial>,
    quad_mesh: &Handle<Mesh>,
    existing: &mut HashMap<String, ExistingSensorRingElement>,
    seen: &mut HashSet<String>,
    center: Vec2,
    radius: f32,
    alpha: f32,
) {
    for index in 0..SENSOR_RING_TICK_COUNT {
        let angle = std::f32::consts::TAU * index as f32 / SENSOR_RING_TICK_COUNT as f32;
        let cardinal = index % (SENSOR_RING_TICK_COUNT / 4) == 0;
        let eighth = index % (SENSOR_RING_TICK_COUNT / 8) == 0;
        let tick_len = if cardinal {
            22.0
        } else if eighth {
            16.0
        } else {
            9.0
        };
        let tick_width = if cardinal { 2.3 } else { 1.4 };
        let tick_alpha = if cardinal { 0.52 } else { 0.28 } * alpha;
        let key = format!("tick:{index}");
        let unit = Vec2::new(angle.cos(), angle.sin());
        let transform = Transform {
            translation: (center + unit * radius).extend(-6.0),
            rotation: Quat::from_rotation_z(angle - std::f32::consts::FRAC_PI_2),
            scale: Vec3::new(tick_width, tick_len, 1.0),
        };
        upsert_sensor_ring_rect(
            commands,
            color_materials,
            quad_mesh,
            existing,
            seen,
            key,
            Color::srgba(0.47, 0.88, 1.0, tick_alpha),
            transform,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_signal_strength_segments(
    commands: &mut Commands<'_, '_>,
    color_materials: &mut Assets<ColorMaterial>,
    quad_mesh: &Handle<Mesh>,
    existing: &mut HashMap<String, ExistingSensorRingElement>,
    seen: &mut HashSet<String>,
    center: Vec2,
    radius: f32,
    alpha: f32,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    controlled_viewport: Vec2,
    contacts_cache: &TacticalContactsCache,
    controlled: &ControlledSensorEntity<'_>,
) {
    let mut sectors = [0.0_f32; SENSOR_RING_SIGNAL_SECTORS];
    for contact in contacts_cache.contacts_by_entity_id.values() {
        if ids_refer_to_same_guid(controlled.entity_id, contact.entity_id.as_str())
            || ids_refer_to_same_guid(
                controlled.guid.0.to_string().as_str(),
                contact.entity_id.as_str(),
            )
        {
            continue;
        }
        let Some(signal_strength) = contact_signal_strength_for_ring(contact) else {
            continue;
        };
        let Some(angle) =
            contact_screen_bearing_rad(camera, camera_transform, controlled_viewport, contact)
        else {
            continue;
        };
        let sector = density_sector_index(angle, SENSOR_RING_SIGNAL_SECTORS) as usize;
        sectors[sector] = sectors[sector].max(signal_strength);
    }

    for (index, strength) in sectors.into_iter().enumerate() {
        let key = format!("signal:{index}");
        if strength <= 0.0 {
            if let Some(stale) = existing.remove(key.as_str()) {
                queue_despawn_if_exists(commands, stale.entity);
            }
            continue;
        }
        let angle =
            std::f32::consts::TAU * (index as f32 + 0.5) / SENSOR_RING_SIGNAL_SECTORS as f32;
        let unit = Vec2::new(angle.cos(), angle.sin());
        let strength = strength.clamp(0.0, 1.0);
        let bar_len = 7.0 + strength * 24.0;
        let bar_width = 2.2 + strength * 1.7;
        let transform = Transform {
            translation: (center + unit * (radius + 8.0 + bar_len * 0.5)).extend(-5.9),
            rotation: Quat::from_rotation_z(angle - std::f32::consts::FRAC_PI_2),
            scale: Vec3::new(bar_width, bar_len, 1.0),
        };
        upsert_sensor_ring_rect(
            commands,
            color_materials,
            quad_mesh,
            existing,
            seen,
            key,
            Color::srgba(0.74, 0.97, 1.0, (0.18 + strength * 0.64) * alpha),
            transform,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_density_segments(
    commands: &mut Commands<'_, '_>,
    color_materials: &mut Assets<ColorMaterial>,
    quad_mesh: &Handle<Mesh>,
    existing: &mut HashMap<String, ExistingSensorRingElement>,
    seen: &mut HashSet<String>,
    center: Vec2,
    radius: f32,
    alpha: f32,
    profile: ResolvedScannerProfile,
    contacts_cache: &TacticalContactsCache,
    controlled: &ControlledSensorEntity<'_>,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    controlled_viewport: Vec2,
) {
    if !profile.supports_density {
        return;
    }
    let mut sectors = [0u8; SENSOR_RING_DENSITY_SECTORS];
    for contact in contacts_cache.contacts_by_entity_id.values() {
        if ids_refer_to_same_guid(controlled.entity_id, contact.entity_id.as_str())
            || ids_refer_to_same_guid(
                controlled.guid.0.to_string().as_str(),
                contact.entity_id.as_str(),
            )
            || !contact.is_live_now
        {
            continue;
        }
        if contact_distance_sq(contact, controlled) > profile.effective_range_m.powi(2) {
            continue;
        }
        if let Some(angle) =
            contact_screen_bearing_rad(camera, camera_transform, controlled_viewport, contact)
        {
            let sector = density_sector_index(angle, SENSOR_RING_DENSITY_SECTORS);
            sectors[sector as usize] = sectors[sector as usize].saturating_add(1);
        }
    }
    for (index, count) in sectors.into_iter().enumerate() {
        if count == 0 {
            continue;
        }
        let angle =
            std::f32::consts::TAU * (index as f32 + 0.5) / SENSOR_RING_DENSITY_SECTORS as f32;
        let unit = Vec2::new(angle.cos(), angle.sin());
        let intensity = (count as f32 / 5.0).clamp(0.18, 0.8);
        let transform = Transform {
            translation: (center + unit * (radius - 18.0)).extend(-6.1),
            rotation: Quat::from_rotation_z(angle - std::f32::consts::FRAC_PI_2),
            scale: Vec3::new(4.0, 10.0 + count.min(6) as f32 * 4.0, 1.0),
        };
        upsert_sensor_ring_rect(
            commands,
            color_materials,
            quad_mesh,
            existing,
            seen,
            format!("density:{index}"),
            Color::srgba(0.32, 0.78, 0.95, intensity * alpha),
            transform,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_contact_markers(
    commands: &mut Commands<'_, '_>,
    asset_io: (&LocalAssetManager, &str, AssetCacheAdapter),
    render_assets: (&mut Assets<Svg>, &mut Assets<Mesh>),
    icon_cache: &mut TacticalMapIconSvgCache,
    existing: &mut HashMap<String, ExistingSensorRingElement>,
    seen: &mut HashSet<String>,
    center: Vec2,
    radius: f32,
    alpha: f32,
    profile: ResolvedScannerProfile,
    contacts_cache: &TacticalContactsCache,
    controlled: &ControlledSensorEntity<'_>,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    controlled_viewport: Vec2,
    tactical_defaults: Option<&TacticalPresentationDefaults>,
) {
    let (svg_assets, meshes) = render_assets;
    let mut contacts = contacts_cache
        .contacts_by_entity_id
        .values()
        .collect::<Vec<_>>();
    contacts.sort_by(|left, right| {
        contact_distance_sq(left, controlled)
            .total_cmp(&contact_distance_sq(right, controlled))
            .then_with(|| left.entity_id.cmp(&right.entity_id))
    });
    contacts.truncate(profile.max_contacts as usize);

    for contact in contacts {
        if ids_refer_to_same_guid(controlled.entity_id, contact.entity_id.as_str())
            || ids_refer_to_same_guid(
                controlled.guid.0.to_string().as_str(),
                contact.entity_id.as_str(),
            )
        {
            continue;
        }
        let distance = contact_distance_sq(contact, controlled).sqrt();
        if distance > profile.effective_range_m && contact.signal_strength.is_none() {
            continue;
        }
        let Some(angle) =
            contact_screen_bearing_rad(camera, camera_transform, controlled_viewport, contact)
        else {
            continue;
        };
        let unit = Vec2::new(angle.cos(), angle.sin());
        let base_asset_id = contact.map_icon_asset_id.as_deref().or_else(|| {
            tactical_defaults.and_then(|defaults| {
                defaults.map_icon_asset_id_for_kind(Some(contact.kind.as_str()))
            })
        });
        let Some(base_asset_id) = base_asset_id else {
            continue;
        };
        let marker_alpha =
            contact_marker_alpha(contact.is_live_now, distance, profile.effective_range_m) * alpha;
        if marker_alpha <= 0.01 {
            continue;
        }
        let alpha_bucket = (marker_alpha.clamp(0.0, 1.0) * 20.0).round() as u8;
        let marker_color =
            tactical_marker_color(TacticalMarkerColorRole::HostileContact).with_alpha(marker_alpha);
        let variant_suffix = format!("sensor-contact-alpha-{alpha_bucket}");
        let Some(svg_handle) = resolve_tactical_marker_svg_with_color(
            asset_io,
            (&mut *svg_assets, &mut *meshes),
            icon_cache,
            base_asset_id,
            variant_suffix.as_str(),
            marker_color,
        ) else {
            continue;
        };
        let target_height_px = contact_marker_size(distance, profile.effective_range_m)
            * tactical_marker_scale_multiplier(contact.kind.as_str());
        let icon_scale = sensor_ring_svg_marker_scale(svg_assets, &svg_handle, target_height_px);
        let heading_rad = contact.heading_rad as f32;
        let desired_center =
            (center + unit * (radius + SENSOR_RING_CONTACT_BAND_OFFSET_PX)).extend(-5.7);
        let translation = tactical_icon_centered_translation(
            svg_assets,
            &svg_handle,
            icon_scale,
            heading_rad,
            desired_center,
        );
        let transform = Transform {
            translation,
            rotation: Quat::from_rotation_z(heading_rad),
            scale: Vec3::splat(icon_scale),
        };
        upsert_sensor_ring_svg(
            commands,
            existing,
            seen,
            format!("contact:{}", contact.entity_id),
            svg_handle,
            transform,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn upsert_sensor_ring_rect(
    commands: &mut Commands<'_, '_>,
    color_materials: &mut Assets<ColorMaterial>,
    quad_mesh: &Handle<Mesh>,
    existing: &mut HashMap<String, ExistingSensorRingElement>,
    seen: &mut HashSet<String>,
    key: String,
    color: Color,
    transform: Transform,
) {
    seen.insert(key.clone());
    if let Some(existing) = existing.remove(key.as_str()) {
        let material_handle = if let Some(material_handle) = existing.material {
            if let Some(material) = color_materials.get_mut(&material_handle) {
                material.color = color;
                material_handle
            } else {
                color_materials.add(ColorMaterial::from(color))
            }
        } else {
            color_materials.add(ColorMaterial::from(color))
        };
        commands.entity(existing.entity).insert((
            Mesh2d(quad_mesh.clone()),
            MeshMaterial2d(material_handle),
            transform,
            Visibility::Visible,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        ));
        return;
    }
    commands.spawn((
        Mesh2d(quad_mesh.clone()),
        MeshMaterial2d(color_materials.add(ColorMaterial::from(color))),
        transform,
        Visibility::Visible,
        RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        TacticalSensorRingElement { key },
    ));
}

fn sensor_ring_svg_marker_scale(
    svg_assets: &Assets<Svg>,
    svg_handle: &Handle<Svg>,
    target_height_px: f32,
) -> f32 {
    let svg_height = svg_assets
        .get(svg_handle)
        .map(|svg| svg.size.y.max(1.0))
        .unwrap_or(16.0);
    (target_height_px.max(2.0) / svg_height).clamp(0.08, 12.0)
}

fn upsert_sensor_ring_svg(
    commands: &mut Commands<'_, '_>,
    existing: &mut HashMap<String, ExistingSensorRingElement>,
    seen: &mut HashSet<String>,
    key: String,
    svg_handle: Handle<Svg>,
    transform: Transform,
) {
    seen.insert(key.clone());
    if let Some(existing) = existing.remove(key.as_str()) {
        if existing.material.is_some() {
            queue_despawn_if_exists(commands, existing.entity);
            commands.spawn((
                Svg2d(svg_handle),
                transform,
                Visibility::Visible,
                RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
                TacticalSensorRingElement { key },
            ));
            return;
        }
        commands.entity(existing.entity).insert((
            Svg2d(svg_handle),
            transform,
            Visibility::Visible,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        ));
        return;
    }

    commands.spawn((
        Svg2d(svg_handle),
        transform,
        Visibility::Visible,
        RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        TacticalSensorRingElement { key },
    ));
}

fn despawn_sensor_ring_elements(
    commands: &mut Commands<'_, '_>,
    elements: &mut SensorRingElementQuery<'_, '_>,
) {
    for (entity, _, _, _, _, _) in elements {
        queue_despawn_if_exists(commands, entity);
    }
}

fn contact_distance_sq(
    contact: &sidereal_net::TacticalContact,
    controlled: &ControlledSensorEntity<'_>,
) -> f32 {
    let controlled_position = controlled_position_xy(controlled);
    let dx = contact.position_xy[0] as f32 - controlled_position.x;
    let dy = contact.position_xy[1] as f32 - controlled_position.y;
    dx.mul_add(dx, dy * dy)
}

fn contact_signal_strength_for_ring(contact: &sidereal_net::TacticalContact) -> Option<f32> {
    if let Some(signal_strength) = contact
        .signal_strength
        .filter(|value| value.is_finite() && *value > 0.0)
    {
        return Some(signal_strength.clamp(0.0, 1.0));
    }

    let kind = contact.kind.trim();
    if kind.eq_ignore_ascii_case("star")
        || kind.eq_ignore_ascii_case("blackhole")
        || kind.eq_ignore_ascii_case("black_hole")
    {
        Some(1.0)
    } else if kind.eq_ignore_ascii_case("planet") {
        Some(0.72)
    } else {
        None
    }
}

fn contact_screen_bearing_rad(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    controlled_viewport: Vec2,
    contact: &sidereal_net::TacticalContact,
) -> Option<f32> {
    let world = Vec3::new(
        contact.position_xy[0] as f32,
        contact.position_xy[1] as f32,
        0.0,
    );
    let Ok(contact_viewport) = camera.world_to_viewport(camera_transform, world) else {
        return None;
    };
    sensor_ring_screen_bearing_rad(controlled_viewport, contact_viewport)
}

fn sensor_ring_screen_bearing_rad(from_viewport: Vec2, to_viewport: Vec2) -> Option<f32> {
    let delta = Vec2::new(
        to_viewport.x - from_viewport.x,
        from_viewport.y - to_viewport.y,
    );
    if delta.length_squared() <= f32::EPSILON {
        return None;
    }
    Some(delta.y.atan2(delta.x).rem_euclid(std::f32::consts::TAU))
}

fn controlled_position_xy(controlled: &ControlledSensorEntity<'_>) -> Vec2 {
    let translation = controlled.global_transform.translation();
    Vec2::new(translation.x, translation.y)
}

fn sensor_ring_radius_px(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    center_world: Vec3,
    center_viewport: Vec2,
    size_m: Option<&SizeM>,
    viewport_width: f32,
    viewport_height: f32,
) -> f32 {
    let fallback = fallback_sensor_ring_radius_px(viewport_width, viewport_height);
    let Some(size_m) = size_m else {
        return fallback;
    };
    let half_extent_world = (size_m.length.max(size_m.width).max(size_m.height) * 0.5).max(0.0);
    if half_extent_world <= f32::EPSILON || !half_extent_world.is_finite() {
        return fallback;
    }

    let top_world = center_world + Vec3::Y * half_extent_world;
    let right_world = center_world + Vec3::X * half_extent_world;
    let Ok(top_viewport) = camera.world_to_viewport(camera_transform, top_world) else {
        return fallback;
    };
    let Ok(right_viewport) = camera.world_to_viewport(camera_transform, right_world) else {
        return fallback;
    };
    let projected_ship_radius_px = center_viewport
        .distance(top_viewport)
        .max(center_viewport.distance(right_viewport));
    let max_radius = max_sensor_ring_radius_px(viewport_width, viewport_height);
    (projected_ship_radius_px * SENSOR_RING_SHIP_RADIUS_MULTIPLIER
        + SENSOR_RING_SHIP_RADIUS_PADDING_PX)
        .clamp(SENSOR_RING_MIN_RADIUS_PX, max_radius)
}

fn fallback_sensor_ring_radius_px(viewport_width: f32, viewport_height: f32) -> f32 {
    let min_extent = viewport_width.min(viewport_height);
    (min_extent * SENSOR_RING_FALLBACK_RADIUS_RATIO).clamp(
        SENSOR_RING_MIN_RADIUS_PX,
        max_sensor_ring_radius_px(viewport_width, viewport_height),
    )
}

fn max_sensor_ring_radius_px(viewport_width: f32, viewport_height: f32) -> f32 {
    (viewport_width.min(viewport_height) * SENSOR_RING_MAX_VIEWPORT_RATIO)
        .max(SENSOR_RING_MIN_RADIUS_PX)
}

fn contact_marker_alpha(is_live_now: bool, distance_m: f32, scanner_range_m: f32) -> f32 {
    let distance_t = (distance_m / scanner_range_m.max(1.0)).clamp(0.0, 1.0);
    let live_alpha = 0.95 - 0.42 * distance_t;
    if is_live_now {
        live_alpha
    } else {
        live_alpha * 0.42
    }
}

fn contact_marker_size(distance_m: f32, scanner_range_m: f32) -> f32 {
    let distance_t = (distance_m / scanner_range_m.max(1.0)).clamp(0.0, 1.0);
    13.0 - 5.0 * distance_t
}

fn density_sector_index(angle_rad: f32, sector_count: usize) -> u8 {
    let sector_count = sector_count.max(1);
    let normalized = angle_rad.rem_euclid(std::f32::consts::TAU);
    let sector = ((normalized / std::f32::consts::TAU) * sector_count as f32).floor() as usize;
    sector.min(sector_count - 1) as u8
}

fn clamp_ring_center(viewport: Vec2, width: f32, height: f32, radius: f32) -> Vec2 {
    let margin = (radius + SENSOR_RING_CENTER_MARGIN_PX).min(width.min(height) * 0.5);
    if width <= margin * 2.0 || height <= margin * 2.0 {
        return Vec2::new(width * 0.5, height * 0.5);
    }
    Vec2::new(
        viewport.x.clamp(margin, width - margin),
        viewport.y.clamp(margin, height - margin),
    )
}

fn viewport_to_overlay(viewport: Vec2, viewport_width: f32, viewport_height: f32, z: f32) -> Vec3 {
    Vec3::new(
        viewport.x - viewport_width * 0.5,
        viewport_height * 0.5 - viewport.y,
        z,
    )
}

fn ids_refer_to_same_guid(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    parse_guid_from_entity_id(left)
        .zip(parse_guid_from_entity_id(right))
        .is_some_and(|(l, r)| l == r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(left: f32, right: f32) {
        assert!(
            (left - right).abs() < 0.0001,
            "expected {left} to be close to {right}"
        );
    }

    #[test]
    fn screen_bearing_places_above_contact_at_top_of_ring() {
        let angle = sensor_ring_screen_bearing_rad(Vec2::new(100.0, 100.0), Vec2::new(100.0, 40.0))
            .expect("non-zero bearing");
        assert_close(angle, std::f32::consts::FRAC_PI_2);
    }

    #[test]
    fn screen_bearing_places_left_contact_at_left_of_ring() {
        let angle = sensor_ring_screen_bearing_rad(Vec2::new(100.0, 100.0), Vec2::new(40.0, 100.0))
            .expect("non-zero bearing");
        assert_close(angle, std::f32::consts::PI);
    }

    #[test]
    fn screen_bearing_places_right_contact_at_right_of_ring() {
        let angle =
            sensor_ring_screen_bearing_rad(Vec2::new(100.0, 100.0), Vec2::new(160.0, 100.0))
                .expect("non-zero bearing");
        assert_close(angle, 0.0);
    }

    #[test]
    fn gravity_well_contacts_emit_signal_for_ring_even_when_exact_visible() {
        let mut contact = sidereal_net::TacticalContact {
            entity_id: "star".to_string(),
            kind: "star".to_string(),
            map_icon_asset_id: None,
            faction_id: None,
            position_xy: [0.0, 0.0],
            size_m: None,
            mass_kg: None,
            heading_rad: 0.0,
            velocity_xy: None,
            is_live_now: true,
            last_seen_tick: 1,
            classification: None,
            contact_quality: None,
            signal_strength: None,
            position_accuracy_m: None,
        };
        assert_eq!(contact_signal_strength_for_ring(&contact), Some(1.0));
        contact.kind = "asteroid".to_string();
        assert_eq!(contact_signal_strength_for_ring(&contact), None);
        contact.signal_strength = Some(0.4);
        assert_eq!(contact_signal_strength_for_ring(&contact), Some(0.4));
    }

    #[test]
    fn density_sector_bucketing_wraps_at_full_turn() {
        assert_eq!(density_sector_index(0.0, 24), 0);
        assert_eq!(density_sector_index(std::f32::consts::TAU - 0.001, 24), 23);
        assert_eq!(density_sector_index(std::f32::consts::TAU + 0.001, 24), 0);
    }
}
