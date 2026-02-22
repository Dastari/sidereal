#import bevy_sprite::mesh2d_vertex_output::VertexOutput

const NUM_LAYERS: f32 = 5.0;
const PI: f32 = 3.14159265359;

@group(2) @binding(0) var<uniform> viewport_time: vec4<f32>;
@group(2) @binding(1) var<uniform> drift_intensity: vec4<f32>;
@group(2) @binding(2) var<uniform> velocity_dir: vec4<f32>;

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

fn star(uv_in: vec2<f32>, radius: f32, drift_dir: vec2<f32>, warp: f32) -> f32 {
    let drift_side = vec2<f32>(-drift_dir.y, drift_dir.x);
    let along = dot(uv_in, drift_dir);
    let across = dot(uv_in, drift_side);

    let streak_len = mix(radius, radius * 18.0, warp * warp);
    let streak_width = mix(radius, radius * 0.15, warp);
    let d = length(vec2<f32>(along / max(streak_len, 0.0001), across / max(streak_width, 0.0001)));
    return smoothstep(1.0, 0.2, d);
}

fn background_star(uv_in: vec2<f32>, radius: f32, star_type: f32) -> f32 {
    let d = length(uv_in);
    
    if star_type < 0.3 {
        return smoothstep(radius, radius * 0.1, d);
    } else if star_type < 0.6 {
        let core = smoothstep(radius, radius * 0.2, d);
        let glow = smoothstep(radius * 2.5, radius * 0.3, d) * 0.4;
        return core + glow;
    } else {
        let core = smoothstep(radius * 0.8, radius * 0.1, d);
        let halo = smoothstep(radius * 3.0, radius * 0.5, d) * 0.25;
        return core + halo;
    }
}

fn twinkle(id: vec2<f32>, time: f32, seed: f32) -> f32 {
    let phase = fract(seed * 7.31) * PI * 2.0;
    let speed = 0.8 + fract(seed * 3.17) * 1.5;
    let base = 0.6 + 0.4 * sin(time * speed + phase);
    let flicker = 0.15 * sin(time * speed * 3.7 + phase * 2.3);
    return clamp(base + flicker, 0.3, 1.0);
}

fn star_color(seed: f32) -> vec3<f32> {
    let temp = fract(seed * 2434.0);
    
    if temp < 0.15 {
        return vec3<f32>(0.9, 0.95, 1.0);
    } else if temp < 0.35 {
        return vec3<f32>(1.0, 0.98, 0.92);
    } else if temp < 0.5 {
        return vec3<f32>(1.0, 0.92, 0.85);
    } else if temp < 0.6 {
        return vec3<f32>(0.85, 0.9, 1.0);
    } else {
        return vec3<f32>(0.95, 0.95, 0.98);
    }
}

fn star_layer(uv: vec2<f32>, drift_dir: vec2<f32>, warp: f32, time: f32, is_background: bool) -> vec3<f32> {
    var col = vec3<f32>(0.0, 0.0, 0.0);

    let gv = fract(uv) - 0.5;
    let id = floor(uv);

    for (var y: i32 = -1; y <= 1; y = y + 1) {
        for (var x: i32 = -1; x <= 1; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y));
            let cell_id = id + offset;
            let n = hash21(cell_id);
            
            var density_threshold = 0.25;
            if is_background {
                density_threshold = 0.18;
            }
            
            let density_gate = fract(n * 911.0);
            if density_gate > density_threshold {
                continue;
            }
            
            let pos_hash = hash22(cell_id * 1.7);
            let local = gv - offset - vec2<f32>(pos_hash.x - 0.5, pos_hash.y - 0.5);
            let size = fract(n * 534.0);
            
            var s: f32;
            var colors: vec3<f32>;
            var brightness: f32;
            
            if is_background {
                let star_type = fract(n * 789.0);
                let radius = mix(0.04, 0.12, size * size);
                s = background_star(local, radius, star_type);
                colors = star_color(n);
                let twinkle_val = twinkle(cell_id, time, n);
                brightness = mix(0.4, 1.0, size) * twinkle_val;
            } else {
                let radius = mix(0.03, 0.08, size * size);
                s = star(local, radius, drift_dir, warp);
                let tint = 0.5 + 0.5 * sin(vec3<f32>(0.35, 0.52, 0.73) * fract(n * 2434.0) * 5.0);
                colors = mix(vec3<f32>(0.74, 0.78, 0.86), vec3<f32>(0.96, 0.97, 1.0), tint);
                let warp_boost = mix(1.0, 2.2, warp * warp);
                brightness = mix(0.3, 0.85, size) * warp_boost;
            }
            
            col += s * brightness * colors;
        }
    }

    return col;
}

fn warp_trail(uv: vec2<f32>, drift_dir: vec2<f32>, warp: f32, time: f32) -> vec3<f32> {
    if warp < 0.1 {
        return vec3<f32>(0.0);
    }
    
    var col = vec3<f32>(0.0);
    let drift_side = vec2<f32>(-drift_dir.y, drift_dir.x);
    
    for (var i: i32 = 0; i < 3; i = i + 1) {
        let fi = f32(i);
        let scale = 80.0 + fi * 40.0;
        let trail_uv = uv * scale + vec2<f32>(fi * 173.0, fi * 91.0);
        let gv = fract(trail_uv) - 0.5;
        let id = floor(trail_uv);
        
        for (var y: i32 = -1; y <= 1; y = y + 1) {
            for (var x: i32 = -1; x <= 1; x = x + 1) {
                let offset = vec2<f32>(f32(x), f32(y));
                let cell_id = id + offset;
                let n = hash21(cell_id);
                
                if fract(n * 567.0) > 0.03 {
                    continue;
                }
                
                let pos_hash = hash22(cell_id * 2.3);
                let local = gv - offset - vec2<f32>(pos_hash.x - 0.5, pos_hash.y - 0.5);
                
                let along = dot(local, drift_dir);
                let across = dot(local, drift_side);
                
                let streak_len = 0.4 * warp;
                let streak_width = 0.003;
                let d = length(vec2<f32>(along / streak_len, across / streak_width));
                let s = smoothstep(1.0, 0.0, d) * warp * 0.6;
                
                let trail_color = vec3<f32>(0.7, 0.85, 1.0);
                col += s * trail_color * (1.0 - fi * 0.25);
            }
        }
    }
    
    return col;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let viewport = viewport_time.xy;
    let time = viewport_time.z;
    let warp = viewport_time.w;
    let drift = drift_intensity.xy;
    let intensity = drift_intensity.z;
    let alpha = drift_intensity.w;
    
    let vel_dir_raw = velocity_dir.xy;
    let vel_speed = velocity_dir.z;

    let aspect = viewport.x / max(viewport.y, 1.0);
    var uv = (in.uv - 0.5) * vec2<f32>(aspect, 1.0);
    let travel = drift;
    uv += travel;
    
    let vel_len = length(vel_dir_raw);
    var warp_dir: vec2<f32>;
    if vel_len > 0.01 {
        warp_dir = vel_dir_raw / vel_len;
    } else {
        warp_dir = vec2<f32>(1.0, 0.0);
    }
    let warp_side = vec2<f32>(-warp_dir.y, warp_dir.x);
    let warp_stretch = mix(1.0, 0.12, warp * warp);

    var col = vec3<f32>(0.0, 0.0, 0.0);
    let inv_layers = 1.0 / NUM_LAYERS;
    var i = 0.0;
    
    loop {
        if i >= 1.0 {
            break;
        }
        let depth = i;
        let is_background = depth < 0.25;
        
        var scale: f32;
        var fade: f32;
        
        if is_background {
            scale = mix(50.0, 35.0, depth * 4.0);
            fade = mix(0.12, 0.18, depth * 4.0);
        } else {
            scale = mix(30.0, 12.0, (depth - 0.25) / 0.75);
            fade = mix(0.15, 0.35, (depth - 0.25) / 0.75);
        }
        
        let layer_drift = travel * mix(0.15, 1.0, depth);
        let layer_uv = uv * scale + vec2<f32>(i * 343.0, i * 127.0) + layer_drift;
        
        var warped_uv: vec2<f32>;
        if is_background {
            warped_uv = layer_uv;
        } else {
            let along = dot(layer_uv, warp_dir);
            let across = dot(layer_uv, warp_side);
            warped_uv = warp_dir * (along * warp_stretch) + warp_side * across;
        }
        
        col += star_layer(warped_uv, warp_dir, warp, time, is_background) * fade;
        i += inv_layers;
    }

    col += warp_trail(uv, warp_dir, warp, time);

    let vignette = 1.0 - length(in.uv - 0.5) * 0.3;
    col *= vignette;
    col *= intensity;
    
    return vec4<f32>(col, alpha);
}
