#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> viewport_time: vec4<f32>;        // x=width, y=height, z=time_s, w=alpha
@group(2) @binding(1) var<uniform> map_center_zoom_mode: vec4<f32>; // x=center_x, y=center_y, z=zoom_px_per_world, w=fx_mode
@group(2) @binding(2) var<uniform> grid_major: vec4<f32>;           // rgb + alpha
@group(2) @binding(3) var<uniform> grid_minor: vec4<f32>;           // rgb + alpha
@group(2) @binding(4) var<uniform> grid_micro: vec4<f32>;           // rgb + alpha
@group(2) @binding(5) var<uniform> grid_glow_alpha: vec4<f32>;      // x=major, y=minor, z=micro
@group(2) @binding(6) var<uniform> fx_params: vec4<f32>;            // x=fx_opacity, y=noise_amount, z=scanline_density, w=scanline_speed
@group(2) @binding(7) var<uniform> fx_params_b: vec4<f32>;          // x=crt_distortion, y=vignette_strength, z=green_tint_mix
@group(2) @binding(8) var<uniform> background_color: vec4<f32>;     // rgb + unused
@group(2) @binding(9) var<uniform> line_widths_px: vec4<f32>;       // x=major, y=minor, z=micro
@group(2) @binding(10) var<uniform> glow_widths_px: vec4<f32>;      // x=major, y=minor, z=micro
@group(2) @binding(11) var fog_mask: texture_2d<f32>;
@group(2) @binding(12) var fog_mask_sampler: sampler;

fn hash21(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn fmod(value: f32, divisor: f32) -> f32 {
    return value - divisor * floor(value / divisor);
}

fn grid_line(coord: f32, spacing: f32, width: f32) -> f32 {
    let half_width = width * 0.5;
    let d = abs(fmod(coord + half_width, spacing) - half_width);
    return smoothstep(width, 0.0, d);
}

fn sample_fog_explored(uv: vec2<f32>) -> f32 {
    let dims = vec2<f32>(textureDimensions(fog_mask));
    let texel = 1.0 / max(dims, vec2<f32>(1.0, 1.0));
    let base_uv = clamp(uv, vec2<f32>(0.001, 0.001), vec2<f32>(0.999, 0.999));
    let center = textureSample(fog_mask, fog_mask_sampler, base_uv).r;
    let sxp = textureSample(fog_mask, fog_mask_sampler, base_uv + vec2<f32>(texel.x, 0.0)).r;
    let sxn = textureSample(fog_mask, fog_mask_sampler, base_uv - vec2<f32>(texel.x, 0.0)).r;
    let syp = textureSample(fog_mask, fog_mask_sampler, base_uv + vec2<f32>(0.0, texel.y)).r;
    let syn = textureSample(fog_mask, fog_mask_sampler, base_uv - vec2<f32>(0.0, texel.y)).r;
    return center * 0.5 + (sxp + sxn + syp + syn) * 0.125;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = clamp(viewport_time.w, 0.0, 1.0);
    if alpha <= 0.001 {
        discard;
    }

    let viewport = max(viewport_time.xy, vec2<f32>(1.0, 1.0));
    let safe_zoom = max(map_center_zoom_mode.z, 1e-6);
    let mode = round(map_center_zoom_mode.w);
    var uv = mesh.uv;

    // Retro mode gets slight barrel distortion before world projection.
    if mode >= 2.0 {
        let centered = uv - vec2<f32>(0.5, 0.5);
        let r2 = dot(centered, centered);
        uv = vec2<f32>(0.5, 0.5) + centered * (1.0 + fx_params_b.x * r2);
    }

    let screen_px = uv * viewport;
    let world_offset = vec2<f32>(
        (screen_px.x - viewport.x * 0.5) / safe_zoom,
        (viewport.y * 0.5 - screen_px.y) / safe_zoom
    );
    let world_pos = map_center_zoom_mode.xy + world_offset;

    let world_per_pixel = 1.0 / safe_zoom;
    let target_major_px = 140.0;
    let target_major_world = world_per_pixel * target_major_px;
    let decade = pow(10.0, floor(log(max(target_major_world, 1e-12)) / log(10.0)));
    let scaled = target_major_world / decade;
    let major_step = select(select(1.0, 2.0, scaled >= 2.0), 5.0, scaled >= 5.0);
    let major_spacing = major_step * decade;
    let minor_spacing = major_spacing / 10.0;
    let micro_spacing = major_spacing / 100.0;

    let major_core_w = max(line_widths_px.x, 0.01) * world_per_pixel;
    let minor_core_w = max(line_widths_px.y, 0.01) * world_per_pixel;
    let micro_core_w = max(line_widths_px.z, 0.01) * world_per_pixel;

    let major_glow_w = max(glow_widths_px.x, 0.01) * world_per_pixel;
    let minor_glow_w = max(glow_widths_px.y, 0.01) * world_per_pixel;
    let micro_glow_w = max(glow_widths_px.z, 0.01) * world_per_pixel;

    let major = max(
        grid_line(world_pos.x, major_spacing, major_core_w),
        grid_line(world_pos.y, major_spacing, major_core_w)
    );
    let minor = max(
        grid_line(world_pos.x, minor_spacing, minor_core_w),
        grid_line(world_pos.y, minor_spacing, minor_core_w)
    );
    let micro = max(
        grid_line(world_pos.x, micro_spacing, micro_core_w),
        grid_line(world_pos.y, micro_spacing, micro_core_w)
    );

    let major_glow = max(
        grid_line(world_pos.x, major_spacing, major_glow_w),
        grid_line(world_pos.y, major_spacing, major_glow_w)
    );
    let minor_glow = max(
        grid_line(world_pos.x, minor_spacing, minor_glow_w),
        grid_line(world_pos.y, minor_spacing, minor_glow_w)
    );
    let micro_glow = max(
        grid_line(world_pos.x, micro_spacing, micro_glow_w),
        grid_line(world_pos.y, micro_spacing, micro_glow_w)
    );

    // Fade dense tiers when zoomed out too far.
    let minor_fade = smoothstep(6.0, 16.0, minor_spacing * safe_zoom);
    let micro_fade = smoothstep(8.0, 18.0, micro_spacing * safe_zoom);

    var color = background_color.rgb;
    color = mix(color, grid_micro.rgb, micro * grid_micro.a * micro_fade);
    color = mix(color, grid_minor.rgb, minor * grid_minor.a * minor_fade);
    color = mix(color, grid_major.rgb, major * grid_major.a);

    color += grid_micro.rgb * micro_glow * grid_glow_alpha.z * micro_fade;
    color += grid_minor.rgb * minor_glow * grid_glow_alpha.y * minor_fade;
    color += grid_major.rgb * major_glow * grid_glow_alpha.x;

    // Origin axes.
    let origin_w = 2.4 * world_per_pixel;
    let origin_x = smoothstep(origin_w, 0.0, abs(world_pos.x));
    let origin_y = smoothstep(origin_w, 0.0, abs(world_pos.y));
    let origin = max(origin_x, origin_y);
    color = mix(color, grid_major.rgb, origin);

    // Tactical fog-of-war: unexplored cells are blackened, with a softened edge.
    let explored = clamp(sample_fog_explored(mesh.uv), 0.0, 1.0);
    var fog_strength = pow(1.0 - explored, 1.18);
    let fog_grain = (hash21(screen_px * 0.2 + vec2<f32>(viewport_time.z * 9.0, 0.0)) - 0.5) * 0.08;
    fog_strength = clamp(fog_strength + fog_grain * fog_strength, 0.0, 1.0);
    color = mix(color, vec3<f32>(0.0, 0.0, 0.0), fog_strength * 0.94);

    // Optional screen-space FX.
    let fx_opacity = clamp(fx_params.x, 0.0, 1.0);
    if mode >= 1.0 && fx_opacity > 0.0 {
        let t = viewport_time.z;
        let noise = hash21(screen_px * 0.5 + vec2<f32>(t * 45.0, -t * 23.0));
        let noise_amp = fx_params.y * fx_opacity;
        color *= 1.0 + (noise - 0.5) * noise_amp;
    }
    if mode >= 2.0 && fx_opacity > 0.0 {
        let t = viewport_time.z;
        let scan = 0.5 + 0.5 * sin(screen_px.y * fx_params.z * 0.01 + t * fx_params.w);
        let scan_mix = 0.12 * fx_opacity;
        color *= 1.0 - scan_mix + scan * scan_mix;

        let vignette = clamp(fx_params_b.y, 0.0, 1.0);
        let centered = uv - vec2<f32>(0.5, 0.5);
        let edge = clamp(length(centered) * 1.8, 0.0, 1.0);
        color *= 1.0 - edge * edge * vignette * 0.5;

        let green_mix = clamp(fx_params_b.z, 0.0, 1.0);
        let green_tint = vec3<f32>(0.52, 1.0, 0.72);
        color = mix(color, green_tint * dot(color, vec3<f32>(0.299, 0.587, 0.114)), green_mix);
    }

    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), alpha);
}
