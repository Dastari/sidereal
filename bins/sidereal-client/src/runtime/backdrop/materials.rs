#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct StarfieldMaterial {
    #[uniform(0)]
    pub viewport_time: Vec4,
    #[uniform(1)]
    pub drift_intensity: Vec4,
    #[uniform(2)]
    pub velocity_dir: Vec4,
    #[uniform(3)]
    pub starfield_params: Vec4,
    #[uniform(4)]
    pub starfield_tint: Vec4,
    #[uniform(5)]
    pub star_core_params: Vec4,
    #[uniform(6)]
    pub star_core_color: Vec4,
    #[uniform(7)]
    pub corona_params: Vec4,
    #[uniform(8)]
    pub corona_color: Vec4,
}

impl Default for StarfieldMaterial {
    fn default() -> Self {
        Self {
            viewport_time: Vec4::new(1920.0, 1080.0, 0.0, 0.0),
            drift_intensity: Vec4::new(0.0, 0.0, 1.0, 1.0),
            velocity_dir: Vec4::new(0.0, 1.0, 1.0, 0.0),
            starfield_params: Vec4::new(0.05, 3.0, 0.35, 1.0),
            starfield_tint: Vec4::new(1.0, 1.0, 1.0, 1.0),
            star_core_params: Vec4::new(1.0, 1.0, 1.0, 0.0),
            star_core_color: Vec4::new(0.72, 0.83, 1.0, 1.0),
            corona_params: Vec4::new(1.0, 1.0, 1.0, 0.0),
            corona_color: Vec4::new(0.44, 0.64, 1.0, 1.0),
        }
    }
}

impl Material2d for StarfieldMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(super::shaders::runtime_shader_handle(
            super::shaders::RuntimeShaderSlot::Starfield,
        ))
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Packed uniforms for the space background shader (one buffer to stay under
/// max_uniform_buffers_per_shader_stage limits on Windows/DX).
#[derive(ShaderType, Debug, Clone)]
pub struct SpaceBackgroundUniforms {
    pub viewport_time: Vec4,
    pub drift_intensity: Vec4,
    pub velocity_dir: Vec4,
    pub space_bg_params: Vec4,
    pub space_bg_tint: Vec4,
    pub space_bg_background: Vec4,
    pub space_bg_flare: Vec4,
    pub space_bg_noise_a: Vec4,
    pub space_bg_noise_b: Vec4,
    pub space_bg_star_mask_a: Vec4,
    pub space_bg_star_mask_b: Vec4,
    pub space_bg_star_mask_c: Vec4,
    pub space_bg_blend_a: Vec4,
    pub space_bg_blend_b: Vec4,
    pub space_bg_section_flags: Vec4, // .x nebula, .y stars, .z flares
    pub space_bg_nebula_color_a: Vec4,
    pub space_bg_nebula_color_b: Vec4,
    pub space_bg_nebula_color_c: Vec4,
    pub space_bg_star_color: Vec4,
    pub space_bg_flare_tint: Vec4,
    pub space_bg_depth_a: Vec4,
    pub space_bg_light_a: Vec4,
    pub space_bg_light_b: Vec4,
    pub space_bg_light_flags: Vec4,
    pub space_bg_shafts_a: Vec4,
    pub space_bg_shafts_b: Vec4,
    pub space_bg_backlight_color: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct SpaceBackgroundMaterial {
    #[uniform(0)]
    pub params: SpaceBackgroundUniforms,
    #[texture(1)]
    #[sampler(2)]
    pub flare_texture: Handle<Image>,
}

impl Default for SpaceBackgroundUniforms {
    fn default() -> Self {
        Self {
            viewport_time: Vec4::new(1920.0, 1080.0, 0.0, 0.0),
            drift_intensity: Vec4::new(0.0, 0.0, 1.0, 1.0),
            velocity_dir: Vec4::new(0.0, 1.0, 1.0, 0.0),
            space_bg_params: Vec4::new(0.35, 2.0, 1.0, 0.85),
            space_bg_tint: Vec4::new(1.0, 1.77, 1.24, 0.0),
            space_bg_background: Vec4::new(0.0, 0.0, 0.0, 1.0),
            space_bg_flare: Vec4::new(1.0, 4.0, 0.54, 0.0),
            space_bg_noise_a: Vec4::new(0.0, 5.0, 0.52, 2.0),
            space_bg_noise_b: Vec4::new(1.0, 0.42, 1.0, 0.0),
            space_bg_star_mask_a: Vec4::new(1.0, 0.0, 4.0, 3.1),
            space_bg_star_mask_b: Vec4::new(0.35, 1.25, 0.42, 1.75),
            space_bg_star_mask_c: Vec4::new(0.83, 5.0, 0.019, 0.022),
            space_bg_blend_a: Vec4::new(1.0, 0.5, 2.0, 1.0),
            space_bg_blend_b: Vec4::new(1.0, 1.0, 1.0, 0.0),
            space_bg_section_flags: Vec4::new(1.0, 1.0, 1.0, 0.0),
            space_bg_nebula_color_a: Vec4::new(0.0, 0.0, 0.196, 0.0),
            space_bg_nebula_color_b: Vec4::new(0.0, 0.073, 0.082, 0.0),
            space_bg_nebula_color_c: Vec4::new(0.187, 0.16, 0.539, 0.0),
            space_bg_star_color: Vec4::new(0.698, 0.682, 2.0, 1.0),
            space_bg_flare_tint: Vec4::new(1.0, 1.0, 2.0, 1.0),
            space_bg_depth_a: Vec4::new(1.03, 0.83, 1.69, 1.08),
            space_bg_light_a: Vec4::new(-0.3, 0.10, 4.0, 0.49),
            space_bg_light_b: Vec4::new(2.2, 1.35, 0.14, 1.0),
            space_bg_light_flags: Vec4::new(1.0, 1.0, 0.0, 1.0),
            space_bg_shafts_a: Vec4::new(1.76, 0.47, 2.65, 16.0),
            space_bg_shafts_b: Vec4::new(1.15, 1.0, 1.45, 0.85),
            space_bg_backlight_color: Vec4::new(1.15, 1.0, 1.45, 1.0),
        }
    }
}

fn resolve_space_background_flare_asset_id(
    settings: &SpaceBackgroundShaderSettings,
) -> Option<String> {
    settings
        .flare_texture_asset_id
        .as_deref()
        .filter(|asset_id| !asset_id.trim().is_empty())
        .map(str::to_string)
}

fn resolve_space_background_flare_handle(
    settings: &SpaceBackgroundShaderSettings,
    flare_cache: &mut std::collections::HashMap<String, Handle<Image>>,
    asset_manager: &assets::LocalAssetManager,
    asset_root: &str,
    cache_adapter: super::resources::AssetCacheAdapter,
    images: &mut Assets<Image>,
) -> (Option<Handle<Image>>, bool) {
    let mut flare_enabled = settings.flare_enabled;
    let Some(flare_asset_id) = resolve_space_background_flare_asset_id(settings) else {
        return (None, false);
    };

    let handle = flare_cache.get(&flare_asset_id).cloned().or_else(|| {
        let handle = assets::cached_image_handle(
            &flare_asset_id,
            asset_manager,
            asset_root,
            cache_adapter,
            images,
        )?;
        flare_cache.insert(flare_asset_id.clone(), handle.clone());
        Some(handle)
    });

    if handle.is_none() {
        flare_enabled = false;
    }

    (handle, flare_enabled)
}

fn populate_space_background_uniforms(
    params: &mut SpaceBackgroundUniforms,
    world_data: &FullscreenExternalWorldData,
    settings: &SpaceBackgroundShaderSettings,
    flare_enabled: bool,
) {
    params.viewport_time = world_data.viewport_time;
    params.drift_intensity = world_data.drift_intensity;
    params.velocity_dir = world_data.velocity_dir;
    params.space_bg_params = Vec4::new(
        settings.intensity.max(0.0),
        settings.drift_scale.max(0.0),
        settings.velocity_glow.max(0.0),
        settings.nebula_strength.max(0.0),
    );
    params.space_bg_tint = settings.tint_rgb.extend(settings.seed);
    params.space_bg_background = settings.background_rgb.extend(1.0);
    params.space_bg_flare = Vec4::new(
        if flare_enabled { 1.0 } else { 0.0 },
        settings.flare_intensity.max(0.0),
        settings.flare_density.clamp(0.0, 1.0),
        settings.flare_size.max(0.01),
    );
    params.space_bg_noise_a = Vec4::new(
        settings.nebula_noise_mode.clamp(0, 1) as f32,
        settings.nebula_octaves.clamp(1, 8) as f32,
        settings.nebula_gain.clamp(0.1, 0.95),
        settings.nebula_lacunarity.clamp(1.1, 4.0),
    );
    params.space_bg_noise_b = Vec4::new(
        settings.nebula_power.clamp(0.2, 4.0),
        settings.nebula_shelf.clamp(0.0, 0.95),
        settings.nebula_ridge_offset.clamp(0.5, 2.5),
        0.0,
    );
    params.space_bg_star_mask_a = Vec4::new(
        if settings.star_mask_enabled { 1.0 } else { 0.0 },
        settings.star_mask_mode.clamp(0, 1) as f32,
        settings.star_mask_octaves.clamp(1, 8) as f32,
        settings.star_mask_scale.clamp(0.2, 8.0),
    );
    params.space_bg_star_mask_b = Vec4::new(
        settings.star_mask_threshold.clamp(0.0, 0.99),
        settings.star_mask_power.clamp(0.2, 4.0),
        settings.star_mask_gain.clamp(0.1, 0.95),
        settings.star_mask_lacunarity.clamp(1.1, 4.0),
    );
    params.space_bg_star_mask_c = Vec4::new(
        settings.star_mask_ridge_offset.clamp(0.5, 2.5),
        settings.star_count.clamp(0.0, 5.0),
        settings.star_size_min.clamp(0.01, 0.35),
        settings.star_size_max.clamp(0.01, 0.35),
    );
    params.space_bg_blend_a = Vec4::new(
        settings.nebula_blend_mode.clamp(0, 26) as f32,
        settings.nebula_opacity.clamp(0.0, 1.0),
        settings.stars_blend_mode.clamp(0, 26) as f32,
        settings.stars_opacity.clamp(0.0, 1.0),
    );
    params.space_bg_blend_b = Vec4::new(
        settings.flares_blend_mode.clamp(0, 26) as f32,
        settings.flares_opacity.clamp(0.0, 1.0),
        settings.zoom_rate.clamp(0.0, 4.0),
        0.0,
    );
    params.space_bg_section_flags = Vec4::new(
        if settings.enable_nebula_layer {
            1.0
        } else {
            0.0
        },
        if settings.enable_stars_layer {
            1.0
        } else {
            0.0
        },
        if settings.enable_flares_layer {
            1.0
        } else {
            0.0
        },
        if settings.enable_background_gradient {
            1.0
        } else {
            0.0
        },
    );
    params.space_bg_nebula_color_a = settings
        .nebula_color_primary_rgb
        .clamp(Vec3::ZERO, Vec3::splat(2.0))
        .extend(0.0);
    params.space_bg_nebula_color_b = settings
        .nebula_color_secondary_rgb
        .clamp(Vec3::ZERO, Vec3::splat(2.0))
        .extend(0.0);
    params.space_bg_nebula_color_c = settings
        .nebula_color_accent_rgb
        .clamp(Vec3::ZERO, Vec3::splat(2.0))
        .extend(0.0);
    params.space_bg_star_color = settings
        .star_color_rgb
        .clamp(Vec3::ZERO, Vec3::splat(2.0))
        .extend(1.0);
    params.space_bg_flare_tint = settings
        .flare_tint_rgb
        .clamp(Vec3::ZERO, Vec3::splat(2.0))
        .extend(1.0);
    params.space_bg_depth_a = Vec4::new(
        settings.depth_layer_separation.clamp(0.0, 2.0),
        settings.depth_parallax_scale.clamp(0.0, 2.0),
        settings.depth_haze_strength.clamp(0.0, 2.0),
        settings.depth_occlusion_strength.clamp(0.0, 3.0),
    );
    params.space_bg_light_a = Vec4::new(
        settings.backlight_screen_x.clamp(-1.5, 1.5),
        settings.backlight_screen_y.clamp(-1.5, 1.5),
        settings.backlight_intensity.clamp(0.0, 20.0),
        settings.backlight_wrap.clamp(0.0, 2.0),
    );
    params.space_bg_light_b = Vec4::new(
        settings.backlight_edge_boost.clamp(0.0, 6.0),
        settings.backlight_bloom_scale.clamp(0.0, 2.0),
        settings.backlight_bloom_threshold.clamp(0.0, 1.0),
        settings.shaft_quality.clamp(0, 2) as f32,
    );
    params.space_bg_light_flags = Vec4::new(
        if settings.enable_backlight { 1.0 } else { 0.0 },
        if settings.enable_light_shafts {
            1.0
        } else {
            0.0
        },
        if settings.shafts_debug_view { 1.0 } else { 0.0 },
        settings.shaft_blend_mode.clamp(0, 26) as f32,
    );
    params.space_bg_shafts_a = Vec4::new(
        settings.shaft_intensity.clamp(0.0, 40.0),
        settings.shaft_length.clamp(0.05, 0.95),
        settings.shaft_falloff.clamp(0.2, 8.0),
        settings.shaft_samples.clamp(4, 24) as f32,
    );
    params.space_bg_shafts_b = settings
        .shaft_color_rgb
        .clamp(Vec3::ZERO, Vec3::splat(3.0))
        .extend(settings.shaft_opacity.clamp(0.0, 1.0));
    params.space_bg_backlight_color = settings
        .backlight_color_rgb
        .clamp(Vec3::ZERO, Vec3::splat(3.0))
        .extend(1.0);
}

impl Material2d for SpaceBackgroundMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(super::shaders::runtime_shader_handle(
            super::shaders::RuntimeShaderSlot::SpaceBackgroundBase,
        ))
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Opaque
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct SpaceBackgroundNebulaMaterial {
    #[uniform(0)]
    pub params: SpaceBackgroundUniforms,
    #[texture(1)]
    #[sampler(2)]
    pub flare_texture: Handle<Image>,
}

impl Material2d for SpaceBackgroundNebulaMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(super::shaders::runtime_shader_handle(
            super::shaders::RuntimeShaderSlot::SpaceBackgroundNebula,
        ))
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct StreamedSpriteShaderMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub image: Handle<Image>,
    #[uniform(2)]
    pub lighting: SharedWorldLightingUniforms,
    #[uniform(3)]
    pub local_rotation: Vec4,
}

impl Material2d for StreamedSpriteShaderMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(super::shaders::runtime_shader_handle(
            super::shaders::RuntimeShaderSlot::GenericSprite,
        ))
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct AsteroidSpriteShaderMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub image: Handle<Image>,
    #[uniform(2)]
    pub lighting: SharedWorldLightingUniforms,
    #[texture(3)]
    #[sampler(4)]
    pub normal_image: Handle<Image>,
    #[uniform(5)]
    pub local_rotation: Vec4,
}

impl Material2d for AsteroidSpriteShaderMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(super::shaders::runtime_shader_handle(
            super::shaders::RuntimeShaderSlot::AsteroidSprite,
        ))
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct PlanetVisualMaterial {
    #[uniform(0)]
    pub params: PlanetBodyUniforms,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct StarVisualMaterial {
    #[uniform(0)]
    pub params: PlanetBodyUniforms,
}

#[derive(ShaderType, Debug, Clone)]
pub struct PlanetBodyUniforms {
    pub identity_a: Vec4,
    pub identity_b: Vec4,
    pub feature_flags_a: Vec4,
    pub feature_flags_b: Vec4,
    pub pass_flags_a: Vec4,
    pub lighting_a: Vec4,
    pub lighting_b: Vec4,
    pub surface_a: Vec4,
    pub surface_b: Vec4,
    pub surface_c: Vec4,
    pub surface_d: Vec4,
    pub clouds_a: Vec4,
    pub atmosphere_a: Vec4,
    pub emissive_a: Vec4,
    pub sun_dir_a: Vec4,
    pub world_lighting: SharedWorldLightingUniforms,
    pub color_primary: Vec4,
    pub color_secondary: Vec4,
    pub color_tertiary: Vec4,
    pub color_atmosphere: Vec4,
    pub color_clouds: Vec4,
    pub color_night_lights: Vec4,
    pub color_emissive: Vec4,
}

#[derive(ShaderType, Debug, Clone)]
pub struct SharedWorldLightingUniforms {
    pub metadata: Vec4,
    pub ambient: Vec4,
    pub backlight: Vec4,
    pub flash: Vec4,
    pub stellar_dir_intensity: [Vec4; super::lighting::MAX_STELLAR_LIGHTS],
    pub stellar_color_params: [Vec4; super::lighting::MAX_STELLAR_LIGHTS],
    pub local_dir_intensity: [Vec4; super::lighting::MAX_LOCAL_LIGHTS],
    pub local_color_radius: [Vec4; super::lighting::MAX_LOCAL_LIGHTS],
}

impl SharedWorldLightingUniforms {
    pub fn from_state(state: &super::lighting::WorldLightingState) -> Self {
        Self::from_state_for_world_position(
            state,
            Vec2::ZERO,
            &super::lighting::CameraLocalLightSet::default(),
        )
    }

    pub fn from_state_for_world_position(
        state: &super::lighting::WorldLightingState,
        world_position: Vec2,
        camera_local_lights: &super::lighting::CameraLocalLightSet,
    ) -> Self {
        let stellar_lights =
            super::lighting::resolve_stellar_lights_for_position(state, world_position.as_dvec2());
        let local_lights = super::lighting::resolve_local_lights_for_position(
            camera_local_lights,
            world_position.as_dvec2(),
        );
        let mut stellar_dir_intensity = [Vec4::ZERO; super::lighting::MAX_STELLAR_LIGHTS];
        let mut stellar_color_params = [Vec4::ZERO; super::lighting::MAX_STELLAR_LIGHTS];
        let mut local_dir_intensity = [Vec4::ZERO; super::lighting::MAX_LOCAL_LIGHTS];
        let mut local_color_radius = [Vec4::ZERO; super::lighting::MAX_LOCAL_LIGHTS];
        let mut stellar_count = 0.0;
        let mut local_count = 0.0;
        for (index, light) in stellar_lights.iter().enumerate() {
            if light.intensity <= 0.001 {
                continue;
            }
            stellar_count += 1.0;
            stellar_dir_intensity[index] = light.direction.extend(light.intensity);
            stellar_color_params[index] = light.color.extend(light.radius_m);
        }
        for (index, light) in local_lights.iter().enumerate() {
            if light.intensity <= 0.001 {
                continue;
            }
            local_count += 1.0;
            local_dir_intensity[index] = light.direction.extend(light.intensity);
            local_color_radius[index] = light.color.extend(light.radius_m);
        }
        Self {
            metadata: Vec4::new(stellar_count, local_count, state.exposure.max(0.0), 0.0),
            ambient: state.ambient_color.extend(state.ambient_intensity),
            backlight: state.backlight_color.extend(state.backlight_intensity),
            flash: state.event_flash_color.extend(state.event_flash_intensity),
            stellar_dir_intensity,
            stellar_color_params,
            local_dir_intensity,
            local_color_radius,
        }
    }
}

fn shader_seed_unit(seed: u32) -> f32 {
    // Shader-side procedural inputs must stay bounded. Feeding the raw persisted seed
    // into per-pixel trig/noise math can produce pathological GPU cost on some drivers.
    let mut x = seed;
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x as f32) / (u32::MAX as f32)
}

impl PlanetBodyUniforms {
    pub fn from_settings(
        settings: &PlanetBodyShaderSettings,
        time_s: f32,
        world_position: Vec2,
        world_lighting: &super::lighting::WorldLightingState,
        camera_local_lights: &super::lighting::CameraLocalLightSet,
    ) -> Self {
        Self::from_settings_with_pass(
            settings,
            time_s,
            world_position,
            Vec4::ZERO,
            world_lighting,
            camera_local_lights,
        )
    }

    pub fn from_settings_with_pass(
        settings: &PlanetBodyShaderSettings,
        time_s: f32,
        world_position: Vec2,
        pass_flags_a: Vec4,
        world_lighting: &super::lighting::WorldLightingState,
        camera_local_lights: &super::lighting::CameraLocalLightSet,
    ) -> Self {
        let world_uniforms = SharedWorldLightingUniforms::from_state_for_world_position(
            world_lighting,
            world_position,
            camera_local_lights,
        );
        let seed_unit = shader_seed_unit(settings.seed);
        Self {
            identity_a: Vec4::new(
                settings.body_kind as f32,
                settings.planet_type as f32,
                seed_unit,
                time_s,
            ),
            identity_b: Vec4::new(
                settings.rotation_speed,
                settings.surface_saturation,
                settings.surface_contrast,
                settings.light_color_mix,
            ),
            feature_flags_a: Vec4::new(
                if settings.enable_surface_detail {
                    1.0
                } else {
                    0.0
                },
                if settings.enable_craters { 1.0 } else { 0.0 },
                if settings.enable_clouds { 1.0 } else { 0.0 },
                if settings.enable_atmosphere { 1.0 } else { 0.0 },
            ),
            feature_flags_b: Vec4::new(
                if settings.enable_specular { 1.0 } else { 0.0 },
                if settings.enable_night_lights {
                    1.0
                } else {
                    0.0
                },
                if settings.enable_emissive { 1.0 } else { 0.0 },
                if settings.enable_ocean_specular {
                    1.0
                } else {
                    0.0
                },
            ),
            pass_flags_a,
            lighting_a: Vec4::new(
                settings.base_radius_scale,
                settings.normal_strength,
                settings.detail_level,
                settings.light_wrap,
            ),
            lighting_b: Vec4::new(
                settings.ambient_strength,
                settings.specular_strength,
                settings.specular_power,
                settings.rim_strength,
            ),
            surface_a: Vec4::new(
                settings.rim_power,
                settings.fresnel_strength,
                settings.cloud_shadow_strength,
                settings.night_glow_strength,
            ),
            surface_b: Vec4::new(
                settings.continent_size,
                settings.ocean_level,
                settings.mountain_height,
                settings.roughness,
            ),
            surface_c: Vec4::new(
                settings.terrain_octaves as f32,
                settings.terrain_lacunarity,
                settings.terrain_gain,
                settings.crater_density,
            ),
            surface_d: Vec4::new(
                settings.crater_size,
                settings.volcano_density,
                settings.ice_cap_size,
                settings.storm_intensity,
            ),
            clouds_a: Vec4::new(
                settings.bands_count,
                settings.spot_density,
                settings.surface_activity,
                settings.corona_intensity,
            ),
            atmosphere_a: Vec4::new(
                settings.cloud_coverage,
                settings.cloud_scale,
                settings.cloud_speed,
                settings.cloud_alpha,
            ),
            emissive_a: Vec4::new(
                settings.atmosphere_thickness,
                settings.atmosphere_falloff,
                settings.atmosphere_alpha,
                settings.city_lights,
            ),
            sun_dir_a: Vec4::new(
                settings.sun_direction_xy.x,
                settings.sun_direction_xy.y,
                0.82,
                settings.sun_intensity,
            ),
            world_lighting: world_uniforms,
            color_primary: settings.color_primary_rgb.extend(1.0),
            color_secondary: settings.color_secondary_rgb.extend(1.0),
            color_tertiary: settings.color_tertiary_rgb.extend(1.0),
            color_atmosphere: settings.color_atmosphere_rgb.extend(1.0),
            color_clouds: settings.color_clouds_rgb.extend(settings.cloud_alpha),
            color_night_lights: settings.color_night_lights_rgb.extend(1.0),
            color_emissive: Vec4::new(
                settings.color_emissive_rgb.x,
                settings.color_emissive_rgb.y,
                settings.color_emissive_rgb.z,
                settings.emissive_strength,
            ),
        }
    }
}

impl Default for PlanetVisualMaterial {
    fn default() -> Self {
        Self {
            params: default_planet_body_uniforms(),
        }
    }
}

impl Default for StarVisualMaterial {
    fn default() -> Self {
        Self {
            params: default_planet_body_uniforms(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeEffectKind {
    BillboardThruster = 1,
    BillboardImpactSpark = 2,
    BillboardExplosion = 3,
    BeamTrailTracer = 10,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct RuntimeEffectMaterial {
    #[uniform(0)]
    pub params: RuntimeEffectUniforms,
    #[uniform(1)]
    pub lighting: SharedWorldLightingUniforms,
}

#[derive(ShaderType, Debug, Clone)]
pub struct RuntimeEffectUniforms {
    pub identity_a: Vec4,
    pub params_a: Vec4,
    pub params_b: Vec4,
    pub color_a: Vec4,
    pub color_b: Vec4,
    pub color_c: Vec4,
}

impl RuntimeEffectUniforms {
    #[allow(clippy::too_many_arguments)]
    pub fn thruster_plume(
        thrust_alpha: f32,
        afterburner_alpha: f32,
        time_s: f32,
        alpha_scale: f32,
        falloff: f32,
        edge_softness: f32,
        noise_strength: f32,
        flicker_hz: f32,
        base_color: Vec4,
        hot_color: Vec4,
        afterburner_color: Vec4,
    ) -> Self {
        Self {
            identity_a: Vec4::new(
                RuntimeEffectKind::BillboardThruster as u32 as f32,
                time_s,
                thrust_alpha,
                alpha_scale,
            ),
            params_a: Vec4::new(falloff, edge_softness, noise_strength, flicker_hz),
            params_b: Vec4::new(afterburner_alpha, 0.0, 0.0, 0.0),
            color_a: base_color,
            color_b: hot_color,
            color_c: afterburner_color,
        }
    }

    pub fn impact_spark(
        age_norm: f32,
        intensity: f32,
        ray_density: f32,
        alpha: f32,
        color: Vec4,
    ) -> Self {
        Self {
            identity_a: Vec4::new(
                RuntimeEffectKind::BillboardImpactSpark as u32 as f32,
                age_norm,
                intensity,
                alpha,
            ),
            params_a: Vec4::new(ray_density, 0.0, 0.0, 0.0),
            params_b: Vec4::ZERO,
            color_a: color,
            color_b: Vec4::ZERO,
            color_c: Vec4::ZERO,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn explosion_burst(
        age_norm: f32,
        intensity: f32,
        expansion: f32,
        alpha: f32,
        noise_strength: f32,
        domain_scale: f32,
        core_color: Vec4,
        rim_color: Vec4,
        smoke_color: Vec4,
    ) -> Self {
        Self {
            identity_a: Vec4::new(
                RuntimeEffectKind::BillboardExplosion as u32 as f32,
                age_norm,
                intensity,
                alpha,
            ),
            params_a: Vec4::new(expansion, noise_strength, 0.0, 0.0),
            params_b: Vec4::new(domain_scale.max(1.0), 0.0, 0.0, 0.0),
            color_a: core_color,
            color_b: rim_color,
            color_c: smoke_color,
        }
    }

    pub fn beam_trail(
        age_norm: f32,
        alpha: f32,
        glow_strength: f32,
        edge_softness: f32,
        noise_strength: f32,
        core_color: Vec4,
        rim_color: Vec4,
    ) -> Self {
        Self {
            identity_a: Vec4::new(
                RuntimeEffectKind::BeamTrailTracer as u32 as f32,
                age_norm,
                alpha,
                glow_strength,
            ),
            params_a: Vec4::new(edge_softness, noise_strength, 0.0, 0.0),
            params_b: Vec4::ZERO,
            color_a: core_color,
            color_b: rim_color,
            color_c: Vec4::ZERO,
        }
    }
}

impl Default for RuntimeEffectMaterial {
    fn default() -> Self {
        Self {
            params: RuntimeEffectUniforms::thruster_plume(
                0.0,
                0.0,
                0.0,
                0.0,
                1.25,
                1.7,
                0.35,
                0.0,
                Vec4::new(1.0, 0.4, 0.15, 1.0),
                Vec4::new(1.0, 0.82, 0.3, 1.0),
                Vec4::new(0.68, 0.88, 1.12, 1.0),
            ),
            lighting: SharedWorldLightingUniforms::from_state(
                &super::lighting::WorldLightingState::default(),
            ),
        }
    }
}
