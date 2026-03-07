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

const PI: f32 = 3.14159265;
const INV_U32_MAX: f32 = 1.0 / 4294967295.0;

fn hash_u32(v: u32) -> u32 {
    var x = v;
    x ^= x >> 16u;
    x *= 2246822519u;
    x ^= x >> 13u;
    x *= 3266489917u;
    x ^= x >> 16u;
    return x;
}

fn seed_u32(seed: f32) -> u32 {
    return u32(clamp(seed * 4096.0 + 17.0, 0.0, 4294967040.0));
}

fn hash21(cell: vec2<i32>, seed: u32) -> f32 {
    let x = bitcast<u32>(cell.x);
    let y = bitcast<u32>(cell.y);
    let h = hash_u32(x ^ hash_u32(y + seed * 1664525u + 1013904223u));
    return f32(h) * INV_U32_MAX;
}

fn hash22(cell: vec2<i32>, seed: u32) -> vec2<f32> {
    let a = hash21(cell, seed + 11u);
    let b = hash21(cell + vec2<i32>(17, 59), seed + 29u);
    return vec2<f32>(a, b);
}

fn noise2d(p: vec2<f32>, seed: u32) -> f32 {
    let i = vec2<i32>(floor(p));
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i, seed);
    let b = hash21(i + vec2<i32>(1, 0), seed);
    let c = hash21(i + vec2<i32>(0, 1), seed);
    let d = hash21(i + vec2<i32>(1, 1), seed);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm3(p: vec2<f32>, seed: u32) -> f32 {
    var v = 0.0;
    var a = 0.58;
    var f = 1.0;
    for (var i = 0; i < 3; i = i + 1) {
        v += noise2d(p * f, seed + u32(i) * 131u) * a;
        f *= 2.03;
        a *= 0.5;
    }
    return v;
}

fn fbm4(p: vec2<f32>, seed: u32) -> f32 {
    var v = 0.0;
    var a = 0.56;
    var f = 1.0;
    for (var i = 0; i < 4; i = i + 1) {
        v += noise2d(p * f, seed + u32(i) * 173u) * a;
        f *= 1.97;
        a *= 0.52;
    }
    return v;
}

fn nebula_field(uv: vec2<f32>, drift: vec2<f32>, zoom: f32, seed: u32) -> vec3<f32> {
    let p = uv * zoom;
    let warp = vec2<f32>(
        fbm3(p * 0.34 + drift * 0.08, seed + 3u),
        fbm3(p * 0.34 - drift * 0.11 + vec2<f32>(7.1, -4.7), seed + 19u)
    ) - 0.5;
    let warped = p + warp * (0.95 + params.space_bg_depth_a.y * 0.28);
    let coarse = fbm4(warped * 0.56 + drift * 0.12, seed + 41u);
    let detail = fbm3(warped * 1.42 - drift * 0.20, seed + 79u);
    let wisps = noise2d(warped * 3.1 + drift * 0.33, seed + 131u);
    let filaments = noise2d(warped * 6.2 - drift * 0.46, seed + 173u);

    let primary = smoothstep(0.40, 0.90, coarse * 0.74 + detail * 0.34);
    let ridge = smoothstep(0.48, 0.94, (1.0 - abs(detail * 2.0 - 1.0)) * 0.78 + filaments * 0.22);
    let haze = smoothstep(0.24, 0.84, coarse * 0.58 + wisps * 0.28 + filaments * 0.14);
    return vec3<f32>(primary, ridge * primary, haze);
}

fn nebula_mask_fast(uv: vec2<f32>, drift: vec2<f32>, zoom: f32, seed: u32) -> f32 {
    let p = uv * zoom;
    let warp = vec2<f32>(
        noise2d(p * 0.32 + drift * 0.06, seed + 211u),
        noise2d(p * 0.32 - drift * 0.09 + vec2<f32>(5.3, -3.1), seed + 223u)
    ) - 0.5;
    let warped = p + warp * 0.72;
    let coarse = fbm3(warped * 0.52 + drift * 0.08, seed + 239u);
    let detail = noise2d(warped * 1.9 - drift * 0.16, seed + 251u);
    return smoothstep(0.42, 0.88, coarse * 0.78 + detail * 0.30);
}

fn directional_beam_mask(dir: vec2<f32>, dist: f32, time: f32, seed: u32) -> f32 {
    let manhattan = abs(dir.x) + abs(dir.y) + 0.0001;
    let polar_like = vec2<f32>(
        dir.x / manhattan,
        dir.y / manhattan
    );
    let beam_axis = polar_like * vec2<f32>(18.0, 31.0);
    let sweep = vec2<f32>(dist * 2.2, time * 0.018);
    let coarse = noise2d(beam_axis + sweep, seed + 281u);
    let detail = noise2d(beam_axis * 2.4 - sweep * 0.65 + vec2<f32>(7.2, -3.4), seed + 293u);
    return smoothstep(0.56, 0.92, coarse * 0.72 + detail * 0.38);
}

fn cheap_god_rays(
    uv: vec2<f32>,
    light_pos: vec2<f32>,
    drift: vec2<f32>,
    zoom: f32,
    seed: u32,
    shaft_length: f32,
    shaft_intensity: f32
) -> f32 {
    let to_light = light_pos - uv;
    let dist = max(length(to_light), 0.0001);
    let dir = to_light / dist;
    let radial = pow(clamp(1.0 - dist * 0.58, 0.0, 1.0), 1.45);
    let beam_mask = directional_beam_mask(dir, dist, params.viewport_time.z, seed);

    let t0 = 0.16 * shaft_length;
    let t1 = 0.32 * shaft_length;
    let t2 = 0.54 * shaft_length;
    let t3 = 0.82 * shaft_length;

    let m0 = nebula_mask_fast(uv + dir * dist * t0, drift, zoom, seed + 331u);
    let m1 = nebula_mask_fast(uv + dir * dist * t1, drift, zoom, seed + 353u);
    let m2 = nebula_mask_fast(uv + dir * dist * t2, drift, zoom, seed + 379u);
    let m3 = nebula_mask_fast(uv + dir * dist * t3, drift, zoom, seed + 401u);
    let occlusion = m0 * 0.38 + m1 * 0.27 + m2 * 0.21 + m3 * 0.14;
    let transmission = pow(clamp(1.0 - occlusion * 0.92, 0.0, 1.0), 1.35);
    let forward_phase = pow(clamp(dot(dir, normalize(vec2<f32>(0.28, -0.96))) * 0.5 + 0.5, 0.0, 1.0), 2.4);
    return radial * beam_mask * transmission * mix(0.72, 1.0, forward_phase) * shaft_intensity;
}

fn star_layer(
    uv: vec2<f32>,
    density: f32,
    threshold: f32,
    size_min: f32,
    size_max: f32,
    drift: vec2<f32>,
    time: f32,
    seed: u32,
    tint: vec3<f32>
) -> vec3<f32> {
    let grid = uv * density + drift;
    let cell = vec2<i32>(floor(grid));
    let local = fract(grid) - 0.5;
    let n = hash21(cell, seed + 101u);
    if n > threshold {
        return vec3<f32>(0.0);
    }

    let offset = (hash22(cell, seed + 149u) - 0.5) * 0.72;
    let d = length(local - offset);
    let size = mix(size_min, size_max, fract(n * 37.0));
    let core = smoothstep(size, size * 0.14, d);
    let halo = smoothstep(size * 3.6, size * 0.18, d) * 0.24;
    let twinkle = 0.9 + 0.1 * sin(time * (1.1 + fract(n * 23.0) * 1.7) + n * 31.0 * PI);
    let warm_cool = mix(vec3<f32>(0.85, 0.90, 1.0), vec3<f32>(1.0, 0.93, 0.84), fract(n * 91.0));
    return (core + halo) * twinkle * warm_cool * tint;
}

fn flare_layer(
    uv: vec2<f32>,
    density: f32,
    size: f32,
    drift: vec2<f32>,
    seed: u32,
    tint: vec3<f32>
) -> vec3<f32> {
    let grid = uv * 6.0 + drift;
    let cell = vec2<i32>(floor(grid));
    let local = fract(grid) - 0.5;
    let n = hash21(cell, seed + 211u);
    if n > density {
        return vec3<f32>(0.0);
    }

    let offset = (hash22(cell, seed + 257u) - 0.5) * 0.7;
    let flare_local = local - offset;
    let d = length(flare_local);
    let radius = max(0.08, size * 0.11);
    if d > radius * 2.2 {
        return vec3<f32>(0.0);
    }

    let flare_uv = clamp(flare_local / (radius * 2.2) + vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(1.0));
    let sample_col = textureSampleLevel(flare_texture, flare_sampler, flare_uv, 0.0).rgb;
    let radial = smoothstep(radius * 1.8, radius * 0.08, d);
    return sample_col * radial * tint;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let res = max(params.viewport_time.xy, vec2<f32>(1.0, 1.0));
    let time = params.viewport_time.z;
    let warp_factor = max(params.viewport_time.w, 0.0);
    let seed = seed_u32(max(params.space_bg_tint.w, 0.0));

    let intensity = max(params.space_bg_params.x, 0.0001);
    let drift_scale = max(params.space_bg_params.y, 0.0);
    let velocity_glow = max(params.space_bg_params.z, 0.0);
    let nebula_strength = max(params.space_bg_params.w, 0.0);

    let enable_nebula = params.space_bg_section_flags.x >= 0.5;
    let enable_stars = params.space_bg_section_flags.y >= 0.5;
    let enable_flares = params.space_bg_section_flags.z >= 0.5 && params.space_bg_flare.x >= 0.5;
    let enable_backlight = params.space_bg_light_flags.x >= 0.5 && enable_nebula;

    let base_background = max(params.space_bg_background.rgb, vec3<f32>(0.0));
    let tint = max(params.space_bg_tint.rgb, vec3<f32>(0.0));
    let nebula_color_a = max(params.space_bg_nebula_color_a.rgb, vec3<f32>(0.0));
    let nebula_color_b = max(params.space_bg_nebula_color_b.rgb, vec3<f32>(0.0));
    let nebula_color_c = max(params.space_bg_nebula_color_c.rgb, vec3<f32>(0.0));
    let star_color = max(params.space_bg_star_color.rgb, vec3<f32>(0.0));
    let flare_tint = max(params.space_bg_flare_tint.rgb, vec3<f32>(0.0));
    let backlight_color = max(params.space_bg_backlight_color.rgb, vec3<f32>(0.0));

    let aspect = res.x / res.y;
    let uv_n = in.uv * 2.0 - 1.0;
    let uv = vec2<f32>(uv_n.x * aspect, uv_n.y);

    let zoom_rate = max(params.space_bg_blend_b.z, 0.0);
    let camera_zoom = 1.0 / max(params.velocity_dir.z, 0.01);
    let inverted_zoom = 1.0 / max(camera_zoom, 0.01);
    let bg_zoom = max(0.05, 1.0 + (inverted_zoom - 1.0) * zoom_rate * 0.18);

    let heading_raw = params.velocity_dir.xy;
    var heading = vec2<f32>(0.0, 1.0);
    if length(heading_raw) > 0.001 {
        heading = normalize(heading_raw);
    }

    let drift = (
        params.drift_intensity.xy * 0.0017 +
        heading * (0.00065 + warp_factor * 0.00075)
    ) * drift_scale;
    let speed_glow = clamp(length(params.drift_intensity.xy) * velocity_glow * 0.05, 0.0, 0.12);

    let vertical_grad = clamp((uv_n.y + 1.0) * 0.5, 0.0, 1.0);
    let horizon_glow = smoothstep(-0.18, 0.92, vertical_grad);
    let deep_black = base_background * 0.07 + vec3<f32>(0.00008, 0.00014, 0.00045);
    let upper_space = base_background * 0.18 + vec3<f32>(0.00055, 0.00085, 0.0021);
    var col = mix(upper_space, deep_black, horizon_glow);
    col += vec3<f32>(0.0008, 0.0012, 0.0024) * (1.0 - horizon_glow) * 0.38;

    if enable_nebula {
        let far_nebula = nebula_field(
            uv * 0.68 + vec2<f32>(4.2, -3.1),
            drift * 0.42,
            bg_zoom * 0.54,
            seed + 97u
        );
        let near_nebula = nebula_field(uv, drift, bg_zoom, seed);

        let nebula_mask = max(near_nebula.x, far_nebula.x * 0.62);
        let ridge = max(near_nebula.y, far_nebula.y * 0.44);
        let haze = clamp(near_nebula.z * 0.72 + far_nebula.z * 0.58, 0.0, 1.0);
        let color_mix = mix(nebula_color_a, nebula_color_b, ridge * 0.64 + haze * 0.16);
        let nebula_tint_mix = mix(color_mix, nebula_color_c, haze * 0.46 + ridge * 0.12);

        let core = nebula_tint_mix * nebula_mask * (0.18 + 0.34 * nebula_strength);
        let ridges = mix(nebula_color_b, nebula_color_c, 0.52) * ridge * (0.12 + nebula_strength * 0.10);
        let mist = mix(nebula_color_a, nebula_tint_mix, 0.58) * haze * (0.05 + params.space_bg_depth_a.z * 0.035);
        col += core + ridges + mist;

        if enable_backlight {
            let backlight_pos = vec2<f32>(params.space_bg_light_a.x * aspect, params.space_bg_light_a.y);
            let dist_to_light = length(uv - backlight_pos);
            let radial = pow(clamp(1.0 - dist_to_light * 0.56, 0.0, 1.0), 1.55);
            let occlusion = 1.0 - nebula_mask * clamp(params.space_bg_depth_a.w, 0.0, 3.0) * 0.45;
            let edge = smoothstep(0.04, 0.32, ridge * 0.8 + haze * 0.2);
            let light_probe = nebula_mask_fast(
                mix(uv, backlight_pos, 0.32),
                drift * 0.6,
                bg_zoom * 0.82,
                seed + 643u
            );
            let scatter = pow(clamp(1.0 - light_probe * 0.82, 0.0, 1.0), 1.4);
            let bloom = smoothstep(
                clamp(params.space_bg_light_b.z, 0.0, 1.0),
                1.0,
                haze * 0.7 + nebula_mask * 0.3
            ) * clamp(params.space_bg_light_b.y, 0.0, 2.0);
            let backlight = radial * max(occlusion, 0.0) * scatter * (0.34 + edge * clamp(params.space_bg_light_b.x, 0.0, 6.0) * 0.16);
            let shaft_strength = clamp(params.space_bg_shafts_a.x, 0.0, 40.0) * clamp(params.space_bg_shafts_b.w, 0.0, 1.0) * 0.02;
            let rays = cheap_god_rays(
                uv,
                backlight_pos,
                drift,
                bg_zoom,
                seed,
                clamp(params.space_bg_shafts_a.y, 0.05, 0.95),
                shaft_strength
            );
            col += backlight_color * backlight * clamp(params.space_bg_light_a.z, 0.0, 20.0) * 0.085;
            col += mix(backlight_color, nebula_color_c, 0.35) * bloom * radial * scatter * 0.045;
            col += max(params.space_bg_shafts_b.rgb, vec3<f32>(0.0)) * rays * (0.3 + bloom * 0.7);
        }
    }

    if enable_stars {
        let star_scale = clamp(params.space_bg_star_mask_c.y, 0.0, 5.0);
        let threshold = mix(0.92, 0.58, clamp(star_scale / 5.0, 0.0, 1.0));
        let size_min = clamp(min(params.space_bg_star_mask_c.z, params.space_bg_star_mask_c.w), 0.01, 0.35);
        let size_max = clamp(max(params.space_bg_star_mask_c.z, params.space_bg_star_mask_c.w), 0.01, 0.35);

        let far = star_layer(
            uv * (0.95 + bg_zoom * 0.08),
            16.0,
            threshold,
            size_min * 0.85,
            size_max * 0.95,
            drift * 0.14 + vec2<f32>(time * 0.0007, -time * 0.0005),
            time,
            seed + 401u,
            star_color * 0.65
        );
        let mid = star_layer(
            uv * (1.05 + bg_zoom * 0.12),
            9.0,
            threshold - 0.10,
            size_min,
            size_max,
            drift * 0.28 + vec2<f32>(time * 0.0011, -time * 0.0008),
            time,
            seed + 457u,
            star_color * 0.95
        );
        let near = star_layer(
            uv * (1.18 + bg_zoom * 0.16),
            5.0,
            threshold - 0.18,
            size_min * 1.1,
            size_max * 1.25,
            drift * 0.48 + vec2<f32>(time * 0.0018, -time * 0.0013),
            time,
            seed + 509u,
            star_color * 1.2
        );

        col += far + mid + near;
    }

    if enable_flares {
        let flare_density = clamp(params.space_bg_flare.z, 0.0, 1.0) * 0.06;
        let flare_size = max(params.space_bg_flare.w, 0.1);
        let flares = flare_layer(
            uv * (0.8 + bg_zoom * 0.08),
            flare_density,
            flare_size,
            drift * 0.24 + vec2<f32>(time * 0.0005, -time * 0.00035),
            seed + 601u,
            flare_tint
        );
        col += flares * max(params.space_bg_flare.y, 0.0) * 0.42;
    }

    col += vec3<f32>(0.04, 0.06, 0.10) * speed_glow;
    col = mix(col, col * tint * 3.0, 0.18);

    let vignette = clamp(1.16 - length(uv_n) * 0.21, 0.78, 1.0);
    col *= vignette * intensity;

    // Cheap final dithering helps hide dark-range banding in low-luminance gradients.
    let pixel = vec2<i32>(in.uv * res);
    let dither = (hash21(pixel, seed + 733u) - 0.5) / 255.0;
    col += vec3<f32>(dither);

    return vec4<f32>(max(col, vec3<f32>(0.0)), 1.0);
}
