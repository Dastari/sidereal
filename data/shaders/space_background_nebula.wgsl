#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct SpaceBackgroundParams {
    viewport_time: vec4<f32>,
    drift_intensity: vec4<f32>,
    velocity_dir: vec4<f32>,
    space_bg_params: vec4<f32>,
    space_bg_tint: vec4<f32>,
    space_bg_background: vec4<f32>,
    space_bg_flare: vec4<f32>,
    space_bg_noise_a: vec4<f32>,
    space_bg_noise_b: vec4<f32>,
    space_bg_star_mask_a: vec4<f32>,
    space_bg_star_mask_b: vec4<f32>,
    space_bg_star_mask_c: vec4<f32>,
    space_bg_blend_a: vec4<f32>,
    space_bg_blend_b: vec4<f32>,
    space_bg_section_flags: vec4<f32>,
    space_bg_nebula_color_a: vec4<f32>,
    space_bg_nebula_color_b: vec4<f32>,
    space_bg_nebula_color_c: vec4<f32>,
    space_bg_star_color: vec4<f32>,
    space_bg_flare_tint: vec4<f32>,
    space_bg_depth_a: vec4<f32>,
    space_bg_light_a: vec4<f32>,
    space_bg_light_b: vec4<f32>,
    space_bg_light_flags: vec4<f32>,
    space_bg_shafts_a: vec4<f32>,
    space_bg_shafts_b: vec4<f32>,
    space_bg_backlight_color: vec4<f32>,
}

@group(2) @binding(0) var<uniform> params: SpaceBackgroundParams;
@group(2) @binding(1) var flare_texture: texture_2d<f32>;
@group(2) @binding(2) var flare_sampler: sampler;

fn sat(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

fn sat3(v: vec3<f32>) -> vec3<f32> {
    return clamp(v, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn aspect_corrected_centered_uv(uv: vec2<f32>, viewport: vec2<f32>) -> vec2<f32> {
    let aspect = viewport.x / max(viewport.y, 1.0);
    return (uv - vec2<f32>(0.5)) * vec2<f32>(aspect, 1.0);
}

fn hash21(p: vec2<f32>, seed: f32) -> f32 {
    let q = p + vec2<f32>(seed * 0.173, seed * 0.347);
    return fract(sin(dot(q, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

fn noise2d(p: vec2<f32>, seed: f32) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i, seed);
    let b = hash21(i + vec2<f32>(1.0, 0.0), seed);
    let c = hash21(i + vec2<f32>(0.0, 1.0), seed);
    let d = hash21(i + vec2<f32>(1.0, 1.0), seed);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn ridge(val: f32, offset: f32) -> f32 {
    let r = max(offset - abs(val * 2.0 - 1.0), 0.0);
    let normalized = r / max(offset, 0.0001);
    return pow(normalized, 1.35);
}

fn fbm2d_config(p: vec2<f32>, seed: f32, octaves: u32, gain: f32, lacunarity: f32) -> f32 {
    var value = 0.0;
    var amplitude = 0.62;
    var frequency = 1.0;
    var max_value = 0.0001;
    for (var i = 0u; i < 8u; i = i + 1u) {
        if i >= octaves {
            break;
        }
        value += amplitude * noise2d(p * frequency, seed);
        max_value += amplitude;
        frequency *= lacunarity;
        amplitude *= gain;
    }
    return sat(value / max_value);
}

fn ridged_fbm2d_config(
    p: vec2<f32>,
    seed: f32,
    octaves: u32,
    gain: f32,
    lacunarity: f32,
    ridge_offset: f32
) -> f32 {
    var value = 0.0;
    var amplitude = 0.62;
    var frequency = 1.0;
    var max_value = 0.0001;
    var prev = 1.0;
    for (var i = 0u; i < 8u; i = i + 1u) {
        if i >= octaves {
            break;
        }
        let n = ridge(noise2d(p * frequency, seed), ridge_offset);
        let layer = n * prev;
        value += layer * amplitude;
        max_value += amplitude;
        prev = sat(layer * 1.75);
        frequency *= lacunarity;
        amplitude *= gain;
    }
    return sat(value / max_value);
}

fn blend_luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

fn rgb_to_hsl(c: vec3<f32>) -> vec3<f32> {
    let maxc = max(max(c.x, c.y), c.z);
    let minc = min(min(c.x, c.y), c.z);
    let l = (maxc + minc) * 0.5;
    let d = maxc - minc;
    if d < 0.00001 {
        return vec3<f32>(0.0, 0.0, l);
    }
    let s = d / (1.0 - abs(2.0 * l - 1.0));
    var h = 0.0;
    if maxc == c.x {
        h = ((c.y - c.z) / d + select(0.0, 6.0, c.y < c.z)) / 6.0;
    } else if maxc == c.y {
        h = ((c.z - c.x) / d + 2.0) / 6.0;
    } else {
        h = ((c.x - c.y) / d + 4.0) / 6.0;
    }
    return vec3<f32>(fract(h), sat(s), sat(l));
}

fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
    let h = fract(hsl.x);
    let s = sat(hsl.y);
    let l = sat(hsl.z);
    let c = (1.0 - abs(2.0 * l - 1.0)) * s;
    let hp = h * 6.0;
    let x = c * (1.0 - abs(fract(hp * 0.5) * 2.0 - 1.0));
    var rgb1 = vec3<f32>(0.0);
    if hp < 1.0 {
        rgb1 = vec3<f32>(c, x, 0.0);
    } else if hp < 2.0 {
        rgb1 = vec3<f32>(x, c, 0.0);
    } else if hp < 3.0 {
        rgb1 = vec3<f32>(0.0, c, x);
    } else if hp < 4.0 {
        rgb1 = vec3<f32>(0.0, x, c);
    } else if hp < 5.0 {
        rgb1 = vec3<f32>(x, 0.0, c);
    } else {
        rgb1 = vec3<f32>(c, 0.0, x);
    }
    let m = l - 0.5 * c;
    return rgb1 + vec3<f32>(m);
}

fn apply_layer_blend(base: vec3<f32>, layer: vec3<f32>, mode: f32, opacity: f32, noise: f32) -> vec3<f32> {
    let op = sat(opacity);
    let b = sat3(base);
    let l = sat3(layer);
    let m = u32(clamp(round(mode), 0.0, 26.0));

    var blended = b + l;
    switch m {
        case 0u: {
            blended = b + l;
        }
        case 1u: {
            blended = 1.0 - (1.0 - b) * (1.0 - l);
        }
        case 2u: {
            blended = max(b, l);
        }
        case 3u: {
            blended = l;
        }
        case 4u: {
            return mix(base, l, select(0.0, 1.0, noise < op));
        }
        case 5u: {
            blended = min(b, l);
        }
        case 6u: {
            blended = b * l;
        }
        case 7u: {
            blended = 1.0 - (1.0 - b) / max(l, vec3<f32>(0.0001));
        }
        case 8u: {
            blended = b + l - 1.0;
        }
        case 9u: {
            blended = select(l, b, blend_luma(b) <= blend_luma(l));
        }
        case 10u: {
            blended = b / max(1.0 - l, vec3<f32>(0.0001));
        }
        case 11u: {
            blended = select(l, b, blend_luma(b) >= blend_luma(l));
        }
        case 12u: {
            blended = select(2.0 * b * l, 1.0 - 2.0 * (1.0 - b) * (1.0 - l), b > vec3<f32>(0.5));
        }
        case 13u: {
            blended = (1.0 - 2.0 * l) * b * b + 2.0 * l * b;
        }
        case 14u: {
            blended = select(2.0 * b * l, 1.0 - 2.0 * (1.0 - b) * (1.0 - l), l > vec3<f32>(0.5));
        }
        case 15u: {
            let low = 1.0 - (1.0 - b) / max(2.0 * l, vec3<f32>(0.0001));
            let high = b / max(2.0 * (1.0 - l), vec3<f32>(0.0001));
            blended = select(low, high, l > vec3<f32>(0.5));
        }
        case 16u: {
            blended = b + 2.0 * l - 1.0;
        }
        case 17u: {
            let low = min(b, 2.0 * l);
            let high = max(b, 2.0 * (l - 0.5));
            blended = select(low, high, l > vec3<f32>(0.5));
        }
        case 18u: {
            let lin = b + 2.0 * l - 1.0;
            blended = select(vec3<f32>(0.0), vec3<f32>(1.0), lin > vec3<f32>(0.5));
        }
        case 19u: {
            blended = abs(b - l);
        }
        case 20u: {
            blended = b + l - 2.0 * b * l;
        }
        case 21u: {
            blended = b - l;
        }
        case 22u: {
            blended = b / max(l, vec3<f32>(0.0001));
        }
        case 23u: {
            let bhsl = rgb_to_hsl(b);
            let lhsl = rgb_to_hsl(l);
            blended = hsl_to_rgb(vec3<f32>(lhsl.x, bhsl.y, bhsl.z));
        }
        case 24u: {
            let bhsl = rgb_to_hsl(b);
            let lhsl = rgb_to_hsl(l);
            blended = hsl_to_rgb(vec3<f32>(bhsl.x, lhsl.y, bhsl.z));
        }
        case 25u: {
            let bhsl = rgb_to_hsl(b);
            let lhsl = rgb_to_hsl(l);
            blended = hsl_to_rgb(vec3<f32>(lhsl.x, lhsl.y, bhsl.z));
        }
        case 26u: {
            let bhsl = rgb_to_hsl(b);
            let lhsl = rgb_to_hsl(l);
            blended = hsl_to_rgb(vec3<f32>(bhsl.x, bhsl.y, lhsl.z));
        }
        default: {
            blended = b + l;
        }
    }
    return mix(base, blended, op);
}

fn nebula_mask_at(
    sample_uv: vec2<f32>,
    background_zoom: f32,
    seed: f32,
    subtle_motion: vec2<f32>,
    depth_parallax_scale: f32,
    ridged_mode: bool,
    nebula_octaves: u32,
    nebula_gain: f32,
    nebula_lacunarity: f32,
    nebula_power: f32,
    nebula_shelf: f32,
    nebula_ridge_offset: f32
) -> f32 {
    let base = sample_uv * background_zoom * 1.35
        + vec2<f32>(seed * 0.17, seed * 0.29)
        + subtle_motion * (0.55 + depth_parallax_scale * 0.45);
    var field_a = 0.0;
    var field_b = 0.0;
    var field_c = 0.0;
    if ridged_mode {
        field_a = ridged_fbm2d_config(
            base + subtle_motion * 0.60,
            seed,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity,
            nebula_ridge_offset
        );
        field_b = ridged_fbm2d_config(
            base * 1.9 + vec2<f32>(11.3, -7.1),
            seed + 19.0,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity,
            nebula_ridge_offset
        );
        field_c = ridged_fbm2d_config(
            base * 0.72 + vec2<f32>(-3.7, 5.4),
            seed + 47.0,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity,
            nebula_ridge_offset
        );
    } else {
        field_a = fbm2d_config(
            base + subtle_motion * 0.60,
            seed,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity
        );
        field_b = fbm2d_config(
            base * 1.9 + vec2<f32>(11.3, -7.1),
            seed + 19.0,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity
        );
        field_c = fbm2d_config(
            base * 0.72 + vec2<f32>(-3.7, 5.4),
            seed + 47.0,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity
        );
    }
    let composite = sat(field_a * 0.66 + field_b * 0.24 + field_c * 0.10);
    let shaped = pow(composite, nebula_power);
    return smoothstep(nebula_shelf, min(nebula_shelf + 0.45, 0.99), shaped);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let res = max(params.viewport_time.xy, vec2<f32>(1.0, 1.0));
    let time = params.viewport_time.z;
    let warp_factor = max(params.viewport_time.w, 0.0);
    let intensity = max(params.space_bg_params.x, 0.0001);
    let drift_scale = max(params.space_bg_params.y, 0.0);
    let velocity_glow = max(params.space_bg_params.z, 0.0);
    let nebula_strength = max(params.space_bg_params.w, 0.0);
    let seed = max(params.space_bg_tint.w, 0.0);
    let blend_noise = hash21(floor(in.uv * res), seed + time * 0.23);
    let enable_nebula_layer = params.space_bg_section_flags.x >= 0.5;
    if !enable_nebula_layer {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let ridged_mode = params.space_bg_noise_a.x >= 0.5;
    let nebula_octaves = u32(clamp(params.space_bg_noise_a.y, 1.0, 8.0));
    let nebula_gain = clamp(params.space_bg_noise_a.z, 0.1, 0.95);
    let nebula_lacunarity = clamp(params.space_bg_noise_a.w, 1.1, 4.0);
    let nebula_power = clamp(params.space_bg_noise_b.x, 0.2, 4.0);
    let nebula_shelf = clamp(params.space_bg_noise_b.y, 0.0, 0.95);
    let nebula_ridge_offset = clamp(params.space_bg_noise_b.z, 0.5, 2.5);
    let nebula_opacity = sat(params.space_bg_blend_a.y);
    let zoom_rate = max(params.space_bg_blend_b.z, 0.0);
    let nebula_color_a = max(params.space_bg_nebula_color_a.rgb, vec3<f32>(0.0));
    let nebula_color_b = max(params.space_bg_nebula_color_b.rgb, vec3<f32>(0.0));
    let nebula_color_c = max(params.space_bg_nebula_color_c.rgb, vec3<f32>(0.0));
    let depth_layer_separation = clamp(params.space_bg_depth_a.x, 0.0, 2.0);
    let depth_parallax_scale = clamp(params.space_bg_depth_a.y, 0.0, 2.0);
    let depth_haze_strength = clamp(params.space_bg_depth_a.z, 0.0, 2.0);
    let depth_occlusion_strength = clamp(params.space_bg_depth_a.w, 0.0, 3.0);
    let backlight_screen = params.space_bg_light_a.xy;
    let backlight_intensity = clamp(params.space_bg_light_a.z, 0.0, 20.0);
    let backlight_wrap = clamp(params.space_bg_light_a.w, 0.0, 2.0);
    let backlight_edge_boost = clamp(params.space_bg_light_b.x, 0.0, 6.0);
    let backlight_bloom_scale = clamp(params.space_bg_light_b.y, 0.0, 2.0);
    let backlight_bloom_threshold = clamp(params.space_bg_light_b.z, 0.0, 1.0);
    let shaft_quality_mode = clamp(params.space_bg_light_b.w, 0.0, 2.0);
    let enable_backlight = params.space_bg_light_flags.x >= 0.5;
    let enable_shafts = params.space_bg_light_flags.y >= 0.5;
    let shafts_debug_view = params.space_bg_light_flags.z >= 0.5;
    let shaft_blend_mode = clamp(params.space_bg_light_flags.w, 0.0, 26.0);
    let shaft_intensity = clamp(params.space_bg_shafts_a.x, 0.0, 40.0);
    let shaft_length = clamp(params.space_bg_shafts_a.y, 0.05, 0.95);
    let shaft_falloff = clamp(params.space_bg_shafts_a.z, 0.2, 8.0);
    let shaft_samples = u32(clamp(params.space_bg_shafts_a.w, 4.0, 24.0));
    let shaft_color = max(params.space_bg_shafts_b.rgb, vec3<f32>(0.0));
    let shaft_opacity = sat(params.space_bg_shafts_b.w);
    let backlight_color = max(params.space_bg_backlight_color.rgb, vec3<f32>(0.0));
    let tint = max(params.space_bg_tint.rgb, vec3<f32>(0.0));

    var shaft_samples_cap = 12u;
    var shaft_octaves_cap = 4u;
    var shaft_jitter_scale = 1.0;
    if shaft_quality_mode < 0.5 {
        shaft_samples_cap = 8u;
        shaft_octaves_cap = 3u;
        shaft_jitter_scale = 1.15;
    } else if shaft_quality_mode > 1.5 {
        shaft_samples_cap = 16u;
        shaft_octaves_cap = 5u;
        shaft_jitter_scale = 0.85;
    }
    let nebula_octaves_shafts = min(nebula_octaves, shaft_octaves_cap);
    let shaft_samples_effective = min(shaft_samples, shaft_samples_cap);

    let aspect = res.x / res.y;
    let centered_uv = aspect_corrected_centered_uv(in.uv, res);
    let uv_n = in.uv * 2.0 - 1.0;
    let uv = centered_uv * 2.0;
    let render_zoom_scale = clamp(params.velocity_dir.z, 0.25, 4.0);
    let camera_zoom = 1.0 / render_zoom_scale;
    let inverted_zoom = 1.0 / max(camera_zoom, 0.25);
    let background_zoom = clamp(1.0 + (inverted_zoom - 1.0) * zoom_rate * 0.12, 0.82, 1.18);
    let zoomed_uv = uv * background_zoom;

    let heading_raw = params.velocity_dir.xy;
    var heading = vec2<f32>(0.0, 1.0);
    if length(heading_raw) > 0.001 {
        heading = normalize(heading_raw);
    }
    let subtle_motion = (
        params.drift_intensity.xy * 0.0018 +
        heading * (0.0007 + warp_factor * 0.0008)
    ) * drift_scale;
    let vignette = clamp(1.08 - dot(centered_uv, centered_uv) * 0.36, 0.80, 1.0);

    let base = zoomed_uv * 1.35
        + vec2<f32>(seed * 0.17, seed * 0.29)
        + subtle_motion * (0.55 + depth_parallax_scale * 0.45);
    var nebula_field_a = 0.0;
    var nebula_field_b = 0.0;
    var nebula_field_c = 0.0;
    if ridged_mode {
        nebula_field_a = ridged_fbm2d_config(
            base + subtle_motion * 0.60,
            seed,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity,
            nebula_ridge_offset
        );
        nebula_field_b = ridged_fbm2d_config(
            base * 1.9 + vec2<f32>(11.3, -7.1),
            seed + 19.0,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity,
            nebula_ridge_offset
        );
        nebula_field_c = ridged_fbm2d_config(
            base * 0.72 + vec2<f32>(-3.7, 5.4),
            seed + 47.0,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity,
            nebula_ridge_offset
        );
    } else {
        nebula_field_a = fbm2d_config(
            base + subtle_motion * 0.60,
            seed,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity
        );
        nebula_field_b = fbm2d_config(
            base * 1.9 + vec2<f32>(11.3, -7.1),
            seed + 19.0,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity
        );
        nebula_field_c = fbm2d_config(
            base * 0.72 + vec2<f32>(-3.7, 5.4),
            seed + 47.0,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity
        );
    }

    let nebula_composite = sat(nebula_field_a * 0.66 + nebula_field_b * 0.24 + nebula_field_c * 0.10);
    let shaped_nebula = pow(nebula_composite, nebula_power);
    let nebula_mask = smoothstep(nebula_shelf, min(nebula_shelf + 0.45, 0.99), shaped_nebula);

    var nebula = mix(nebula_color_a, nebula_color_b, nebula_field_b);
    nebula = mix(nebula, nebula_color_c, nebula_field_c * 0.55);

    let cloud_body = nebula * nebula_mask * (0.58 * nebula_strength);
    let ridge_mask = smoothstep(0.58, 0.91, nebula_field_a) * nebula_mask;
    let cloud_ridge = nebula * ridge_mask * (0.22 * nebula_strength);
    let nebula_haze = nebula * nebula_mask * (0.14 * depth_haze_strength);
    let nebula_layer = (cloud_body + cloud_ridge + nebula_haze) * intensity * vignette;

    let near_depth = smoothstep(
        max(nebula_shelf - 0.12 * depth_layer_separation, 0.0),
        min(nebula_shelf + 0.30, 0.995),
        sat(shaped_nebula + nebula_field_b * 0.10)
    );
    let mid_depth = nebula_mask;
    let far_depth = smoothstep(
        min(nebula_shelf + 0.07 * depth_layer_separation, 0.99),
        min(nebula_shelf + 0.52, 0.999),
        sat(shaped_nebula * 0.82 + nebula_field_c * 0.22)
    );
    let nebula_presence = sat(mid_depth * 0.68 + far_depth * 0.22 + near_depth * 0.10);
    let lit_presence = smoothstep(0.14, 0.92, nebula_presence);

    let backlight_pos = vec2<f32>(backlight_screen.x * aspect, backlight_screen.y);
    let dist_to_light = length(uv - backlight_pos);
    let radial_light = pow(sat(1.0 - dist_to_light * 0.55), 1.85);
    let trans_far = sat(1.0 - far_depth * depth_occlusion_strength * 0.55);
    let trans_mid = sat(1.0 - mid_depth * depth_occlusion_strength * 0.82);
    let trans_near = sat(1.0 - near_depth * depth_occlusion_strength);
    let transmission = pow(trans_far * trans_mid * trans_near, max(0.2, 1.6 - backlight_wrap));
    let edge_strength = sat(length(vec2<f32>(dpdx(shaped_nebula), dpdy(shaped_nebula))) * backlight_edge_boost);
    let bloom = smoothstep(backlight_bloom_threshold, 1.0, mid_depth) * backlight_bloom_scale * 0.35 * lit_presence;

    let light_vec = backlight_pos - uv;
    let light_vec_len = max(length(light_vec), 0.0001);
    let light_dir = light_vec / light_vec_len;
    let light_perp = vec2<f32>(-light_dir.y, light_dir.x);
    let pixel = in.uv * res;
    let shaft_jitter = fract(
        52.9829189 * fract(dot(pixel + vec2<f32>(seed * 3.1, seed * 1.7), vec2<f32>(0.06711056, 0.00583715)))
    );
    let ray_fan = smoothstep(0.01, 0.16, dist_to_light) * (1.0 - smoothstep(1.1, 3.0, dist_to_light));

    var shafts = 0.0;
    var shaft_bloom_drive = 0.0;
    if enable_shafts && shaft_intensity > 0.0 && shaft_opacity > 0.0 {
        let source_mask_center = nebula_mask_at(
            backlight_pos,
            background_zoom,
            seed,
            subtle_motion,
            depth_parallax_scale,
            ridged_mode,
            nebula_octaves_shafts,
            nebula_gain,
            nebula_lacunarity,
            nebula_power,
            nebula_shelf,
            nebula_ridge_offset
        );
        let source_mask_x = nebula_mask_at(
            backlight_pos + vec2<f32>(0.055, 0.0),
            background_zoom,
            seed,
            subtle_motion,
            depth_parallax_scale,
            ridged_mode,
            nebula_octaves_shafts,
            nebula_gain,
            nebula_lacunarity,
            nebula_power,
            nebula_shelf,
            nebula_ridge_offset
        );
        let source_mask_y = nebula_mask_at(
            backlight_pos + vec2<f32>(0.0, 0.055),
            background_zoom,
            seed,
            subtle_motion,
            depth_parallax_scale,
            ridged_mode,
            nebula_octaves_shafts,
            nebula_gain,
            nebula_lacunarity,
            nebula_power,
            nebula_shelf,
            nebula_ridge_offset
        );
        let source_aperture = clamp(
            abs(source_mask_center - source_mask_x) + abs(source_mask_center - source_mask_y),
            0.0,
            1.5
        );
        var shaft_trans = 1.0;
        var shaft_aperture_accum = 0.0;
        var shaft_bloom_accum = 0.0;
        var prev_mask = source_mask_center;
        let shaft_span = clamp(shaft_length, 0.05, 0.95);
        let sample_count_f = f32(shaft_samples_effective);
        for (var i = 0u; i < 16u; i = i + 1u) {
            if i >= shaft_samples_effective {
                break;
            }
            let t_linear = (f32(i) + shaft_jitter) / sample_count_f;
            let t = mix(t_linear, t_linear * t_linear, 0.35);
            let march_uv = uv + light_dir * light_vec_len * t * shaft_span;
            let jitter_width = (0.0013 + (1.0 - t) * 0.0031) * shaft_jitter_scale / max(aspect, 0.001);
            let step_phase = fract(shaft_jitter + f32(i) * 0.75487766);
            let jitter = (step_phase - 0.5) * 2.0;
            let sample_uv = march_uv + light_perp * (jitter * jitter_width);
            let sample_mask = nebula_mask_at(
                sample_uv,
                background_zoom,
                seed,
                subtle_motion,
                depth_parallax_scale,
                ridged_mode,
                nebula_octaves_shafts,
                nebula_gain,
                nebula_lacunarity,
                nebula_power,
                nebula_shelf,
                nebula_ridge_offset
            );
            let cloud_block = smoothstep(0.22, 0.88, sample_mask);
            let gap = 1.0 - cloud_block;
            let occ = clamp(cloud_block * depth_occlusion_strength * 1.15, 0.0, 4.2);
            let aperture = clamp(abs(sample_mask - prev_mask) * 3.6 + gap * 0.95, 0.0, 2.4);
            prev_mask = sample_mask;
            let sample_bloom = smoothstep(backlight_bloom_threshold, 1.0, sample_mask);
            let falloff = pow(1.0 - t, shaft_falloff);
            shaft_trans *= exp(-occ * 0.24);
            shaft_aperture_accum += aperture * shaft_trans * falloff * (0.35 + gap * 0.65);
            shaft_bloom_accum += sample_bloom * falloff * (0.45 + gap * 0.55);
        }
        let source_lit = smoothstep(backlight_bloom_threshold, 1.0, source_mask_center);
        let shaft_visibility = clamp(
            (0.35 + source_lit * 0.65) * (0.45 + source_aperture * 0.55) * (0.45 + edge_strength * 0.55),
            0.0,
            2.0
        );
        shaft_bloom_drive = clamp((shaft_bloom_accum / sample_count_f) * backlight_bloom_scale, 0.0, 4.0);
        shafts = (shaft_aperture_accum / sample_count_f)
            * ray_fan
            * radial_light
            * shaft_intensity
            * shaft_bloom_drive
            * (0.55 + backlight_intensity * 0.32)
            * shaft_visibility
            * (0.35 + lit_presence * 0.65);
    }

    let quality_t = shaft_quality_mode * 0.5;
    let shaft_visual = clamp(
        pow(
            sat(1.0 - exp(-shafts * mix(1.35, 1.85, quality_t))),
            mix(1.35, 0.78, quality_t)
        ) * (0.55 + backlight_bloom_scale * 0.70),
        0.0,
        2.6
    );

    let backlight_layer = backlight_color
        * backlight_intensity
        * radial_light
        * (transmission + edge_strength * 0.65 + bloom)
        * lit_presence
        * intensity
        * vignette
        * select(0.0, 1.0, enable_backlight);
    let local_density = sat(far_depth * 0.28 + mid_depth * 0.46 + near_depth * 0.72);
    let scattering_weight = (1.0 - exp(-local_density * 3.4)) * (0.55 + edge_strength * 0.45);
    let shaft_layer = shaft_color
        * shaft_visual
        * (0.45 + lit_presence * 0.55)
        * vignette
        * select(0.0, 1.0, enable_shafts);
    let lit_nebula_base = nebula_layer + backlight_layer * scattering_weight;
    var lit_nebula_layer = apply_layer_blend(
        lit_nebula_base,
        shaft_layer,
        shaft_blend_mode,
        shaft_opacity,
        blend_noise
    );
    lit_nebula_layer = lit_nebula_layer * mix(vec3<f32>(1.0), tint * 2.0, 0.20);
    lit_nebula_layer += vec3<f32>(0.05, 0.08, 0.14) * clamp(warp_factor * velocity_glow * 0.045, 0.0, 0.06);

    if shafts_debug_view {
        let debug_shafts = sat(shaft_visual);
        let debug_bloom = sat(shaft_bloom_drive * 0.6);
        let debug_density = sat(nebula_presence);
        return vec4<f32>(vec3<f32>(debug_shafts, debug_bloom, debug_density), 1.0);
    }

    let alpha = sat(max(nebula_presence * nebula_opacity, shaft_visual * shaft_opacity * 0.8));
    return vec4<f32>(lit_nebula_layer, alpha);
}
