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
const EFFECT_KIND_BILLBOARD_EXPLOSION: f32 = 3.0;
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
    let from_nozzle = clamp(1.0 - uv.y, 0.0, 1.0);

    let t = effect.identity_a.y;
    let thrust_alpha = clamp(effect.identity_a.z, 0.0, 1.0);
    let alpha_scale = max(0.0, effect.identity_a.w);
    let afterburner_alpha = clamp(effect.params_b.x, 0.0, 1.0);

    let falloff = max(0.05, effect.params_a.x);
    let edge_softness = clamp(effect.params_a.y * 0.35, 0.02, 0.98);
    let noise_strength = max(0.0, effect.params_a.z);
    let flicker_hz = max(0.0, effect.params_a.w);

    let along_curve = pow(from_nozzle, falloff);
    let radius_near = mix(0.56, 0.72, thrust_alpha);
    let radius_far = mix(0.08, 0.03, thrust_alpha);
    let radius_profile = mix(radius_near, radius_far, along_curve);
    let radial = abs(centered_x) / max(0.001, radius_profile);
    let edge = 1.0 - smoothstep(1.0 - edge_softness, 1.0, radial);

    let core = 1.0 - smoothstep(0.0, 0.42, radial);
    let sheath = 1.0 - smoothstep(0.25, 1.0, radial);
    let nozzle_heat = pow(1.0 - from_nozzle, 0.28);
    let tail_fade = 1.0 - smoothstep(0.72, 1.0, from_nozzle);
    let longitudinal = nozzle_heat * tail_fade;

    let flicker_noise = hash21(
        vec2<f32>(floor(from_nozzle * 44.0), floor(t * (2.0 + flicker_hz)))
    ) * 2.0 - 1.0;
    let side_noise = hash21(
        vec2<f32>(floor((centered_x + 1.0) * 18.0), floor((t + from_nozzle) * 9.0))
    ) * 2.0 - 1.0;
    let flicker = 1.0 + flicker_noise * noise_strength * (0.25 + thrust_alpha * 0.75);
    let plume_breakup = 1.0 + side_noise * noise_strength * from_nozzle * 0.28;

    let base_rgb = effect.color_a.rgb;
    let hot_rgb = effect.color_b.rgb;
    let afterburner_rgb = effect.color_c.rgb;

    let thermal_rgb = mix(
        base_rgb,
        hot_rgb,
        clamp(nozzle_heat * (0.7 + thrust_alpha * 0.3), 0.0, 1.0)
    );
    let tail_rgb = mix(thermal_rgb, base_rgb, clamp(from_nozzle * 0.72, 0.0, 1.0));
    let final_rgb = mix(tail_rgb, afterburner_rgb, afterburner_alpha * (0.35 + nozzle_heat * 0.65));

    let intensity = (0.28 + core * 0.82 + sheath * 0.24)
        * (0.42 + 0.58 * thrust_alpha)
        * longitudinal
        * plume_breakup;
    let alpha = clamp(edge * intensity * flicker * alpha_scale, 0.0, 1.0);
    let ambient_tint = lighting.ambient.rgb * lighting.ambient.w;
    let backlight_tint =
        lighting.backlight.rgb * lighting.backlight.w * (0.25 + (1.0 - from_nozzle) * 0.45);
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

fn render_explosion(mesh: VertexOutput) -> vec4<f32> {
    let uv = mesh.uv * 2.0 - 1.0;
    let r = length(uv);
    let age = clamp(effect.identity_a.y, 0.0, 1.0);
    let life = 1.0 - age;
    let intensity = max(effect.identity_a.z, 0.0);
    let alpha = clamp(effect.identity_a.w, 0.0, 1.0);
    let expansion = max(effect.params_a.x, 0.1);
    let noise_strength = max(effect.params_a.y, 0.0);

    let noise = hash21(vec2<f32>(floor((uv.x + 1.2) * 11.0), floor((uv.y + age) * 13.0))) * 2.0 - 1.0;
    let warped_r = r + noise * noise_strength * 0.08;

    let core_radius = mix(0.18, 0.04, age);
    let core = exp(-pow(warped_r / max(core_radius, 0.001), 2.0)) * mix(1.8, 0.35, age);

    let ring_radius = mix(0.14, 0.72 * expansion, age);
    let ring_width = mix(0.2, 0.08, age);
    let shock = exp(-pow((warped_r - ring_radius) / max(ring_width, 0.001), 2.0));

    let plume = smoothstep(1.0, 0.12, warped_r)
        * smoothstep(0.0, 0.55, warped_r)
        * mix(0.45, 0.22, age);
    let smoke = smoothstep(1.2, 0.3, warped_r)
        * smoothstep(0.05, 0.88, warped_r)
        * age
        * 0.38;

    let energy = (core + shock * 1.25 + plume) * intensity;
    let core_rgb = effect.color_a.rgb * (core * 1.1 + plume * 0.25);
    let rim_rgb = effect.color_b.rgb * (shock * 1.4 + plume * 0.55);
    let smoke_rgb = effect.color_c.rgb * smoke;
    let rgb = core_rgb + rim_rgb + smoke_rgb;
    let out_alpha = clamp((energy + smoke * 0.45) * alpha * mix(1.0, 0.28, age), 0.0, 1.0);

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
    if abs(kind - EFFECT_KIND_BILLBOARD_EXPLOSION) < 0.5 {
        return render_explosion(mesh);
    }
    if abs(kind - EFFECT_KIND_BEAM_TRAIL_TRACER) < 0.5 {
        return render_beam_trail(mesh);
    }
    discard;
}
