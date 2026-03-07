// Asteroid 2D Sprite Shader
//
// Converted from the prior 3D/PBR mesh shader to the current 2D sprite material
// pipeline used by StreamedSpriteShaderMaterial.
//
// Bindings match sprite material contract:
// @group(2) @binding(0) texture_2d
// @group(2) @binding(1) sampler

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var image: texture_2d<f32>;
@group(2) @binding(1) var image_sampler: sampler;
struct SharedWorldLightingUniforms {
    primary_dir_intensity: vec4<f32>,
    primary_color_elevation: vec4<f32>,
    ambient: vec4<f32>,
    backlight: vec4<f32>,
    flash: vec4<f32>,
    local_dir_intensity: vec4<f32>,
    local_color_radius: vec4<f32>,
}

@group(2) @binding(2) var<uniform> lighting: SharedWorldLightingUniforms;

const PI: f32 = 3.14159265359;

fn saturate(v: f32) -> f32 {
    return clamp(v, 0.0, 1.0);
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
    let uv = mesh.uv;
    let base = textureSample(image, image_sampler, uv);

    if base.a <= 0.001 {
        discard;
    }

    // Radial falloff keeps center fuller and edges rockier.
    let centered = uv * 2.0 - vec2<f32>(1.0, 1.0);
    let r = clamp(length(centered), 0.0, 1.0);
    let body_falloff = smoothstep(1.0, 0.2, 1.0 - r);

    let grain = fbm2d(uv * 18.0 + vec2<f32>(2.1, 7.4));
    let craters = crater_mask(uv);
    let veins = vein_mask(uv) * smoothstep(0.2, 0.9, grain);

    // Rock base tint: slightly warm/cool variation from noise.
    let rock_tint = mix(
        vec3<f32>(0.28, 0.23, 0.19),
        vec3<f32>(0.48, 0.42, 0.36),
        grain
    );

    // Mineral tint injected along veins.
    let mineral_tint = mix(
        vec3<f32>(0.75, 0.48, 0.21),
        vec3<f32>(0.25, 0.62, 0.86),
        fbm2d(uv * 7.0 + vec2<f32>(19.0, 5.0))
    );

    var color = base.rgb;

    // Re-color texture toward rocky palette but preserve source detail.
    color = mix(color, color * rock_tint, 0.65);

    // Crater bowls darken; rims slightly brighten.
    color *= 1.0 - craters * 0.45;
    let rim_highlight = smoothstep(0.2, 0.9, craters) * 0.08;
    color += rim_highlight;

    // Veins + tiny glow pockets.
    color = mix(color, mineral_tint, veins * 0.55);
    let gem = smoothstep(0.78, 0.93, fbm2d(uv * 34.0 + vec2<f32>(5.5, 14.3))) * veins;
    color += mineral_tint * gem * 0.18;

    // Gentle edge darkening for spherical read.
    color *= mix(0.72, 1.0, body_falloff);

    // Preserve source alpha masking.
    let sphere_normal = normalize(vec3<f32>(
        centered.x,
        centered.y,
        sqrt(max(0.0, 1.0 - dot(centered, centered)))
    ));
    let primary_dir = normalize(lighting.primary_dir_intensity.xyz);
    let primary_ndl = saturate(dot(sphere_normal, primary_dir));
    let wrap = saturate(primary_ndl * 0.78 + 0.22);
    let backlight = pow(saturate(dot(sphere_normal, -primary_dir)), 1.8);
    let primary_light = lighting.primary_color_elevation.rgb
        * lighting.primary_dir_intensity.w
        * wrap;
    let local_ndl = saturate(dot(sphere_normal, normalize(lighting.local_dir_intensity.xyz)));
    let local_light = lighting.local_color_radius.rgb
        * lighting.local_dir_intensity.w
        * (0.22 + local_ndl * 0.78);
    let ambient_light = lighting.ambient.rgb * lighting.ambient.w;
    let backlight_term = lighting.backlight.rgb * lighting.backlight.w * backlight;
    let flash = lighting.flash.rgb * lighting.flash.w;
    let lit_color = color * (ambient_light + primary_light + backlight_term + local_light)
        + flash * 0.18
        + veins * 0.05;
    return vec4<f32>(clamp(lit_color, vec3<f32>(0.0), vec3<f32>(1.0)), base.a);
}
