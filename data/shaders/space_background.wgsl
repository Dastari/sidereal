#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct SpaceBackgroundParams {
    viewport_time: vec4<f32>,
    drift_intensity: vec4<f32>,
    velocity_dir: vec4<f32>,
    space_bg_params: vec4<f32>,   // .x intensity, .y drift scale, .z velocity glow, .w nebula strength
    space_bg_tint: vec4<f32>,     // .rgb color tint, .w deterministic seed
    space_bg_background: vec4<f32>,
    space_bg_flare: vec4<f32>,
    space_bg_noise_a: vec4<f32>,
    space_bg_noise_b: vec4<f32>,
    space_bg_star_mask_a: vec4<f32>,
    space_bg_star_mask_b: vec4<f32>,
    space_bg_star_mask_c: vec4<f32>,
    space_bg_blend_a: vec4<f32>,
    space_bg_blend_b: vec4<f32>,
    space_bg_nebula_color_a: vec4<f32>,
    space_bg_nebula_color_b: vec4<f32>,
    space_bg_nebula_color_c: vec4<f32>,
    space_bg_flare_tint: vec4<f32>,
}

@group(2) @binding(0) var<uniform> params: SpaceBackgroundParams;
@group(2) @binding(1) var flare_texture: texture_2d<f32>;
@group(2) @binding(2) var flare_sampler: sampler;

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

fn fbm2d(p: vec2<f32>, seed: f32) -> f32 {
    var value = 0.0;
    var amplitude = 0.58;
    var frequency = 1.0;
    for (var i = 0; i < 5; i = i + 1) {
        value += amplitude * noise2d(p * frequency, seed);
        frequency *= 2.0;
        amplitude *= 0.52;
    }
    return value;
}

fn fbm2d_config(p: vec2<f32>, seed: f32, octaves: u32, gain: f32, lacunarity: f32) -> f32 {
    var value = 0.0;
    var amplitude = 0.62;
    var frequency = 1.0;
    var max_value = 0.0001;
    for (var i = 0; i < 8; i = i + 1) {
        if (u32(i) >= octaves) {
            break;
        }
        value += amplitude * noise2d(p * frequency, seed);
        max_value += amplitude;
        frequency *= lacunarity;
        amplitude *= gain;
    }
    return clamp(value / max_value, 0.0, 1.0);
}

fn ridge(val: f32, offset: f32) -> f32 {
    let r = max(offset - abs(val * 2.0 - 1.0), 0.0);
    let normalized = r / max(offset, 0.0001);
    return pow(normalized, 1.35);
}

fn ridged_fbm2d(p: vec2<f32>, seed: f32) -> f32 {
    var value = 0.0;
    var amplitude = 0.62;
    var frequency = 1.0;
    var max_value = 0.0001;
    var prev = 1.0;
    for (var i = 0; i < 5; i = i + 1) {
        let n = ridge(noise2d(p * frequency, seed), 1.0);
        let layer = n * prev;
        value += layer * amplitude;
        max_value += amplitude;
        prev = clamp(layer * 1.75, 0.0, 1.0);
        frequency *= 2.0;
        amplitude *= 0.50;
    }
    return clamp(value / max_value, 0.0, 1.0);
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
    for (var i = 0; i < 8; i = i + 1) {
        if (u32(i) >= octaves) {
            break;
        }
        let n = ridge(noise2d(p * frequency, seed), ridge_offset);
        let layer = n * prev;
        value += layer * amplitude;
        max_value += amplitude;
        prev = clamp(layer * 1.75, 0.0, 1.0);
        frequency *= lacunarity;
        amplitude *= gain;
    }
    return clamp(value / max_value, 0.0, 1.0);
}

fn subtle_star_layer(
    uv: vec2<f32>,
    density: f32,
    threshold: f32,
    size: f32,
    twinkle_rate: f32,
    time: f32,
    seed: f32
) -> vec3<f32> {
    let grid_uv = uv * density;
    let id = floor(grid_uv);
    let local = fract(grid_uv) - 0.5;
    let n = hash21(id, seed);
    if n > threshold {
        return vec3<f32>(0.0);
    }

    let offset = vec2<f32>(
        hash21(id + vec2<f32>(7.3, 2.9), seed),
        hash21(id + vec2<f32>(1.1, 8.7), seed)
    ) - 0.5;
    let d = length(local - offset * 0.68);
    let core = smoothstep(size, size * 0.16, d);
    let halo = smoothstep(size * 3.0, 0.0, d) * 0.17;
    let twinkle = 0.94 + 0.06 * sin(time * twinkle_rate + n * 29.0);
    let tint = mix(
        vec3<f32>(0.75, 0.84, 1.0),
        vec3<f32>(0.90, 0.95, 1.0),
        fract(n * 19.0)
    );
    return (core + halo) * twinkle * tint;
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
    if (n > spawn_prob) {
        return vec3<f32>(0.0);
    }

    let jitter = vec2<f32>(
        hash21(id + vec2<f32>(2.3, 1.7), seed + 107.0),
        hash21(id + vec2<f32>(4.9, 8.1), seed + 107.0)
    ) - 0.5;
    let flare_local = local - jitter * 0.64;
    let flare_radius = 0.11 * size;
    let d = length(flare_local);
    if (d > flare_radius * 2.0) {
        return vec3<f32>(0.0);
    }

    let flare_uv = clamp((flare_local / (flare_radius * 2.0)) + vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(1.0));
    let sample_col = textureSample(flare_texture, flare_sampler, flare_uv).rgb;
    let radial = smoothstep(flare_radius * 1.4, 0.0, d);
    let color_jitter = mix(
        vec3<f32>(0.78, 0.87, 1.00),
        vec3<f32>(1.00, 0.93, 0.86),
        fract(n * 37.0)
    );
    return sample_col * radial * color_jitter;
}

fn apply_layer_blend(base: vec3<f32>, layer: vec3<f32>, mode: f32, opacity: f32) -> vec3<f32> {
    let op = clamp(opacity, 0.0, 1.0);
    let add_col = base + layer * op;
    let screen_col = 1.0 - (1.0 - base) * (1.0 - layer * op);
    let lighten_col = max(base, base + layer * op);
    let is_screen = mode >= 0.5 && mode < 1.5;
    let is_lighten = mode >= 1.5;
    return select(select(add_col, screen_col, is_screen), lighten_col, is_lighten);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let res = max(params.viewport_time.xy, vec2<f32>(1.0, 1.0));
    let time = params.viewport_time.z;
    let warp_factor = max(params.viewport_time.w, 0.0); // retained for subtle velocity glow only
    let intensity = max(params.space_bg_params.x, 0.0001);
    let drift_scale = max(params.space_bg_params.y, 0.0);
    let velocity_glow = max(params.space_bg_params.z, 0.0);
    let nebula_strength = max(params.space_bg_params.w, 0.0);
    let seed = max(params.space_bg_tint.w, 0.0);
    let base_background = max(params.space_bg_background.rgb, vec3<f32>(0.0));
    let flare_enabled = params.space_bg_flare.x >= 0.5;
    let flare_intensity = max(params.space_bg_flare.y, 0.0);
    let flare_density = clamp(params.space_bg_flare.z, 0.0, 1.0);
    let flare_size = max(params.space_bg_flare.w, 0.1);
    let ridged_mode = params.space_bg_noise_a.x >= 0.5;
    let nebula_octaves = u32(clamp(params.space_bg_noise_a.y, 1.0, 8.0));
    let nebula_gain = clamp(params.space_bg_noise_a.z, 0.1, 0.95);
    let nebula_lacunarity = clamp(params.space_bg_noise_a.w, 1.1, 4.0);
    let nebula_power = clamp(params.space_bg_noise_b.x, 0.2, 4.0);
    let nebula_shelf = clamp(params.space_bg_noise_b.y, 0.0, 0.95);
    let nebula_ridge_offset = clamp(params.space_bg_noise_b.z, 0.5, 2.5);
    let star_mask_enabled = params.space_bg_star_mask_a.x >= 0.5;
    let star_mask_ridged = params.space_bg_star_mask_a.y >= 0.5;
    let star_mask_octaves = u32(clamp(params.space_bg_star_mask_a.z, 1.0, 8.0));
    let star_mask_scale = clamp(params.space_bg_star_mask_a.w, 0.2, 8.0);
    let star_mask_threshold = clamp(params.space_bg_star_mask_b.x, 0.0, 0.99);
    let star_mask_power = clamp(params.space_bg_star_mask_b.y, 0.2, 4.0);
    let star_mask_gain = clamp(params.space_bg_star_mask_b.z, 0.1, 0.95);
    let star_mask_lacunarity = clamp(params.space_bg_star_mask_b.w, 1.1, 4.0);
    let star_mask_ridge_offset = clamp(params.space_bg_star_mask_c.x, 0.5, 2.5);
    let nebula_blend_mode = clamp(params.space_bg_blend_a.x, 0.0, 2.0);
    let nebula_opacity = clamp(params.space_bg_blend_a.y, 0.0, 1.0);
    let stars_blend_mode = clamp(params.space_bg_blend_a.z, 0.0, 2.0);
    let stars_opacity = clamp(params.space_bg_blend_a.w, 0.0, 1.0);
    let flares_blend_mode = clamp(params.space_bg_blend_b.x, 0.0, 2.0);
    let flares_opacity = clamp(params.space_bg_blend_b.y, 0.0, 1.0);
    let nebula_color_a = max(params.space_bg_nebula_color_a.rgb, vec3<f32>(0.0));
    let nebula_color_b = max(params.space_bg_nebula_color_b.rgb, vec3<f32>(0.0));
    let nebula_color_c = max(params.space_bg_nebula_color_c.rgb, vec3<f32>(0.0));
    let flare_tint = max(params.space_bg_flare_tint.rgb, vec3<f32>(0.0));
    let aspect = res.x / res.y;

    let uv_n = in.uv * 2.0 - 1.0;
    let uv = vec2<f32>(uv_n.x * aspect, uv_n.y);

    let heading_raw = params.velocity_dir.xy;
    var heading = vec2<f32>(0.0, 1.0); // for drift direction only
    if (length(heading_raw) > 0.001) {
        heading = normalize(heading_raw);
    }
    // Intentionally tiny movement to keep the background almost static.
    let subtle_motion = (
        params.drift_intensity.xy * 0.0018 +
        heading * (0.0007 + warp_factor * 0.0008)
    ) * drift_scale;

    let background_grad = clamp((uv_n.y + 1.0) * 0.5, 0.0, 1.0);
    let deep_space = mix(
        base_background * 0.72,
        base_background * 1.18 + vec3<f32>(0.002, 0.003, 0.008),
        background_grad
    );
    let vignette = clamp(1.08 - length(uv_n) * 0.17, 0.80, 1.0);

    // Deterministic seeded nebula with almost no temporal movement.
    let base = uv * 1.35 + vec2<f32>(seed * 0.17, seed * 0.29);
    let nebula_field_a = select(
        fbm2d_config(
            base + subtle_motion * 0.60,
            seed,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity
        ),
        ridged_fbm2d_config(
            base + subtle_motion * 0.60,
            seed,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity,
            nebula_ridge_offset
        ),
        ridged_mode
    );
    let nebula_field_b = select(
        fbm2d_config(
            base * 1.9 + vec2<f32>(11.3, -7.1),
            seed + 19.0,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity
        ),
        ridged_fbm2d_config(
            base * 1.9 + vec2<f32>(11.3, -7.1),
            seed + 19.0,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity,
            nebula_ridge_offset
        ),
        ridged_mode
    );
    let nebula_field_c = select(
        fbm2d_config(
            base * 0.72 + vec2<f32>(-3.7, 5.4),
            seed + 47.0,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity
        ),
        ridged_fbm2d_config(
            base * 0.72 + vec2<f32>(-3.7, 5.4),
            seed + 47.0,
            nebula_octaves,
            nebula_gain,
            nebula_lacunarity,
            nebula_ridge_offset
        ),
        ridged_mode
    );

    let nebula_composite = clamp(
        nebula_field_a * 0.66 + nebula_field_b * 0.24 + nebula_field_c * 0.10,
        0.0,
        1.0
    );
    let shaped_nebula = pow(nebula_composite, nebula_power);
    let nebula_mask = smoothstep(
        nebula_shelf,
        min(nebula_shelf + 0.45, 0.99),
        shaped_nebula
    );

    var nebula = mix(nebula_color_a, nebula_color_b, nebula_field_b);
    nebula = mix(nebula, nebula_color_c, nebula_field_c * 0.55);

    // Soft cloud body + subtle emissive ridges.
    let cloud_body = nebula * nebula_mask * (0.58 * nebula_strength);
    let ridge = smoothstep(0.58, 0.91, nebula_field_a) * nebula_mask;
    let cloud_ridge = nebula * ridge * (0.22 * nebula_strength);
    let nebula_layer = (cloud_body + cloud_ridge) * intensity * vignette;

    // Very subtle stars with tiny drift only.
    let stars_far = subtle_star_layer(
        uv + subtle_motion * 0.22,
        34.0,
        0.090,
        0.090,
        0.40,
        time,
        seed + 3.0
    ) * 0.20;
    let stars_mid = subtle_star_layer(
        uv + subtle_motion * 0.35,
        22.0,
        0.070,
        0.104,
        0.55,
        time,
        seed + 11.0
    ) * 0.30;
    let stars_near = subtle_star_layer(
        uv + subtle_motion * 0.50,
        14.0,
        0.050,
        0.118,
        0.72,
        time,
        seed + 29.0
    ) * 0.40;

    let star_mask_base = uv * star_mask_scale + subtle_motion * 0.45 + vec2<f32>(seed * 0.07, seed * 0.11);
    let star_mask_noise = select(
        fbm2d_config(
            star_mask_base,
            seed + 201.0,
            star_mask_octaves,
            star_mask_gain,
            star_mask_lacunarity
        ),
        ridged_fbm2d_config(
            star_mask_base,
            seed + 201.0,
            star_mask_octaves,
            star_mask_gain,
            star_mask_lacunarity,
            star_mask_ridge_offset
        ),
        star_mask_ridged
    );
    let star_mask = select(
        1.0,
        smoothstep(
            star_mask_threshold,
            min(star_mask_threshold + 0.35, 0.99),
            pow(star_mask_noise, star_mask_power)
        ),
        star_mask_enabled
    );
    let stars_layer = (stars_far + stars_mid + stars_near) * star_mask * intensity * vignette;
    let flares_layer = subtle_flare_layer(
        uv,
        subtle_motion * 0.75,
        flare_density,
        flare_size,
        seed + 131.0
    ) * flare_tint * flare_intensity * select(0.0, 1.0, flare_enabled) * star_mask * intensity * vignette;

    let background_col = deep_space * vignette;
    var composed = background_col;
    composed = apply_layer_blend(composed, nebula_layer, nebula_blend_mode, nebula_opacity);
    composed = apply_layer_blend(composed, stars_layer, stars_blend_mode, stars_opacity);
    composed = apply_layer_blend(composed, flares_layer, flares_blend_mode, flares_opacity);
    composed += vec3<f32>(0.05, 0.08, 0.14) * clamp(warp_factor * velocity_glow * 0.045, 0.0, 0.06);
    composed = mix(composed, composed * params.space_bg_tint.rgb * 2.8, 0.20);

    var col = composed;
    col = min(col, vec3<f32>(1.0));

    return vec4<f32>(col, 1.0);
}
