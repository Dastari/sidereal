use crate::{
    CollisionOutlineM, ProceduralSprite, compute_collision_half_extents_from_rgba_alpha,
    generate_rdp_collision_outline_from_rgba,
};
use std::f32::consts::TAU;

#[derive(Debug, Clone)]
pub struct ProceduralSpriteImageSet {
    pub width: u32,
    pub height: u32,
    pub albedo_rgba: Vec<u8>,
    pub normal_rgba: Vec<u8>,
}

pub fn generate_procedural_sprite_image_set(
    seed_key: &str,
    sprite: &ProceduralSprite,
) -> Result<ProceduralSpriteImageSet, String> {
    match sprite.generator_id.as_str() {
        "asteroid_rocky_v1" => generate_asteroid_rocky_v1(seed_key, sprite),
        other => Err(format!(
            "unsupported procedural sprite generator_id={other}"
        )),
    }
}

pub fn compute_collision_half_extents_from_procedural_sprite(
    seed_key: &str,
    sprite: &ProceduralSprite,
    target_length_m: f32,
) -> Result<(f32, f32), String> {
    let images = generate_procedural_sprite_image_set(seed_key, sprite)?;
    compute_collision_half_extents_from_rgba_alpha(
        images.width,
        images.height,
        &images.albedo_rgba,
        target_length_m,
    )
}

pub fn generate_rdp_collision_outline_from_procedural_sprite(
    seed_key: &str,
    sprite: &ProceduralSprite,
    target_half_extents_x: f32,
    target_half_extents_y: f32,
) -> Result<CollisionOutlineM, String> {
    let images = generate_procedural_sprite_image_set(seed_key, sprite)?;
    generate_rdp_collision_outline_from_rgba(
        images.width,
        images.height,
        &images.albedo_rgba,
        target_half_extents_x,
        target_half_extents_y,
    )
    .ok_or_else(|| "procedural sprite collision outline generation failed".to_string())
}

fn seed_from_key(seed_key: &str) -> u64 {
    let mut seed = 0xcbf29ce484222325u64;
    for byte in seed_key.as_bytes() {
        seed ^= u64::from(*byte);
        seed = seed.wrapping_mul(0x100000001b3);
    }
    seed
}

fn hash01(seed: u64, salt: u64) -> f32 {
    let mixed = seed
        .wrapping_add(salt.wrapping_mul(0x9e3779b97f4a7c15))
        .rotate_left((salt as u32) & 31);
    let value = ((mixed >> 40) & 0x00ff_ffff) as f32;
    value / 16_777_215.0
}

fn generate_asteroid_rocky_v1(
    seed_key: &str,
    sprite: &ProceduralSprite,
) -> Result<ProceduralSpriteImageSet, String> {
    let width = sprite.resolution_px.max(64);
    let height = sprite.resolution_px.max(64);
    let seed = seed_from_key(seed_key);
    let mut albedo_rgba = vec![0u8; (width * height * 4) as usize];
    let mut normal_rgba = vec![0u8; (width * height * 4) as usize];
    let mut heights = vec![0.0f32; (width * height) as usize];
    let crater_count = sprite.crater_count.clamp(1, 12) as usize;
    let mut crater_centers = [(0.0f32, 0.0f32, 0.0f32); 12];

    for (idx, crater) in crater_centers.iter_mut().enumerate().take(crater_count) {
        let salt = idx as u64 * 17;
        let angle = hash01(seed, salt + 1) * TAU;
        let radius = 0.08 + hash01(seed, salt + 2) * 0.38;
        let size = 0.10 + hash01(seed, salt + 3) * 0.14;
        *crater = (angle.cos() * radius, angle.sin() * radius, size);
    }

    for y in 0..height {
        for x in 0..width {
            let fx = (x as f32 + 0.5) / width as f32;
            let fy = (y as f32 + 0.5) / height as f32;
            let nx = fx * 2.0 - 1.0;
            let ny = fy * 2.0 - 1.0;
            let angle = ny.atan2(nx);
            let dist = (nx * nx + ny * ny).sqrt();

            let lobe_0 = (angle * 2.0 + hash01(seed, 11) * TAU).sin();
            let lobe_1 = (angle * 3.0 + hash01(seed, 12) * TAU).cos();
            let lobe_2 = (angle * 5.0 + hash01(seed, 13) * TAU).sin();
            let silhouette_radius = 0.74
                + lobe_0 * sprite.lobe_amplitude
                + lobe_1 * (sprite.lobe_amplitude * 0.62)
                + lobe_2 * (sprite.lobe_amplitude * 0.42)
                + (hash01(seed, (x as u64 + 1) * 3 + (y as u64 + 1) * 5) - 0.5) * sprite.edge_noise;

            let edge = silhouette_radius - dist;
            if edge <= -0.03 {
                continue;
            }

            let alpha = (edge / 0.03).clamp(0.0, 1.0);
            let lighting = (0.55 + (-nx * 0.32) + (ny * 0.18)).clamp(0.18, 1.0);
            let grain = hash01(seed, x as u64 * 131 + y as u64 * 977);
            let mut shade = 0.38 + lighting * 0.42 + (grain - 0.5) * 0.12;
            let mut height_value = 0.55 + lighting * 0.18 + (grain - 0.5) * 0.06;

            for crater in crater_centers.iter().take(crater_count) {
                let dx = nx - crater.0;
                let dy = ny - crater.1;
                let crater_dist = (dx * dx + dy * dy).sqrt();
                if crater_dist < crater.2 {
                    let t = 1.0 - (crater_dist / crater.2).clamp(0.0, 1.0);
                    shade -= t * 0.18;
                    height_value -= t * 0.28;
                }
            }

            shade = shade.clamp(0.08, 0.92);
            height_value = height_value.clamp(0.0, 1.0);

            let dark = sprite.palette_dark_rgb;
            let light = sprite.palette_light_rgb;
            let r = ((dark[0] + (light[0] - dark[0]) * shade).clamp(0.0, 1.0) * 255.0) as u8;
            let g = ((dark[1] + (light[1] - dark[1]) * shade).clamp(0.0, 1.0) * 255.0) as u8;
            let b = ((dark[2] + (light[2] - dark[2]) * shade).clamp(0.0, 1.0) * 255.0) as u8;
            let idx = ((y * width + x) * 4) as usize;
            albedo_rgba[idx] = r;
            albedo_rgba[idx + 1] = g;
            albedo_rgba[idx + 2] = b;
            albedo_rgba[idx + 3] = (alpha * 255.0) as u8;
            heights[(y * width + x) as usize] = height_value * alpha;
        }
    }

    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            let alpha = albedo_rgba[idx + 3];
            if alpha == 0 {
                normal_rgba[idx] = 128;
                normal_rgba[idx + 1] = 128;
                normal_rgba[idx + 2] = 255;
                normal_rgba[idx + 3] = 0;
                continue;
            }
            let sample = |sx: i32, sy: i32| -> f32 {
                let px = sx.clamp(0, width as i32 - 1) as u32;
                let py = sy.clamp(0, height as i32 - 1) as u32;
                heights[(py * width + px) as usize]
            };
            let h_l = sample(x as i32 - 1, y as i32);
            let h_r = sample(x as i32 + 1, y as i32);
            let h_d = sample(x as i32, y as i32 - 1);
            let h_u = sample(x as i32, y as i32 + 1);
            let nx = (h_l - h_r) * 2.6;
            let ny = (h_d - h_u) * 2.6;
            let nz = 1.0f32;
            let inv_len = (nx * nx + ny * ny + nz * nz).sqrt().recip();
            let npx = (nx * inv_len * 0.5 + 0.5).clamp(0.0, 1.0);
            let npy = (ny * inv_len * 0.5 + 0.5).clamp(0.0, 1.0);
            let npz = (nz * inv_len * 0.5 + 0.5).clamp(0.0, 1.0);
            normal_rgba[idx] = (npx * 255.0) as u8;
            normal_rgba[idx + 1] = (npy * 255.0) as u8;
            normal_rgba[idx + 2] = (npz * 255.0) as u8;
            normal_rgba[idx + 3] = alpha;
        }
    }

    Ok(ProceduralSpriteImageSet {
        width,
        height,
        albedo_rgba,
        normal_rgba,
    })
}
