//! Tactical sensor ring HUD presentation.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::window::PrimaryWindow;
use sidereal_game::{
    EntityGuid, MountedOn, ScannerComponent, ScannerContactDetailTier, VisibilityRangeM,
};
use sidereal_runtime_sync::parse_guid_from_entity_id;
use std::collections::{HashMap, HashSet};

use super::app_state::{ClientSession, LocalPlayerViewState};
use super::components::{ControlledEntity, GameplayCamera, UiOverlayCamera, WorldEntity};
use super::dev_console::{DevConsoleState, is_console_open};
use super::ecs_util::queue_despawn_if_exists;
use super::platform::UI_OVERLAY_RENDER_LAYER;
use super::resources::{
    ActiveScannerProfileCache, ResolvedScannerProfile, TacticalContactsCache, TacticalMapUiState,
    TacticalSensorRingUiState,
};

const SENSOR_RING_TICK_COUNT: usize = 96;
const SENSOR_RING_DENSITY_SECTORS: usize = 24;
const SENSOR_RING_RADIUS_RATIO: f32 = 0.22;
const SENSOR_RING_MIN_RADIUS_PX: f32 = 140.0;
const SENSOR_RING_MAX_RADIUS_PX: f32 = 260.0;
const SENSOR_RING_CONTACT_BAND_OFFSET_PX: f32 = 18.0;
const SENSOR_RING_CENTER_MARGIN_PX: f32 = 32.0;
const SENSOR_RING_FADE_RATE: f32 = 12.0;

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
    material: Handle<ColorMaterial>,
}

struct ControlledSensorEntity<'a> {
    entity_id: &'a str,
    guid: &'a EntityGuid,
    transform: &'a Transform,
    global_transform: &'a GlobalTransform,
}

type ControlledScannerProfileQuery<'w, 's> = Query<
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

type MountedScannerProfileQuery<'w, 's> = Query<
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
            &'_ Transform,
            &'_ GlobalTransform,
        ),
        (With<WorldEntity>, Without<TacticalSensorRingElement>),
    >,
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut color_materials: ResMut<'_, Assets<ColorMaterial>>,
    mut render_cache: Local<'_, SensorRingRenderCache>,
    mut elements: Query<
        '_,
        '_,
        (
            Entity,
            &'_ TacticalSensorRingElement,
            &'_ MeshMaterial2d<ColorMaterial>,
            &'_ mut Transform,
            &'_ mut Visibility,
        ),
        With<TacticalSensorRingElement>,
    >,
) {
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
            |(controlled, guid, transform, global_transform)| ControlledSensorEntity {
                entity_id: controlled.entity_id.as_str(),
                guid,
                transform,
                global_transform,
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
    let Ok(center_viewport) = camera.world_to_viewport(camera_transform, center_world) else {
        return;
    };
    let viewport_width = window.width().max(1.0);
    let viewport_height = window.height().max(1.0);
    let radius = (viewport_width.min(viewport_height) * SENSOR_RING_RADIUS_RATIO)
        .clamp(SENSOR_RING_MIN_RADIUS_PX, SENSOR_RING_MAX_RADIUS_PX);
    let center_viewport =
        clamp_ring_center(center_viewport, viewport_width, viewport_height, radius);
    let center =
        viewport_to_overlay(center_viewport, viewport_width, viewport_height, -6.0).truncate();
    let quad_mesh = render_cache
        .quad_mesh
        .get_or_insert_with(|| meshes.add(Rectangle::new(1.0, 1.0)))
        .clone();

    let mut existing = HashMap::<String, ExistingSensorRingElement>::new();
    for (entity, element, material, _, _) in &mut elements {
        existing.insert(
            element.key.clone(),
            ExistingSensorRingElement {
                entity,
                material: material.0.clone(),
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
    draw_forward_awareness_cue(
        &mut commands,
        &mut color_materials,
        &quad_mesh,
        &mut existing,
        &mut seen,
        center,
        radius,
        alpha,
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
    );
    draw_contact_markers(
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
fn draw_forward_awareness_cue(
    commands: &mut Commands<'_, '_>,
    color_materials: &mut Assets<ColorMaterial>,
    quad_mesh: &Handle<Mesh>,
    existing: &mut HashMap<String, ExistingSensorRingElement>,
    seen: &mut HashSet<String>,
    center: Vec2,
    radius: f32,
    alpha: f32,
) {
    for offset in -4_i32..=4 {
        let angle = std::f32::consts::FRAC_PI_2 + offset as f32 * 0.026;
        let unit = Vec2::new(angle.cos(), angle.sin());
        let transform = Transform {
            translation: (center + unit * (radius + 11.0)).extend(-5.9),
            rotation: Quat::from_rotation_z(angle - std::f32::consts::FRAC_PI_2),
            scale: Vec3::new(3.0, 20.0 - offset.unsigned_abs() as f32 * 2.4, 1.0),
        };
        upsert_sensor_ring_rect(
            commands,
            color_materials,
            quad_mesh,
            existing,
            seen,
            format!("forward:{offset}"),
            Color::srgba(0.74, 0.97, 1.0, 0.55 * alpha),
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
        let delta = Vec2::new(
            contact.position_xy[0] as f32 - controlled.transform.translation.x,
            contact.position_xy[1] as f32 - controlled.transform.translation.y,
        );
        if delta.length() > profile.effective_range_m {
            continue;
        }
        if let Some(angle) =
            sensor_ring_visual_bearing_rad(controlled_heading_rad(controlled.transform), delta)
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
) {
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
        let delta = Vec2::new(
            contact.position_xy[0] as f32 - controlled.transform.translation.x,
            contact.position_xy[1] as f32 - controlled.transform.translation.y,
        );
        let distance = delta.length();
        if distance > profile.effective_range_m && contact.signal_strength.is_none() {
            continue;
        }
        let Some(angle) =
            sensor_ring_visual_bearing_rad(controlled_heading_rad(controlled.transform), delta)
        else {
            continue;
        };
        let unit = Vec2::new(angle.cos(), angle.sin());
        let role = contact_marker_role(contact.classification.as_deref(), contact.kind.as_str());
        let (r, g, b) = contact_marker_rgb(role);
        let marker_alpha =
            contact_marker_alpha(contact.is_live_now, distance, profile.effective_range_m) * alpha;
        let marker_size = contact_marker_size(distance, profile.effective_range_m);
        let transform = Transform {
            translation: (center + unit * (radius + SENSOR_RING_CONTACT_BAND_OFFSET_PX))
                .extend(-5.7),
            rotation: contact_marker_rotation(profile, contact, angle),
            scale: Vec3::new(marker_size, marker_size, 1.0),
        };
        upsert_sensor_ring_rect(
            commands,
            color_materials,
            quad_mesh,
            existing,
            seen,
            format!("contact:{}", contact.entity_id),
            Color::srgba(r, g, b, marker_alpha),
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
        if let Some(material) = color_materials.get_mut(&existing.material) {
            material.color = color;
        } else {
            commands.entity(existing.entity).insert(MeshMaterial2d(
                color_materials.add(ColorMaterial::from(color)),
            ));
        }
        commands.entity(existing.entity).insert((
            Mesh2d(quad_mesh.clone()),
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

fn despawn_sensor_ring_elements(
    commands: &mut Commands<'_, '_>,
    elements: &mut Query<
        '_,
        '_,
        (
            Entity,
            &'_ TacticalSensorRingElement,
            &'_ MeshMaterial2d<ColorMaterial>,
            &'_ mut Transform,
            &'_ mut Visibility,
        ),
        With<TacticalSensorRingElement>,
    >,
) {
    for (entity, _, _, _, _) in elements {
        queue_despawn_if_exists(commands, entity);
    }
}

fn contact_distance_sq(
    contact: &sidereal_net::TacticalContact,
    controlled: &ControlledSensorEntity<'_>,
) -> f32 {
    let dx = contact.position_xy[0] as f32 - controlled.transform.translation.x;
    let dy = contact.position_xy[1] as f32 - controlled.transform.translation.y;
    dx.mul_add(dx, dy * dy)
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

fn contact_marker_rotation(
    profile: ResolvedScannerProfile,
    contact: &sidereal_net::TacticalContact,
    bearing_angle: f32,
) -> Quat {
    if profile.detail_tier >= ScannerContactDetailTier::Telemetry && contact.velocity_xy.is_some() {
        Quat::from_rotation_z(contact.heading_rad as f32)
    } else {
        Quat::from_rotation_z(bearing_angle + std::f32::consts::FRAC_PI_4)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContactMarkerRole {
    Unknown,
    Friendly,
    Hostile,
    Neutral,
    Landmark,
}

fn contact_marker_role(classification: Option<&str>, kind: &str) -> ContactMarkerRole {
    match classification {
        Some("friendly") => ContactMarkerRole::Friendly,
        Some("hostile") => ContactMarkerRole::Hostile,
        Some("neutral") => ContactMarkerRole::Neutral,
        Some("unknown") | None => {
            if matches!(kind, "landmark" | "planet" | "star" | "static_landmark") {
                ContactMarkerRole::Landmark
            } else {
                ContactMarkerRole::Unknown
            }
        }
        _ => ContactMarkerRole::Unknown,
    }
}

fn contact_marker_rgb(role: ContactMarkerRole) -> (f32, f32, f32) {
    match role {
        ContactMarkerRole::Unknown => (0.58, 0.76, 0.9),
        ContactMarkerRole::Friendly => (0.24, 0.68, 1.0),
        ContactMarkerRole::Hostile => (1.0, 0.16, 0.18),
        ContactMarkerRole::Neutral => (0.98, 0.78, 0.28),
        ContactMarkerRole::Landmark => (0.86, 0.88, 0.78),
    }
}

fn controlled_heading_rad(transform: &Transform) -> f32 {
    let (_, _, heading_rad) = transform.rotation.to_euler(EulerRot::XYZ);
    heading_rad
}

fn sensor_ring_visual_bearing_rad(observer_heading_rad: f32, delta_world_xy: Vec2) -> Option<f32> {
    if delta_world_xy.length_squared() <= f32::EPSILON {
        return None;
    }
    let world_angle = delta_world_xy.y.atan2(delta_world_xy.x);
    Some(
        (world_angle - observer_heading_rad + std::f32::consts::FRAC_PI_2)
            .rem_euclid(std::f32::consts::TAU),
    )
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
    fn bearing_places_forward_contact_at_top_of_ring() {
        let angle =
            sensor_ring_visual_bearing_rad(0.0, Vec2::new(10.0, 0.0)).expect("non-zero bearing");
        assert_close(angle, std::f32::consts::FRAC_PI_2);
    }

    #[test]
    fn bearing_is_relative_to_controlled_heading() {
        let angle =
            sensor_ring_visual_bearing_rad(std::f32::consts::FRAC_PI_2, Vec2::new(0.0, 10.0))
                .expect("non-zero bearing");
        assert_close(angle, std::f32::consts::FRAC_PI_2);
    }

    #[test]
    fn density_sector_bucketing_wraps_at_full_turn() {
        assert_eq!(density_sector_index(0.0, 24), 0);
        assert_eq!(density_sector_index(std::f32::consts::TAU - 0.001, 24), 23);
        assert_eq!(density_sector_index(std::f32::consts::TAU + 0.001, 24), 0);
    }

    #[test]
    fn contact_role_uses_disclosed_classification_only() {
        assert_eq!(
            contact_marker_role(Some("hostile"), "unknown"),
            ContactMarkerRole::Hostile
        );
        assert_eq!(
            contact_marker_role(Some("unknown"), "planet"),
            ContactMarkerRole::Landmark
        );
        assert_eq!(
            contact_marker_role(None, "ship"),
            ContactMarkerRole::Unknown
        );
    }
}
