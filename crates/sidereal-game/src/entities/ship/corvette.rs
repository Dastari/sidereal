//! Corvette ship archetype: bundle, defaults, spawn helper, and deterministic spawn position.
//! Canonical starter ship granted on registration.

use bevy::prelude::*;
use image::ImageReader;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::{
    AfterburnerCapability, CollisionAabbM, CollisionOutlineM, Engine, FlightComputer, FlightTuning,
    FuelTank, HealthPool, MaxVelocityMps, SizeM,
};

// -----------------------------------------------------------------------------
// Defaults (single source for this archetype)
// -----------------------------------------------------------------------------

pub fn default_corvette_flight_computer() -> FlightComputer {
    FlightComputer {
        profile: "basic_fly_by_wire".to_string(),
        throttle: 0.0,
        yaw_input: 0.0,
        brake_active: false,
        turn_rate_deg_s: 90.0,
    }
}

pub fn default_corvette_mass_kg() -> f32 {
    15_000.0
}

pub fn default_corvette_size() -> SizeM {
    SizeM {
        length: 25.0,
        width: 25.0,
        height: 8.0,
    }
}

pub fn default_corvette_collision_aabb() -> CollisionAabbM {
    // Tight hull-oriented hitbox: keeps wing tips mostly cosmetic while preserving center-mass hits.
    CollisionAabbM {
        half_extents: Vec3::new(7.2, 10.6, default_corvette_size().height * 0.5),
    }
}

pub fn default_corvette_collision_outline() -> CollisionOutlineM {
    static OUTLINE: OnceLock<CollisionOutlineM> = OnceLock::new();
    OUTLINE
        .get_or_init(compute_corvette_collision_outline)
        .clone()
}

fn compute_corvette_collision_outline() -> CollisionOutlineM {
    const CORVETTE_SPRITE_BYTES: &[u8] =
        include_bytes!("../../../../../data/sprites/ships/corvette.png");
    let Some(outline) = generate_outline_from_sprite(CORVETTE_SPRITE_BYTES) else {
        return fallback_corvette_collision_outline();
    };
    outline
}

fn generate_outline_from_sprite(sprite_png: &[u8]) -> Option<CollisionOutlineM> {
    let image = ImageReader::new(std::io::Cursor::new(sprite_png))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?
        .to_rgba8();
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return None;
    }

    let mut opaque = vec![false; (width * height) as usize];
    let mut min_x = width;
    let mut max_x = 0;
    let mut min_y = height;
    let mut max_y = 0;
    let mut any_opaque = false;
    for y in 0..height {
        for x in 0..width {
            let alpha = image.get_pixel(x, y).0[3];
            let is_opaque_px = alpha >= 16;
            opaque[(y * width + x) as usize] = is_opaque_px;
            if is_opaque_px {
                any_opaque = true;
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }
    }
    if !any_opaque {
        return None;
    }

    let contour = trace_primary_contour(width as i32, height as i32, &opaque)?;
    let simplified = rdp_simplify_closed(&contour, 1.35);
    if simplified.len() < 3 {
        return None;
    }

    let bbox_w_px = ((max_x - min_x + 1) as f32).max(1.0);
    let bbox_h_px = ((max_y - min_y + 1) as f32).max(1.0);
    let target = default_corvette_collision_aabb().half_extents;
    const OUTLINE_SCALE_BIAS: f32 = 1.10;
    // Sprite pixel X corresponds to ship forward length in this art, while pixel Y corresponds
    // to ship width, so map axes accordingly.
    let scale_x = ((target.y * 2.0) / bbox_w_px) * OUTLINE_SCALE_BIAS;
    let scale_y = ((target.x * 2.0) / bbox_h_px) * OUTLINE_SCALE_BIAS;
    // Center the collider around opaque content (not full texture canvas),
    // which avoids front/back drift when sprites have asymmetric padding.
    let center_x = (min_x as f32 + (max_x as f32 + 1.0)) * 0.5;
    let center_y = (min_y as f32 + (max_y as f32 + 1.0)) * 0.5;
    let points = simplified
        .into_iter()
        .map(|p| {
            let local_x_px = p.x - center_x;
            let local_y_px = center_y - p.y;
            Vec2::new(local_x_px * scale_x, local_y_px * scale_y)
        })
        .collect::<Vec<_>>();
    if points.len() < 3 {
        return None;
    }
    Some(CollisionOutlineM { points })
}

fn fallback_corvette_collision_outline() -> CollisionOutlineM {
    // Manual hull-ish fallback aligned with the corvette silhouette.
    CollisionOutlineM {
        points: vec![
            Vec2::new(-2.3, 11.6),
            Vec2::new(2.3, 11.6),
            Vec2::new(7.8, 6.2),
            Vec2::new(8.4, -1.5),
            Vec2::new(3.6, -10.8),
            Vec2::new(-3.6, -10.8),
            Vec2::new(-8.4, -1.5),
            Vec2::new(-7.8, 6.2),
        ],
    }
}

fn is_opaque(mask: &[bool], width: i32, height: i32, x: i32, y: i32) -> bool {
    if x < 0 || y < 0 || x >= width || y >= height {
        return false;
    }
    mask[(y * width + x) as usize]
}

fn trace_primary_contour(width: i32, height: i32, mask: &[bool]) -> Option<Vec<Vec2>> {
    let mut edges = Vec::<(IVec2, IVec2)>::new();
    for y in 0..height {
        for x in 0..width {
            if !is_opaque(mask, width, height, x, y) {
                continue;
            }
            if !is_opaque(mask, width, height, x, y - 1) {
                edges.push((IVec2::new(x, y), IVec2::new(x + 1, y)));
            }
            if !is_opaque(mask, width, height, x + 1, y) {
                edges.push((IVec2::new(x + 1, y), IVec2::new(x + 1, y + 1)));
            }
            if !is_opaque(mask, width, height, x, y + 1) {
                edges.push((IVec2::new(x + 1, y + 1), IVec2::new(x, y + 1)));
            }
            if !is_opaque(mask, width, height, x - 1, y) {
                edges.push((IVec2::new(x, y + 1), IVec2::new(x, y)));
            }
        }
    }
    if edges.is_empty() {
        return None;
    }

    let mut by_start = HashMap::<IVec2, Vec<usize>>::new();
    for (idx, (start, _)) in edges.iter().enumerate() {
        by_start.entry(*start).or_default().push(idx);
    }
    let mut visited = vec![false; edges.len()];
    let mut best_loop = Vec::<IVec2>::new();

    for idx in 0..edges.len() {
        if visited[idx] {
            continue;
        }
        let (start, mut end) = edges[idx];
        let mut current = idx;
        let mut loop_points = vec![start];
        visited[current] = true;
        let mut closed = false;

        for _ in 0..=edges.len() {
            if end == start {
                closed = true;
                break;
            }
            loop_points.push(end);
            let next = by_start.get(&end).and_then(|candidates| {
                candidates
                    .iter()
                    .copied()
                    .find(|candidate| !visited[*candidate])
            });
            let Some(next_idx) = next else {
                break;
            };
            visited[next_idx] = true;
            current = next_idx;
            end = edges[current].1;
        }

        if closed && loop_points.len() > best_loop.len() {
            best_loop = loop_points;
        }
    }

    if best_loop.len() < 3 {
        return None;
    }
    Some(
        best_loop
            .into_iter()
            .map(|p| Vec2::new(p.x as f32, p.y as f32))
            .collect(),
    )
}

fn rdp_simplify_closed(points: &[Vec2], epsilon: f32) -> Vec<Vec2> {
    if points.len() < 4 {
        return points.to_vec();
    }
    let mut open = points.to_vec();
    if open.first() == open.last() {
        open.pop();
    }
    let simplified = rdp_simplify(&open, epsilon);
    if simplified.len() >= 3 {
        simplified
    } else {
        open
    }
}

fn rdp_simplify(points: &[Vec2], epsilon: f32) -> Vec<Vec2> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let first = points[0];
    let last = points[points.len() - 1];
    let mut max_dist = 0.0_f32;
    let mut max_idx = 0usize;
    for (idx, point) in points.iter().enumerate().take(points.len() - 1).skip(1) {
        let dist = perpendicular_distance(*point, first, last);
        if dist > max_dist {
            max_dist = dist;
            max_idx = idx;
        }
    }
    if max_dist > epsilon {
        let mut left = rdp_simplify(&points[..=max_idx], epsilon);
        let right = rdp_simplify(&points[max_idx..], epsilon);
        let _ = left.pop();
        left.extend(right);
        left
    } else {
        vec![first, last]
    }
}

fn perpendicular_distance(point: Vec2, line_start: Vec2, line_end: Vec2) -> f32 {
    let line = line_end - line_start;
    let len_sq = line.length_squared();
    if len_sq <= f32::EPSILON {
        return (point - line_start).length();
    }
    let t = ((point - line_start).dot(line) / len_sq).clamp(0.0, 1.0);
    let projection = line_start + line * t;
    (point - projection).length()
}

pub fn default_corvette_flight_tuning() -> FlightTuning {
    // Brake and auto-brake accel set so tuning does not limit decel; engine reverse thrust is the limit (same as forward).
    let forward_accel_mps2 =
        300_000.0 / (default_corvette_mass_kg() + 50.0 + 500.0 + 1100.0 * 2.0 + 120.0);
    FlightTuning {
        max_linear_accel_mps2: 120.0,
        passive_brake_accel_mps2: forward_accel_mps2,
        active_brake_accel_mps2: forward_accel_mps2,
        drag_per_s: 0.4,
    }
}

pub fn default_corvette_max_velocity_mps() -> MaxVelocityMps {
    MaxVelocityMps(100.0)
}

pub fn default_corvette_health_pool() -> HealthPool {
    HealthPool {
        current: 1000.0,
        maximum: 1000.0,
    }
}

pub fn default_corvette_asset_id() -> &'static str {
    "corvette_01"
}

pub fn default_starfield_shader_asset_id() -> &'static str {
    "starfield_wgsl"
}

pub fn default_space_background_shader_asset_id() -> &'static str {
    "space_background_wgsl"
}

pub fn default_space_bg_flare_white_asset_id() -> &'static str {
    "space_bg_flare_white_png"
}

pub fn default_space_bg_flare_blue_asset_id() -> &'static str {
    "space_bg_flare_blue_png"
}

pub fn default_space_bg_flare_red_asset_id() -> &'static str {
    "space_bg_flare_red_png"
}

pub fn default_space_bg_flare_sun_asset_id() -> &'static str {
    "space_bg_flare_sun_png"
}

/// Default engine stats for corvette (used by bundle and graph records).
/// Forward thrust halved; reverse and braking use same magnitude as forward.
pub fn default_corvette_engine() -> Engine {
    let forward_thrust = 300_000.0; // half of previous 600_000
    Engine {
        thrust: forward_thrust,
        reverse_thrust: forward_thrust,
        torque_thrust: 1_500_000.0,
        burn_rate_kg_s: 0.8,
    }
}

/// Default fuel tank for corvette modules.
pub fn default_corvette_fuel_tank() -> FuelTank {
    FuelTank { fuel_kg: 1000.0 }
}

pub fn default_corvette_afterburner_capability() -> AfterburnerCapability {
    AfterburnerCapability {
        enabled: true,
        multiplier: 1.5,
        fuel_burn_multiplier: 2.0,
        max_afterburner_velocity_mps: Some(250.0),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorvetteModuleKind {
    FlightComputer,
    Engine,
    FuelTank,
    BallisticGatling,
}

#[derive(Debug, Clone, Copy)]
pub struct CorvetteModuleSpec {
    pub module_id: &'static str,
    pub hardpoint_id: &'static str,
    pub display_name: &'static str,
    pub mass_kg: f32,
    pub kind: CorvetteModuleKind,
}

pub fn default_corvette_module_specs() -> [CorvetteModuleSpec; 5] {
    [
        CorvetteModuleSpec {
            module_id: "flight_computer",
            hardpoint_id: "computer_core",
            display_name: "Flight Computer MK1",
            mass_kg: 50.0,
            kind: CorvetteModuleKind::FlightComputer,
        },
        CorvetteModuleSpec {
            module_id: "engine_main",
            hardpoint_id: "engine_main_aft",
            display_name: "Engine Main",
            mass_kg: 500.0,
            kind: CorvetteModuleKind::Engine,
        },
        CorvetteModuleSpec {
            module_id: "fuel_tank_left",
            hardpoint_id: "fuel_left",
            display_name: "Fuel Tank Port",
            mass_kg: 1100.0,
            kind: CorvetteModuleKind::FuelTank,
        },
        CorvetteModuleSpec {
            module_id: "fuel_tank_right",
            hardpoint_id: "fuel_right",
            display_name: "Fuel Tank Starboard",
            mass_kg: 1100.0,
            kind: CorvetteModuleKind::FuelTank,
        },
        CorvetteModuleSpec {
            module_id: "weapon_gatling_fore",
            hardpoint_id: "weapon_fore_center",
            display_name: "Ballistic Gatling",
            mass_kg: 120.0,
            kind: CorvetteModuleKind::BallisticGatling,
        },
    ]
}
