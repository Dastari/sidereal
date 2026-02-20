// Rich 2D Space Background for Sidereal Client
// Kept compatible with current Material2d bindings.
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> viewport_time: vec4<f32>; // xy = viewport size, z = time
@group(2) @binding(1) var<uniform> colors: vec4<f32>;        // base tint rgb + master intensity a

fn hash21(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p_in: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var p = p_in;
    for (var i = 0; i < 6; i++) {
        v += a * noise(p);
        p = p * 2.03 + vec2<f32>(13.7, 9.2);
        a *= 0.5;
    }
    return v;
}

fn star_layer(uv: vec2<f32>, density: f32, size_min: f32, size_max: f32) -> f32 {
    let grid_scale = density;
    let gv = fract(uv * grid_scale) - 0.5;
    let id = floor(uv * grid_scale);
    let n = hash21(id);
    if n > 0.03 {
        return 0.0;
    }
    let size = mix(size_min, size_max, fract(n * 917.0));
    let d = length(gv);
    let core = smoothstep(size, 0.0, d);
    let glow = smoothstep(size * 3.0, 0.0, d) * 0.25;
    return core + glow;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let resolution = max(viewport_time.xy, vec2<f32>(1.0, 1.0));
    let time = viewport_time.z;
    let master_intensity = max(colors.a, 0.0001);

    let uv_n = in.uv * 2.0 - 1.0;
    let aspect = resolution.x / resolution.y;
    let uv = vec2<f32>(uv_n.x * aspect, uv_n.y);

    // Domain-warped nebula fields.
    let warp = vec2<f32>(
        fbm(uv * 0.35 + vec2<f32>(time * 0.015, -time * 0.01)),
        fbm(uv * 0.35 + vec2<f32>(-time * 0.013, time * 0.012))
    );
    let p = uv + (warp - 0.5) * 1.4;
    let n1 = fbm(p * 0.75 + vec2<f32>(time * 0.01, time * 0.008));
    let n2 = fbm(p * 1.3 + vec2<f32>(-time * 0.006, time * 0.01));
    let nebula = smoothstep(0.25, 0.9, n1 * 0.65 + n2 * 0.35);

    // "Galaxy band" across screen.
    let band = exp(-abs(uv.y + (fbm(uv * 0.25) - 0.5) * 0.35) * 3.2);
    let galaxy = band * (0.25 + 0.75 * fbm(uv * 2.2 + vec2<f32>(time * 0.002, 0.0)));

    // Multi-depth stars.
    let stars_far = star_layer(uv + vec2<f32>(time * 0.002, 0.0), 220.0, 0.010, 0.018) * 0.45;
    let stars_mid = star_layer(uv + vec2<f32>(time * 0.005, -time * 0.002), 120.0, 0.012, 0.024) * 0.7;
    let stars_near = star_layer(uv + vec2<f32>(time * 0.009, -time * 0.004), 80.0, 0.014, 0.032) * 1.0;
    let stars = stars_far + stars_mid + stars_near;

    // Base palette derived from material tint.
    let c0 = vec3<f32>(0.015, 0.025, 0.05);
    let c1 = mix(vec3<f32>(0.05, 0.08, 0.16), colors.rgb * vec3<f32>(0.8, 0.95, 1.1), 0.45);
    let c2 = mix(vec3<f32>(0.2, 0.12, 0.36), colors.rgb * vec3<f32>(1.15, 0.9, 1.3), 0.55);

    var col = mix(c0, c1, nebula * 0.8);
    col = mix(col, c2, nebula * nebula * 0.55);
    col += vec3<f32>(0.22, 0.22, 0.28) * galaxy * 0.35;
    col += vec3<f32>(1.0, 0.97, 0.93) * stars * 0.6;

    // Soft vignette keeps center readable and edges deep.
    let vignette = clamp(1.05 - length(uv_n) * 0.42, 0.35, 1.0);
    col *= vignette * master_intensity;

    return vec4<f32>(col, 1.0);
}
