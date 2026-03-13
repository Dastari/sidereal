#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

const MAX_SHOCKWAVES: u32 = 8u;

struct ExplosionDistortionSettings {
    metadata: vec4<f32>,
    shockwaves: array<vec4<f32>, 8>,
}

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var<uniform> settings: ExplosionDistortionSettings;

fn shockwave_offset(
    uv: vec2<f32>,
    center_uv: vec2<f32>,
    radius_uv: f32,
    strength: f32,
    aspect: f32,
) -> vec2<f32> {
    let to_center = uv - center_uv;
    let corrected = vec2<f32>(to_center.x * aspect, to_center.y);
    let distance_to_center = length(corrected);
    if distance_to_center <= 0.0001 {
        return vec2<f32>(0.0, 0.0);
    }

    let ring_width = max(radius_uv * 0.28, 0.012);
    let ring = smoothstep(radius_uv + ring_width, radius_uv, distance_to_center)
        * smoothstep(radius_uv - ring_width, radius_uv, distance_to_center);
    let inner_fade = smoothstep(radius_uv * 0.25, radius_uv * 0.9, distance_to_center);
    let normalized_dir = corrected / distance_to_center;
    let offset_strength = ring * inner_fade * strength;
    return vec2<f32>(normalized_dir.x / aspect, normalized_dir.y) * offset_strength;
}

@fragment
fn fragment_main(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let texture_size = vec2<f32>(textureDimensions(screen_texture));
    let safe_height = max(texture_size.y, 1.0);
    let aspect = texture_size.x / safe_height;

    var uv = in.uv;
    let count = u32(settings.metadata.x);
    let clamped_count = min(count, MAX_SHOCKWAVES);
    for (var i: u32 = 0u; i < clamped_count; i = i + 1u) {
        let shockwave = settings.shockwaves[i];
        uv += shockwave_offset(uv, shockwave.xy, shockwave.z, shockwave.w, aspect);
    }

    uv = clamp(uv, vec2<f32>(0.001, 0.001), vec2<f32>(0.999, 0.999));
    let color = textureSample(screen_texture, screen_sampler, uv);
    return vec4<f32>(color.rgb, 1.0);
}
