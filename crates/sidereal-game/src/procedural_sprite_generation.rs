use crate::{
    CollisionOutlineM, ProceduralSprite, ProceduralSpriteSurfaceStyle,
    compute_collision_half_extents_from_rgba_alpha, generate_rdp_collision_outline_from_rgba,
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

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn hash01_grid(seed: u64, x: i32, y: i32, salt: u64) -> f32 {
    let mixed = seed
        ^ (x as i64 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (y as i64 as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f)
        ^ salt.wrapping_mul(0x1656_67b1_9e37_79f9);
    hash01(mixed, salt ^ 0xa24b_aed4_963e_e407)
}

fn value_noise2d(seed: u64, x: f32, y: f32, salt: u64) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let a = hash01_grid(seed, ix, iy, salt);
    let b = hash01_grid(seed, ix + 1, iy, salt);
    let c = hash01_grid(seed, ix, iy + 1, salt);
    let d = hash01_grid(seed, ix + 1, iy + 1, salt);
    let ab = a + (b - a) * ux;
    let cd = c + (d - c) * ux;
    ab + (cd - ab) * uy
}

fn fbm2d(seed: u64, x: f32, y: f32, salt: u64, octaves: usize) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 0.5;
    let mut frequency = 1.0;
    let mut norm = 0.0;
    for octave in 0..octaves {
        value += value_noise2d(
            seed,
            x * frequency,
            y * frequency,
            salt + octave as u64 * 31,
        ) * amplitude;
        norm += amplitude;
        frequency *= 2.0;
        amplitude *= 0.5;
    }
    if norm <= f32::EPSILON {
        0.0
    } else {
        value / norm
    }
}

fn generate_asteroid_rocky_v1(
    seed_key: &str,
    sprite: &ProceduralSprite,
) -> Result<ProceduralSpriteImageSet, String> {
    let width = sprite.resolution_px.max(64);
    let height = sprite.resolution_px.max(64);
    let effective_seed_key = sprite.family_seed_key.as_deref().unwrap_or(seed_key);
    let seed = seed_from_key(effective_seed_key);
    let mut albedo_rgba = vec![0u8; (width * height * 4) as usize];
    let mut normal_rgba = vec![0u8; (width * height * 4) as usize];
    let mut heights = vec![0.0f32; (width * height) as usize];
    let crater_count = sprite.crater_count.clamp(1, 12) as usize;
    let mut crater_centers = [(0.0f32, 0.0f32, 0.0f32); 12];
    let pixel_step = sprite.pixel_step_px.clamp(1, 8);
    let style_silhouette = match sprite.surface_style {
        ProceduralSpriteSurfaceStyle::Rocky => 1.0,
        ProceduralSpriteSurfaceStyle::Carbonaceous => 0.82,
        ProceduralSpriteSurfaceStyle::Metallic => 0.72,
        ProceduralSpriteSurfaceStyle::Shard => 1.42,
        ProceduralSpriteSurfaceStyle::GemRich => 0.95,
    };
    let style_crater_scale = match sprite.surface_style {
        ProceduralSpriteSurfaceStyle::Metallic => 0.55,
        ProceduralSpriteSurfaceStyle::Shard => 0.45,
        ProceduralSpriteSurfaceStyle::GemRich => 0.72,
        ProceduralSpriteSurfaceStyle::Rocky | ProceduralSpriteSurfaceStyle::Carbonaceous => 1.0,
    };

    for (idx, crater) in crater_centers.iter_mut().enumerate().take(crater_count) {
        let salt = idx as u64 * 17;
        let angle = hash01(seed, salt + 1) * TAU;
        let radius = 0.08 + hash01(seed, salt + 2) * 0.38;
        let size = 0.10 + hash01(seed, salt + 3) * 0.14;
        *crater = (angle.cos() * radius, angle.sin() * radius, size);
    }

    for y in 0..height {
        for x in 0..width {
            let qx = (x / pixel_step) * pixel_step;
            let qy = (y / pixel_step) * pixel_step;
            let fx = (qx as f32 + pixel_step as f32 * 0.5) / width as f32;
            let fy = (qy as f32 + pixel_step as f32 * 0.5) / height as f32;
            let nx = fx * 2.0 - 1.0;
            let ny = fy * 2.0 - 1.0;
            let angle = ny.atan2(nx);
            let dist = (nx * nx + ny * ny).sqrt();

            let lobe_0 = (angle * 2.0 + hash01(seed, 11) * TAU).sin();
            let lobe_1 = (angle * 3.0 + hash01(seed, 12) * TAU).cos();
            let lobe_2 = (angle * 5.0 + hash01(seed, 13) * TAU).sin();
            let shard_lobe = (angle * 8.0 + hash01(seed, 14) * TAU).sin().abs();
            let broad_edge_noise = (fbm2d(seed, nx * 2.2 + 5.1, ny * 2.2 - 3.4, 101, 4) - 0.5)
                * sprite.edge_noise
                * 1.35;
            let medium_edge_noise = (fbm2d(seed, nx * 5.4 - 2.0, ny * 5.4 + 8.7, 139, 3) - 0.5)
                * sprite.edge_noise
                * 0.35;
            let silhouette_radius = 0.74
                + lobe_0 * sprite.lobe_amplitude * style_silhouette
                + lobe_1 * (sprite.lobe_amplitude * 0.62)
                + lobe_2 * (sprite.lobe_amplitude * 0.42)
                + shard_lobe * sprite.lobe_amplitude * 0.18
                + broad_edge_noise
                + medium_edge_noise;

            let edge = silhouette_radius - dist;
            if edge <= -0.03 {
                continue;
            }

            let alpha = (edge / 0.03).clamp(0.0, 1.0);
            let grain = hash01(seed, qx as u64 * 131 + qy as u64 * 977);
            let broad_grain = fbm2d(seed, nx * 3.1 + 13.0, ny * 3.1 - 7.0, 211, 4);
            let local_grain = fbm2d(seed, nx * 9.0 - 17.0, ny * 9.0 + 4.0, 251, 3);
            let body_round = (1.0 - (dist / silhouette_radius.max(0.2)).powf(1.7)).clamp(0.0, 1.0);
            let mut shade =
                0.22 + body_round * 0.34 + (broad_grain - 0.5) * 0.24 + (grain - 0.5) * 0.05;
            let mut height_value =
                0.28 + body_round * 0.48 + (broad_grain - 0.5) * 0.22 + (local_grain - 0.5) * 0.08;
            let ridge_a = (angle * 6.0 + dist * 10.5 + hash01(seed, 71) * TAU)
                .sin()
                .abs();
            let ridge_b = (nx * 12.0 - ny * 9.0 + hash01(seed, 73) * TAU).sin().abs();
            let facet_ridge = (1.0 - ridge_a).powf(5.0) * 0.55 + (1.0 - ridge_b).powf(7.0) * 0.35;
            let facet_shadow = ridge_a.min(ridge_b).powf(3.0) * 0.10;
            shade += facet_ridge * 0.10;
            shade -= facet_shadow * 0.65;
            height_value += facet_ridge * 0.24;
            height_value -= facet_shadow * 0.26;

            for crater in crater_centers.iter().take(crater_count) {
                let dx = nx - crater.0;
                let dy = ny - crater.1;
                let crater_dist = (dx * dx + dy * dy).sqrt();
                let crater_radius = crater.2 * style_crater_scale;
                if crater_dist < crater_radius {
                    let t = 1.0 - (crater_dist / crater_radius).clamp(0.0, 1.0);
                    shade -= t * 0.18;
                    height_value -= t * 0.28;
                }
            }

            let crack_wave = ((angle * 7.0 + dist * 11.0 + hash01(seed, 41) * TAU).sin()).abs();
            let crack = (1.0 - crack_wave).max(0.0).powf(8.0)
                * sprite.crack_intensity.clamp(0.0, 1.0)
                * (0.4 + dist * 0.8);
            shade -= crack * 0.18;
            height_value -= crack * 0.22;

            let vein_wave = ((nx * 13.0 + ny * 7.0 + hash01(seed, 53) * TAU).sin()).abs();
            let vein = (1.0 - vein_wave).max(0.0).powf(10.0)
                * sprite.mineral_vein_intensity.clamp(0.0, 1.0)
                * (0.35 + grain * 0.65);

            shade = (shade * 8.0).round() / 8.0;
            shade = shade.clamp(0.06, 0.88);
            height_value = height_value.clamp(0.0, 1.0);

            let dark = sprite.palette_dark_rgb;
            let light = sprite.palette_light_rgb;
            let mut color = [
                dark[0] + (light[0] - dark[0]) * shade,
                dark[1] + (light[1] - dark[1]) * shade,
                dark[2] + (light[2] - dark[2]) * shade,
            ];
            let mineral = sprite.mineral_accent_rgb;
            let vein_mix = vein * 0.28;
            color[0] = color[0] + (mineral[0] - color[0]) * vein_mix;
            color[1] = color[1] + (mineral[1] - color[1]) * vein_mix;
            color[2] = color[2] + (mineral[2] - color[2]) * vein_mix;
            let dust = smoothstep(0.42, 0.78, local_grain) * 0.08;
            color[0] += dust;
            color[1] += dust * 0.85;
            color[2] += dust * 0.62;
            let r = (color[0].clamp(0.0, 1.0) * 255.0) as u8;
            let g = (color[1].clamp(0.0, 1.0) * 255.0) as u8;
            let b = (color[2].clamp(0.0, 1.0) * 255.0) as u8;
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
