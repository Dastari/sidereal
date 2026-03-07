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
    space_bg_blend_b: vec4<f32>,  // .x flares blend mode, .y flares opacity, .z zoom rate
    space_bg_section_flags: vec4<f32>, // .x enable nebula, .y enable stars, .z enable flares
    space_bg_nebula_color_a: vec4<f32>,
    space_bg_nebula_color_b: vec4<f32>,
    space_bg_nebula_color_c: vec4<f32>,
    space_bg_star_color: vec4<f32>,
    space_bg_flare_tint: vec4<f32>,
    space_bg_depth_a: vec4<f32>,      // .x layer separation, .y parallax scale, .z haze strength, .w occlusion strength
    space_bg_light_a: vec4<f32>,      // .x backlight screen x, .y backlight screen y, .z backlight intensity, .w wrap
    space_bg_light_b: vec4<f32>,      // .x edge boost, .y bloom scale, .z bloom threshold, .w shaft quality
    space_bg_light_flags: vec4<f32>,  // .x enable backlight, .y enable shafts, .z shafts debug view, .w shaft blend mode
    space_bg_shafts_a: vec4<f32>,     // .x shaft intensity, .y shaft length, .z shaft falloff, .w shaft samples
    space_bg_shafts_b: vec4<f32>,     // .rgb shaft color, .w shaft opacity
    space_bg_backlight_color: vec4<f32>,
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
    let d = length(local - offset * 0.68);
    let core = smoothstep(size, size * 0.16, d);
    let halo = smoothstep(size * 3.0, 0.0, d) * 0.17;
    let twinkle = 0.94 + 0.06 * sin(time * twinkle_rate + n * 29.0);
    let tint = mix(
        vec3<f32>(0.75, 0.84, 1.0),
        vec3<f32>(0.90, 0.95, 1.0),
        fract(n * 19.0)
    ) * color_tint;
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
    let composite = clamp(field_a * 0.66 + field_b * 0.24 + field_c * 0.10, 0.0, 1.0);
    let shaped = pow(composite, nebula_power);
    return smoothstep(nebula_shelf, min(nebula_shelf + 0.45, 0.99), shaped);
}

fn nebula_mask_at_fast(
    sample_uv: vec2<f32>,
    background_zoom: f32,
    seed: f32,
    subtle_motion: vec2<f32>,
    depth_parallax_scale: f32,
    ridged_mode: bool,
    nebula_power: f32,
    nebula_shelf: f32,
    nebula_ridge_offset: f32
) -> f32 {
    let base = sample_uv * background_zoom * 1.35
        + vec2<f32>(seed * 0.17, seed * 0.29)
        + subtle_motion * (0.55 + depth_parallax_scale * 0.45);
    var field_a = 0.0;
    var field_b = 0.0;
    if ridged_mode {
        field_a = ridged_fbm2d_config(
            base + subtle_motion * 0.60,
            seed,
            2u,
            0.56,
            2.0,
            nebula_ridge_offset
        );
        field_b = ridged_fbm2d_config(
            base * 1.9 + vec2<f32>(11.3, -7.1),
            seed + 19.0,
            2u,
            0.56,
            2.0,
            nebula_ridge_offset
        );
    } else {
        field_a = fbm2d_config(
            base + subtle_motion * 0.60,
            seed,
            2u,
            0.56,
            2.0
        );
        field_b = fbm2d_config(
            base * 1.9 + vec2<f32>(11.3, -7.1),
            seed + 19.0,
            2u,
            0.56,
            2.0
        );
    }
    let composite = clamp(field_a * 0.78 + field_b * 0.22, 0.0, 1.0);
    let shaped = pow(composite, nebula_power);
    return smoothstep(nebula_shelf, min(nebula_shelf + 0.45, 0.99), shaped);
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
    return vec3<f32>(fract(h), clamp(s, 0.0, 1.0), clamp(l, 0.0, 1.0));
}

fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
    let h = fract(hsl.x);
    let s = clamp(hsl.y, 0.0, 1.0);
    let l = clamp(hsl.z, 0.0, 1.0);
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
    let op = clamp(opacity, 0.0, 1.0);
    let b = clamp(base, vec3<f32>(0.0), vec3<f32>(1.0));
    let l = clamp(layer, vec3<f32>(0.0), vec3<f32>(1.0));
    let m = u32(clamp(round(mode), 0.0, 26.0));

    var blended = b + l; // default legacy add
    switch m {
        case 0u: { // Linear Dodge (Add) [legacy]
            blended = b + l;
        }
        case 1u: { // Screen [legacy]
            blended = 1.0 - (1.0 - b) * (1.0 - l);
        }
        case 2u: { // Lighten [legacy]
            blended = max(b, l);
        }
        case 3u: { // Normal
            blended = l;
        }
        case 4u: { // Dissolve
            let pick = select(0.0, 1.0, noise < op);
            return mix(base, l, pick);
        }
        case 5u: { // Darken
            blended = min(b, l);
        }
        case 6u: { // Multiply
            blended = b * l;
        }
        case 7u: { // Color Burn
            blended = 1.0 - (1.0 - b) / max(l, vec3<f32>(0.0001));
        }
        case 8u: { // Linear Burn
            blended = b + l - 1.0;
        }
        case 9u: { // Darker Color
            blended = select(l, b, blend_luma(b) <= blend_luma(l));
        }
        case 10u: { // Color Dodge
            blended = b / max(1.0 - l, vec3<f32>(0.0001));
        }
        case 11u: { // Lighter Color
            blended = select(l, b, blend_luma(b) >= blend_luma(l));
        }
        case 12u: { // Overlay
            blended = select(2.0 * b * l, 1.0 - 2.0 * (1.0 - b) * (1.0 - l), b > vec3<f32>(0.5));
        }
        case 13u: { // Soft Light
            blended = (1.0 - 2.0 * l) * b * b + 2.0 * l * b;
        }
        case 14u: { // Hard Light
            blended = select(2.0 * b * l, 1.0 - 2.0 * (1.0 - b) * (1.0 - l), l > vec3<f32>(0.5));
        }
        case 15u: { // Vivid Light
            let low = 1.0 - (1.0 - b) / max(2.0 * l, vec3<f32>(0.0001));
            let high = b / max(2.0 * (1.0 - l), vec3<f32>(0.0001));
            blended = select(low, high, l > vec3<f32>(0.5));
        }
        case 16u: { // Linear Light
            blended = b + 2.0 * l - 1.0;
        }
        case 17u: { // Pin Light
            let low = min(b, 2.0 * l);
            let high = max(b, 2.0 * (l - 0.5));
            blended = select(low, high, l > vec3<f32>(0.5));
        }
        case 18u: { // Hard Mix
            let lin = b + 2.0 * l - 1.0;
            blended = select(vec3<f32>(0.0), vec3<f32>(1.0), lin > vec3<f32>(0.5));
        }
        case 19u: { // Difference
            blended = abs(b - l);
        }
        case 20u: { // Exclusion
            blended = b + l - 2.0 * b * l;
        }
        case 21u: { // Subtract
            blended = b - l;
        }
        case 22u: { // Divide
            blended = b / max(l, vec3<f32>(0.0001));
        }
        case 23u: { // Hue
            let bhsl = rgb_to_hsl(b);
            let lhsl = rgb_to_hsl(l);
            blended = hsl_to_rgb(vec3<f32>(lhsl.x, bhsl.y, bhsl.z));
        }
        case 24u: { // Saturation
            let bhsl = rgb_to_hsl(b);
            let lhsl = rgb_to_hsl(l);
            blended = hsl_to_rgb(vec3<f32>(bhsl.x, lhsl.y, bhsl.z));
        }
        case 25u: { // Color
            let bhsl = rgb_to_hsl(b);
            let lhsl = rgb_to_hsl(l);
            blended = hsl_to_rgb(vec3<f32>(lhsl.x, lhsl.y, bhsl.z));
        }
        case 26u: { // Luminosity
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
    let blend_noise = hash21(floor(in.uv * res), seed + time * 0.23);
    let enable_nebula_layer = params.space_bg_section_flags.x >= 0.5;
    let enable_stars_layer = params.space_bg_section_flags.y >= 0.5;
    let enable_flares_layer = params.space_bg_section_flags.z >= 0.5;
    let ridged_mode = params.space_bg_noise_a.x >= 0.5;
    let nebula_octaves = u32(clamp(params.space_bg_noise_a.y, 1.0, 8.0));
    let nebula_octaves_main = min(nebula_octaves, 5u);
    let nebula_gain = clamp(params.space_bg_noise_a.z, 0.1, 0.95);
    let nebula_lacunarity = clamp(params.space_bg_noise_a.w, 1.1, 4.0);
    let nebula_power = clamp(params.space_bg_noise_b.x, 0.2, 4.0);
    let nebula_shelf = clamp(params.space_bg_noise_b.y, 0.0, 0.95);
    let nebula_ridge_offset = clamp(params.space_bg_noise_b.z, 0.5, 2.5);
    let star_mask_enabled = params.space_bg_star_mask_a.x >= 0.5;
    let star_mask_ridged = params.space_bg_star_mask_a.y >= 0.5;
    let star_mask_octaves = u32(clamp(params.space_bg_star_mask_a.z, 1.0, 8.0));
    let star_mask_octaves_effective = min(star_mask_octaves, 4u);
    let star_mask_scale = clamp(params.space_bg_star_mask_a.w, 0.2, 8.0);
    let star_mask_threshold = clamp(params.space_bg_star_mask_b.x, 0.0, 0.99);
    let star_mask_power = clamp(params.space_bg_star_mask_b.y, 0.2, 4.0);
    let star_mask_gain = clamp(params.space_bg_star_mask_b.z, 0.1, 0.95);
    let star_mask_lacunarity = clamp(params.space_bg_star_mask_b.w, 1.1, 4.0);
    let star_mask_ridge_offset = clamp(params.space_bg_star_mask_c.x, 0.5, 2.5);
    let star_count = clamp(params.space_bg_star_mask_c.y, 0.0, 5.0);
    let star_size_a = clamp(params.space_bg_star_mask_c.z, 0.01, 0.35);
    let star_size_b = clamp(params.space_bg_star_mask_c.w, 0.01, 0.35);
    let star_size_min = min(star_size_a, star_size_b);
    let star_size_max = max(star_size_a, star_size_b);
    let star_color = max(params.space_bg_star_color.rgb, vec3<f32>(0.0));
    let nebula_blend_mode = clamp(params.space_bg_blend_a.x, 0.0, 26.0);
    let nebula_opacity = clamp(params.space_bg_blend_a.y, 0.0, 1.0);
    let stars_blend_mode = clamp(params.space_bg_blend_a.z, 0.0, 26.0);
    let stars_opacity = clamp(params.space_bg_blend_a.w, 0.0, 1.0);
    let flares_blend_mode = clamp(params.space_bg_blend_b.x, 0.0, 26.0);
    let flares_opacity = clamp(params.space_bg_blend_b.y, 0.0, 1.0);
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
    let shaft_falloff = clamp(params.space_bg_shafts_a.z, 0.2, 8.0);
    let shaft_samples = u32(clamp(params.space_bg_shafts_a.w, 4.0, 24.0));
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
    let shaft_color = max(params.space_bg_shafts_b.rgb, vec3<f32>(0.0));
    let shaft_opacity = clamp(params.space_bg_shafts_b.w, 0.0, 1.0);
    let backlight_color = max(params.space_bg_backlight_color.rgb, vec3<f32>(0.0));
    let aspect = res.x / res.y;

    let uv_n = in.uv * 2.0 - 1.0;
    let uv = vec2<f32>(uv_n.x * aspect, uv_n.y);
    // Fullscreen backgrounds are view composition and must remain visually present across the
    // gameplay zoom range. Keep any zoom response subtle and tightly clamped so the sampled
    // background field never collapses into an effectively empty range when zooming in.
    let render_zoom_scale = clamp(params.velocity_dir.z, 0.25, 4.0);
    let camera_zoom = 1.0 / render_zoom_scale;
    let inverted_zoom = 1.0 / max(camera_zoom, 0.25);
    let background_zoom = clamp(1.0 + (inverted_zoom - 1.0) * zoom_rate * 0.12, 0.82, 1.18);
    let zoomed_uv = uv * background_zoom;

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

    var lit_nebula_layer = vec3<f32>(0.0);
    var star_mask = 1.0;
    if enable_nebula_layer {
        // Deterministic seeded nebula with almost no temporal movement.
        let base = zoomed_uv * 1.35 + vec2<f32>(seed * 0.17, seed * 0.29) + subtle_motion * (0.55 + depth_parallax_scale * 0.45);
        var nebula_field_a = 0.0;
        var nebula_field_b = 0.0;
        var nebula_field_c = 0.0;
        if ridged_mode {
            nebula_field_a = ridged_fbm2d_config(
                base + subtle_motion * 0.60,
                seed,
                nebula_octaves_main,
                nebula_gain,
                nebula_lacunarity,
                nebula_ridge_offset
            );
            nebula_field_b = ridged_fbm2d_config(
                base * 1.9 + vec2<f32>(11.3, -7.1),
                seed + 19.0,
                nebula_octaves_main,
                nebula_gain,
                nebula_lacunarity,
                nebula_ridge_offset
            );
            nebula_field_c = ridged_fbm2d_config(
                base * 0.72 + vec2<f32>(-3.7, 5.4),
                seed + 47.0,
                nebula_octaves_main,
                nebula_gain,
                nebula_lacunarity,
                nebula_ridge_offset
            );
        } else {
            nebula_field_a = fbm2d_config(
                base + subtle_motion * 0.60,
                seed,
                nebula_octaves_main,
                nebula_gain,
                nebula_lacunarity
            );
            nebula_field_b = fbm2d_config(
                base * 1.9 + vec2<f32>(11.3, -7.1),
                seed + 19.0,
                nebula_octaves_main,
                nebula_gain,
                nebula_lacunarity
            );
            nebula_field_c = fbm2d_config(
                base * 0.72 + vec2<f32>(-3.7, 5.4),
                seed + 47.0,
                nebula_octaves_main,
                nebula_gain,
                nebula_lacunarity
            );
        }

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

        // Keep nebula generation as single source-of-truth, then derive depth proxies
        // from the same density/mask for lighting/shafts.
        let cloud_body = nebula * nebula_mask * (0.58 * nebula_strength);
        let ridge_mask = smoothstep(0.58, 0.91, nebula_field_a) * nebula_mask;
        let cloud_ridge = nebula * ridge_mask * (0.22 * nebula_strength);
        let nebula_haze = nebula * nebula_mask * (0.14 * depth_haze_strength);
        let nebula_layer = (cloud_body + cloud_ridge + nebula_haze) * intensity * vignette;

        let near_depth = smoothstep(
            max(nebula_shelf - 0.12 * depth_layer_separation, 0.0),
            min(nebula_shelf + 0.30, 0.995),
            clamp(shaped_nebula + nebula_field_b * 0.10, 0.0, 1.0)
        );
        let mid_depth = nebula_mask;
        let far_depth = smoothstep(
            min(nebula_shelf + 0.07 * depth_layer_separation, 0.99),
            min(nebula_shelf + 0.52, 0.999),
            clamp(shaped_nebula * 0.82 + nebula_field_c * 0.22, 0.0, 1.0)
        );
        let nebula_presence = clamp(mid_depth * 0.68 + far_depth * 0.22 + near_depth * 0.10, 0.0, 1.0);
        let lit_presence = smoothstep(0.14, 0.92, nebula_presence);

        // Backlight derived from same nebula depth proxies.
        let backlight_pos = vec2<f32>(backlight_screen.x * aspect, backlight_screen.y);
        let dist_to_light = length(uv - backlight_pos);
        let radial_light = pow(clamp(1.0 - dist_to_light * 0.55, 0.0, 1.0), 1.85);
        let trans_far = clamp(1.0 - far_depth * depth_occlusion_strength * 0.55, 0.0, 1.0);
        let trans_mid = clamp(1.0 - mid_depth * depth_occlusion_strength * 0.82, 0.0, 1.0);
        let trans_near = clamp(1.0 - near_depth * depth_occlusion_strength, 0.0, 1.0);
        let transmission = pow(trans_far * trans_mid * trans_near, max(0.2, 1.6 - backlight_wrap));
        let edge_strength = clamp(
            length(vec2<f32>(dpdx(shaped_nebula), dpdy(shaped_nebula))) * backlight_edge_boost,
            0.0,
            1.0
        );
        let bloom = smoothstep(backlight_bloom_threshold, 1.0, mid_depth) * backlight_bloom_scale * 0.35 * lit_presence;

        // Crepuscular shafts from backlight passing through the same nebula mask field.
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
            if (i >= shaft_samples_effective) {
                break;
            }
            let t_linear = (f32(i) + shaft_jitter) / sample_count_f;
            // Concentrate a few more samples near the source and reduce visible step bands.
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
        shaft_bloom_drive = clamp(
            (shaft_bloom_accum / sample_count_f) * backlight_bloom_scale,
            0.0,
            4.0
        );
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
                clamp(1.0 - exp(-shafts * mix(1.35, 1.85, quality_t)), 0.0, 1.0),
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
        let local_density = clamp(far_depth * 0.28 + mid_depth * 0.46 + near_depth * 0.72, 0.0, 1.0);
        let scattering_weight = (1.0 - exp(-local_density * 3.4)) * (0.55 + edge_strength * 0.45);
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
            let debug_shafts = clamp(shaft_visual, 0.0, 1.0);
            let debug_bloom = clamp(shaft_bloom_drive * 0.6, 0.0, 1.0);
            let debug_density = clamp(nebula_presence, 0.0, 1.0);
            return vec4<f32>(vec3<f32>(debug_shafts, debug_bloom, debug_density), 1.0);
        }
    }

    // Very subtle stars with tiny drift only.
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
        ) * 0.20;
        let stars_mid = subtle_star_layer(
            zoomed_uv + subtle_motion * 0.35,
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
        ) * 0.40;
        let star_mask_base = zoomed_uv * star_mask_scale + subtle_motion * 0.45 + vec2<f32>(seed * 0.07, seed * 0.11);
        var star_mask_noise = 1.0;
        if star_mask_ridged {
            star_mask_noise = ridged_fbm2d_config(
                star_mask_base,
                seed + 201.0,
                star_mask_octaves_effective,
                star_mask_gain,
                star_mask_lacunarity,
                star_mask_ridge_offset
            );
        } else {
            star_mask_noise = fbm2d_config(
                star_mask_base,
                seed + 201.0,
                star_mask_octaves_effective,
                star_mask_gain,
                star_mask_lacunarity
            );
        }
        star_mask = 1.0;
        if star_mask_enabled {
            star_mask = smoothstep(
                star_mask_threshold,
                min(star_mask_threshold + 0.35, 0.99),
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

    var col = composed;
    col = min(col, vec3<f32>(1.0));

    return vec4<f32>(col, 1.0);
}
