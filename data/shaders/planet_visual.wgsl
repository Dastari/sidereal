#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct PlanetBodyUniforms {
    identity_a: vec4<f32>,
    identity_b: vec4<f32>,
    feature_flags_a: vec4<f32>,
    feature_flags_b: vec4<f32>,
    pass_flags_a: vec4<f32>,
    lighting_a: vec4<f32>,
    lighting_b: vec4<f32>,
    surface_a: vec4<f32>,
    surface_b: vec4<f32>,
    surface_c: vec4<f32>,
    surface_d: vec4<f32>,
    clouds_a: vec4<f32>,
    atmosphere_a: vec4<f32>,
    emissive_a: vec4<f32>,
    sun_dir_a: vec4<f32>,
    world_light_primary_dir_intensity: vec4<f32>,
    world_light_primary_color_elevation: vec4<f32>,
    world_light_ambient: vec4<f32>,
    world_light_backlight: vec4<f32>,
    world_light_flash: vec4<f32>,
    world_light_local_dir_intensity: vec4<f32>,
    world_light_local_color_radius: vec4<f32>,
    color_primary: vec4<f32>,
    color_secondary: vec4<f32>,
    color_tertiary: vec4<f32>,
    color_atmosphere: vec4<f32>,
    color_clouds: vec4<f32>,
    color_night_lights: vec4<f32>,
    color_emissive: vec4<f32>,
}

@group(2) @binding(0) var<uniform> params: PlanetBodyUniforms;

const PI: f32 = 3.14159265359;

fn saturate(v: f32) -> f32 {
    return clamp(v, 0.0, 1.0);
}

fn apply_saturation(color: vec3<f32>, saturation: f32) -> vec3<f32> {
    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    return mix(vec3<f32>(luminance), color, saturation);
}

fn apply_contrast(color: vec3<f32>, contrast: f32) -> vec3<f32> {
    return (color - vec3<f32>(0.5)) * contrast + vec3<f32>(0.5);
}

fn tone_map(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((color * (a * color + vec3<f32>(b))) / (color * (c * color + vec3<f32>(d)) + vec3<f32>(e)), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn rand4(p: vec4<f32>) -> f32 {
    return fract(sin(p.x * 1234.0 + p.y * 2345.0 + p.z * 3456.0 + p.w * 4567.0) * 5678.0);
}

fn smoothnoise4(p: vec4<f32>) -> f32 {
    let e = vec2<f32>(0.0, 1.0);
    let i = floor(p);
    var f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(
            mix(
                mix(rand4(i + vec4<f32>(e.x, e.x, e.x, e.x)), rand4(i + vec4<f32>(e.y, e.x, e.x, e.x)), f.x),
                mix(rand4(i + vec4<f32>(e.x, e.y, e.x, e.x)), rand4(i + vec4<f32>(e.y, e.y, e.x, e.x)), f.x),
                f.y
            ),
            mix(
                mix(rand4(i + vec4<f32>(e.x, e.x, e.y, e.x)), rand4(i + vec4<f32>(e.y, e.x, e.y, e.x)), f.x),
                mix(rand4(i + vec4<f32>(e.x, e.y, e.y, e.x)), rand4(i + vec4<f32>(e.y, e.y, e.y, e.x)), f.x),
                f.y
            ),
            f.z
        ),
        mix(
            mix(
                mix(rand4(i + vec4<f32>(e.x, e.x, e.x, e.y)), rand4(i + vec4<f32>(e.y, e.x, e.x, e.y)), f.x),
                mix(rand4(i + vec4<f32>(e.x, e.y, e.x, e.y)), rand4(i + vec4<f32>(e.y, e.y, e.x, e.y)), f.x),
                f.y
            ),
            mix(
                mix(rand4(i + vec4<f32>(e.x, e.x, e.y, e.y)), rand4(i + vec4<f32>(e.y, e.x, e.y, e.y)), f.x),
                mix(rand4(i + vec4<f32>(e.x, e.y, e.y, e.y)), rand4(i + vec4<f32>(e.y, e.y, e.y, e.y)), f.x),
                f.y
            ),
            f.z
        ),
        f.w
    );
}

fn rotate_x(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(v.x, c * v.y - s * v.z, s * v.y + c * v.z);
}

fn rotate_y(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(c * v.x + s * v.z, v.y, -s * v.x + c * v.z);
}

fn simplex_noise3(p: vec3<f32>) -> f32 {
    let s = vec3<f32>(7.0, 157.0, 113.0);
    let ip = floor(p);
    var fp = fract(p);
    fp = fp * fp * (3.0 - 2.0 * fp);

    let h = dot(ip, s);
    let n000 = fract(sin(h) * 43758.5453123);
    let n100 = fract(sin(h + s.x) * 43758.5453123);
    let n010 = fract(sin(h + s.y) * 43758.5453123);
    let n110 = fract(sin(h + s.x + s.y) * 43758.5453123);
    let n001 = fract(sin(h + s.z) * 43758.5453123);
    let n101 = fract(sin(h + s.x + s.z) * 43758.5453123);
    let n011 = fract(sin(h + s.y + s.z) * 43758.5453123);
    let n111 = fract(sin(h + s.x + s.y + s.z) * 43758.5453123);

    let n00 = mix(n000, n100, fp.x);
    let n01 = mix(n001, n101, fp.x);
    let n10 = mix(n010, n110, fp.x);
    let n11 = mix(n011, n111, fp.x);
    let n0 = mix(n00, n10, fp.y);
    let n1 = mix(n01, n11, fp.y);
    return mix(n0, n1, fp.z);
}

fn fbm3(p: vec3<f32>, octaves: i32, lacunarity: f32, gain: f32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var i = 0;
    loop {
        if i >= octaves {
            break;
        }
        value += simplex_noise3(p * frequency) * amplitude;
        frequency *= lacunarity;
        amplitude *= gain;
        i = i + 1;
    }
    return value;
}

fn fbm4_time(p: vec3<f32>, time_phase: f32, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var i = 0;
    loop {
        if i >= octaves {
            break;
        }
        let t = cos(time_phase * (0.35 + f32(i) * 0.17)) * (18.0 + f32(i) * 11.0);
        value += amplitude * smoothnoise4(vec4<f32>(p * frequency, t));
        frequency *= 2.0;
        amplitude *= 0.5;
        i = i + 1;
    }
    return value;
}

fn sphere_uv(normal: vec3<f32>) -> vec2<f32> {
    let longitude = atan2(normal.x, normal.z);
    let latitude = asin(clamp(normal.y, -1.0, 1.0));
    return vec2<f32>(longitude / (2.0 * PI) + 0.5, latitude / PI + 0.5);
}

fn make_view_sphere_normal(disc_uv: vec2<f32>) -> vec3<f32> {
    let z = sqrt(max(0.0, 1.0 - dot(disc_uv, disc_uv)));
    return normalize(vec3<f32>(disc_uv.x, disc_uv.y, z));
}

fn surface_point_from_billboard(disc_uv: vec2<f32>) -> vec3<f32> {
    let view_n = make_view_sphere_normal(disc_uv);
    let axial_tilt = 0.26;
    let spin_angle = params.identity_a.w * params.identity_b.x + params.identity_a.z * 0.173;
    return rotate_y(rotate_x(view_n, axial_tilt), spin_angle);
}

fn continent_mask(sphere_p: vec3<f32>) -> f32 {
    let continent_warp = vec3<f32>(
        fbm3(sphere_p.yzx * 1.4, 3, 2.0, 0.54),
        fbm3(sphere_p.zxy * 1.6, 3, 2.0, 0.54),
        fbm3(sphere_p.xyz * 1.2, 3, 2.0, 0.54)
    ) - vec3<f32>(0.5, 0.5, 0.5);
    let warped = normalize(sphere_p + continent_warp * 0.16);
    let broad = fbm3(warped * mix(0.55, 2.1, 1.0 - params.surface_b.x), 4, 2.0, 0.52);
    let detail = fbm3(warped * 4.6, 2, 2.1, 0.55);
    let shelves = fbm3(warped * 2.2, 2, 2.0, 0.5);
    return saturate(broad * 0.76 + detail * 0.1 + shelves * 0.14);
}

fn micro_relief(sphere_p: vec3<f32>, detail_level: f32) -> f32 {
    let hi_freq = mix(14.0, 52.0, detail_level);
    let micro = fbm3(
        sphere_p * hi_freq + vec3<f32>(params.identity_a.z * 0.011),
        3,
        2.35,
        0.48
    );
    let ridged = 1.0 - abs(fbm3(
        sphere_p.zxy * (hi_freq * 0.72) - vec3<f32>(params.identity_a.z * 0.007),
        2,
        2.6,
        0.5
    ) * 2.0 - 1.0);
    return micro * 0.65 + ridged * 0.35;
}

fn ridge_relief(sphere_p: vec3<f32>) -> f32 {
    let ridge_a = 1.0 - abs(fbm3(sphere_p * 6.8, 4, 2.15, 0.52) * 2.0 - 1.0);
    let ridge_b = 1.0 - abs(fbm3(sphere_p.zxy * 12.0, 3, 2.25, 0.48) * 2.0 - 1.0);
    return ridge_a * 0.62 + ridge_b * 0.38;
}

fn terran_land_factor(sphere_p: vec3<f32>, height: f32) -> f32 {
    let continents = continent_mask(sphere_p);
    let land_shape = continents + height * 0.12;
    return smoothstep(params.surface_b.y - 0.02, params.surface_b.y + 0.015, land_shape);
}

fn crater_mask(sphere_p: vec3<f32>) -> f32 {
    if params.feature_flags_a.y < 0.5 {
        return 0.0;
    }
    let crater_field = fbm3(sphere_p * (6.0 + params.surface_d.x * 12.0), 4, 2.35, 0.5);
    let micro = fbm3(sphere_p * 18.0, 2, 2.1, 0.55);
    let threshold = 1.0 - params.surface_c.w * 0.9;
    return smoothstep(threshold - 0.08, threshold + 0.04, crater_field + micro * 0.15);
}

fn lava_river_mask(sphere_p: vec3<f32>) -> f32 {
    let flow = fbm3(sphere_p * 7.2 + vec3<f32>(params.identity_a.w * 0.06, 0.0, 0.0), 4, 2.2, 0.52);
    let veins = fbm3(sphere_p * 15.0 - vec3<f32>(params.identity_a.w * 0.11, 0.0, 0.0), 3, 2.4, 0.48);
    let cutoff = mix(0.22, 0.06, saturate(params.surface_d.y));
    return 1.0 - smoothstep(cutoff, cutoff + 0.09, abs(flow - veins * 0.72));
}

fn planet_height(sphere_p: vec3<f32>, planet_type: f32) -> f32 {
    if params.feature_flags_a.x < 0.5 {
        return 0.0;
    }
    let detail_level = saturate(params.lighting_a.z);
    let octaves = i32(clamp(params.surface_c.x, 1.0, 8.0));
    let macro_warp = vec3<f32>(
        fbm3(sphere_p.yzx * mix(1.4, 3.6, detail_level), 3, 2.0, 0.55),
        fbm3(sphere_p.zxy * mix(1.2, 3.2, detail_level), 3, 2.0, 0.55),
        fbm3(sphere_p.xyz * mix(1.6, 4.0, detail_level), 3, 2.0, 0.55)
    ) - vec3<f32>(0.5, 0.5, 0.5);
    let warped = normalize(sphere_p + macro_warp * mix(0.05, 0.24, detail_level));
    let terrain = fbm3(
        warped * mix(1.8, 7.4, detail_level),
        octaves,
        max(params.surface_c.y, 1.1),
        clamp(params.surface_c.z, 0.1, 0.95)
    );
    let continents = continent_mask(warped);
    let micro = micro_relief(warped, detail_level);
    let ridges = ridge_relief(warped);
    if planet_type < 0.5 {
        let land_factor = terran_land_factor(warped, terrain);
        let ocean_factor = 1.0 - land_factor;
        let highland_mask = smoothstep(0.18, 0.76, terrain + continents * 0.1);
        let land_macro = terrain * mix(0.2, 0.42, params.surface_b.z) + continents * 0.28;
        let ocean_basin = terrain * 0.03 + continents * 0.02;
        let macro_height = mix(ocean_basin, land_macro, land_factor);
        let ridge_term = ridges * highland_mask * mix(0.08, 0.22, params.surface_b.z) * land_factor;
        let micro_term = micro
            * mix(0.002, 0.05, detail_level)
            * mix(0.12, 1.0, land_factor)
            * (0.2 + highland_mask * 0.8);
        let shelf_term = micro * ocean_factor * 0.006 * (1.0 - smoothstep(0.0, 0.22, abs(continents - params.surface_b.y)));
        return macro_height + ridge_term + micro_term + shelf_term;
    }
    if planet_type < 1.5 {
        return terrain * 0.42 + fbm3(warped * 11.0, 3, 2.2, 0.52) * 0.18 + micro * mix(0.03, 0.12, detail_level);
    }
    if planet_type < 2.5 {
        return terrain * 0.36 + lava_river_mask(warped) * 0.22 + micro * mix(0.04, 0.16, detail_level);
    }
    if planet_type < 3.5 {
        return terrain * 0.24 + fbm3(warped * 9.2, 3, 2.1, 0.5) * params.surface_d.z * 0.18 + micro * mix(0.02, 0.08, detail_level);
    }
    if planet_type < 4.5 {
        let time_phase = params.identity_a.w * params.atmosphere_a.z * 0.18 + params.identity_a.z * 6.28318;
        let band_noise = fbm4_time(vec3<f32>(warped.y * max(params.clouds_a.x, 1.0), warped.x * 0.7, warped.z * 0.7), time_phase, 4);
        let turbulent = fbm4_time(warped * 2.8, time_phase * 1.4 + 3.7, 3);
        let bands = band_noise * 2.0 - 1.0;
        return bands * 0.08 + turbulent * params.surface_d.w * 0.14 + micro * mix(0.008, 0.03, detail_level);
    }
    return terrain * 0.18 - crater_mask(warped) * 0.2 + micro * mix(0.02, 0.1, detail_level);
}

fn planet_surface_color(sphere_p: vec3<f32>, height: f32, light_term: f32, planet_type: f32) -> vec3<f32> {
    let primary = params.color_primary.rgb;
    let secondary = params.color_secondary.rgb;
    let tertiary = params.color_tertiary.rgb;

    if planet_type < 0.5 {
        let continents = continent_mask(sphere_p);
        let land_shape = continents + height * 0.12;
        let coastline = smoothstep(params.surface_b.y - 0.02, params.surface_b.y + 0.015, land_shape);
        let shallows = 1.0 - smoothstep(0.018, 0.09, abs(land_shape - params.surface_b.y));
        let mountain_mask = smoothstep(0.2, 0.64, height + params.surface_b.z * 0.24);
        let humidity = fbm3(sphere_p * 6.4, 3, 2.0, 0.5);
        let ice_lat = smoothstep(1.0 - params.surface_d.z, 1.0, abs(sphere_p.y));
        let ice_noise = fbm3(sphere_p * 10.0, 3, 2.0, 0.55);
        let polar_ice = saturate(ice_lat * 0.84 + ice_noise * 0.18 - 0.08);
        let ocean_deep = tertiary * vec3<f32>(0.58, 0.78, 1.08);
        let ocean_shallow = mix(ocean_deep, params.color_atmosphere.rgb * 0.45 + tertiary * vec3<f32>(0.72, 0.9, 1.0), 0.62);
        let land_low = mix(primary * 0.76, primary * 1.02, humidity * 0.42 + 0.2);
        let land_high = mix(secondary * 0.74, secondary * 1.02, mountain_mask);
        let ridges = ridge_relief(sphere_p);
        var surface = mix(ocean_deep, ocean_shallow, shallows);
        surface = mix(surface, land_low, coastline);
        surface = mix(surface, land_high, coastline * mountain_mask);
        surface = mix(surface, land_high * vec3<f32>(0.78, 0.82, 0.88), coastline * mountain_mask * ridges * 0.52);
        surface = mix(surface, vec3<f32>(0.92, 0.96, 1.0), polar_ice * 0.82);
        return surface;
    }
    if planet_type < 1.5 {
        let dunes = fbm3(sphere_p * 7.0, 4, 2.0, 0.55);
        let mesas = fbm3(sphere_p * 15.0, 3, 2.3, 0.5);
        var surface = mix(primary, secondary, dunes);
        surface = mix(surface, tertiary, smoothstep(0.74, 0.92, mesas));
        return surface;
    }
    if planet_type < 2.5 {
        let lava = lava_river_mask(sphere_p);
        let cooled = fbm3(sphere_p * 4.2, 4, 2.0, 0.55);
        var surface = mix(primary, secondary, cooled * 0.34);
        surface = mix(surface, params.color_emissive.rgb, lava * (0.64 + params.color_emissive.a * 0.5));
        surface += params.color_emissive.rgb * params.color_emissive.a * lava * (0.22 + (1.0 - light_term) * 0.42);
        return surface;
    }
    if planet_type < 3.5 {
        let ice = smoothstep(0.38, 0.82, sphere_p.y * params.surface_d.z + height);
        let fracture = fbm3(sphere_p * 12.0, 3, 2.2, 0.55);
        var surface = mix(secondary, primary, ice);
        surface = mix(surface, tertiary, smoothstep(0.76, 0.94, fracture) * 0.34);
        return surface;
    }
    if planet_type < 4.5 {
        let time_phase = params.identity_a.w * params.atmosphere_a.z * 0.18 + params.identity_a.z * 6.28318;
        let flow = vec3<f32>(sphere_p.y * max(params.clouds_a.x, 1.0), sphere_p.x * 0.7, sphere_p.z * 0.7);
        let band_noise = fbm4_time(flow, time_phase, 4);
        let band_noise_b = fbm4_time(flow * 1.8 + vec3<f32>(1.7, -0.8, 2.1), time_phase * 1.37 + 5.2, 3);
        let storm_core = fbm4_time(sphere_p * 3.1 + vec3<f32>(0.0, height * 0.18, 0.0), time_phase * 1.8 + 9.4, 4);
        let storm_swirl = fbm4_time(sphere_p.zxy * 5.8, time_phase * 2.25 + 2.7, 3);
        let bands = saturate((band_noise * 0.72 + band_noise_b * 0.28) * 1.18 - 0.08);
        let storm = saturate(storm_core * 0.82 + storm_swirl * 0.26 - (1.0 - params.surface_d.w) * 0.92);
        var surface = mix(primary, secondary, bands);
        surface = mix(surface, tertiary, storm * params.clouds_a.y);
        surface = mix(surface, secondary * 0.82 + primary * 0.18, smoothstep(0.42, 0.82, bands) * 0.22);
        return surface;
    }
    let moon_noise = fbm3(sphere_p * 4.0, 4, 2.0, 0.55);
    let craters = crater_mask(sphere_p);
    var surface = mix(primary, secondary, moon_noise);
    surface = mix(surface, tertiary, craters * 0.52);
    return surface;
}

fn terran_surface_masks(sphere_p: vec3<f32>, height: f32) -> vec4<f32> {
    let continents = continent_mask(sphere_p);
    let land_shape = continents + height * 0.12;
    let coastline = smoothstep(params.surface_b.y - 0.02, params.surface_b.y + 0.015, land_shape);
    let shallows = 1.0 - smoothstep(0.018, 0.09, abs(land_shape - params.surface_b.y));
    let mountain_mask = smoothstep(0.2, 0.64, height + params.surface_b.z * 0.24);
    return vec4<f32>(continents, coastline, shallows, mountain_mask);
}

fn evolving_weather_field(sphere_p: vec3<f32>, time_s: f32) -> vec3<f32> {
    let macro_warp = vec3<f32>(
        fbm3(sphere_p.yzx * 1.4 + vec3<f32>(time_s * 0.012, -time_s * 0.009, 0.0), 3, 2.0, 0.55),
        fbm3(sphere_p.zxy * 1.2 + vec3<f32>(-time_s * 0.01, 0.0, time_s * 0.008), 3, 2.0, 0.55),
        fbm3(sphere_p.xyz * 1.6 + vec3<f32>(0.0, time_s * 0.007, -time_s * 0.011), 3, 2.0, 0.55)
    ) - vec3<f32>(0.5, 0.5, 0.5);
    let jet = vec3<f32>(time_s * 0.06, sin(sphere_p.y * PI) * time_s * 0.018, -time_s * 0.025);
    let shear = vec3<f32>(sphere_p.y * 0.42, sphere_p.z * 0.25, sphere_p.x * 0.3);
    return sphere_p + macro_warp * (0.2 + params.surface_a.z * 0.25) + jet + shear;
}

fn terran_cloud_density(sphere_p: vec3<f32>) -> f32 {
    let time_s = params.identity_a.w * params.atmosphere_a.z;
    let weather_p = evolving_weather_field(sphere_p, time_s);
    let macro_shape = fbm3(weather_p * 1.35, 4, 2.0, 0.56);
    let billow = fbm3(weather_p * (2.2 + params.atmosphere_a.y * 1.8), 5, 2.0, 0.54);
    let detail = fbm3(weather_p * 8.5 + vec3<f32>(macro_shape * 1.3), 3, 2.3, 0.5);
    let anvils = fbm3(weather_p.zxy * 5.8 - vec3<f32>(time_s * 0.08, 0.0, 0.0), 3, 2.1, 0.52);
    var density = saturate(macro_shape * 0.42 + billow * 0.58 + detail * 0.2 + anvils * 0.14);
    let threshold = 1.0 - params.atmosphere_a.x * 0.58;
    density *= smoothstep(threshold - 0.08, threshold + 0.04, density);
    let soft = smoothstep(0.24, 0.84, density);
    let puffs = smoothstep(0.54, 0.92, density + detail * 0.2);
    return soft * mix(0.78, 1.0, puffs) * params.color_clouds.a;
}

fn gas_cloud_density(sphere_p: vec3<f32>) -> f32 {
    let time_s = params.identity_a.w * params.atmosphere_a.z;
    let weather_p = evolving_weather_field(sphere_p, time_s * 0.65);
    let bands = sin((weather_p.y + fbm3(weather_p * 4.0, 3, 2.0, 0.55) * 0.12) * max(params.clouds_a.x, 1.0) * PI);
    let storms = fbm3(weather_p * 7.5 + vec3<f32>(time_s * 0.12, 0.0, 0.0), 4, 2.2, 0.52);
    let turbulence = fbm3(weather_p * 14.0 - vec3<f32>(time_s * 0.18, 0.0, 0.0), 2, 2.3, 0.55);
    var density = saturate((bands * 0.5 + 0.5) * 0.5 + storms * 0.52 + turbulence * 0.16);
    let threshold = 1.0 - params.atmosphere_a.x * 0.52;
    density *= smoothstep(threshold - 0.1, threshold + 0.05, density);
    return smoothstep(0.3, 0.9, density) * params.color_clouds.a;
}

fn cloud_shell_point_from_billboard(disc_uv: vec2<f32>, shell_radius_norm: f32) -> vec3<f32> {
    let z = sqrt(max(0.0, shell_radius_norm * shell_radius_norm - dot(disc_uv, disc_uv)));
    let view_n = normalize(vec3<f32>(disc_uv.x, disc_uv.y, z));
    let spin_angle = params.identity_a.w * params.identity_b.x * 1.22 + params.identity_a.z * 0.191;
    return rotate_y(rotate_x(view_n, 0.26), spin_angle);
}

fn hash12(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let n00 = hash12(i + vec2<f32>(0.0, 0.0));
    let n10 = hash12(i + vec2<f32>(1.0, 0.0));
    let n01 = hash12(i + vec2<f32>(0.0, 1.0));
    let n11 = hash12(i + vec2<f32>(1.0, 1.0));
    return mix(mix(n00, n10, u.x), mix(n01, n11, u.x), u.y);
}

fn fbm2(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    for (var i = 0; i < 4; i = i + 1) {
        value += noise2(p * frequency) * amplitude;
        frequency *= 2.07;
        amplitude *= 0.54;
    }
    return value;
}

fn planet_cloud_shadow(sphere_p: vec3<f32>, sun_dir: vec3<f32>, planet_type: f32) -> f32 {
    if params.feature_flags_a.z < 0.5 {
        return 0.0;
    }
    let offset_point = normalize(sphere_p + sun_dir * 0.08);
    let density = select(gas_cloud_density(offset_point), terran_cloud_density(offset_point), planet_type < 4.0);
    let terminator_bias = 1.0 - saturate(dot(sphere_p, sun_dir) * 0.5 + 0.5);
    return density * params.surface_a.z * (0.45 + terminator_bias * 0.55);
}

fn atmosphere_response(dist: f32, radius: f32, atmosphere_radius: f32, light_term: f32) -> vec3<f32> {
    let edge = saturate(1.0 - (dist - radius) / max(atmosphere_radius - radius, 0.0001));
    let outer_haze = pow(edge, 3.0) * 0.2;
    let mid_glow = pow(edge, 8.0) * (0.18 + light_term * 0.1);
    let inner_rim = pow(edge, 22.0) * (0.14 + light_term * 0.22);
    return params.color_atmosphere.rgb * (outer_haze + mid_glow + inner_rim);
}

fn star_surface_color(sphere_p: vec3<f32>) -> vec3<f32> {
    let time_s = params.identity_a.w;
    let longitude = atan2(sphere_p.x, sphere_p.z);
    let latitude = sphere_p.y;
    let equator = 1.0 - abs(latitude);

    // Bias the star toward horizontal/equatorial flow so the disc reads side-on.
    let shear_p = vec3<f32>(
        longitude * 1.55 + time_s * (0.2 + params.clouds_a.z * 0.12),
        latitude * (2.4 + params.clouds_a.x * 0.08),
        equator * 1.2,
    );
    let convection = fbm4_time(shear_p * 2.8, time_s * 0.37 + params.identity_a.z * 5.7, 4);
    let convection_detail = fbm4_time(shear_p.zxy * 5.4 + vec3<f32>(1.8, -0.7, 2.4), time_s * 0.63 + 2.9, 3);
    let lateral_ribbons = fbm4_time(
        vec3<f32>(longitude * 2.6 + time_s * 0.34, latitude * 7.8, equator * 2.1),
        time_s * 0.58 + 9.1,
        3,
    );
    let plume_noise = fbm4_time(
        vec3<f32>(sphere_p.xz * 6.4, latitude * 3.2),
        time_s * 0.82 + 13.4,
        3,
    );

    let cell_shape = saturate(convection * 0.64 + convection_detail * 0.36);
    let granules = 1.0 - abs(cell_shape * 2.0 - 1.0);
    let dark_lanes = smoothstep(0.18, 0.72, granules * 0.72 + lateral_ribbons * 0.28);
    let warm_cells = smoothstep(0.26, 0.86, cell_shape * 0.84 + equator * 0.16);
    let bright_filaments = smoothstep(
        0.48,
        0.9,
        lateral_ribbons * 0.56 + plume_noise * 0.24 + equator * 0.2,
    );
    let flare_knots = smoothstep(
        0.68,
        0.95,
        plume_noise * 0.58 + bright_filaments * 0.22 + warm_cells * 0.2,
    );
    let molten_flow = fbm4_time(
        vec3<f32>(
            longitude * 4.1 + time_s * 0.41,
            latitude * 6.4 + lateral_ribbons * 0.9,
            equator * 2.8 + plume_noise * 0.4,
        ),
        time_s * 0.92 + 21.6,
        4,
    );
    let molten_detail = fbm4_time(
        vec3<f32>(
            longitude * 8.8 - time_s * 0.56,
            latitude * 10.6,
            equator * 3.7,
        ),
        time_s * 1.14 + 4.8,
        3,
    );
    let caustic_flow = 1.0 - abs((molten_flow * 0.72 + molten_detail * 0.28) * 2.0 - 1.0);
    let orange_channels = smoothstep(0.34, 0.74, caustic_flow * 0.82 + dark_lanes * 0.18);
    let ember_pockets = smoothstep(0.58, 0.92, molten_detail * 0.68 + plume_noise * 0.32);

    var color = mix(params.color_tertiary.rgb * 0.58, params.color_secondary.rgb * 0.9, warm_cells);
    color = mix(color, params.color_tertiary.rgb * 0.34, dark_lanes * 0.54 + orange_channels * 0.22);
    color = mix(color, params.color_secondary.rgb * 0.54 + params.color_tertiary.rgb * 0.34, orange_channels * 0.72);
    color = mix(color, params.color_primary.rgb * 0.98, bright_filaments * 0.42 + ember_pockets * 0.08);
    color += params.color_emissive.rgb * (0.14 + params.color_emissive.a * 0.22) * flare_knots;
    color += params.color_secondary.rgb * 0.08 * ember_pockets;
    color *= 0.66 + bright_filaments * 0.14 + equator * 0.08 + orange_channels * 0.06;
    return color;
}

fn star_corona_color(
    quad_uv: vec2<f32>,
    dist: f32,
    atmosphere_radius: f32,
    body_kind: f32,
    sphere_p: vec3<f32>,
    view_n: vec3<f32>,
) -> vec4<f32> {
    if body_kind < 0.5 || body_kind > 1.5 {
        return vec4<f32>(0.0);
    }

    let corona_band = 0.22 + params.clouds_a.w * 0.26;
    let corona_mask = 1.0 - smoothstep(atmosphere_radius, atmosphere_radius + corona_band, dist);
    if corona_mask <= 0.0001 {
        return vec4<f32>(0.0);
    }

    let longitude = atan2(sphere_p.x, sphere_p.z);
    let latitude = sphere_p.y;
    let equator = 1.0 - abs(latitude);
    let radial = saturate((dist - atmosphere_radius) / max(corona_band, 0.0001));
    let time_s = params.identity_a.w;
    let base_noise = fbm4_time(
        vec3<f32>(longitude * 1.9 + time_s * 0.16, latitude * 5.0, radial * 3.2),
        time_s * 0.46 + params.identity_a.z * 9.3,
        4,
    );
    let streamer_noise = fbm4_time(
        vec3<f32>(longitude * 3.8 + time_s * 0.28, latitude * 9.4, radial * 8.8 - time_s * 0.38),
        time_s * 0.64 + params.identity_a.z * 13.7,
        3,
    );
    let cme_noise = fbm4_time(
        vec3<f32>(longitude * 1.55 + 4.0, latitude * 3.2 + equator * 2.6, radial * 4.8 - time_s * 0.18),
        time_s * 0.38 + params.identity_a.z * 17.1,
        3,
    );

    let wisps = smoothstep(0.38, 0.9, streamer_noise * 0.72 + base_noise * 0.28);
    let turbulence = smoothstep(0.3, 0.9, base_noise);
    let cme = smoothstep(0.8, 0.96, cme_noise + wisps * 0.14 + equator * 0.08);
    let radial_falloff = pow(1.0 - radial, 2.4);
    let limb = pow(1.0 - saturate(view_n.z), 0.65);
    let density = corona_mask
        * radial_falloff
        * limb
        * (0.06 + wisps * 0.24 + turbulence * 0.08 + equator * 0.08)
        * (1.0 + cme * 0.85);

    let corona_color = mix(params.color_tertiary.rgb * 0.78, params.color_secondary.rgb * 0.96, wisps * 0.62 + equator * 0.12);
    let hot_filaments = mix(params.color_emissive.rgb, params.color_primary.rgb, 0.22);
    let color = corona_color * (0.28 + wisps * 0.26 + turbulence * 0.08)
        + hot_filaments * cme * (0.18 + params.color_emissive.a * 0.12);
    let alpha = density * (0.12 + params.clouds_a.w * 0.08 + params.color_emissive.a * 0.04);
    return vec4<f32>(color, alpha);
}

fn black_hole_surface_color(view_n: vec3<f32>, dist_norm: f32) -> vec3<f32> {
    let lens = pow(1.0 - saturate(view_n.z), 2.6);
    let inner = 1.0 - smoothstep(0.0, 0.78, dist_norm);
    let halo = params.color_atmosphere.rgb * lens * (0.16 + params.clouds_a.w * 0.26);
    return mix(params.color_primary.rgb * 0.02, params.color_secondary.rgb * 0.06, inner) + halo;
}

fn render_cloud_pass(
    mesh: VertexOutput,
    body_kind: f32,
    planet_type: f32,
    body_radius: f32,
    pass_mode: f32,
) -> vec4<f32> {
    if body_kind > 0.5 || params.feature_flags_a.z < 0.5 || pass_mode < 0.5 {
        discard;
    }
    let cloud_shell_radius = body_radius * (1.03 + params.emissive_a.x * 0.25);
    let quad_uv = mesh.uv * 2.0 - vec2<f32>(1.0, 1.0);
    let dist = length(quad_uv);
    if dist > cloud_shell_radius {
        discard;
    }

    let disc_uv = quad_uv / max(cloud_shell_radius, 0.0001);
    let disc_len = length(disc_uv);
    let visible_disc = select(disc_uv, disc_uv / disc_len, disc_len > 1.0);
    let sphere_p = cloud_shell_point_from_billboard(
        visible_disc,
        cloud_shell_radius / max(body_radius, 0.0001),
    );
    let view_shell_n = normalize(vec3<f32>(
        visible_disc.x,
        visible_disc.y,
        sqrt(max(0.0, 1.0 - dot(visible_disc, visible_disc)))
    ));
    let mask = select(
        gas_cloud_density(sphere_p),
        terran_cloud_density(sphere_p),
        planet_type < 4.0
    );
    let edge = 1.0 - smoothstep(cloud_shell_radius - 0.08, cloud_shell_radius, dist);
    let body_occlusion = smoothstep(body_radius - 0.012, body_radius + 0.015, dist);
    let sun_dir = normalize(params.world_light_primary_dir_intensity.xyz);
    let lit = saturate(dot(view_shell_n, sun_dir) * 0.5 + 0.5)
        * max(params.world_light_primary_dir_intensity.w * params.sun_dir_a.w, 0.0);
    let local_dir = normalize(params.world_light_local_dir_intensity.xyz);
    let local_lit = saturate(dot(view_shell_n, local_dir) * 0.5 + 0.5)
        * params.world_light_local_dir_intensity.w;
    let shadowed = smoothstep(0.1, 0.85, lit);
    let hemisphere_alpha = select(body_occlusion, 1.0 - body_occlusion, pass_mode > 1.5);
    let alpha = mask * edge * hemisphere_alpha * (0.08 + shadowed * 0.54);
    let color = mix(
        params.color_clouds.rgb * (0.24 + params.world_light_ambient.w * 0.45),
        params.color_clouds.rgb * mix(vec3<f32>(1.0), params.world_light_primary_color_elevation.rgb, 0.28),
        lit * 0.72 + 0.08
    ) + params.world_light_local_color_radius.rgb * local_lit * 0.28
      + params.world_light_flash.rgb * params.world_light_flash.w * 0.22;
    return vec4<f32>(color, alpha);
}

fn render_ring_pass(mesh: VertexOutput, body_kind: f32, pass_mode: f32) -> vec4<f32> {
    if pass_mode < 0.5 {
        return vec4<f32>(0.0);
    }
    let quad_uv = mesh.uv * 2.0 - vec2<f32>(1.0, 1.0);
    let ring_tilt = 0.42;
    let ring_y = quad_uv.y * 2.35;
    let ring_uv = vec2<f32>(quad_uv.x, ring_y);
    let radius = length(ring_uv);
    let arc_side = quad_uv.y * ring_tilt + quad_uv.x * 0.08;
    let split_blend = smoothstep(-0.045, 0.045, arc_side);
    let pass_weight = select(1.0 - split_blend, split_blend, pass_mode > 1.5);
    if pass_weight <= 0.001 {
        return vec4<f32>(0.0);
    }

    if body_kind > 1.5 {
        let inner = 0.26;
        let outer = 0.86;
        if radius < inner || radius > outer {
            return vec4<f32>(0.0);
        }
        let band = 1.0 - smoothstep(inner, inner + 0.06, radius);
        let outer_band = smoothstep(outer - 0.18, outer, radius);
        let radial = fbm2(vec2<f32>(radius * 22.0, params.identity_a.w * params.identity_b.x * 0.3));
        let az = fbm2(vec2<f32>(atan2(quad_uv.y, quad_uv.x) * 2.0, radius * 8.0));
        let arc_soft = smoothstep(0.02, 0.22, abs(quad_uv.y));
        let alpha = (1.0 - band) * (1.0 - outer_band) * (0.32 + radial * 0.42 + az * 0.2) * arc_soft * pass_weight;
        let color = mix(params.color_atmosphere.rgb, params.color_emissive.rgb, radial * 0.7 + params.clouds_a.w * 0.3)
            * (params.world_light_primary_color_elevation.rgb * (0.35 + params.world_light_primary_dir_intensity.w * 0.65)
            + params.world_light_ambient.rgb * params.world_light_ambient.w)
            + params.world_light_local_color_radius.rgb * params.world_light_local_dir_intensity.w * 0.22
            + params.world_light_flash.rgb * params.world_light_flash.w * 0.4;
        return vec4<f32>(color, alpha * (0.35 + params.clouds_a.w * 0.55));
    }

    if body_kind < 0.5 && params.identity_a.y > 3.5 && params.identity_a.y < 4.5 {
        let inner = 0.44;
        let outer = 0.72;
        if radius < inner || radius > outer {
            return vec4<f32>(0.0);
        }
        let dust = fbm2(vec2<f32>(radius * 26.0, atan2(quad_uv.y, quad_uv.x) * 3.0 + params.identity_a.w * 0.05));
        let gaps = fbm2(vec2<f32>(radius * 12.0 - params.identity_a.w * 0.02, quad_uv.x * 5.0));
        let arc_soft = smoothstep(0.03, 0.24, abs(quad_uv.y));
        let alpha = smoothstep(inner, inner + 0.06, radius)
            * (1.0 - smoothstep(outer - 0.04, outer, radius))
            * saturate(dust * 0.9 - gaps * 0.25)
            * arc_soft
            * pass_weight
            * (0.12 + params.clouds_a.w * 0.18 + params.surface_d.w * 0.18);
        let color = mix(params.color_secondary.rgb, params.color_primary.rgb, dust * 0.6)
            * (params.world_light_primary_color_elevation.rgb * (0.35 + params.world_light_primary_dir_intensity.w * 0.65)
            + params.world_light_ambient.rgb * params.world_light_ambient.w)
            + params.world_light_local_color_radius.rgb * params.world_light_local_dir_intensity.w * 0.18
            + params.world_light_flash.rgb * params.world_light_flash.w * 0.2;
        return vec4<f32>(color, alpha);
    }

    return vec4<f32>(0.0);
}

fn perturbed_normal(sphere_p: vec3<f32>, body_kind: f32, planet_type: f32) -> vec3<f32> {
    if body_kind > 0.5 || params.feature_flags_a.x < 0.5 {
        return sphere_p;
    }
    let eps = mix(0.002, 0.014, saturate(params.lighting_a.z));
    let tangent = normalize(vec3<f32>(sphere_p.z, 0.0, -sphere_p.x));
    let bitangent = normalize(cross(sphere_p, tangent));
    let h = planet_height(sphere_p, planet_type);
    let ht = planet_height(normalize(sphere_p + tangent * eps), planet_type);
    let hb = planet_height(normalize(sphere_p + bitangent * eps), planet_type);
    let dh_t = (ht - h) / eps;
    let dh_b = (hb - h) / eps;
    let base_normal = normalize(sphere_p - tangent * dh_t * params.lighting_a.y - bitangent * dh_b * params.lighting_a.y);
    if planet_type < 0.5 {
        let land_factor = terran_land_factor(sphere_p, h);
        let water_normal_factor = mix(0.08, 1.0, land_factor);
        return normalize(mix(sphere_p, base_normal, water_normal_factor));
    }
    return base_normal;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let body_kind = params.identity_a.x;
    let planet_type = params.identity_a.y;
    let cloud_pass_mode = params.pass_flags_a.x;
    let ring_pass_mode = params.pass_flags_a.y;
    let radius = mix(0.58, 0.8, saturate(params.lighting_a.x));
    if ring_pass_mode > 0.5 {
        let ring_color = render_ring_pass(mesh, body_kind, ring_pass_mode);
        if ring_color.a <= 0.0001 {
            discard;
        }
        return ring_color;
    }
    if cloud_pass_mode > 0.5 {
        return render_cloud_pass(mesh, body_kind, planet_type, radius, cloud_pass_mode);
    }
    let atmosphere_radius = radius + mix(0.04, 0.16, saturate(params.emissive_a.x + params.clouds_a.w * 0.2));
    let quad_uv = mesh.uv * 2.0 - vec2<f32>(1.0, 1.0);
    let dist = length(quad_uv);
    let extra_outer = select(0.0, 0.38 + params.clouds_a.w * 0.12, body_kind > 0.5);
    if dist > atmosphere_radius + extra_outer {
        discard;
    }

    let body_mask = 1.0 - smoothstep(radius - 0.02, radius + 0.01, dist);
    let atmosphere_mask = 1.0 - smoothstep(radius - 0.01, atmosphere_radius, dist);
    let disc_uv = quad_uv / max(radius, 0.0001);
    let disc_len = length(disc_uv);
    let visible_disc = select(disc_uv, disc_uv / disc_len, disc_len > 1.0);
    let sphere_p = surface_point_from_billboard(visible_disc);
    let view_n = make_view_sphere_normal(visible_disc);
    let normal = perturbed_normal(sphere_p, body_kind, planet_type);
    let height = planet_height(sphere_p, planet_type);

    let sun_dir = normalize(params.world_light_primary_dir_intensity.xyz);
    let sun_color = params.world_light_primary_color_elevation.rgb;
    let ambient_color = params.world_light_ambient.rgb;
    let ambient_strength = params.world_light_ambient.w;
    let backlight_color = params.world_light_backlight.rgb;
    let backlight_strength = params.world_light_backlight.w;
    let flash_color = params.world_light_flash.rgb;
    let flash_strength = params.world_light_flash.w;
    let local_dir = normalize(params.world_light_local_dir_intensity.xyz);
    let local_intensity = params.world_light_local_dir_intensity.w;
    let local_color = params.world_light_local_color_radius.rgb;
    let view_dir = vec3<f32>(0.0, 0.0, 1.0);
    let wrapped_light = saturate((dot(normal, sun_dir) + params.lighting_a.w) / (1.0 + params.lighting_a.w));
    let sun_intensity = max(params.world_light_primary_dir_intensity.w * params.sun_dir_a.w, 0.0);
    let local_wrapped_light = saturate((dot(normal, local_dir) + params.lighting_a.w) / (1.0 + params.lighting_a.w));
    let half_vec = normalize(sun_dir + view_dir);
    let specular = pow(saturate(dot(normal, half_vec)), max(params.lighting_b.z, 1.0)) * params.lighting_b.y;
    let fresnel = pow(1.0 - saturate(dot(normal, view_dir)), max(params.surface_a.x, 0.5)) * params.surface_a.y;
    let rim = pow(1.0 - saturate(dot(view_n, view_dir)), max(params.surface_a.x, 0.5)) * params.lighting_b.w;
    let cloud_shadow_mask = select(0.0, planet_cloud_shadow(sphere_p, sun_dir, planet_type), body_kind < 0.5);

    var color = vec3<f32>(0.0);
    var water_specular_mask = 0.0;
    if body_kind < 0.5 {
        color = planet_surface_color(sphere_p, height, wrapped_light, planet_type);
        let ambient_mix = params.lighting_b.x * ambient_strength;
        let direct_mix = wrapped_light * sun_intensity;
        let local_mix = local_wrapped_light * local_intensity;
        let diffuse_tint = mix(vec3<f32>(1.0, 1.0, 1.0), sun_color, params.identity_b.w);
        color = color * (ambient_color * ambient_mix + diffuse_tint * direct_mix + local_color * local_mix);
        color *= 1.0 - cloud_shadow_mask * (0.32 + (1.0 - wrapped_light) * 0.28);
        if params.feature_flags_b.x > 0.5 {
            color += mix(vec3<f32>(1.0, 1.0, 1.0), sun_color, 0.28) * specular;
        }
        if planet_type < 0.5 {
            let terran_masks = terran_surface_masks(sphere_p, height);
            let coastline = terran_masks.y;
            let shallows = terran_masks.z;
            water_specular_mask = (1.0 - coastline) * mix(1.0, 0.45, shallows);
            if params.feature_flags_b.w > 0.5 && params.feature_flags_b.x > 0.5 {
                color += vec3<f32>(specular * (0.6 + shallows * 0.18) * water_specular_mask * 1.3);
                color += local_color * local_mix * water_specular_mask * 0.24;
            }
        }
        if params.feature_flags_a.w > 0.5 {
            color += params.color_atmosphere.rgb * fresnel * params.emissive_a.x * 0.32;
            color += (params.color_atmosphere.rgb * 0.7 + backlight_color * backlight_strength) * rim * params.emissive_a.z * 0.34;
        }
        if params.feature_flags_b.y > 0.5 {
            color += params.color_night_lights.rgb * params.emissive_a.w * (1.0 - wrapped_light) * params.surface_a.w;
        }
    } else if body_kind < 1.5 {
        color = star_surface_color(sphere_p);
        let stellar_core = 0.52 + params.color_emissive.a * 0.18;
        color *= stellar_core;
        color += params.color_emissive.rgb * (0.04 + fresnel * 0.08);
        color += backlight_color * backlight_strength * rim * 0.02;
        if params.feature_flags_a.w > 0.5 {
            color += (params.color_atmosphere.rgb * 0.32 + params.color_secondary.rgb * 0.14)
                * rim
                * (0.08 + params.clouds_a.w * 0.18);
        }
        if params.feature_flags_b.z > 0.5 {
            color += params.color_emissive.rgb * params.color_emissive.a * (0.08 + fresnel * 0.12);
        }
    } else {
        color = black_hole_surface_color(view_n, dist / max(radius, 0.0001));
        let ambient_mix = params.lighting_b.x * 0.55 * ambient_strength * smoothstep(0.0, 0.08, sun_intensity);
        color = color * ambient_color * ambient_mix
            + color * sun_color * (wrapped_light * sun_intensity * 1.2)
            + local_color * local_intensity * 0.08;
        if params.feature_flags_a.w > 0.5 {
            color += (params.color_atmosphere.rgb + backlight_color * backlight_strength) * rim * 0.14;
        }
    }

    let atmo_response = atmosphere_response(dist, radius, atmosphere_radius, wrapped_light);
    var atmosphere_alpha = 0.0;
    if params.feature_flags_a.w > 0.5 {
        atmosphere_alpha = atmosphere_mask * params.emissive_a.z * 0.52;
        if body_kind < 0.5 {
            atmosphere_alpha *= 0.8 + water_specular_mask * 0.05;
        } else if body_kind < 1.5 {
            atmosphere_alpha = max(
                atmosphere_alpha,
                atmosphere_mask * (0.16 + params.clouds_a.w * 0.18 + params.color_emissive.a * 0.08),
            );
        } else {
            atmosphere_alpha = max(atmosphere_alpha, atmosphere_mask * (0.12 + params.clouds_a.w * 0.18));
        }
    }

    let out_alpha = max(body_mask, atmosphere_alpha);
    let star_corona = star_corona_color(quad_uv, dist, atmosphere_radius, body_kind, sphere_p, view_n);
    let out_color = mix(
        select(vec3<f32>(0.0), atmo_response, params.feature_flags_a.w > 0.5),
        color + atmo_response * (0.45 + rim * 0.3) + flash_color * flash_strength,
        body_mask,
    );
    let total_alpha = max(out_alpha, star_corona.a);
    var graded_color = mix(out_color, out_color * out_color, 0.12);
    if body_kind < 0.5 {
        graded_color = apply_saturation(graded_color, params.identity_b.y);
        graded_color = apply_contrast(graded_color, params.identity_b.z);
    } else if body_kind < 1.5 {
        graded_color = apply_saturation(graded_color, 1.28);
        graded_color = apply_contrast(graded_color, 1.14);
    }
    let final_color = tone_map(max(graded_color + star_corona.rgb, vec3<f32>(0.0)));
    return vec4<f32>(saturate(final_color.r), saturate(final_color.g), saturate(final_color.b), total_alpha);
}
