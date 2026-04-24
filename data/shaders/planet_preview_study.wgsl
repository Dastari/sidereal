struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var<uniform> time: f32;
@group(0) @binding(1) var<uniform> surface_family: f32;
@group(0) @binding(2) var<uniform> rotation_speed: f32;
@group(0) @binding(3) var<uniform> planet_radius: f32;
@group(0) @binding(4) var<uniform> roughness: f32;
@group(0) @binding(5) var<uniform> height_scale: f32;
@group(0) @binding(6) var<uniform> ambient_strength: f32;
@group(0) @binding(7) var<uniform> specular_strength: f32;
@group(0) @binding(8) var<uniform> crater_density: f32;
@group(0) @binding(9) var<uniform> atmosphere_strength: f32;
@group(0) @binding(10) var<uniform> band_strength: f32;
@group(0) @binding(11) var<uniform> glow_strength: f32;
@group(0) @binding(12) var<uniform> light_direction: vec3<f32>;
@group(0) @binding(13) var<uniform> surface_color_deep: vec3<f32>;
@group(0) @binding(14) var<uniform> surface_color_mid: vec3<f32>;
@group(0) @binding(15) var<uniform> surface_color_high: vec3<f32>;
@group(0) @binding(16) var<uniform> accent_color: vec3<f32>;
@group(0) @binding(17) var<uniform> atmosphere_color: vec3<f32>;
@group(0) @binding(18) var<uniform> emissive_color: vec3<f32>;

const PI: f32 = 3.14159265359;
const PREVIEW_ASPECT_RATIO: f32 = 1.5;

fn saturate(v: f32) -> f32 {
    return clamp(v, 0.0, 1.0);
}

fn hash11(p: f32) -> f32 {
    return fract(sin(p * 127.1) * 43758.5453123);
}

fn hash21(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    let q = vec2<f32>(
        dot(p, vec2<f32>(127.1, 311.7)),
        dot(p, vec2<f32>(269.5, 183.3)),
    );
    return fract(sin(q) * 43758.5453123);
}

fn hash31(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3<f32>(127.1, 311.7, 74.7))) * 43758.5453123);
}

fn noise2(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let local = fract(p);
    let weights = local * local * (3.0 - 2.0 * local);

    let a = hash21(cell);
    let b = hash21(cell + vec2<f32>(1.0, 0.0));
    let c = hash21(cell + vec2<f32>(0.0, 1.0));
    let d = hash21(cell + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, weights.x), mix(c, d, weights.x), weights.y);
}

fn noise3(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let local = fract(p);
    let weights = local * local * (3.0 - 2.0 * local);

    let n000 = hash31(cell);
    let n100 = hash31(cell + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash31(cell + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash31(cell + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash31(cell + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash31(cell + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash31(cell + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash31(cell + vec3<f32>(1.0, 1.0, 1.0));

    let nx00 = mix(n000, n100, weights.x);
    let nx10 = mix(n010, n110, weights.x);
    let nx01 = mix(n001, n101, weights.x);
    let nx11 = mix(n011, n111, weights.x);
    let nxy0 = mix(nx00, nx10, weights.y);
    let nxy1 = mix(nx01, nx11, weights.y);
    return mix(nxy0, nxy1, weights.z);
}

fn fbm2(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    for (var octave: i32 = 0; octave < 4; octave = octave + 1) {
        value += noise2(p * frequency) * amplitude;
        frequency *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

fn fbm3(p: vec3<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    for (var octave: i32 = 0; octave < 4; octave = octave + 1) {
        value += noise3(p * frequency) * amplitude;
        frequency *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

fn ridged_fbm3(p: vec3<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    for (var octave: i32 = 0; octave < 4; octave = octave + 1) {
        let n = noise3(p * frequency);
        value += (1.0 - abs(n * 2.0 - 1.0)) * amplitude;
        frequency *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

fn rotate_y(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(c * v.x + s * v.z, v.y, -s * v.x + c * v.z);
}

fn sphere_uv(normal: vec3<f32>) -> vec2<f32> {
    let longitude = atan2(normal.x, normal.z);
    let latitude = asin(clamp(normal.y, -1.0, 1.0));
    return vec2<f32>(longitude / (2.0 * PI) + 0.5, latitude / PI + 0.5);
}

fn preview_background(uv: vec2<f32>) -> vec3<f32> {
    let vignette_uv = uv * vec2<f32>(PREVIEW_ASPECT_RATIO, 1.0);
    let vignette = 1.0 - saturate(length(vignette_uv - vec2<f32>(0.75, 0.5)) * 0.9);
    let nebula = fbm2(uv * vec2<f32>(3.0, 2.0) + vec2<f32>(time * 0.01, 0.0));
    let stars = smoothstep(0.988, 1.0, hash21(floor(uv * vec2<f32>(180.0, 120.0))));
    return vec3<f32>(0.025, 0.035, 0.055)
        + vec3<f32>(0.04, 0.02, 0.07) * nebula * 0.35
        + vec3<f32>(0.16, 0.1, 0.05) * vignette * 0.1
        + vec3<f32>(1.0, 0.95, 0.9) * stars * 0.6;
}

fn crater_shape(uv: vec2<f32>, density: f32) -> f32 {
    let scale = mix(8.0, 20.0, density);
    let tiled = vec2<f32>(uv.x * scale * 1.8, uv.y * scale);
    let cell = floor(tiled);
    let local = fract(tiled);

    var rim = 0.0;
    var basin = 0.0;
    for (var y: i32 = -1; y <= 1; y = y + 1) {
        for (var x: i32 = -1; x <= 1; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y));
            let sample_cell = cell + offset;
            let jitter = hash22(sample_cell);
            let center = offset + 0.18 + jitter * 0.64;
            let radius = mix(0.08, 0.24, hash21(sample_cell + vec2<f32>(13.0, 29.0)));
            let dist = length(local - center);
            let crater_basin = 1.0 - smoothstep(radius * 0.12, radius, dist);
            let crater_rim = smoothstep(radius * 0.72, radius * 0.96, dist)
                * (1.0 - smoothstep(radius * 0.96, radius * 1.24, dist));
            basin = max(basin, crater_basin);
            rim = max(rim, crater_rim);
        }
    }

    return rim * 0.9 - basin * 0.38;
}

fn rocky_height(normal: vec3<f32>, is_mars: bool) -> f32 {
    let terrain_macro = ridged_fbm3(normal * mix(2.4, 4.8, roughness));
    let detail = fbm3(normal.zxy * mix(8.0, 18.0, roughness));
    let crater = crater_shape(sphere_uv(normal), crater_density);
    let dust = fbm3(normal.yzx * 5.6 + vec3<f32>(0.0, time * 0.012, 0.0));
    let mars_bias = select(0.0, dust * 0.18 + normal.y * 0.06, is_mars);
    return (terrain_macro * 0.72 + detail * 0.28 + crater * 0.42 + mars_bias) * height_scale;
}

fn gas_giant_bands(normal: vec3<f32>) -> f32 {
    let latitude = normal.y * 0.5 + 0.5;
    let flow = fbm3(vec3<f32>(normal.x * 4.0, latitude * 8.0 + time * 0.04, normal.z * 4.0));
    let bands = sin((latitude * mix(8.0, 20.0, band_strength) + flow * 0.5) * PI * 2.0);
    return bands * 0.5 + 0.5;
}

fn gas_giant_storm(normal: vec3<f32>) -> f32 {
    let warped = vec3<f32>(
        normal.x * 4.0 + time * 0.05,
        normal.y * 2.0,
        normal.z * 4.0 - time * 0.03,
    );
    let storm_noise = fbm3(warped * 2.0);
    return smoothstep(0.72, 0.92, storm_noise) * band_strength;
}

fn star_surface(normal: vec3<f32>) -> vec3<f32> {
    let convection = fbm3(normal * 7.0 + vec3<f32>(time * 0.08, 0.0, 0.0));
    let filaments = ridged_fbm3(normal.yzx * 12.0 - vec3<f32>(time * 0.12));
    let flare = pow(saturate(normal.y * 0.5 + 0.5), 3.0) * 0.2;
    var color = mix(surface_color_mid, surface_color_high, smoothstep(0.25, 0.9, convection));
    color += accent_color * filaments * 0.28;
    color += emissive_color * (0.28 + filaments * glow_strength * 0.62 + flare);
    return color;
}

fn sample_height(normal: vec3<f32>) -> f32 {
    if (surface_family < 0.5) {
        return rocky_height(normal, false);
    }
    if (surface_family < 1.5) {
        return rocky_height(normal, true);
    }
    if (surface_family < 2.5) {
        return (gas_giant_bands(normal) - 0.5) * height_scale * 0.14;
    }
    return 0.0;
}

fn perturbed_normal(surface_normal: vec3<f32>) -> vec3<f32> {
    if (surface_family >= 3.5) {
        return surface_normal;
    }

    var tangent = vec3<f32>(surface_normal.z, 0.0, -surface_normal.x);
    if (length(tangent) < 0.0001) {
        tangent = vec3<f32>(1.0, 0.0, 0.0);
    }
    tangent = normalize(tangent);
    let bitangent = normalize(cross(surface_normal, tangent));
    let eps = mix(0.004, 0.012, roughness);
    let h = sample_height(surface_normal);
    let ht = sample_height(normalize(surface_normal + tangent * eps));
    let hb = sample_height(normalize(surface_normal + bitangent * eps));
    let dh_t = (ht - h) / eps;
    let dh_b = (hb - h) / eps;
    return normalize(surface_normal - tangent * dh_t * 1.2 - bitangent * dh_b * 1.2);
}

fn rocky_color(height: f32, crater: f32, is_mars: bool) -> vec3<f32> {
    let low = smoothstep(-0.18 * height_scale, 0.12 * height_scale, height);
    let high = smoothstep(0.12 * height_scale, 0.55 * height_scale, height);
    var color = mix(surface_color_deep, surface_color_mid, low);
    color = mix(color, surface_color_high, high);

    if (is_mars) {
        let dust = smoothstep(0.42, 0.88, fbm2(vec2<f32>(height * 14.0 + time * 0.03, crater * 9.0)));
        color = mix(color, accent_color, dust * 0.28);
    } else {
        color = mix(color, accent_color, saturate(crater) * 0.18);
    }

    return color;
}

fn gas_giant_color(normal: vec3<f32>) -> vec3<f32> {
    let bands = gas_giant_bands(normal);
    let storms = gas_giant_storm(normal);
    let polar = smoothstep(0.55, 0.95, abs(normal.y));
    var color = mix(surface_color_deep, surface_color_mid, bands);
    color = mix(color, surface_color_high, polar * 0.45 + (1.0 - bands) * 0.18);
    color += accent_color * storms * 0.52;
    return color;
}

fn rocky_branch_color(normal: vec3<f32>, is_mars: bool) -> vec3<f32> {
    let height = rocky_height(normal, is_mars);
    let crater = crater_shape(sphere_uv(normal), crater_density);
    return rocky_color(height, crater, is_mars);
}

fn gamma_encode(color: vec3<f32>) -> vec3<f32> {
    return pow(max(color, vec3<f32>(0.0)), vec3<f32>(0.45454545));
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    let background = preview_background(uv);
    let centered = vec2<f32>(
        (uv.x - 0.5) * PREVIEW_ASPECT_RATIO * 2.0,
        (0.5 - uv.y) * 2.0,
    );
    let dist = length(centered);

    let star_branch = surface_family >= 2.5 && surface_family < 3.5;
    let stellar_branch = surface_family >= 3.5;
    let atmosphere_extent = planet_radius + atmosphere_strength * select(0.45, 0.75, stellar_branch);

    if (dist > atmosphere_extent) {
        return vec4<f32>(gamma_encode(background), 1.0);
    }

    let light = normalize(light_direction);
    let view_dir = vec3<f32>(0.0, 0.0, 1.0);
    let disc_uv = centered / max(planet_radius, 0.0001);
    let disc_len = length(disc_uv);
    let disc_mask = 1.0 - smoothstep(1.0, 1.02, disc_len);
    let sphere_normal = normalize(vec3<f32>(disc_uv, sqrt(max(0.0, 1.0 - disc_len * disc_len))));
    let rotated_normal = rotate_y(sphere_normal, time * rotation_speed);
    let shaded_normal = perturbed_normal(rotated_normal);
    let wrapped_light = saturate((dot(shaded_normal, light) + 0.28) / 1.28);
    let half_vec = normalize(light + view_dir);
    let specular = pow(saturate(dot(shaded_normal, half_vec)), mix(8.0, 36.0, specular_strength))
        * specular_strength;
    let fresnel = pow(1.0 - saturate(dot(sphere_normal, view_dir)), mix(1.8, 5.0, atmosphere_strength));

    var surface_color = surface_color_mid;
    var lit_color = background;
    if (surface_family < 0.5) {
        surface_color = rocky_branch_color(rotated_normal, false);
        lit_color = surface_color * (ambient_strength + wrapped_light * 1.12);
        lit_color += vec3<f32>(specular * 0.18);
    } else if (surface_family < 1.5) {
        surface_color = rocky_branch_color(rotated_normal, true);
        lit_color = surface_color * (ambient_strength + wrapped_light * 1.04);
        lit_color += accent_color * specular * 0.12;
    } else if (surface_family < 2.5) {
        surface_color = gas_giant_color(rotated_normal);
        lit_color = surface_color * (ambient_strength * 1.15 + wrapped_light * 0.98);
        lit_color += surface_color_high * specular * 0.2;
    } else {
        surface_color = star_surface(rotated_normal);
        let pulse = 0.92 + sin(time * 0.7 + hash11(rotated_normal.x * 13.0)) * 0.04;
        lit_color = surface_color * pulse;
    }

    var atmosphere = vec3<f32>(0.0);
    if (stellar_branch) {
        let outer = 1.0 - smoothstep(planet_radius, atmosphere_extent, dist);
        let corona = pow(outer, 1.8) * (0.35 + glow_strength * 0.8);
        atmosphere = atmosphere_color * corona + emissive_color * corona * 0.9;
    } else if (star_branch || surface_family < 2.5) {
        atmosphere = atmosphere_color * fresnel * atmosphere_strength * (0.25 + wrapped_light * 0.75);
        if (surface_family < 0.5) {
            atmosphere *= 0.22;
        } else if (surface_family < 1.5) {
            atmosphere *= 0.65;
        }
    }

    let composed = mix(background + atmosphere, lit_color + atmosphere, disc_mask);
    return vec4<f32>(gamma_encode(composed), 1.0);
}
