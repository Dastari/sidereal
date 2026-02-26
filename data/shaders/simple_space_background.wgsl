#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> viewport_time: vec4<f32>;
@group(2) @binding(1) var<uniform> colors: vec4<f32>;
@group(2) @binding(2) var<uniform> motion: vec4<f32>;

fn hash21(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

fn star_layer(uv: vec2<f32>, density: f32, threshold: f32, size: f32, time: f32) -> vec3<f32> {
    let id = floor(uv * density);
    let gv = fract(uv * density) - 0.5;
    let n = hash21(id);
    if n > threshold {
        return vec3<f32>(0.0);
    }
    let local = gv - (vec2<f32>(hash21(id + vec2<f32>(3.1, 7.9)), hash21(id + vec2<f32>(5.7, 11.3))) - 0.5) * 0.7;
    let d = length(local);
    let twinkle = 0.75 + 0.25 * sin(time * (1.4 + fract(n * 31.0) * 2.5) + n * 40.0);
    let core = smoothstep(size, size * 0.15, d);
    let glow = smoothstep(size * 2.6, 0.0, d) * 0.25;
    let tint = vec3<f32>(0.82, 0.88, 1.0) + vec3<f32>(0.18, 0.08, -0.05) * fract(n * 91.0);
    return (core + glow) * twinkle * tint;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let res = max(viewport_time.xy, vec2<f32>(1.0, 1.0));
    let time = viewport_time.z;
    let intensity = max(colors.a, 0.0001);

    let uv_n = in.uv * 2.0 - 1.0;
    let aspect = res.x / res.y;
    let uv = vec2<f32>(uv_n.x * aspect, uv_n.y);

    let drift = motion.xy;
    let velocity = motion.zw;
    let speed = length(velocity);

    let far_uv = uv + drift * 0.12 + vec2<f32>(time * 0.0008, -time * 0.0005);
    let mid_uv = uv + drift * 0.28 + vec2<f32>(time * 0.0015, -time * 0.0010);
    let near_uv = uv + drift * 0.5 + vec2<f32>(time * 0.0025, -time * 0.0017);

    let base_top = vec3<f32>(0.012, 0.02, 0.055);
    let base_bottom = vec3<f32>(0.02, 0.028, 0.08);
    let base = mix(base_bottom, base_top, clamp((uv_n.y + 1.0) * 0.5, 0.0, 1.0));

    let haze = 0.06 * smoothstep(-0.1, 0.9, 0.5 + 0.5 * sin(uv.x * 1.5 + time * 0.03));

    let stars_far = star_layer(far_uv, 20.0, 0.30, 0.08, time) * 0.45;
    let stars_mid = star_layer(mid_uv, 11.0, 0.22, 0.10, time) * 0.8;
    let stars_near = star_layer(near_uv, 7.0, 0.16, 0.13, time) * 1.25;

    let speed_glow = clamp(speed * 0.0009, 0.0, 0.12);
    var col = base + vec3<f32>(haze) + stars_far + stars_mid + stars_near;
    col += vec3<f32>(0.08, 0.1, 0.16) * speed_glow;
    col = mix(col, col * colors.rgb * 4.0, 0.2);

    let vignette = clamp(1.15 - length(uv_n) * 0.22, 0.72, 1.0);
    col *= vignette * intensity;
    return vec4<f32>(col, 1.0);
}
