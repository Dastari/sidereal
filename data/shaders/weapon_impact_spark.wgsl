#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> spark_params: vec4<f32>; // x=age_norm, y=intensity, z=ray_density, w=alpha
@group(2) @binding(1) var<uniform> spark_color: vec4<f32>;  // rgb tint, a reserved

fn hash11(x: f32) -> f32 {
    return fract(sin(x * 91.3458) * 47453.5453);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv * 2.0 - 1.0;
    let r = length(uv);
    let age = clamp(spark_params.x, 0.0, 1.0);
    let life = 1.0 - age;
    let intensity = max(spark_params.y, 0.0);
    let density = clamp(spark_params.z, 0.25, 6.0);
    let alpha = clamp(spark_params.w, 0.0, 1.0);

    // Bright core that quickly collapses with age.
    let core_radius = mix(0.34, 0.05, age);
    let core = smoothstep(core_radius, 0.0, r);

    // Angular radial rays with slight temporal jitter.
    let angle = atan2(uv.y, uv.x);
    let ray_count = mix(6.0, 14.0, clamp(density / 2.0, 0.0, 1.0));
    let seed = floor((angle + 3.14159265) / (6.2831853 / ray_count));
    let jitter = hash11(seed + floor(age * 29.0));
    let spoke = pow(max(0.0, cos((angle + jitter * 0.5) * ray_count)), 10.0);
    let ray_falloff = smoothstep(0.9, 0.05, r) * smoothstep(0.02, 0.35, r);
    let rays = spoke * ray_falloff * mix(0.6, 1.3, life);

    // Expanding shock ring and hot halo.
    let ring_center = mix(0.06, 0.56, age);
    let ring_width = mix(0.12, 0.04, age);
    let ring = exp(-pow((r - ring_center) / max(ring_width, 0.001), 2.0));
    let halo = smoothstep(1.1, 0.2, r) * smoothstep(0.0, 0.55, r) * 0.4;

    let energy = (core * 1.7 + rays * 1.1 + ring * 0.9 + halo) * intensity;
    let rgb = spark_color.rgb * (0.7 + 0.6 * core) * energy;
    let out_alpha = clamp(energy * alpha * mix(1.0, 0.35, age), 0.0, 1.0);

    return vec4<f32>(rgb, out_alpha);
}
