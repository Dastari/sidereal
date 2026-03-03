#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct ThrusterPlumeParams {
    shape_params: vec4<f32>,      // x: falloff, y: edge_softness, z: noise_strength, w: thrust_alpha
    state_params: vec4<f32>,      // x: afterburner_alpha, y: time_s, z: alpha, w: flicker_hz
    base_color: vec4<f32>,        // rgb + alpha intensity
    hot_color: vec4<f32>,         // rgb + alpha intensity
    afterburner_color: vec4<f32>, // rgb + alpha intensity
}

@group(2) @binding(0) var<uniform> plume: ThrusterPlumeParams;

fn hash21(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    let centered_x = (uv.x - 0.5) * 2.0;
    let along = clamp(uv.y, 0.0, 1.0);

    let thrust_alpha = clamp(plume.shape_params.w, 0.0, 1.0);
    let afterburner_alpha = clamp(plume.state_params.x, 0.0, 1.0);
    let alpha_scale = max(0.0, plume.state_params.z);

    let falloff = max(0.05, plume.shape_params.x);
    let edge_softness = clamp(plume.shape_params.y * 0.35, 0.02, 0.98);
    let noise_strength = max(0.0, plume.shape_params.z);
    let t = plume.state_params.y;
    let flicker_hz = max(0.0, plume.state_params.w);

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

    let base_rgb = plume.base_color.rgb;
    let hot_rgb = plume.hot_color.rgb;
    let afterburner_rgb = plume.afterburner_color.rgb;

    let thermal_rgb = mix(base_rgb, hot_rgb, clamp(flame_ramp * (0.65 + thrust_alpha * 0.35), 0.0, 1.0));
    let final_rgb = mix(thermal_rgb, afterburner_rgb, afterburner_alpha);

    let intensity = (0.25 + 0.75 * core) * (0.45 + 0.55 * thrust_alpha) * longitudinal;
    let alpha = clamp(edge * intensity * flicker * alpha_scale, 0.0, 1.0);

    return vec4<f32>(final_rgb, alpha);
}
