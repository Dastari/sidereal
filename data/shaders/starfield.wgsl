#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Three parallax layers: far (slow, tiny, dim) -> mid -> close (fast, large, bright). Wrapping via fract().
const NUM_LAYERS: f32 = 3.0;

@group(2) @binding(0) var<uniform> viewport_time: vec4<f32>;
@group(2) @binding(1) var<uniform> drift_intensity: vec4<f32>;   // .xy = accumulated scroll (Y flipped on CPU for screen); shader aspect-corrects only
@group(2) @binding(2) var<uniform> velocity_dir: vec4<f32>;      // .xy = heading (unit vector), .z = magnitude (speed)

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

// depth 0 = far (tiny, dim), 0.5 = mid, 1 = close (large, bright). Far radius modest so far layer reads as distant stars.
fn star_layer(uv: vec2<f32>, depth: f32, vel_dir: vec2<f32>, warp: f32) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    let gv = fract(uv) - 0.5;
    let id = floor(uv);

    let density = mix(0.52, 0.18, depth);

    for (var y: i32 = -1; y <= 1; y = y + 1) {
        for (var x: i32 = -1; x <= 1; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y));
            let cell_id = id + offset;
            
            if (hash21(cell_id * 1.73) > density) {
                continue;
            }

            let pos_hash = hash22(cell_id * 2.13);
            let local = gv - offset - (pos_hash - 0.5);

            let radius = mix(0.045, 0.12, depth * depth);
            let elongation = warp * mix(0.45, 1.45, depth);

            let s = star(local, radius, vel_dir, elongation);
            let brightness = mix(0.7, 2.0, depth * depth) * (1.0 + warp * depth * 0.55);

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
    
    let aspect = viewport.x / max(viewport.y, 1.0);
    // Aspect-correct travel only (direction handled by CPU Y-flip)
    var travel = drift_intensity.xy;
    travel = vec2<f32>(travel.x * aspect, travel.y);

    let heading = velocity_dir.xy;
    var vel_dir = vec2<f32>(1.0, 0.0);
    if (length(heading) > 0.01) {
        vel_dir = normalize(vec2<f32>(-heading.x, heading.y));
    }

    var uv = (in.uv - 0.5) * vec2<f32>(aspect, 1.0);

    var col = vec3<f32>(0.0);
    let inv_layers = 1.0 / NUM_LAYERS;
    var i = 0.0;
    
    // Scroll factors: far = slow (0.12), near = 1.2. Layer alpha: far 25%, mid 50%, near 100%.
    loop {
        if (i >= 1.0) { break; }
        let depth = i;
        
        let scale = mix(55.0, 12.0, pow(depth, 0.7));
        let scroll_factor = mix(0.12, 1.2, depth);
        let layer_drift = travel * scroll_factor;
        let layer_uv = uv * scale + vec2<f32>(i * 271.0, i * 389.0) + layer_drift;
        let layer_alpha = mix(0.25, 1.0, depth);

        col += star_layer(layer_uv, depth, vel_dir, warp) * mix(0.9, 1.5, depth) * layer_alpha;

        i += inv_layers;
    }

    col = min(col, vec3<f32>(1.9));

    let vignette = 1.0 - length(in.uv - 0.5) * 0.29;
    col *= vignette * intensity;

    let luma = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    let star_alpha = min(1.0, luma * 2.85 * user_alpha);

    return vec4<f32>(1.0, 1.0, 1.0, star_alpha);
}