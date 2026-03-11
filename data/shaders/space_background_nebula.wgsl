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

fn aspect_corrected_centered_uv(uv: vec2<f32>, viewport: vec2<f32>) -> vec2<f32> {
    let safe_viewport = max(viewport, vec2<f32>(1.0, 1.0));
    return (uv * safe_viewport - safe_viewport * 0.5) / max(safe_viewport.x, safe_viewport.y);
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

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let res = max(params.viewport_time.xy, vec2<f32>(1.0, 1.0));
    let warp_factor = max(params.viewport_time.w, 0.0);

    let intensity = max(params.space_bg_params.x, 0.0001);
    let drift_scale = max(params.space_bg_params.y, 0.0);
    let velocity_glow = max(params.space_bg_params.z, 0.0);
    let nebula_strength = max(params.space_bg_params.w, 0.0);

    let seed = max(params.space_bg_tint.w, 0.0);
    let enable_nebula_layer = params.space_bg_section_flags.x >= 0.5;

    let ridged_mode = params.space_bg_noise_a.x >= 0.5;
    let nebula_octaves = u32(clamp(params.space_bg_noise_a.y, 1.0, 4.0));
    let nebula_gain = clamp(params.space_bg_noise_a.z, 0.1, 0.95);
    let nebula_lacunarity = clamp(params.space_bg_noise_a.w, 1.1, 3.2);
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
    let enable_backlight = params.space_bg_light_flags.x >= 0.5 && enable_nebula_layer;
    let backlight_color = max(params.space_bg_backlight_color.rgb, vec3<f32>(0.0));

    let centered_uv = aspect_corrected_centered_uv(in.uv, res);
    let uv = centered_uv * 2.0;
    let screen_scale = res / max(res.x, res.y);

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

    let vignette = clamp(1.08 - dot(centered_uv, centered_uv) * 0.36, 0.80, 1.0);

    var lit_nebula_layer = vec3<f32>(0.0);

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

        let backlight_pos = backlight_screen * screen_scale;
        let light_vec = backlight_pos - uv;
        let light_dist2 = dot(light_vec, light_vec);

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

        let local_density = sat(neb.far_depth * 0.28 + neb.mid_depth * 0.46 + neb.near_depth * 0.72);
        let scattering_weight = sat(local_density * (0.90 + edge_strength * 0.60));
        lit_nebula_layer = nebula_layer + backlight_layer * scattering_weight;
    }

    var composed = lit_nebula_layer;
    composed += vec3<f32>(0.05, 0.08, 0.14) * clamp(warp_factor * velocity_glow * 0.045, 0.0, 0.06);
    let rgb = max(mix(composed, composed * params.space_bg_tint.rgb * 2.8, 0.20), vec3<f32>(0.0));
    let alpha = clamp(max(max(rgb.r, rgb.g), rgb.b) * max(nebula_opacity, 0.05), 0.0, 1.0);
    return vec4<f32>(min(rgb, vec3<f32>(1.0)), alpha);
}
