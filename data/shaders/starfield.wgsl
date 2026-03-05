#import bevy_sprite::mesh2d_vertex_output::VertexOutput

const MAX_LAYERS: i32 = 8;

@group(2) @binding(0) var<uniform> viewport_time: vec4<f32>;
@group(2) @binding(1) var<uniform> drift_intensity: vec4<f32>;   // .xy = accumulated scroll (Y flipped on CPU for screen); shader aspect-corrects only
@group(2) @binding(2) var<uniform> velocity_dir: vec4<f32>;      // .xy = heading (unit vector), .z = camera zoom scale, .w reserved
@group(2) @binding(3) var<uniform> starfield_params: vec4<f32>;  // .x = density, .y = layer count, .z = initial z offset, .w = alpha
@group(2) @binding(4) var<uniform> starfield_tint: vec4<f32>;    // .rgb = color tint, .w = intensity
@group(2) @binding(5) var<uniform> star_core_params: vec4<f32>;   // .x = size, .y = intensity, .z = alpha, .w reserved
@group(2) @binding(6) var<uniform> star_core_color: vec4<f32>;    // .rgb = star color, .w reserved
@group(2) @binding(7) var<uniform> corona_params: vec4<f32>;      // .x = size, .y = intensity, .z = alpha, .w reserved
@group(2) @binding(8) var<uniform> corona_color: vec4<f32>;       // .rgb = corona color, .w reserved

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
// density_scale: 0 = no stars, 1 = full density (current look).
fn star_layer(uv: vec2<f32>, depth: f32, vel_dir: vec2<f32>, warp: f32, density_scale: f32) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    let gv = fract(uv) - 0.5;
    let id = floor(uv);

    let density = mix(0.52, 0.18, depth);

    for (var y: i32 = -1; y <= 1; y = y + 1) {
        for (var x: i32 = -1; x <= 1; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y));
            let cell_id = id + offset;
            
            if (hash21(cell_id * 1.73) > density * density_scale) {
                continue;
            }

            let pos_hash = hash22(cell_id * 2.13);
            let local = gv - offset - (pos_hash - 0.5);

            let star_size = clamp(star_core_params.x, 0.1, 10.0);
            let star_intensity = max(star_core_params.y, 0.0);
            let star_alpha = clamp(star_core_params.z, 0.0, 1.0);
            let corona_size = clamp(corona_params.x, 0.1, 10.0);
            let corona_intensity = max(corona_params.y, 0.0);
            let corona_alpha = clamp(corona_params.z, 0.0, 1.0);

            let radius = mix(0.045, 0.12, depth * depth) * star_size;
            let elongation = warp * mix(0.45, 1.45, depth);

            let s = star(local, radius, vel_dir, elongation);
            let d = length(local);
            let soft_halo = smoothstep(radius * (2.9 * corona_size), radius * 0.35, d) * (0.28 * corona_alpha);
            let brightness = mix(0.7, 2.0, depth * depth) * (1.0 + warp * depth * 0.55);
            let star_tint = star_core_color.rgb;
            let glow_tint = corona_color.rgb;

            col += ((s * star_tint * star_alpha * star_intensity) + (soft_halo * glow_tint * corona_intensity)) * brightness;
        }
    }
    return col;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let viewport = viewport_time.xy;
    let warp = viewport_time.w;
    let intensity = drift_intensity.z * max(starfield_tint.w, 0.0);
    let user_alpha = drift_intensity.w * clamp(starfield_params.w, 0.0, 1.0);
    
    let aspect = viewport.x / max(viewport.y, 1.0);
    // Aspect-correct travel only (direction handled by CPU Y-flip)
    var travel = drift_intensity.xy;
    travel = vec2<f32>(travel.x * aspect, travel.y);

    let heading = velocity_dir.xy;
    // Bevy orthographic scale grows when zooming out; invert for shader-space zoom response.
    let zoom_scale = 1.0 / max(velocity_dir.z, 0.01);
    let density_scale = clamp(starfield_params.x, 0.0, 1.0);
    let layer_count = clamp(i32(starfield_params.y + 0.5), 1, MAX_LAYERS);
    let initial_z_offset = clamp(starfield_params.z, 0.0, 1.0);
    var vel_dir = vec2<f32>(1.0, 0.0);
    if (length(heading) > 0.01) {
        vel_dir = normalize(vec2<f32>(-heading.x, heading.y));
    }

    var uv = (in.uv - 0.5) * vec2<f32>(aspect, 1.0);

    var col = vec3<f32>(0.0);
    // Scroll factors: far = slow (0.12), near = 1.2. Layer alpha: far 25%, near 100%.
    for (var layer_idx: i32 = 0; layer_idx < MAX_LAYERS; layer_idx = layer_idx + 1) {
        if (layer_idx >= layer_count) {
            break;
        }
        let depth = f32(layer_idx) / max(f32(layer_count - 1), 1.0);
        let effective_depth = depth * (1.0 - initial_z_offset);

        // Depth-dependent zoom response: near layers react more than far layers.
        let layer_zoom = mix(
            1.0 + (zoom_scale - 1.0) * 0.18,
            1.0 + (zoom_scale - 1.0) * 0.92,
            effective_depth
        );
        let scale = mix(55.0, 12.0, pow(effective_depth, 0.7)) / max(layer_zoom, 0.01);
        let scroll_factor = mix(0.12, 1.2, effective_depth);
        let layer_drift = travel * scroll_factor;
        let layer_seed = f32(layer_idx);
        let layer_uv = uv * scale + vec2<f32>(layer_seed * 271.0, layer_seed * 389.0) + layer_drift;
        let layer_alpha = mix(0.36, 0.10, effective_depth);

        col += star_layer(layer_uv, effective_depth, vel_dir, warp, density_scale) * mix(0.9, 1.5, effective_depth) * layer_alpha;

    }

    col = min(col * starfield_tint.rgb, vec3<f32>(1.9));

    let vignette = 1.0 - length(in.uv - 0.5) * 0.29;
    col *= vignette * intensity;

    let luma = dot(col, vec3<f32>(0.2126, 0.7152, 0.0722));
    let star_alpha = min(1.0, luma * 1.55 * user_alpha);

    return vec4<f32>(col, star_alpha);
}
