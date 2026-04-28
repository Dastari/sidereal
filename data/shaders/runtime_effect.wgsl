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
    metadata: vec4<f32>,
    ambient: vec4<f32>,
    backlight: vec4<f32>,
    flash: vec4<f32>,
    stellar_dir_intensity: array<vec4<f32>, 2>,
    stellar_color_params: array<vec4<f32>, 2>,
    local_dir_intensity: array<vec4<f32>, 8>,
    local_color_radius: array<vec4<f32>, 8>,
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

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (vec2<f32>(3.0) - 2.0 * f);
    let a = hash21(i + vec2<f32>(0.0, 0.0));
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm2(p: vec2<f32>) -> f32 {
    var q = p;
    var amplitude = 0.5;
    var value = 0.0;
    for (var octave = 0; octave < 4; octave = octave + 1) {
        value = value + value_noise(q) * amplitude;
        q = q * 2.03 + vec2<f32>(17.2, 9.7);
        amplitude = amplitude * 0.5;
    }
    return value;
}

fn render_thruster(mesh: VertexOutput) -> vec4<f32> {
    let uv = mesh.uv;
    let centered_x = (uv.x - 0.5) * 2.0;
    let tail_t = clamp(1.0 - uv.y, 0.0, 1.0);

    let t = effect.identity_a.y;
    let thrust_alpha = clamp(effect.identity_a.z, 0.0, 1.0);
    let alpha_scale = max(0.0, effect.identity_a.w);
    let afterburner_alpha = clamp(effect.params_b.x, 0.0, 1.0);

    let falloff = max(0.05, effect.params_a.x);
    let edge_softness = clamp(effect.params_a.y * 0.35, 0.02, 0.98);
    let noise_strength = max(0.0, effect.params_a.z);
    let flicker_hz = max(0.0, effect.params_a.w);

    let nozzle_gate = 1.0 - smoothstep(0.0, 0.035, tail_t);
    let tail_fade = 1.0 - smoothstep(0.76, 1.0, tail_t);
    let stream_fade = smoothstep(0.0, 0.08, tail_t) * tail_fade;
    let bell = sin(tail_t * 3.14159265);
    let expansion = pow(clamp(tail_t, 0.0, 1.0), max(0.12, falloff * 0.72));
    let flare_radius = mix(0.16, 0.72 + afterburner_alpha * 0.12, smoothstep(0.0, 0.48, expansion));
    let tail_pinch = mix(1.0, 0.32, smoothstep(0.62, 1.0, tail_t));
    let radius_profile = max(0.04, flare_radius * tail_pinch + bell * 0.12 * thrust_alpha);

    let flow = vec2<f32>(
        centered_x * (3.4 + thrust_alpha * 1.8),
        tail_t * (8.0 + thrust_alpha * 4.0) - t * (0.8 + flicker_hz * 0.08)
    );
    let coarse_noise = fbm2(flow);
    let fine_noise = fbm2(flow * 2.25 + vec2<f32>(11.0, -7.0));
    let shear = (coarse_noise - 0.5) * noise_strength * (0.18 + tail_t * 0.22);
    let strand_wave = sin(centered_x * 16.0 + tail_t * 21.0 - t * (5.0 + flicker_hz * 0.18));
    let strand_noise = (fine_noise - 0.5) * 2.0;
    let distorted_x = centered_x + shear + strand_wave * strand_noise * noise_strength * 0.04;
    let radial = abs(distorted_x) / radius_profile;

    let edge_width = clamp(edge_softness * 0.22, 0.08, 0.48);
    let outer = (1.0 - smoothstep(0.72, 1.0 + edge_width, radial)) * stream_fade;
    let rim = smoothstep(0.34, 0.86, radial) * (1.0 - smoothstep(0.86, 1.14, radial)) * stream_fade;
    let core_width = mix(0.2, 0.08, smoothstep(0.12, 0.82, tail_t));
    let core = (1.0 - smoothstep(core_width, core_width + 0.22, abs(distorted_x)))
        * (0.35 + 0.65 * tail_fade);
    let nozzle_core = nozzle_gate * (1.0 - smoothstep(0.1, 0.5, abs(centered_x)));

    let diamond_phase = tail_t * mix(5.5, 8.5, thrust_alpha) - t * (0.45 + flicker_hz * 0.015);
    let diamond_wave = pow(abs(sin(diamond_phase * 3.14159265)), 18.0);
    let diamond_window = smoothstep(0.12, 0.26, tail_t) * (1.0 - smoothstep(0.82, 1.0, tail_t));
    let diamond_core = (1.0 - smoothstep(0.08, 0.42, abs(distorted_x)))
        * diamond_wave
        * diamond_window
        * (0.15 + afterburner_alpha * 0.85);

    let flicker_noise = fbm2(vec2<f32>(tail_t * 7.0, t * max(0.1, flicker_hz * 0.32)));
    let flicker = 1.0 + (flicker_noise - 0.5) * noise_strength * (0.28 + thrust_alpha * 0.72);
    let turbulent_cut = smoothstep(0.08, 0.62, coarse_noise + outer * 0.18);

    let base_rgb = effect.color_a.rgb;
    let hot_rgb = effect.color_b.rgb;
    let afterburner_rgb = effect.color_c.rgb;

    let thermal_mix = clamp(core * 0.7 + nozzle_core + diamond_core * 0.8, 0.0, 1.0);
    let plume_rgb = mix(base_rgb * 0.78, hot_rgb * 1.32, thermal_mix);
    let afterburn_rgb = mix(plume_rgb, afterburner_rgb * 1.55, afterburner_alpha * (diamond_core + nozzle_core * 0.5));
    let halo_rgb = base_rgb * (0.28 + rim * 0.42);
    let final_rgb = afterburn_rgb * (outer * 0.78 + core * 1.45 + nozzle_core * 1.8 + diamond_core * 1.65)
        + halo_rgb * outer;

    let intensity = (outer * 0.58 + rim * 0.28 + core * 0.95 + nozzle_core * 0.9 + diamond_core * 0.85)
        * (0.38 + 0.62 * thrust_alpha)
        * turbulent_cut;
    let alpha = clamp(intensity * flicker * alpha_scale, 0.0, 1.0);
    let ambient_tint = lighting.ambient.rgb * lighting.ambient.w;
    let backlight_tint =
        lighting.backlight.rgb * lighting.backlight.w * (0.2 + (1.0 - tail_t) * 0.5);
    let flash_tint = lighting.flash.rgb * lighting.flash.w * 0.25;
    let scene_tint = ambient_tint + backlight_tint + flash_tint;
    let lit_rgb = final_rgb + scene_tint * (0.2 + outer * 0.28);

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
    let domain_scale = max(effect.params_b.x, 1.0);
    let uv = (mesh.uv * 2.0 - 1.0) * domain_scale;
    let r = length(uv);
    let angle = atan2(uv.y, uv.x);
    let age = clamp(effect.identity_a.y, 0.0, 1.0);
    let life = 1.0 - age;
    let intensity = max(effect.identity_a.z, 0.0);
    let alpha = clamp(effect.identity_a.w, 0.0, 1.0);
    let expansion = max(effect.params_a.x, 0.1);
    let noise_strength = max(effect.params_a.y, 0.0);

    let noise = hash21(vec2<f32>(floor((uv.x + 1.2) * 11.0), floor((uv.y + age) * 13.0))) * 2.0 - 1.0;
    let distortion_band = smoothstep(0.12, 0.26, r)
        * smoothstep(0.95, 0.28, r)
        * life;
    let distortion_dir = vec2<f32>(cos(angle), sin(angle));
    let distorted_uv = uv + distortion_dir * distortion_band * noise_strength * 0.16;
    let distorted_r = length(distorted_uv) + noise * noise_strength * 0.07;

    let core_radius = mix(0.32, 0.06, age);
    let core = smoothstep(core_radius, 0.0, distorted_r) * mix(2.2, 0.55, age);

    let shock_radius = mix(0.08, 0.84 * expansion, age);
    let shock_width = mix(0.22, 0.08, age);
    let shock = (1.0 - smoothstep(0.0, shock_width, abs(distorted_r - shock_radius)))
        * mix(1.35, 0.55, age);

    let plume = smoothstep(1.08, 0.16, distorted_r)
        * smoothstep(0.02, 0.64, distorted_r)
        * mix(0.82, 0.28, age);
    let smoke = smoothstep(1.18, 0.32, distorted_r)
        * smoothstep(0.12, 0.92, distorted_r)
        * age
        * 0.46;
    let light = smoothstep(0.62, 0.0, distorted_r) * mix(1.65, 0.72, age);

    let energy = (core + shock * 1.55 + plume * 0.85 + light) * intensity;
    let core_rgb = effect.color_a.rgb * (core * 1.45 + light * 0.95 + plume * 0.18);
    let rim_rgb = effect.color_b.rgb * (shock * 1.9 + plume * 0.72);
    let smoke_rgb = effect.color_c.rgb * smoke;
    let rgb = core_rgb + rim_rgb + smoke_rgb;
    let out_alpha = clamp(
        (energy + smoke * 0.55) * alpha * mix(1.0, 0.34, age),
        0.0,
        1.0
    );

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
    let core = 1.0 - smoothstep(0.0, 0.14, radial);
    let hot_core = 1.0 - smoothstep(0.0, 0.055, radial);
    let sheath = 1.0 - smoothstep(0.16, 0.42 + edge_softness * 0.22, radial);
    let halo = 1.0 - smoothstep(0.32, 1.0, radial);

    let head_fade = 1.0 - smoothstep(0.86, 1.0, uv.y);
    let tail_fade = smoothstep(0.0, 0.16, uv.y);
    let longitudinal = head_fade * tail_fade;
    let stream_fill = smoothstep(0.04, 0.2, uv.y) * (1.0 - smoothstep(0.72, 1.0, uv.y));
    let node_phase = uv.y * 18.0 - age * 22.0;
    let node = pow(0.5 + 0.5 * sin(node_phase), 7.0);
    let axial_pulse = 0.86 + node * 0.32 + 0.12 * sin(uv.y * 43.0 + age * 11.0);
    let grain = (hash21(vec2<f32>(floor(uv.y * 48.0), floor(age * 37.0))) * 2.0 - 1.0) * noise_strength;

    let core_energy = (core * 1.25 + hot_core * 1.65 + sheath * 0.42)
        * longitudinal
        * axial_pulse
        * (1.0 + grain * 0.1);
    let glow = (halo * 0.78 + sheath * 0.35)
        * glow_strength
        * (stream_fill * 0.72 + longitudinal * 0.28)
        * (0.76 + node * 0.24);
    let rgb = effect.color_b.rgb * (glow * 0.78 + sheath * 0.18)
        + effect.color_a.rgb * (core_energy + hot_core * 0.45);
    let out_alpha = clamp((core_energy + glow * 0.68) * alpha, 0.0, 1.0);
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
