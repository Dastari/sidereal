#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var image: texture_2d<f32>;
@group(2) @binding(1) var image_sampler: sampler;

const OUTLINE_COLOR: vec3<f32> = vec3<f32>(1.0, 0.92, 0.12);
const ALPHA_EPSILON: f32 = 0.01;

fn sample_alpha(uv: vec2<f32>) -> f32 {
    return textureSample(image, image_sampler, uv).a;
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
        return vec4<f32>(OUTLINE_COLOR, 0.95);
    }

    if base.a <= 0.001 {
        discard;
    }

    return base;
}
