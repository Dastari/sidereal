// Asteroid 2D Sprite Shader
//
// Converted from the prior 3D/PBR mesh shader to the current 2D asteroid
// material pipeline used by AsteroidSpriteShaderMaterial.
//
// Bindings match sprite material contract:
// @group(2) @binding(0) texture_2d
// @group(2) @binding(1) sampler
// @group(2) @binding(2) SharedWorldLightingUniforms
// @group(2) @binding(3) generated normal texture
// @group(2) @binding(4) generated normal sampler
// @group(2) @binding(5) local rotation: x=cos(theta), y=sin(theta)

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var image: texture_2d<f32>;
@group(2) @binding(1) var image_sampler: sampler;
@group(2) @binding(3) var normal_image: texture_2d<f32>;
@group(2) @binding(4) var normal_sampler: sampler;
@group(2) @binding(5) var<uniform> local_rotation: vec4<f32>;

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

@group(2) @binding(2) var<uniform> lighting: SharedWorldLightingUniforms;

fn saturate(v: f32) -> f32 {
    return clamp(v, 0.0, 1.0);
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

fn safe_normalize(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    if len > 0.0001 {
        return v / len;
    }
    return fallback;
}

fn normal_from_map(uv: vec2<f32>) -> vec3<f32> {
    let sample = textureSample(normal_image, normal_sampler, uv).rgb;
    let decoded = sample * 2.0 - vec3<f32>(1.0, 1.0, 1.0);
    return normalize(vec3<f32>(decoded.x * 1.18, -decoded.y * 1.18, max(decoded.z, 0.34)));
}

fn sample_alpha(uv: vec2<f32>) -> f32 {
    let clamped_uv = clamp(uv, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
    return textureSample(image, image_sampler, clamped_uv).a;
}

fn world_dir_to_local(v: vec3<f32>) -> vec3<f32> {
    let c = local_rotation.x;
    let s = local_rotation.y;
    return vec3<f32>(
        c * v.x + s * v.y,
        -s * v.x + c * v.y,
        v.z
    );
}

fn uv_to_sprite_local_xy(uv: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
}

fn hash12(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn noise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let a = hash12(i + vec2<f32>(0.0, 0.0));
    let b = hash12(i + vec2<f32>(1.0, 0.0));
    let c = hash12(i + vec2<f32>(0.0, 1.0));
    let d = hash12(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm2d(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var f = 1.0;
    for (var i = 0; i < 5; i = i + 1) {
        v += a * noise2d(p * f);
        f *= 2.0;
        a *= 0.5;
    }
    return v;
}

fn crater_mask(uv: vec2<f32>) -> f32 {
    // Small domain-warped crater field in UV-space.
    let warp = vec2<f32>(
        fbm2d(uv * 9.0 + vec2<f32>(3.1, 6.7)),
        fbm2d(uv * 9.0 + vec2<f32>(8.4, 1.9))
    );
    let p = uv * 12.0 + (warp - 0.5) * 0.8;
    let cell = floor(p);
    let local = fract(p);

    var crater = 0.0;
    for (var x = -1; x <= 1; x = x + 1) {
        for (var y = -1; y <= 1; y = y + 1) {
            let offs = vec2<f32>(f32(x), f32(y));
            let cid = cell + offs;
            let center = vec2<f32>(hash12(cid + 1.13), hash12(cid + 7.77));
            let d = length(local - offs - center);
            let radius = mix(0.08, 0.28, hash12(cid + 13.37));

            // Bowl + rim shape
            let bowl = smoothstep(radius, radius * 0.45, d);
            let rim = smoothstep(radius * 1.05, radius * 0.92, d)
                - smoothstep(radius * 0.92, radius * 0.8, d);
            crater = max(crater, bowl * 0.9 + rim * 0.35);
        }
    }
    return clamp(crater, 0.0, 1.0);
}

fn vein_mask(uv: vec2<f32>) -> f32 {
    let n = fbm2d(uv * 22.0 + vec2<f32>(12.7, 4.3));
    let ridge = abs(n - 0.52);
    return 1.0 - smoothstep(0.015, 0.06, ridge);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = clamp(mesh.uv, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
    let base = textureSample(image, image_sampler, uv);

    if base.a <= 0.001 {
        discard;
    }

    let dims = max(vec2<f32>(textureDimensions(image)), vec2<f32>(1.0, 1.0));
    let texel = vec2<f32>(1.0, 1.0) / dims;
    let alpha_l = sample_alpha(uv - vec2<f32>(texel.x, 0.0));
    let alpha_r = sample_alpha(uv + vec2<f32>(texel.x, 0.0));
    let alpha_d = sample_alpha(uv - vec2<f32>(0.0, texel.y));
    let alpha_u = sample_alpha(uv + vec2<f32>(0.0, texel.y));
    let alpha_min = min(min(alpha_l, alpha_r), min(alpha_d, alpha_u));
    let edge_contrast = saturate((base.a - alpha_min) * 2.4);

    let centered = uv_to_sprite_local_xy(uv);
    let r = clamp(length(centered), 0.0, 1.0);
    let edge_shadow = max(edge_contrast, smoothstep(0.62, 1.0, r));

    let broad_grain = fbm2d(uv * 5.5 + vec2<f32>(2.1, 7.4));
    let mid_grain = fbm2d(uv * 15.0 + vec2<f32>(8.6, 1.7));
    let fine_grain = fbm2d(uv * 42.0 + vec2<f32>(13.5, 4.2));
    let craters = crater_mask(uv) * 0.72;
    let cracks = pow(1.0 - abs(fbm2d(uv * 16.0 + vec2<f32>(7.3, 2.4)) - 0.5) * 2.0, 7.0)
        * (0.35 + smoothstep(0.15, 0.95, r) * 0.65);
    let veins = vein_mask(uv) * smoothstep(0.45, 0.92, mid_grain) * 0.12;
    let fracture_ridge_a = pow(1.0 - abs(sin(centered.x * 9.0 - centered.y * 6.0 + mid_grain * 4.2)), 6.0);
    let fracture_ridge_b = pow(1.0 - abs(sin(centered.x * -5.0 + centered.y * 12.0 + broad_grain * 5.0)), 7.0);
    let ridges = max(fracture_ridge_a * 0.75, fracture_ridge_b * 0.55)
        * (1.0 - smoothstep(0.25, 0.98, r));

    let base_value = clamp(
        luminance(base.rgb) * 0.24 + broad_grain * 0.24 + mid_grain * 0.12 + fine_grain * 0.04 + 0.18,
        0.0,
        1.0
    );
    let stone_dark = vec3<f32>(0.105, 0.108, 0.105);
    let stone_mid = vec3<f32>(0.36, 0.35, 0.325);
    let stone_light = vec3<f32>(0.74, 0.71, 0.64);
    var color = mix(stone_dark, stone_mid, smoothstep(0.14, 0.58, base_value));
    color = mix(color, stone_light, smoothstep(0.52, 0.96, base_value) * 0.82);

    let warm_cool_mottle = mix(
        vec3<f32>(0.88, 0.94, 1.05),
        vec3<f32>(1.08, 1.04, 0.92),
        broad_grain
    );
    color *= warm_cool_mottle;
    color *= 0.94 + fine_grain * 0.10;
    color += vec3<f32>(0.025, 0.023, 0.020) * smoothstep(0.82, 0.98, fine_grain);

    let cavity = saturate(craters * 0.58 + cracks * 0.40 + veins * 0.28);
    color *= 1.0 - cavity * 0.42;
    color += vec3<f32>(0.08, 0.078, 0.068) * ridges * 0.12;
    color *= 1.0 - edge_shadow * 0.20;
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    let sphere_normal = normalize(vec3<f32>(
        centered.x,
        centered.y,
        sqrt(max(0.0, 1.0 - dot(centered, centered)))
    ));
    let relief_normal = normal_from_map(uv);
    let surface_normal = normalize(sphere_normal * 0.68 + relief_normal * 0.72);
    var dominant_dir_world = vec3<f32>(-0.35, 0.45, 0.82);
    var dominant_dir = safe_normalize(world_dir_to_local(dominant_dir_world), vec3<f32>(-0.35, 0.45, 0.82));
    var dominant_color = lighting.ambient.rgb;
    var dominant_macro_ndl = 0.0;
    var primary_light = vec3<f32>(0.0);
    let stellar_count = min(u32(lighting.metadata.x), 2u);
    for (var i: u32 = 0u; i < stellar_count; i = i + 1u) {
        let slot = lighting.stellar_dir_intensity[i];
        if slot.w <= 0.001 {
            continue;
        }
        let light_dir_world = safe_normalize(slot.xyz, dominant_dir_world);
        let light_dir = safe_normalize(world_dir_to_local(light_dir_world), dominant_dir);
        let macro_ndl_i = saturate(dot(sphere_normal, light_dir));
        let relief_ndl_i = saturate(dot(surface_normal, light_dir));
        let primary_ndl_i = mix(macro_ndl_i, relief_ndl_i, 0.24);
        let wrapped_key = saturate(primary_ndl_i * 0.98 + 0.02);
        let banded_key = mix(wrapped_key, floor(wrapped_key * 6.0 + 0.5) / 6.0, 0.10);
        let light_color = lighting.stellar_color_params[i].rgb;
        primary_light += light_color * slot.w * (banded_key * 1.24);
        if macro_ndl_i * slot.w > dominant_macro_ndl {
            dominant_macro_ndl = macro_ndl_i * slot.w;
            dominant_dir_world = light_dir_world;
            dominant_dir = light_dir;
            dominant_color = light_color;
        }
    }
    let macro_ndl = saturate(dominant_macro_ndl);
    let primary_ndl = macro_ndl;
    let backlight = pow(saturate(dot(surface_normal, -dominant_dir)), 2.1);
    var local_light = vec3<f32>(0.0);
    let local_count = min(u32(lighting.metadata.y), 8u);
    for (var i: u32 = 0u; i < local_count; i = i + 1u) {
        let slot = lighting.local_dir_intensity[i];
        if slot.w <= 0.001 {
            continue;
        }
        let local_dir_world = safe_normalize(slot.xyz, dominant_dir_world);
        let local_dir = safe_normalize(world_dir_to_local(local_dir_world), dominant_dir);
        let local_ndl = mix(
            saturate(dot(sphere_normal, local_dir)),
            saturate(dot(surface_normal, local_dir)),
            0.30
        );
        local_light += lighting.local_color_radius[i].rgb * slot.w * (local_ndl * 0.38);
    }
    let ambient_light = lighting.ambient.rgb * lighting.ambient.w * 0.20 + vec3<f32>(0.010, 0.012, 0.016);
    let backlight_term = lighting.backlight.rgb * lighting.backlight.w * backlight * 0.05;
    let flash = lighting.flash.rgb * lighting.flash.w;
    let shadow_occlusion = clamp(
        1.0 - (1.0 - macro_ndl) * 0.68 - cavity * 0.18 - edge_shadow * 0.12,
        0.12,
        1.0
    );
    let shadow_tint = mix(
        vec3<f32>(0.34, 0.38, 0.46),
        vec3<f32>(1.06, 1.02, 0.92),
        smoothstep(0.08, 0.88, macro_ndl)
    );
    let view_dir = vec3<f32>(0.0, 0.0, 1.0);
    let half_dir = safe_normalize(dominant_dir + view_dir, view_dir);
    let stone_glint = pow(saturate(dot(surface_normal, half_dir)), 28.0)
        * primary_ndl
        * (0.25 + fine_grain * 0.75);
    let ridge_highlight = dominant_color * ridges * macro_ndl * 0.08;
    let bevel_highlight = dominant_color * edge_contrast * macro_ndl * 0.08;
    let glint_highlight = dominant_color * stone_glint * 0.03;
    let lit_color =
        color * (ambient_light + primary_light + backlight_term + local_light) * shadow_occlusion * shadow_tint
        + flash * 0.16
        + ridge_highlight
        + bevel_highlight
        + glint_highlight
        + vec3<f32>(veins * 0.012);
    return vec4<f32>(clamp(lit_color, vec3<f32>(0.0), vec3<f32>(1.0)), base.a);
}
