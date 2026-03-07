use bevy::prelude::*;
use image::ImageReader;
use std::collections::HashMap;

use crate::CollisionOutlineM;

const OUTLINE_ALPHA_THRESHOLD: u8 = 16;
const OUTLINE_SIMPLIFY_EPSILON: f32 = 1.35;
const OUTLINE_SCALE_BIAS: f32 = 1.0;

pub fn generate_rdp_collision_outline_from_sprite_png(
    sprite_png: &[u8],
    target_half_extents_x: f32,
    target_half_extents_y: f32,
) -> Result<CollisionOutlineM, String> {
    let image = ImageReader::new(std::io::Cursor::new(sprite_png))
        .with_guessed_format()
        .map_err(|err| err.to_string())?
        .decode()
        .map_err(|err| err.to_string())?
        .to_rgba8();
    generate_rdp_collision_outline_from_rgba(
        image.width(),
        image.height(),
        image.as_raw(),
        target_half_extents_x,
        target_half_extents_y,
    )
    .ok_or_else(|| {
        "rdp collision outline generation failed for provided sprite payload".to_string()
    })
}

pub fn compute_collision_half_extents_from_sprite_length(
    sprite_png: &[u8],
    target_length_m: f32,
) -> Result<(f32, f32), String> {
    if !(target_length_m.is_finite() && target_length_m > 0.0) {
        return Err("target_length_m must be a positive finite number".to_string());
    }
    let image = ImageReader::new(std::io::Cursor::new(sprite_png))
        .with_guessed_format()
        .map_err(|err| err.to_string())?
        .decode()
        .map_err(|err| err.to_string())?
        .to_rgba8();
    compute_collision_half_extents_from_rgba_alpha(
        image.width(),
        image.height(),
        image.as_raw(),
        target_length_m,
    )
}

pub fn generate_rdp_collision_outline_from_rgba(
    width: u32,
    height: u32,
    rgba: &[u8],
    target_half_extents_x: f32,
    target_half_extents_y: f32,
) -> Option<CollisionOutlineM> {
    if width == 0 || height == 0 {
        return None;
    }
    if rgba.len() < (width as usize) * (height as usize) * 4 {
        return None;
    }

    let mut opaque = vec![false; (width * height) as usize];
    let mut any_opaque = false;
    for y in 0..height {
        for x in 0..width {
            let alpha = rgba[((y * width + x) * 4 + 3) as usize];
            let is_opaque_px = alpha >= OUTLINE_ALPHA_THRESHOLD;
            opaque[(y * width + x) as usize] = is_opaque_px;
            if is_opaque_px {
                any_opaque = true;
            }
        }
    }
    if !any_opaque {
        return None;
    }

    let contour = trace_primary_contour(width as i32, height as i32, &opaque)?;
    let simplified = rdp_simplify_closed(&contour, OUTLINE_SIMPLIFY_EPSILON);
    if simplified.len() < 3 {
        return None;
    }

    let image_w_px = (width as f32).max(1.0);
    let image_h_px = (height as f32).max(1.0);
    let scale_x = ((target_half_extents_x * 2.0) / image_w_px) * OUTLINE_SCALE_BIAS;
    let scale_y = ((target_half_extents_y * 2.0) / image_h_px) * OUTLINE_SCALE_BIAS;
    let center_x = image_w_px * 0.5;
    let center_y = image_h_px * 0.5;
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

pub fn compute_collision_half_extents_from_rgba_alpha(
    width: u32,
    height: u32,
    rgba: &[u8],
    target_length_m: f32,
) -> Result<(f32, f32), String> {
    if !(target_length_m.is_finite() && target_length_m > 0.0) {
        return Err("target_length_m must be a positive finite number".to_string());
    }
    if width == 0 || height == 0 {
        return Err("sprite image must have non-zero width and height".to_string());
    }
    if rgba.len() < (width as usize) * (height as usize) * 4 {
        return Err("rgba sprite buffer is smaller than width * height * 4".to_string());
    }

    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut any_opaque = false;
    for y in 0..height {
        for x in 0..width {
            let alpha = rgba[((y * width + x) * 4 + 3) as usize];
            if alpha < OUTLINE_ALPHA_THRESHOLD {
                continue;
            }
            any_opaque = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if !any_opaque {
        return Err("sprite image must contain at least one opaque pixel".to_string());
    }

    let occupied_w = (max_x - min_x + 1) as f32;
    let occupied_h = (max_y - min_y + 1) as f32;
    if occupied_w >= occupied_h {
        let target_minor_m = target_length_m * (occupied_h / occupied_w);
        Ok((target_length_m * 0.5, target_minor_m * 0.5))
    } else {
        let target_minor_m = target_length_m * (occupied_w / occupied_h);
        Ok((target_minor_m * 0.5, target_length_m * 0.5))
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
