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

struct NebulaMainSample {
    field_a: f32,
    field_b: f32,
    field_c: f32,
    shaped: f32,
    mask: f32,
    near_depth: f32,
    mid_depth: f32,
    far_depth: f32,
    presence: f32,
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

fn hash21(p: vec2<f32>, seed: f32) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031 + seed * vec3<f32>(0.0973, 0.1099, 0.13787));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
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

fn ridge_fast(val: f32, offset: f32) -> f32 {
    let r = max(offset - abs(val * 2.0 - 1.0), 0.0) / max(offset, 0.0001);
    return r * r * (1.25 - 0.25 * r);
}

fn fbm2d_config(p: vec2<f32>, seed: f32, octaves: u32, gain: f32, lacunarity: f32) -> f32 {
    var value = 0.0;
    var amplitude = 0.62;
    var frequency = 1.0;
    var max_value = 0.0001;
    for (var i = 0u; i < 4u; i = i + 1u) {
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
    for (var i = 0u; i < 4u; i = i + 1u) {
        if i >= octaves {
            break;
        }
        let n = ridge_fast(noise2d(p * frequency, seed), ridge_offset);
        let layer = n * prev;
        value += layer * amplitude;
        max_value += amplitude;
        prev = sat(layer * 1.7);
        frequency *= lacunarity;
        amplitude *= gain;
    }
    return sat(value / max_value);
}

fn noise_field(
    p: vec2<f32>,
    seed: f32,
    ridged_mode: bool,
    octaves: u32,
    gain: f32,
    lacunarity: f32,
    ridge_offset: f32
) -> f32 {
    if ridged_mode {
        return ridged_fbm2d_config(p, seed, octaves, gain, lacunarity, ridge_offset);
    }
    return fbm2d_config(p, seed, octaves, gain, lacunarity);
}

fn nebula_sample_main(
    sample_uv: vec2<f32>,
    background_zoom: f32,
    seed: f32,
    subtle_motion: vec2<f32>,
    depth_parallax_scale: f32,
    ridged_mode: bool,
    octaves: u32,
    gain: f32,
    lacunarity: f32,
    nebula_power: f32,
    nebula_shelf: f32,
    nebula_ridge_offset: f32,
    depth_layer_separation: f32
) -> NebulaMainSample {
    let oct = min(octaves, 4u);
    let base = sample_uv * background_zoom * 1.32
        + vec2<f32>(seed * 0.17, seed * 0.29)
        + subtle_motion * (0.55 + depth_parallax_scale * 0.42);

    let field_a = noise_field(
        base + subtle_motion * 0.58,
        seed,
        ridged_mode,
        oct,
        gain,
        lacunarity,
        nebula_ridge_offset
    );
    let field_b = noise_field(
        base * 1.82 + vec2<f32>(11.3, -7.1),
        seed + 19.0,
        ridged_mode,
        oct,
        gain,
        lacunarity,
        nebula_ridge_offset
    );
    let field_c = noise_field(
        base * 0.74 + vec2<f32>(-3.7, 5.4),
        seed + 47.0,
        ridged_mode,
        max(1u, oct - 1u),
        gain,
        lacunarity,
        nebula_ridge_offset
    );

    let composite = sat(field_a * 0.68 + field_b * 0.24 + field_c * 0.08);
    let shaped = pow(composite, nebula_power);
    let mask = smoothstep(nebula_shelf, min(nebula_shelf + 0.42, 0.99), shaped);

    let near_depth = smoothstep(
        max(nebula_shelf - 0.10 * depth_layer_separation, 0.0),
        min(nebula_shelf + 0.28, 0.995),
        sat(shaped + field_b * 0.08)
    );
    let far_depth = smoothstep(
        min(nebula_shelf + 0.08 * depth_layer_separation, 0.99),
        min(nebula_shelf + 0.50, 0.999),
        sat(shaped * 0.84 + field_c * 0.20)
    );
    let presence = sat(mask * 0.70 + far_depth * 0.20 + near_depth * 0.10);

    return NebulaMainSample(
        field_a,
        field_b,
        field_c,
        shaped,
        mask,
        near_depth,
        mask,
        far_depth,
        presence
    );
}

fn nebula_mask_fast(
    sample_uv: vec2<f32>,
    background_zoom: f32,
    seed: f32,
    subtle_motion: vec2<f32>,
    depth_parallax_scale: f32,
    ridged_mode: bool,
    octaves: u32,
    gain: f32,
    lacunarity: f32,
    nebula_power: f32,
    nebula_shelf: f32,
    nebula_ridge_offset: f32
) -> f32 {
    let base = sample_uv * background_zoom * 1.28
        + vec2<f32>(seed * 0.17, seed * 0.29)
        + subtle_motion * (0.52 + depth_parallax_scale * 0.38);

    let oct = min(octaves, 3u);
    let a = noise_field(
        base + subtle_motion * 0.52,
        seed,
        ridged_mode,
        oct,
        gain,
        lacunarity,
        nebula_ridge_offset
    );
    let b = noise_field(
        base * 1.74 + vec2<f32>(11.3, -7.1),
        seed + 19.0,
        ridged_mode,
        oct,
        gain,
        lacunarity,
        nebula_ridge_offset
    );
    let composite = sat(a * 0.80 + b * 0.20);
    let shaped = pow(composite, nebula_power);
    return smoothstep(nebula_shelf, min(nebula_shelf + 0.38, 0.99), shaped);
}

fn subtle_star_layer(
    uv: vec2<f32>,
    density: f32,
    threshold: f32,
    size: f32,
    twinkle_rate: f32,
    time: f32,
    seed: f32,
    count_scale: f32,
    color_tint: vec3<f32>
) -> vec3<f32> {
    let grid_uv = uv * density;
    let id = floor(grid_uv);
    let local = fract(grid_uv) - 0.5;
    let n = hash21(id, seed);
    let spawn_threshold = clamp(threshold * count_scale, 0.0, 0.999);
    if n > spawn_threshold {
        return vec3<f32>(0.0);
    }

    let offset = vec2<f32>(
        hash21(id + vec2<f32>(7.3, 2.9), seed),
        hash21(id + vec2<f32>(1.1, 8.7), seed)
    ) - 0.5;
    let p = local - offset * 0.68;
    let d2 = dot(p, p);
    let r = max(size, 0.001);
    let core = 1.0 - smoothstep(r * r * 0.02, r * r, d2);
    let halo = 1.0 - smoothstep(0.0, (r * 2.8) * (r * 2.8), d2);
    let tw_phase = fract(time * twinkle_rate + n * 29.0);
    let twinkle = 0.94 + 0.06 * (1.0 - abs(tw_phase * 2.0 - 1.0));
    let tint = mix(
        vec3<f32>(0.75, 0.84, 1.0),
        vec3<f32>(0.90, 0.95, 1.0),
        fract(n * 19.0)
    ) * color_tint;
    return (core + halo * 0.14) * twinkle * tint;
}

fn subtle_flare_layer(
    uv: vec2<f32>,
    drift: vec2<f32>,
    density: f32,
    size: f32,
    seed: f32
) -> vec3<f32> {
    let cell_density = 9.0;
    let grid_uv = (uv + drift) * cell_density;
    let id = floor(grid_uv);
    let local = fract(grid_uv) - 0.5;
    let n = hash21(id, seed + 79.0);
    let spawn_prob = density * 0.055;
    if n > spawn_prob {
        return vec3<f32>(0.0);
    }

    let jitter = vec2<f32>(
        hash21(id + vec2<f32>(2.3, 1.7), seed + 107.0),
        hash21(id + vec2<f32>(4.9, 8.1), seed + 107.0)
    ) - 0.5;
    let flare_local = local - jitter * 0.64;
    let flare_radius = 0.11 * size;
    let d2 = dot(flare_local, flare_local);
    if d2 > (flare_radius * 2.0) * (flare_radius * 2.0) {
        return vec3<f32>(0.0);
    }

    let flare_uv = clamp(
        flare_local / max(flare_radius * 2.0, 0.0001) + vec2<f32>(0.5),
        vec2<f32>(0.0),
        vec2<f32>(1.0)
    );
    let sample_col = textureSample(flare_texture, flare_sampler, flare_uv).rgb;
    let radial = 1.0 - smoothstep(0.0, (flare_radius * 1.4) * (flare_radius * 1.4), d2);
    let color_jitter = mix(
        vec3<f32>(0.78, 0.87, 1.00),
        vec3<f32>(1.00, 0.93, 0.86),
        fract(n * 37.0)
    );
    return sample_col * radial * color_jitter;
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
        case 6u: {
            blended = b * l;
        }
        case 12u: {
            blended = select(2.0 * b * l, 1.0 - 2.0 * (1.0 - b) * (1.0 - l), b > vec3<f32>(0.5));
        }
        case 13u: {
            blended = (1.0 - 2.0 * l) * b * b + 2.0 * l * b;
        }
        case 19u: {
            blended = abs(b - l);
        }
        default: {
            blended = b + l;
        }
    }
    return mix(base, blended, op);
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
    let base_background = max(params.space_bg_background.rgb, vec3<f32>(0.0));

    let flare_enabled = params.space_bg_flare.x >= 0.5;
    let flare_intensity = max(params.space_bg_flare.y, 0.0);
    let flare_density = sat(params.space_bg_flare.z);
    let flare_size = max(params.space_bg_flare.w, 0.1);

    let blend_noise = hash21(floor(in.uv * res), seed + floor(time * 8.0) * 0.03125);

    let enable_nebula_layer = params.space_bg_section_flags.x >= 0.5;
    let enable_stars_layer = params.space_bg_section_flags.y >= 0.5;
    let enable_flares_layer = params.space_bg_section_flags.z >= 0.5;

    let ridged_mode = params.space_bg_noise_a.x >= 0.5;
    let nebula_octaves = u32(clamp(params.space_bg_noise_a.y, 1.0, 4.0));
    let nebula_gain = clamp(params.space_bg_noise_a.z, 0.1, 0.95);
    let nebula_lacunarity = clamp(params.space_bg_noise_a.w, 1.1, 3.2);
    let nebula_power = clamp(params.space_bg_noise_b.x, 0.2, 4.0);
    let nebula_shelf = clamp(params.space_bg_noise_b.y, 0.0, 0.95);
    let nebula_ridge_offset = clamp(params.space_bg_noise_b.z, 0.5, 2.5);

    let star_mask_enabled = params.space_bg_star_mask_a.x >= 0.5;
    let star_mask_ridged = params.space_bg_star_mask_a.y >= 0.5;
    let star_mask_octaves = u32(clamp(params.space_bg_star_mask_a.z, 1.0, 3.0));
    let star_mask_scale = clamp(params.space_bg_star_mask_a.w, 0.2, 8.0);
    let star_mask_threshold = clamp(params.space_bg_star_mask_b.x, 0.0, 0.99);
    let star_mask_power = clamp(params.space_bg_star_mask_b.y, 0.2, 4.0);
    let star_mask_gain = clamp(params.space_bg_star_mask_b.z, 0.1, 0.95);
    let star_mask_lacunarity = clamp(params.space_bg_star_mask_b.w, 1.1, 3.2);
    let star_mask_ridge_offset = clamp(params.space_bg_star_mask_c.x, 0.5, 2.5);
    let star_count = clamp(params.space_bg_star_mask_c.y, 0.0, 5.0);
    let star_size_a = clamp(params.space_bg_star_mask_c.z, 0.01, 0.35);
    let star_size_b = clamp(params.space_bg_star_mask_c.w, 0.01, 0.35);
    let star_size_min = min(star_size_a, star_size_b);
    let star_size_max = max(star_size_a, star_size_b);
    let star_color = max(params.space_bg_star_color.rgb, vec3<f32>(0.0));

    let nebula_blend_mode = clamp(params.space_bg_blend_a.x, 0.0, 26.0);
    let nebula_opacity = sat(params.space_bg_blend_a.y);
    let stars_blend_mode = clamp(params.space_bg_blend_a.z, 0.0, 26.0);
    let stars_opacity = sat(params.space_bg_blend_a.w);
    let flares_blend_mode = clamp(params.space_bg_blend_b.x, 0.0, 26.0);
    let flares_opacity = sat(params.space_bg_blend_b.y);
    let zoom_rate = max(params.space_bg_blend_b.z, 0.0);

    let nebula_color_a = max(params.space_bg_nebula_color_a.rgb, vec3<f32>(0.0));
    let nebula_color_b = max(params.space_bg_nebula_color_b.rgb, vec3<f32>(0.0));
    let nebula_color_c = max(params.space_bg_nebula_color_c.rgb, vec3<f32>(0.0));
    let flare_tint = max(params.space_bg_flare_tint.rgb, vec3<f32>(0.0));

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

    let enable_backlight = params.space_bg_light_flags.x >= 0.5 && enable_nebula_layer;
    let enable_shafts = params.space_bg_light_flags.y >= 0.5 && enable_nebula_layer;
    let shafts_debug_view = params.space_bg_light_flags.z >= 0.5;
    let shaft_blend_mode = clamp(params.space_bg_light_flags.w, 0.0, 26.0);

    let shaft_intensity = clamp(params.space_bg_shafts_a.x, 0.0, 40.0);
    let shaft_length = clamp(params.space_bg_shafts_a.y, 0.05, 0.95);
    let shaft_falloff = clamp(params.space_bg_shafts_a.z, 0.2, 6.0);
    let shaft_samples = u32(clamp(params.space_bg_shafts_a.w, 4.0, 16.0));

    var shaft_samples_cap = 8u;
    var shaft_octaves_cap = 2u;
    var shaft_jitter_scale = 1.0;
    if shaft_quality_mode < 0.5 {
        shaft_samples_cap = 6u;
        shaft_octaves_cap = 2u;
        shaft_jitter_scale = 1.15;
    } else if shaft_quality_mode > 1.5 {
        shaft_samples_cap = 12u;
        shaft_octaves_cap = 3u;
        shaft_jitter_scale = 0.90;
    }

    let shaft_samples_effective = min(shaft_samples, shaft_samples_cap);
    let shaft_octaves = min(nebula_octaves, shaft_octaves_cap);
    let shaft_color = max(params.space_bg_shafts_b.rgb, vec3<f32>(0.0));
    let shaft_opacity = sat(params.space_bg_shafts_b.w);
    let backlight_color = max(params.space_bg_backlight_color.rgb, vec3<f32>(0.0));

    let aspect = res.x / res.y;
    let uv_n = in.uv * 2.0 - 1.0;
    let uv = vec2<f32>(uv_n.x * aspect, uv_n.y);

    let render_zoom_scale = clamp(params.velocity_dir.z, 0.25, 4.0);
    let background_zoom = clamp(1.0 + (render_zoom_scale - 1.0) * zoom_rate * 0.12, 0.82, 1.18);
    let zoomed_uv = uv * background_zoom;

    let heading_raw = params.velocity_dir.xy;
    let heading_len2 = dot(heading_raw, heading_raw);
    var heading = vec2<f32>(0.0, 1.0);
    if heading_len2 > 0.000001 {
        heading = heading_raw * inverseSqrt(heading_len2);
    }

    let subtle_motion = (
        params.drift_intensity.xy * 0.0018 +
        heading * (0.0007 + warp_factor * 0.0008)
    ) * drift_scale;

    let background_grad = sat((uv_n.y + 1.0) * 0.5);
    let deep_space = mix(
        base_background * 0.72,
        base_background * 1.18 + vec3<f32>(0.002, 0.003, 0.008),
        background_grad
    );
    let vignette = clamp(1.08 - dot(uv_n, uv_n) * 0.09, 0.80, 1.0);

    var lit_nebula_layer = vec3<f32>(0.0);
    var star_mask = 1.0;

    if enable_nebula_layer {
        let neb = nebula_sample_main(
            zoomed_uv,
            background_zoom,
            seed,
            subtle_motion,
            depth_parallax_scale,
            ridged_mode,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity,
            nebula_power,
            nebula_shelf,
            nebula_ridge_offset,
            depth_layer_separation
        );

        var nebula = mix(nebula_color_a, nebula_color_b, neb.field_b);
        nebula = mix(nebula, nebula_color_c, neb.field_c * 0.50);

        let cloud_body = nebula * neb.mask * (0.60 * nebula_strength);
        let ridge_mask = smoothstep(0.60, 0.90, neb.field_a) * neb.mask;
        let cloud_ridge = nebula * ridge_mask * (0.18 * nebula_strength);
        let nebula_haze = nebula * neb.mask * (0.12 * depth_haze_strength);
        let nebula_layer = (cloud_body + cloud_ridge + nebula_haze) * intensity * vignette;

        let lit_presence = smoothstep(0.12, 0.90, neb.presence);

        let backlight_pos = vec2<f32>(backlight_screen.x * aspect, backlight_screen.y);
        let light_vec = backlight_pos - uv;
        let light_dist2 = dot(light_vec, light_vec);
        let light_inv_len = inverseSqrt(max(light_dist2, 0.000001));
        let light_dir = light_vec * light_inv_len;
        let light_perp = vec2<f32>(-light_dir.y, light_dir.x);

        var radial_light = sat(1.0 - light_dist2 * 0.30);
        radial_light = radial_light * radial_light * (3.0 - 2.0 * radial_light);

        let trans_far = sat(1.0 - neb.far_depth * depth_occlusion_strength * 0.55);
        let trans_mid = sat(1.0 - neb.mid_depth * depth_occlusion_strength * 0.80);
        let trans_near = sat(1.0 - neb.near_depth * depth_occlusion_strength);
        let transmission = trans_far * trans_mid * trans_near;
        let transmission_shaped = mix(transmission * transmission, transmission, sat(backlight_wrap * 0.5));

        let edge_strength = sat(fwidth(neb.shaped) * backlight_edge_boost * 2.0);
        let bloom = smoothstep(backlight_bloom_threshold, 1.0, neb.mid_depth) * backlight_bloom_scale * 0.30;

        let backlight_layer = backlight_color
            * backlight_intensity
            * radial_light
            * (transmission_shaped + edge_strength * 0.50 + bloom)
            * lit_presence
            * intensity
            * vignette
            * select(0.0, 1.0, enable_backlight);

        let source_mask_center = nebula_mask_fast(
            backlight_pos,
            background_zoom,
            seed,
            subtle_motion,
            depth_parallax_scale,
            ridged_mode,
            shaft_octaves,
            nebula_gain,
            nebula_lacunarity,
            nebula_power,
            nebula_shelf,
            nebula_ridge_offset
        );
        let source_mask_perp = nebula_mask_fast(
            backlight_pos + light_perp * 0.04,
            background_zoom,
            seed,
            subtle_motion,
            depth_parallax_scale,
            ridged_mode,
            shaft_octaves,
            nebula_gain,
            nebula_lacunarity,
            nebula_power,
            nebula_shelf,
            nebula_ridge_offset
        );
        let source_aperture = abs(source_mask_center - source_mask_perp);

        let pixel = in.uv * res;
        let shaft_jitter = hash21(pixel, seed + floor(time * 6.0) * 0.13);
        let ray_fan = smoothstep(0.0001, 0.0256, light_dist2) * (1.0 - smoothstep(1.21, 9.0, light_dist2));

        var shafts = 0.0;
        var shaft_bloom_drive = 0.0;

        if enable_shafts && shaft_intensity > 0.0 && shaft_opacity > 0.0 {
            var shaft_trans = 1.0;
            var shaft_accum = 0.0;
            var shaft_bloom_accum = 0.0;
            var prev_mask = source_mask_center;
            let sample_count_f = f32(shaft_samples_effective);

            for (var i = 0u; i < 12u; i = i + 1u) {
                if i >= shaft_samples_effective {
                    break;
                }
                let t0 = (f32(i) + shaft_jitter) / sample_count_f;
                let t = mix(t0, t0 * t0, 0.40);
                let march_uv = uv + light_vec * (t * shaft_length);
                let step_phase = fract(shaft_jitter + f32(i) * 0.75487766);
                let jitter = (step_phase - 0.5) * 2.0;
                let jitter_width = (0.0014 + (1.0 - t) * 0.0028) * shaft_jitter_scale / max(aspect, 0.001);
                let sample_uv = march_uv + light_perp * (jitter * jitter_width);

                let sample_mask = nebula_mask_fast(
                    sample_uv,
                    background_zoom,
                    seed,
                    subtle_motion,
                    depth_parallax_scale,
                    ridged_mode,
                    shaft_octaves,
                    nebula_gain,
                    nebula_lacunarity,
                    nebula_power,
                    nebula_shelf,
                    nebula_ridge_offset
                );

                let gap = 1.0 - smoothstep(0.20, 0.82, sample_mask);
                let aperture = abs(sample_mask - prev_mask);
                prev_mask = sample_mask;

                let fall = pow(1.0 - t, shaft_falloff);
                shaft_accum += (gap * 0.78 + aperture * 1.35) * shaft_trans * fall;
                shaft_bloom_accum += smoothstep(backlight_bloom_threshold, 1.0, sample_mask) * fall;
                shaft_trans *= 1.0 - sat(sample_mask * depth_occlusion_strength * 0.18);
            }

            shaft_bloom_drive = (shaft_bloom_accum / sample_count_f) * backlight_bloom_scale;
            shafts = (shaft_accum / sample_count_f)
                * ray_fan
                * radial_light
                * shaft_intensity
                * (0.45 + source_aperture * 0.55)
                * (0.40 + lit_presence * 0.60)
                * (0.60 + backlight_intensity * 0.25);
        }

        let shaft_visual = sat((shafts / (1.0 + shafts)) * (0.55 + backlight_bloom_scale * 0.65) + shaft_bloom_drive * 0.08);
        let local_density = sat(neb.far_depth * 0.28 + neb.mid_depth * 0.46 + neb.near_depth * 0.72);
        let scattering_weight = sat(local_density * (0.90 + edge_strength * 0.60));
        let shaft_layer = shaft_color
            * shaft_visual
            * (0.45 + lit_presence * 0.55)
            * vignette
            * select(0.0, 1.0, enable_shafts);

        let lit_nebula_base = nebula_layer + backlight_layer * scattering_weight;
        lit_nebula_layer = apply_layer_blend(
            lit_nebula_base,
            shaft_layer,
            shaft_blend_mode,
            shaft_opacity,
            blend_noise
        );

        if shafts_debug_view {
            return vec4<f32>(
                vec3<f32>(
                    sat(shaft_visual),
                    sat(shaft_bloom_drive * 0.7),
                    sat(neb.presence)
                ),
                1.0
            );
        }
    }

    var stars_layer = vec3<f32>(0.0);
    if enable_stars_layer {
        let stars_far = subtle_star_layer(
            zoomed_uv + subtle_motion * 0.22,
            34.0,
            0.090,
            star_size_min,
            0.40,
            time,
            seed + 3.0,
            star_count,
            star_color
        ) * 0.18;

        let stars_mid = subtle_star_layer(
            zoomed_uv + subtle_motion * 0.36,
            22.0,
            0.070,
            mix(star_size_min, star_size_max, 0.5),
            0.55,
            time,
            seed + 11.0,
            star_count,
            star_color
        ) * 0.30;

        let stars_near = subtle_star_layer(
            zoomed_uv + subtle_motion * 0.50,
            14.0,
            0.050,
            star_size_max,
            0.72,
            time,
            seed + 29.0,
            star_count,
            star_color
        ) * 0.42;

        if star_mask_enabled {
            let star_mask_base = zoomed_uv * star_mask_scale
                + subtle_motion * 0.45
                + vec2<f32>(seed * 0.07, seed * 0.11);

            let star_mask_noise = noise_field(
                star_mask_base,
                seed + 201.0,
                star_mask_ridged,
                star_mask_octaves,
                star_mask_gain,
                star_mask_lacunarity,
                star_mask_ridge_offset
            );

            star_mask = smoothstep(
                star_mask_threshold,
                min(star_mask_threshold + 0.30, 0.99),
                pow(star_mask_noise, star_mask_power)
            );
        }

        stars_layer = (stars_far + stars_mid + stars_near) * star_mask * intensity * vignette;
    }

    var flares_layer = vec3<f32>(0.0);
    if enable_flares_layer && flare_enabled && flare_intensity > 0.0 && flare_density > 0.0 {
        flares_layer = subtle_flare_layer(
            zoomed_uv,
            subtle_motion * 0.75,
            flare_density,
            flare_size,
            seed + 131.0
        ) * flare_tint * flare_intensity * star_mask * intensity * vignette;
    }

    let background_col = deep_space * vignette;
    var composed = background_col;
    composed = apply_layer_blend(composed, lit_nebula_layer, nebula_blend_mode, nebula_opacity, blend_noise);
    composed = apply_layer_blend(composed, stars_layer, stars_blend_mode, stars_opacity, blend_noise);
    composed = apply_layer_blend(composed, flares_layer, flares_blend_mode, flares_opacity, blend_noise);
    composed += vec3<f32>(0.05, 0.08, 0.14) * clamp(warp_factor * velocity_glow * 0.045, 0.0, 0.06);
    composed = mix(composed, composed * params.space_bg_tint.rgb * 2.8, 0.20);

    return vec4<f32>(min(composed, vec3<f32>(1.0)), 1.0);
}