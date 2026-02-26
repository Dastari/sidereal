#import bevy_sprite::mesh2d_vertex_output::VertexOutput

const NUM_LAYERS: f32 = 5.0;

@group(2) @binding(0) var<uniform> viewport_time: vec4<f32>;
@group(2) @binding(1) var<uniform> drift_intensity: vec4<f32>;   // .xy = per-frame UV drift from ship velocity
@group(2) @binding(2) var<uniform> velocity_dir: vec4<f32>;      // .xy = normalized dir, .z = raw ship speed

fn hash21(p_in: vec2<f32>) -> f32 {
    var p = fract(p_in * vec2<f32>(123.23, 456.34));
    p += dot(p, p + 45.45);
    return fract(p.x * p.y);
}

fn hash22(p_in: vec2<f32>) -> vec2<f32> {
    var p = fract(p_in * vec2<f32>(123.23, 456.34));
    p += dot(p, p + 45.45);
    return fract(vec2<f32>(p.x * p.y, p.y * p.x * 1.5));
}

fn star(local: vec2<f32>, radius: f32, dir: vec2<f32>, elongation: f32) -> f32 {
    if (elongation < 0.08) {
        let d = length(local);
        return smoothstep(radius, radius * 0.35, d);
    }
    
    let side = vec2<f32>(-dir.y, dir.x);
    let along = dot(local, dir);
    let across = dot(local, side);
    
    let streak_len = radius * mix(1.0, 8.0, elongation);
    let streak_width = radius * mix(1.0, 0.25, elongation);
    
    let d = length(vec2<f32>(along / max(streak_len, 0.0001), across / max(streak_width, 0.0001)));
    return smoothstep(1.0, 0.20, d);
}

fn star_layer(uv: vec2<f32>, depth: f32, vel_dir: vec2<f32>, warp: f32) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    let gv = fract(uv) - 0.5;
    let id = floor(uv);

    let density = mix(0.48, 0.21, depth);

    for (var y: i32 = -1; y <= 1; y = y + 1) {
        for (var x: i32 = -1; x <= 1; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y));
            let cell_id = id + offset;
            
            if (hash21(cell_id * 1.73) > density) {
                continue;
            }

            let pos_hash = hash22(cell_id * 2.13);
            let local = gv - offset - (pos_hash - 0.5);

            let radius = mix(0.015, 0.039, depth * depth);
            let elongation = warp * mix(0.45, 1.45, depth);

            let s = star(local, radius, vel_dir, elongation);
            let brightness = mix(0.92, 1.78, depth * depth) * (1.0 + warp * depth * 0.55);

            col += s * brightness;
        }
    }
    return col;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let viewport = viewport_time.xy;
    let warp = viewport_time.w;
    let intensity = drift_intensity.z;
    let user_alpha = drift_intensity.w;
    
    // === SHIP-VELOCITY PARALLAX (this is the fix) ===
    let travel = drift_intensity.xy;          // pre-scaled per-frame UV drift from controlled ship
    let vel_raw = velocity_dir.xy;
    let vel_len = length(vel_raw);
    var vel_dir = vec2<f32>(1.0, 0.0);
    if (vel_len > 0.01) {
        vel_dir = vel_raw / vel_len;
    }

    let aspect = viewport.x / max(viewport.y, 1.0);
    var uv = (in.uv - 0.5) * vec2<f32>(aspect, 1.0);

    var col = vec3<f32>(0.0);
    let inv_layers = 1.0 / NUM_LAYERS;
    var i = 0.0;
    
    loop {
        if (i >= 1.0) { break; }
        let depth = i;
        
        let scale = mix(72.0, 14.0, pow(depth, 0.72));
        let parallax_mult = mix(0.18, 6.8, depth * depth);   // stronger near-layer whip for velocity feel
        
        let layer_drift = travel * parallax_mult;
        let layer_uv = uv * scale + vec2<f32>(i * 271.0, i * 389.0) + layer_drift;

        col += star_layer(layer_uv, depth, vel_dir, warp) * mix(0.96, 1.48, depth);

        i += inv_layers;
    }

    col = min(col, vec3<f32>(1.9));

    let vignette = 1.0 - length(in.uv - 0.5) * 0.29;
    col *= vignette * intensity;

    // Transparent background
    let luma = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    let star_alpha = clamp(luma * 2.85, 0.0, 1.0);

    return vec4<f32>(col, star_alpha * user_alpha);
}