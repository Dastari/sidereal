#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct SharedWorldLightingUniforms {
    metadata: vec4<f32>,
    ambient: vec4<f32>,
    backlight: vec4<f32>,
    flash: vec4<f32>,
    stellar_dir_intensity: array<vec4<f32>, 2>,
    stellar_color_params: array<vec4<f32>, 2>,
    local_dir_intensity: array<vec4<f32>, 8>,
    local_color_radius: array<vec4<f32>, 8>,
}

struct PlanetBodyUniforms {
    identity_a: vec4<f32>,
    identity_b: vec4<f32>,
    feature_flags_a: vec4<f32>,
    feature_flags_b: vec4<f32>,
    pass_flags_a: vec4<f32>,
    lighting_a: vec4<f32>,
    lighting_b: vec4<f32>,
    surface_a: vec4<f32>,
    surface_b: vec4<f32>,
    surface_c: vec4<f32>,
    surface_d: vec4<f32>,
    clouds_a: vec4<f32>,
    atmosphere_a: vec4<f32>,
    emissive_a: vec4<f32>,
    sun_dir_a: vec4<f32>,
    world_lighting: SharedWorldLightingUniforms,
    color_primary: vec4<f32>,
    color_secondary: vec4<f32>,
    color_tertiary: vec4<f32>,
    color_atmosphere: vec4<f32>,
    color_clouds: vec4<f32>,
    color_night_lights: vec4<f32>,
    color_emissive: vec4<f32>,
}

@group(2) @binding(0) var<uniform> params: PlanetBodyUniforms;

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;

fn saturate(v: f32) -> f32 {
    return clamp(v, 0.0, 1.0);
}

fn color_chroma(color: vec3<f32>) -> f32 {
    return max(max(color.r, color.g), color.b) - min(min(color.r, color.g), color.b);
}

fn apply_saturation(color: vec3<f32>, saturation: f32) -> vec3<f32> {
    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    return mix(vec3<f32>(luminance), color, saturation);
}

fn apply_contrast(color: vec3<f32>, contrast: f32) -> vec3<f32> {
    return (color - vec3<f32>(0.5)) * contrast + vec3<f32>(0.5);
}

fn tone_map(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((color * (a * color + vec3<f32>(b))) / (color * (c * color + vec3<f32>(d)) + vec3<f32>(e)), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn hash12(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let n00 = hash12(i);
    let n10 = hash12(i + vec2<f32>(1.0, 0.0));
    let n01 = hash12(i + vec2<f32>(0.0, 1.0));
    let n11 = hash12(i + vec2<f32>(1.0, 1.0));
    return mix(mix(n00, n10, u.x), mix(n01, n11, u.x), u.y);
}

fn fbm2(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    for (var i = 0; i < 5; i = i + 1) {
        value += noise2(p * frequency) * amplitude;
        frequency *= 2.04;
        amplitude *= 0.53;
    }
    return value;
}

fn disc_surface_color(disc_uv: vec2<f32>, radius: f32) -> vec3<f32> {
    let time_s = params.identity_a.w;
    let detail = saturate(params.lighting_a.z);
    let uv = disc_uv / max(radius, 0.0001);
    let spin = params.identity_a.w * params.identity_b.x + params.identity_a.z * TAU;
    let flow = vec2<f32>(
        uv.x * cos(spin) - uv.y * sin(spin),
        uv.x * sin(spin) + uv.y * cos(spin),
    );
    let warp = vec2<f32>(
        fbm2(flow * 2.2 + vec2<f32>(time_s * 0.08, params.identity_a.z * 9.0)),
        fbm2(flow.yx * 2.7 + vec2<f32>(params.identity_a.z * 13.0, -time_s * 0.06)),
    ) - vec2<f32>(0.5);
    let cell_uv = flow * (8.0 + detail * 10.0) + warp * (0.5 + detail * 0.7);
    let cells_a = fbm2(cell_uv * 2.0 + vec2<f32>(time_s * 0.16, 0.0));
    let cells_b = fbm2(cell_uv.yx * 4.2 + vec2<f32>(0.0, -time_s * 0.21));
    let granules = 1.0 - abs((cells_a * 0.65 + cells_b * 0.35) * 2.0 - 1.0);
    let lanes = smoothstep(0.28, 0.78, 1.0 - granules);
    let bright = smoothstep(0.52, 0.92, granules * 0.62 + cells_a * 0.28 + cells_b * 0.1);
    let mottles = fbm2(cell_uv * 10.0 + vec2<f32>(time_s * 0.45, params.identity_a.z * 41.0));

    var color = mix(params.color_tertiary.rgb * 0.46, params.color_secondary.rgb * 0.9, bright);
    color = mix(color, params.color_primary.rgb * 1.12, bright * 0.24 + smoothstep(0.64, 0.96, mottles) * 0.1);
    color *= 0.58 + bright * 0.34 + mottles * 0.1;
    color *= 1.0 - lanes * mix(0.24, 0.48, detail);
    color += params.color_emissive.rgb * params.color_emissive.a * bright * 0.06;
    return color;
}

fn prominence_color(quad_uv: vec2<f32>, dist: f32, radius: f32) -> vec4<f32> {
    let max_outer = max(0.035, 0.985 - radius);
    let edge = max(dist - radius, 0.0);
    if dist < radius - 0.02 || edge > max_outer {
        return vec4<f32>(0.0);
    }

    let time_s = params.identity_a.w;
    let star_time = time_s * max(0.05, abs(params.atmosphere_a.z) * 4.0);
    let dir = quad_uv / max(dist, 0.0001);
    let angle = atan2(dir.y, dir.x);
    let corona_drive = max(params.clouds_a.w, 0.85);
    let flare_density = saturate(params.clouds_a.y * 4.0 + params.clouds_a.z * 0.75);
    let base_len = min(0.05 + corona_drive * 0.055, max_outer);
    let variable_len = min(0.22 + corona_drive * 0.22, max_outer);
    let angular_a = fbm2(vec2<f32>(cos(angle * 7.0 + star_time * 0.18), sin(angle * 7.0 + star_time * 0.18)) * 1.8 + vec2<f32>(params.identity_a.z * 11.0, 0.0));
    let angular_b = fbm2(vec2<f32>(cos(angle * 17.0 - star_time * 0.4), sin(angle * 17.0 - star_time * 0.4)) * 1.2 + vec2<f32>(0.0, params.identity_a.z * 23.0));
    let extension = smoothstep(0.34, 0.88, angular_a * 0.65 + angular_b * 0.35);
    let flare_len = min(base_len + variable_len * extension * extension, max_outer);
    let radial = saturate(edge / max(flare_len, 0.0001));

    let curl = fbm2(vec2<f32>(angle * 3.2 + radial * 1.7, star_time * 0.22 + params.identity_a.z * 3.0)) - 0.5;
    let filament_a = pow(0.5 + 0.5 * sin(angle * 38.0 + radial * 8.0 + curl * 4.2 + star_time * 1.6), 8.0);
    let filament_b = pow(0.5 + 0.5 * sin(angle * 23.0 - radial * 12.0 - curl * 3.4 - star_time * 1.05), 10.0);
    let filament_noise = fbm2(vec2<f32>(angle * 4.8 + curl * 1.4, radial * 7.0 - star_time * 0.52));
    let filament = max(filament_a, filament_b) * smoothstep(0.34, 0.88, filament_noise + extension * 0.24 + flare_density * 0.16);
    let root_glow = 1.0 - smoothstep(0.0, 0.18, radial);
    let taper = pow(1.0 - radial, 1.55);
    var tongue_density = 0.0;
    var tongue_heat = 0.0;
    let active_tongues = mix(3.0, 12.0, flare_density);
    for (var i = 0; i < 12; i = i + 1) {
        let active_mask = 1.0 - step(active_tongues, f32(i));
        let seed = f32(i) * 37.21 + params.identity_a.z * 113.0;
        let center = hash12(vec2<f32>(seed, 0.31)) * TAU - PI;
        let length = min(mix(0.13, 0.36, hash12(vec2<f32>(seed, 1.17))) * (0.75 + corona_drive * 0.34), max_outer);
        let local = saturate(edge / max(length, 0.0001));
        let whip = (fbm2(vec2<f32>(local * 3.2 + seed, star_time * 0.48)) - 0.5) * 0.42 * local;
        let curl = sin(local * 7.4 + star_time * mix(0.45, 1.15, hash12(vec2<f32>(seed, 2.2))) + seed) * 0.11 * local;
        let da = atan2(sin(angle - center - whip - curl), cos(angle - center - whip - curl));
        let width = mix(0.018, 0.045, hash12(vec2<f32>(seed, 3.7))) * (1.1 - local * 0.82);
        let core = 1.0 - smoothstep(width * 0.45, width, abs(da));
        let sheath = 1.0 - smoothstep(width, width * 2.4, abs(da));
        let length_mask = (1.0 - smoothstep(0.72, 1.0, local)) * smoothstep(0.0, 0.06, local);
        let flicker = 0.78 + 0.28 * fbm2(vec2<f32>(seed, star_time * 1.9 + local * 2.0));
        let tongue = (core * 0.9 + sheath * 0.42) * length_mask * flicker * active_mask;
        tongue_density = max(tongue_density, tongue);
        tongue_heat = max(tongue_heat, core * (1.0 - local) + sheath * 0.35);
    }
    let density = (root_glow * 0.34 + filament * taper * (1.0 + extension * 0.65) + tongue_density * 1.25)
        * smoothstep(radius - 0.004, radius + 0.01, dist)
        * (1.0 - smoothstep(0.96, 1.0, radial))
        * (1.0 - smoothstep(0.93, 0.995, max(abs(quad_uv.x), abs(quad_uv.y))));

    let heat = 1.0 - smoothstep(0.0, 0.68, radial);
    let back_color = params.color_night_lights.rgb;
    let cool = mix(mix(back_color, params.color_tertiary.rgb, 0.7), params.color_secondary.rgb, heat);
    let hot = mix(params.color_emissive.rgb, params.color_primary.rgb, 0.5);
    let color = mix(cool, hot, root_glow * 0.72 + filament * 0.18 + tongue_heat * 0.55) * density * (1.65 + filament * 1.25 + tongue_density * 1.45);
    let alpha = saturate(density * (0.46 + corona_drive * 0.28 + params.color_emissive.a * 0.12));
    return vec4<f32>(color, alpha);
}

fn prominence_loop_color(quad_uv: vec2<f32>, dist: f32, radius: f32) -> vec4<f32> {
    let time_s = params.identity_a.w;
    let event_rate = mix(0.012, 0.16, saturate(abs(params.atmosphere_a.z) * 1.8));
    let epoch_time = time_s * event_rate;
    let epoch = floor(epoch_time);
    let local_t = fract(epoch_time);
    let life = smoothstep(0.04, 0.2, local_t) * (1.0 - smoothstep(0.62, 0.96, local_t));
    if life <= 0.001 {
        return vec4<f32>(0.0);
    }

    let dir = quad_uv / max(dist, 0.0001);
    let angle = atan2(dir.y, dir.x);
    var out_color = vec3<f32>(0.0);
    var out_alpha = 0.0;
    let active_loops = clamp(params.clouds_a.x * 0.34, 0.0, 3.0);
    for (var i = 0; i < 6; i = i + 1) {
        let active_mask = 1.0 - step(active_loops, f32(i));
        let seed = epoch + f32(i) * 19.37 + params.identity_a.z * 97.0;
        let center = hash12(vec2<f32>(seed, 1.7)) * TAU - PI;
        let width = mix(0.14, 0.42, hash12(vec2<f32>(seed, 2.9)));
        let height = mix(0.08, 0.26, hash12(vec2<f32>(seed, 4.1))) * max(params.clouds_a.w, 0.85);
        let thickness = mix(0.012, 0.032, hash12(vec2<f32>(seed, 6.3)));
        let da = atan2(sin(angle - center), cos(angle - center));
        let x = da / max(width, 0.0001);
        if abs(x) < 1.0 {
            let jitter = (fbm2(vec2<f32>(x * 5.0 + seed, time_s * 0.8)) - 0.5) * 0.014;
            let target_r = radius + height * (1.0 - x * x) + jitter;
            let d = abs(dist - target_r);
            let arch_core = 1.0 - smoothstep(thickness * 0.42, thickness, d);
            let arch_sheath = 1.0 - smoothstep(thickness, thickness * 2.8, d);
            let arch = (arch_core * 0.82 + arch_sheath * 0.38)
                * (1.0 - smoothstep(0.78, 1.0, abs(x)))
                * life
                * active_mask;
            let flicker = 0.78 + hash12(vec2<f32>(seed, floor(time_s * 10.0))) * 0.28;
            let loop_color = mix(params.color_secondary.rgb, params.color_emissive.rgb, 0.42);
            out_color += loop_color * arch * flicker * 1.35;
            out_alpha = max(out_alpha, arch * 0.48 * flicker);
        }
    }
    return vec4<f32>(out_color, out_alpha);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    if params.pass_flags_a.x > 0.5 || params.pass_flags_a.y > 0.5 {
        discard;
    }

    let radius = mix(0.5, 0.66, saturate(params.lighting_a.x));
    let quad_uv = mesh.uv * 2.0 - vec2<f32>(1.0, 1.0);
    let dist = length(quad_uv);
    let max_outer = max(0.035, 0.985 - radius);
    if dist > radius + max_outer {
        discard;
    }

    let body_mask = 1.0 - smoothstep(radius - 0.02, radius + 0.006, dist);
    let disc_uv = quad_uv / max(radius, 0.0001);
    let body = disc_surface_color(quad_uv, radius);
    let rim = 1.0 - smoothstep(radius - 0.035, radius + 0.006, dist);
    let inner_limb = smoothstep(radius - 0.08, radius, dist) * body_mask;
    var color = body * (0.58 + params.color_emissive.a * 0.16);
    color += mix(params.color_emissive.rgb, params.color_primary.rgb, 0.58) * inner_limb * 0.22;
    color += params.color_atmosphere.rgb * rim * 0.08;

    let corona = prominence_color(quad_uv, dist, radius);
    let loops = prominence_loop_color(quad_uv, dist, radius);
    let edge = max(dist - radius, 0.0);
    let glow_radius = min(max_outer, 0.075 + params.emissive_a.x * 0.45 + params.clouds_a.w * 0.055);
    let glow_shell = pow(1.0 - smoothstep(0.0, max(glow_radius, 0.0001), edge), 1.75)
        * (1.0 - body_mask)
        * params.emissive_a.z
        * (0.55 + params.clouds_a.w * 0.28);
    let glow_color = mix(params.color_night_lights.rgb, params.color_atmosphere.rgb, 0.68)
        + params.color_emissive.rgb * 0.18;
    let limb_edge = 1.0 - smoothstep(0.0, 0.026, abs(dist - radius));
    let limb_color = params.color_atmosphere.rgb * 0.72 + params.color_emissive.rgb * 0.24 + params.color_primary.rgb * 0.18;
    let limb_glow = limb_edge * (0.38 + params.emissive_a.z * 0.42);
    var final_linear = max(
        color * body_mask + corona.rgb + loops.rgb + glow_color * glow_shell + limb_color * limb_glow,
        vec3<f32>(0.0),
    );
    final_linear = apply_saturation(final_linear, max(1.0, params.identity_b.y));
    final_linear = apply_contrast(final_linear, max(1.0, params.identity_b.z));
    final_linear = max(final_linear, vec3<f32>(0.0));

    let authored_chroma = max(
        max(color_chroma(params.color_primary.rgb), color_chroma(params.color_secondary.rgb)),
        max(max(color_chroma(params.color_tertiary.rgb), color_chroma(params.color_atmosphere.rgb)), color_chroma(params.color_emissive.rgb)),
    );
    if authored_chroma < 0.015 {
        final_linear = vec3<f32>(dot(final_linear, vec3<f32>(0.2126, 0.7152, 0.0722)));
    }

    let final_color = tone_map(final_linear);
    let alpha = max(body_mask, max(max(corona.a, loops.a), max(glow_shell * 0.28, limb_glow * 0.36)));
    return vec4<f32>(final_color, alpha);
}
