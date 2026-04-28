#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var image: texture_2d<f32>;
@group(2) @binding(1) var image_sampler: sampler;

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
@group(2) @binding(3) var<uniform> local_rotation: vec4<f32>;

const OUTLINE_COLOR: vec3<f32> = vec3<f32>(1.0, 0.92, 0.12);
const ALPHA_EPSILON: f32 = 0.01;

fn saturate(v: f32) -> f32 {
    return clamp(v, 0.0, 1.0);
}

fn sample_alpha(uv: vec2<f32>) -> f32 {
    return textureSample(image, image_sampler, uv).a;
}

fn safe_normalize(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    if len > 0.0001 {
        return v / len;
    }
    return fallback;
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

fn sprite_surface_normal(uv: vec2<f32>, texel: vec2<f32>) -> vec3<f32> {
    let centered = uv_to_sprite_local_xy(uv);
    let sphere_z = sqrt(max(0.0, 1.0 - dot(centered, centered) * 0.52));
    let alpha_l = sample_alpha(uv - vec2<f32>(texel.x, 0.0));
    let alpha_r = sample_alpha(uv + vec2<f32>(texel.x, 0.0));
    let alpha_up = sample_alpha(uv - vec2<f32>(0.0, texel.y));
    let alpha_down = sample_alpha(uv + vec2<f32>(0.0, texel.y));
    let alpha_gradient = vec2<f32>(alpha_l - alpha_r, alpha_up - alpha_down) * 0.38;
    return safe_normalize(
        vec3<f32>(centered * 0.32 + alpha_gradient, sphere_z),
        vec3<f32>(0.0, 0.0, 1.0)
    );
}

fn lit_sprite_color(base_rgb: vec3<f32>, surface_normal: vec3<f32>) -> vec3<f32> {
    var light = lighting.ambient.rgb * lighting.ambient.w + vec3<f32>(0.018, 0.020, 0.025);

    let stellar_count = min(u32(lighting.metadata.x), 2u);
    for (var i: u32 = 0u; i < stellar_count; i = i + 1u) {
        let slot = lighting.stellar_dir_intensity[i];
        if slot.w <= 0.001 {
            continue;
        }
        let dir = safe_normalize(
            world_dir_to_local(safe_normalize(slot.xyz, vec3<f32>(0.0, 0.0, 1.0))),
            vec3<f32>(0.0, 0.0, 1.0)
        );
        let wrapped = saturate((dot(surface_normal, dir) + 0.28) / 1.28);
        light += lighting.stellar_color_params[i].rgb * slot.w * wrapped;
    }

    let local_count = min(u32(lighting.metadata.y), 8u);
    for (var i: u32 = 0u; i < local_count; i = i + 1u) {
        let slot = lighting.local_dir_intensity[i];
        if slot.w <= 0.001 {
            continue;
        }
        let dir = safe_normalize(
            world_dir_to_local(safe_normalize(slot.xyz, vec3<f32>(0.0, 0.0, 1.0))),
            vec3<f32>(0.0, 0.0, 1.0)
        );
        let wrapped = saturate((dot(surface_normal, dir) + 0.42) / 1.42);
        light += lighting.local_color_radius[i].rgb * slot.w * wrapped * 0.42;
    }

    light += lighting.backlight.rgb * lighting.backlight.w * pow(1.0 - saturate(surface_normal.z), 2.0) * 0.16;
    let flash = lighting.flash.rgb * lighting.flash.w * 0.22;
    return base_rgb * light + flash;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    let base = textureSample(image, image_sampler, uv);

    let dims = vec2<f32>(textureDimensions(image));
    let texel = vec2<f32>(1.0, 1.0) / max(dims, vec2<f32>(1.0, 1.0));

    let a_n = sample_alpha(uv + vec2<f32>(0.0, texel.y));
    let a_s = sample_alpha(uv + vec2<f32>(0.0, -texel.y));
    let a_e = sample_alpha(uv + vec2<f32>(texel.x, 0.0));
    let a_w = sample_alpha(uv + vec2<f32>(-texel.x, 0.0));
    let a_ne = sample_alpha(uv + vec2<f32>(texel.x, texel.y));
    let a_nw = sample_alpha(uv + vec2<f32>(-texel.x, texel.y));
    let a_se = sample_alpha(uv + vec2<f32>(texel.x, -texel.y));
    let a_sw = sample_alpha(uv + vec2<f32>(-texel.x, -texel.y));

    let neighbor_max = max(
        max(max(a_n, a_s), max(a_e, a_w)),
        max(max(a_ne, a_nw), max(a_se, a_sw))
    );

    if base.a < ALPHA_EPSILON && neighbor_max >= ALPHA_EPSILON {
        let outline_normal = sprite_surface_normal(uv, texel);
        return vec4<f32>(lit_sprite_color(OUTLINE_COLOR, outline_normal), 0.95);
    }

    if base.a <= 0.001 {
        discard;
    }

    let normal = sprite_surface_normal(uv, texel);
    let lit = lit_sprite_color(base.rgb, normal);
    return vec4<f32>(saturate(lit.r), saturate(lit.g), saturate(lit.b), base.a);
}
