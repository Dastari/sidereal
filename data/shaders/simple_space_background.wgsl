// Rich 2D Space Background for Sidereal Client
// Material2d pipeline with viewport_time + colors uniforms.
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> viewport_time: vec4<f32>;
@group(2) @binding(1) var<uniform> colors: vec4<f32>;
@group(2) @binding(2) var<uniform> motion: vec4<f32>;

fn hash21(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    let px = fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
    let py = fract(sin(dot(p, vec2<f32>(269.5, 183.3))) * 43758.5453);
    return vec2<f32>(px, py);
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

fn fbm4(p_in: vec2<f32>) -> f32 {
    var v = 0.0; var a = 0.5; var p = p_in;
    for (var i = 0; i < 4; i++) {
        v += a * noise(p);
        p = p * 2.03 + vec2<f32>(13.7, 9.2);
        a *= 0.5;
    }
    return v;
}

fn fbm5(p_in: vec2<f32>) -> f32 {
    var v = 0.0; var a = 0.5; var p = p_in;
    for (var i = 0; i < 5; i++) {
        v += a * noise(p);
        p = p * 2.03 + vec2<f32>(13.7, 9.2);
        a *= 0.5;
    }
    return v;
}

fn turbulence4(p_in: vec2<f32>) -> f32 {
    var v = 0.0; var a = 0.5; var p = p_in;
    for (var i = 0; i < 4; i++) {
        v += a * abs(noise(p) * 2.0 - 1.0);
        p = p * 2.03 + vec2<f32>(13.7, 9.2);
        a *= 0.5;
    }
    return v;
}

fn star_tint(seed: f32) -> vec3<f32> {
    let t = fract(seed * 2434.0);
    if t < 0.15 { return vec3<f32>(0.6, 0.75, 1.0); }
    if t < 0.35 { return vec3<f32>(0.9, 0.92, 1.0); }
    if t < 0.55 { return vec3<f32>(1.0, 0.98, 0.9); }
    if t < 0.7  { return vec3<f32>(1.0, 0.85, 0.65); }
    if t < 0.85 { return vec3<f32>(1.0, 0.7, 0.5); }
    return vec3<f32>(1.0, 0.55, 0.4);
}

// Stars with resolution-aware sizing.
// ppu = pixels per UV unit (derived from viewport resolution).
fn stars(uv: vec2<f32>, density: f32, min_px: f32, max_px: f32,
         thresh: f32, time: f32, ppu: f32) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    let cell_px = ppu / density;
    let gv = fract(uv * density) - 0.5;
    let id = floor(uv * density);
    let n = hash21(id);
    if n > thresh { return col; }
    let off = hash22(id) - 0.5;
    let local = gv - off * 0.6;
    let sz_px = mix(min_px, max_px, fract(n * 917.0));
    let sz = sz_px / cell_px;
    let d = length(local);
    let core = smoothstep(sz, sz * 0.1, d);
    let glow = smoothstep(sz * 3.5, 0.0, d) * 0.35;
    let tw = 0.75 + 0.25 * sin(time * (1.5 + fract(n * 73.0) * 3.0) + n * 100.0);
    col = star_tint(n) * (core + glow) * tw;
    return col;
}

fn hero(uv: vec2<f32>, time: f32, ppu: f32) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    let cell_density = 6.0;
    let cell_px = ppu / cell_density;
    let gv = fract(uv * cell_density) - 0.5;
    let id = floor(uv * cell_density);
    let n = hash21(id);
    if n > 0.04 { return col; }
    let off = hash22(id) - 0.5;
    let local = gv - off * 0.5;
    let d = length(local);
    let sz_px = mix(3.0, 6.0, fract(n * 347.0));
    let sz = sz_px / cell_px;
    let glow_r = sz * 5.0;
    if d > glow_r { return col; }

    let core = smoothstep(sz * 0.5, 0.0, d);
    let halo = smoothstep(glow_r, sz, d) * 0.2;

    let angle = atan2(local.y, local.x);
    let flare1 = pow(abs(sin(angle * 2.0)), 16.0) * smoothstep(glow_r * 0.8, 0.0, d);
    let flare2 = pow(abs(cos(angle * 2.0)), 16.0) * smoothstep(glow_r * 0.8, 0.0, d);

    let tw = 0.8 + 0.2 * sin(time * 0.6 + n * 200.0);
    col = star_tint(n) * (core + halo + (flare1 + flare2) * 0.3) * tw;
    return col;
}

fn spiral_galaxies(uv: vec2<f32>) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    let scale = 3.0;
    let gv = fract(uv * scale) - 0.5;
    let id = floor(uv * scale);

    for (var dx = -1; dx <= 1; dx++) {
        for (var dy = -1; dy <= 1; dy++) {
            let offset = vec2<f32>(f32(dx), f32(dy));
            let cell_id = id + offset;
            let h = hash21(cell_id + vec2<f32>(777.0, 333.0));
            if h > 0.04 { continue; }

            let gpos = hash22(cell_id * 1.7) - 0.5;
            let local = gv - offset - gpos * 0.6;
            let dist = length(local);
            let galaxy_size = mix(0.06, 0.14, fract(h * 917.0));
            if dist > galaxy_size { continue; }

            let angle = atan2(local.y, local.x);
            let arms = 2.0 + floor(fract(h * 347.0) * 3.0);
            let spiral = sin(angle * arms + dist * 25.0);
            let nd = dist / galaxy_size;
            let core = exp(-nd * nd * 8.0);
            let disk = exp(-nd * 3.0) * (0.5 + spiral * 0.5);
            let intensity = (core * 0.7 + disk * 0.3);

            let temp = fract(h * 2711.0);
            var gc = vec3<f32>(1.0, 0.92, 0.75);
            if temp < 0.3 { gc = vec3<f32>(0.8, 0.85, 1.0); }
            col += gc * intensity * 0.25;
        }
    }
    return col;
}

fn lightning(uv: vec2<f32>, time: f32, neb: f32) -> vec3<f32> {
    if neb < 0.2 { return vec3<f32>(0.0); }

    let cell_scale = 2.5;
    let gv = fract(uv * cell_scale) - 0.5;
    let id = floor(uv * cell_scale);
    let h = hash21(id + vec2<f32>(13.1, 7.7));
    if h > 0.15 { return vec3<f32>(0.0); }

    let flash_period = 2.5 + fract(h * 73.0) * 4.0;
    let flash_phase = fract(time * 0.3 / flash_period + h * 100.0);
    if flash_phase > 0.08 { return vec3<f32>(0.0); }
    let flash_i = smoothstep(0.0, 0.025, flash_phase) * smoothstep(0.08, 0.04, flash_phase);

    let start = hash22(id * 2.9) - 0.5;
    let end_pt = hash22(id * 4.1) - 0.5;
    let bolt_dir = normalize(end_pt - start);
    let bolt_len = length(end_pt - start);
    let to_start = gv - start;
    let along = dot(to_start, bolt_dir);
    if along < 0.0 || along > bolt_len { return vec3<f32>(0.0); }

    let perp_vec = to_start - bolt_dir * along;
    let noise_off = noise(id * 5.0 + vec2<f32>(along * 8.0, 0.0)) * 0.04;
    let perp_dist = abs(length(perp_vec) - noise_off);
    let bolt = smoothstep(0.012, 0.0, perp_dist);
    let flicker = 0.5 + 0.5 * sin(time * 25.0 + h * 777.0);
    let neb_gate = smoothstep(0.0, 0.35, neb);
    return vec3<f32>(0.5, 0.65, 1.0) * bolt * flash_i * flicker * neb_gate * 0.7;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let res = max(viewport_time.xy, vec2<f32>(1.0, 1.0));
    let time = viewport_time.z;
    let mi = max(colors.a, 0.0001);
    let drift = motion.xy;
    let velocity_xy = motion.zw;
    let velocity_speed = length(velocity_xy);
    let velocity_dir = select(
        vec2<f32>(0.0, 1.0),
        velocity_xy / velocity_speed,
        velocity_speed > 0.0001
    );

    let uv_n = in.uv * 2.0 - 1.0;
    let aspect = res.x / res.y;
    let uv = vec2<f32>(uv_n.x * aspect, uv_n.y);
    let uv_far = uv + drift * 0.12;
    let uv_mid = uv + drift * 0.28;
    let uv_near = uv + drift * 0.5;
    let drift_along = dot(uv_mid, velocity_dir);

    let ppu = res.x / (aspect * 2.0);

    // ---- Nebula ----
    let w1 = vec2<f32>(
        fbm4(uv_far * 0.8 + vec2<f32>(time * 0.01, -time * 0.007)),
        fbm4(uv_far * 0.8 + vec2<f32>(-time * 0.008, time * 0.009))
    );
    let warped = uv_far + (w1 - 0.5) * 1.5;
    let n1 = fbm5(warped * 0.5 + vec2<f32>(time * 0.006, time * 0.004));
    let n2 = fbm4(warped * 0.8 + vec2<f32>(-time * 0.004, time * 0.005));
    let turb = turbulence4(warped * 0.6);
    let neb_raw = n1 * 0.45 + n2 * 0.3 + turb * 0.25;
    let neb = smoothstep(0.15, 0.65, neb_raw);

    let nb = vec3<f32>(0.1, 0.15, 0.45);
    let np = vec3<f32>(0.3, 0.1, 0.5);
    let nt = vec3<f32>(0.08, 0.25, 0.3);
    let nw = vec3<f32>(0.35, 0.15, 0.08);
    let nm = fract(n1 * 3.7 + n2 * 2.1);
    var nc: vec3<f32>;
    if nm < 0.35 {
        nc = mix(nb, np, nm / 0.35);
    } else if nm < 0.65 {
        nc = mix(np, nt, (nm - 0.35) / 0.3);
    } else {
        nc = mix(nt, nw, (nm - 0.65) / 0.35);
    }
    let bright_spots = pow(turb, 2.5) * 1.2;
    nc *= (1.0 + bright_spots);

    // ---- Galaxies ----
    let galaxies = spiral_galaxies(uv_far * 0.7 + vec2<f32>(time * 0.001, 0.0));

    // ---- Milky-way band ----
    let bo = (fbm4(uv_mid * 0.2 + vec2<f32>(0.0, time * 0.003)) - 0.5) * 0.35;
    let band = exp(-abs(uv_mid.y + bo) * 3.0);
    let gd = fbm4(uv_mid * 2.5 + vec2<f32>(time * 0.002, 0.0));
    let galaxy_band = band * (0.3 + 0.7 * gd) * 0.35;

    // ---- Dust ----
    let dn = fbm4(uv_mid * 1.5 + vec2<f32>(time * 0.005, -time * 0.003));
    let dust = smoothstep(0.42, 0.68, dn) * 0.08;

    // ---- Stars ----
    // Increase star sizes and density thresholds for better visibility
    let sf = stars(uv_far + vec2<f32>(time * 0.001, 0.0),
                   25.0, 2.0, 4.0, 0.45, time, ppu);  // denser, larger faint stars
    let sm = stars(uv_mid + vec2<f32>(time * 0.002, -time * 0.001),
                   15.0, 3.0, 5.0, 0.35, time, ppu);  // denser, larger medium stars
    let sn = stars(uv_near + vec2<f32>(time * 0.004, -time * 0.002),
                   8.0, 4.0, 7.0, 0.25, time, ppu);   // denser, larger near stars
    let sh = hero(uv_near + vec2<f32>(time * 0.001, time * 0.0005), time, ppu);

    let smask = 1.0 - neb * 0.3;  // less nebula masking
    let all_s = (sf * 0.6 + sm * 1.0 + sn * 1.5 + sh * 2.0) * smask;  // boost all star layers

    // ---- Lightning ----
    let ltn = lightning(uv_mid + velocity_dir * drift_along * 0.02, time, neb);

    // ---- Composite ----
    // Darker but visible base color
    let base = vec3<f32>(0.02, 0.03, 0.08);
    var col = base;
    
    // Boost nebula significantly for visibility
    col += nc * neb * 2.0;
    
    // Galaxy band
    col += vec3<f32>(0.2, 0.25, 0.35) * galaxy_band;
    col += galaxies * 1.5;
    
    // Dust wisps
    col += vec3<f32>(0.15, 0.12, 0.1) * dust;
    
    // Stars - boost brightness significantly
    col += all_s * 3.0;
    
    // Lightning
    col += ltn * 1.5;

    // Color tinting
    col = mix(col, col * colors.rgb * 4.0, 0.2);

    // Vignette
    let vig = clamp(1.2 - length(uv_n) * 0.25, 0.6, 1.0);
    col *= vig * mi;

    return vec4<f32>(col, 1.0);
}
