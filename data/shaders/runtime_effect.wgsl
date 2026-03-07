#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct RuntimeEffectUniforms {
    identity_a: vec4<f32>,
    params_a: vec4<f32>,
    params_b: vec4<f32>,
    color_a: vec4<f32>,
    color_b: vec4<f32>,
    color_c: vec4<f32>,
}

struct SharedWorldLightingUniforms {
    primary_dir_intensity: vec4<f32>,
    primary_color_elevation: vec4<f32>,
    ambient: vec4<f32>,
    backlight: vec4<f32>,
    flash: vec4<f32>,
    local_dir_intensity: vec4<f32>,
    local_color_radius: vec4<f32>,
}

@group(2) @binding(0) var<uniform> effect: RuntimeEffectUniforms;
@group(2) @binding(1) var<uniform> lighting: SharedWorldLightingUniforms;

const EFFECT_KIND_BILLBOARD_THRUSTER: f32 = 1.0;
const EFFECT_KIND_BILLBOARD_IMPACT_SPARK: f32 = 2.0;
const EFFECT_KIND_BEAM_TRAIL_TRACER: f32 = 10.0;

fn saturate(v: f32) -> f32 {
    return clamp(v, 0.0, 1.0);
}

fn hash21(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn hash11(x: f32) -> f32 {
    return fract(sin(x * 91.3458) * 47453.5453);
}

fn render_thruster(mesh: VertexOutput) -> vec4<f32> {
    let uv = mesh.uv;
    let centered_x = (uv.x - 0.5) * 2.0;
    let along = clamp(uv.y, 0.0, 1.0);

    let t = effect.identity_a.y;
    let thrust_alpha = clamp(effect.identity_a.z, 0.0, 1.0);
    let alpha_scale = max(0.0, effect.identity_a.w);
    let afterburner_alpha = clamp(effect.params_b.x, 0.0, 1.0);

    let falloff = max(0.05, effect.params_a.x);
    let edge_softness = clamp(effect.params_a.y * 0.35, 0.02, 0.98);
    let noise_strength = max(0.0, effect.params_a.z);
    let flicker_hz = max(0.0, effect.params_a.w);

    let along_curve = pow(along, falloff);
    let radius_near = 0.65;
    let radius_far = 0.04;
    let radius_profile = mix(radius_near, radius_far, along_curve);
    let radial = abs(centered_x) / max(0.001, radius_profile);
    let edge = 1.0 - smoothstep(1.0 - edge_softness, 1.0, radial);

    let core = 1.0 - smoothstep(0.0, 0.5, radial);
    let flame_ramp = pow(along, 1.15);
    let longitudinal = pow(1.0 - along, 0.45) * (1.0 - smoothstep(0.9, 1.0, along));

    let flicker_noise = hash21(vec2<f32>(along * 43.0, floor(t * (2.0 + flicker_hz)))) * 2.0 - 1.0;
    let flicker = 1.0 + flicker_noise * noise_strength * (0.25 + thrust_alpha * 0.75);

    let base_rgb = effect.color_a.rgb;
    let hot_rgb = effect.color_b.rgb;
    let afterburner_rgb = effect.color_c.rgb;

    let thermal_rgb = mix(base_rgb, hot_rgb, clamp(flame_ramp * (0.65 + thrust_alpha * 0.35), 0.0, 1.0));
    let final_rgb = mix(thermal_rgb, afterburner_rgb, afterburner_alpha);

    let intensity = (0.25 + 0.75 * core) * (0.45 + 0.55 * thrust_alpha) * longitudinal;
    let alpha = clamp(edge * intensity * flicker * alpha_scale, 0.0, 1.0);
    let ambient_tint = lighting.ambient.rgb * lighting.ambient.w;
    let backlight_tint = lighting.backlight.rgb * lighting.backlight.w * (0.25 + along * 0.45);
    let flash_tint = lighting.flash.rgb * lighting.flash.w * 0.25;
    let local_tint = lighting.local_color_radius.rgb * lighting.local_dir_intensity.w * 0.2;
    let scene_tint = ambient_tint + backlight_tint + flash_tint + local_tint;
    let lit_rgb = final_rgb + scene_tint * 0.35;

    return vec4<f32>(lit_rgb, alpha);
}

fn render_impact_spark(mesh: VertexOutput) -> vec4<f32> {
    let uv = mesh.uv * 2.0 - 1.0;
    let r = length(uv);
    let age = clamp(effect.identity_a.y, 0.0, 1.0);
    let life = 1.0 - age;
    let intensity = max(effect.identity_a.z, 0.0);
    let alpha = clamp(effect.identity_a.w, 0.0, 1.0);
    let density = clamp(effect.params_a.x, 0.25, 6.0);

    let core_radius = mix(0.34, 0.05, age);
    let core = smoothstep(core_radius, 0.0, r);

    let angle = atan2(uv.y, uv.x);
    let ray_count = mix(6.0, 14.0, clamp(density / 2.0, 0.0, 1.0));
    let seed = floor((angle + 3.14159265) / (6.2831853 / ray_count));
    let jitter = hash11(seed + floor(age * 29.0));
    let spoke = pow(max(0.0, cos((angle + jitter * 0.5) * ray_count)), 10.0);
    let ray_falloff = smoothstep(0.9, 0.05, r) * smoothstep(0.02, 0.35, r);
    let rays = spoke * ray_falloff * mix(0.6, 1.3, life);

    let ring_center = mix(0.06, 0.56, age);
    let ring_width = mix(0.12, 0.04, age);
    let ring = exp(-pow((r - ring_center) / max(ring_width, 0.001), 2.0));
    let halo = smoothstep(1.1, 0.2, r) * smoothstep(0.0, 0.55, r) * 0.4;

    let energy = (core * 1.7 + rays * 1.1 + ring * 0.9 + halo) * intensity;
    let rgb = effect.color_a.rgb * (0.7 + 0.6 * core) * energy;
    let out_alpha = clamp(energy * alpha * mix(1.0, 0.35, age), 0.0, 1.0);

    return vec4<f32>(rgb, out_alpha);
}

fn render_beam_trail(mesh: VertexOutput) -> vec4<f32> {
    let uv = mesh.uv;
    let centered = uv * 2.0 - 1.0;
    let age = clamp(effect.identity_a.y, 0.0, 1.0);
    let alpha = clamp(effect.identity_a.z, 0.0, 1.0);
    let glow_strength = max(effect.identity_a.w, 0.0);
    let edge_softness = clamp(effect.params_a.x, 0.02, 0.95);
    let noise_strength = max(effect.params_a.y, 0.0);

    let radial = abs(centered.x);
    let core = 1.0 - smoothstep(0.0, 0.24, radial);
    let edge = 1.0 - smoothstep(0.55 - edge_softness * 0.35, 0.55, radial);
    let tip_fade = smoothstep(1.0, 0.72, uv.y) * smoothstep(0.0, 0.08, uv.y);
    let pulse = 0.8 + 0.2 * sin((uv.y * 14.0) - age * 18.0);
    let grain = (hash21(vec2<f32>(floor(uv.y * 32.0), floor(age * 29.0))) * 2.0 - 1.0) * noise_strength;
    let energy = max(core, edge * 0.75) * tip_fade * pulse * (1.0 + grain * 0.12);
    let glow = edge * glow_strength * tip_fade * 0.55;
    let rgb = mix(effect.color_b.rgb, effect.color_a.rgb, clamp(core * 1.2, 0.0, 1.0))
        * (energy + glow);
    let out_alpha = clamp((energy + glow * 0.6) * alpha, 0.0, 1.0);
    return vec4<f32>(rgb, out_alpha);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let kind = effect.identity_a.x;
    if abs(kind - EFFECT_KIND_BILLBOARD_THRUSTER) < 0.5 {
        return render_thruster(mesh);
    }
    if abs(kind - EFFECT_KIND_BILLBOARD_IMPACT_SPARK) < 0.5 {
        return render_impact_spark(mesh);
    }
    if abs(kind - EFFECT_KIND_BEAM_TRAIL_TRACER) < 0.5 {
        return render_beam_trail(mesh);
    }
    discard;
}
