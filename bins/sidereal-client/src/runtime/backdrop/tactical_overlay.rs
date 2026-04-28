#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct TacticalMapOverlayMaterial {
    #[uniform(0)]
    pub viewport_time: Vec4, // x=width, y=height, z=time_s, w=alpha
    #[uniform(1)]
    pub map_center_zoom_mode: Vec4, // x=center_x, y=center_y, z=zoom_px_per_world, w=fx_mode
    #[uniform(2)]
    pub grid_major: Vec4, // rgb + alpha
    #[uniform(3)]
    pub grid_minor: Vec4, // rgb + alpha
    #[uniform(4)]
    pub grid_micro: Vec4, // rgb + alpha
    #[uniform(5)]
    pub grid_glow_alpha: Vec4, // x=major, y=minor, z=micro, w=unused
    #[uniform(6)]
    pub fx_params: Vec4, // x=fx_opacity, y=noise_amount, z=scanline_density, w=scanline_speed
    #[uniform(7)]
    pub fx_params_b: Vec4, // x=crt_distortion, y=vignette_strength, z=green_tint_mix, w=unused
    #[uniform(8)]
    pub background_color: Vec4, // rgb + unused
    #[uniform(9)]
    pub line_widths_px: Vec4, // x=major, y=minor, z=micro, w=unused
    #[uniform(10)]
    pub glow_widths_px: Vec4, // x=major, y=minor, z=micro, w=unused
    #[texture(11)]
    #[sampler(12)]
    pub fog_mask: Handle<Image>,
    #[uniform(13)]
    pub gravity_well_params: Vec4, // x=count, y=warp_strength, z=density_strength, w=unused
    #[uniform(14)]
    pub gravity_well_0: Vec4, // xy=center, z=radius_m, w=mass_scale
    #[uniform(15)]
    pub gravity_well_1: Vec4, // xy=center, z=radius_m, w=mass_scale
    #[uniform(16)]
    pub gravity_well_2: Vec4, // xy=center, z=radius_m, w=mass_scale
    #[uniform(17)]
    pub gravity_well_3: Vec4, // xy=center, z=radius_m, w=mass_scale
}

impl Default for TacticalMapOverlayMaterial {
    fn default() -> Self {
        Self {
            viewport_time: Vec4::new(1920.0, 1080.0, 0.0, 0.0),
            map_center_zoom_mode: Vec4::new(0.0, 0.0, 1.0, 1.0),
            grid_major: Vec4::new(0.22, 0.34, 0.48, 0.14),
            grid_minor: Vec4::new(0.22, 0.34, 0.48, 0.126),
            grid_micro: Vec4::new(0.22, 0.34, 0.48, 0.113),
            grid_glow_alpha: Vec4::new(0.02, 0.018, 0.016, 0.0),
            fx_params: Vec4::new(0.45, 0.12, 360.0, 0.65),
            fx_params_b: Vec4::new(0.02, 0.24, 0.0, 0.0),
            background_color: Vec4::new(0.005, 0.008, 0.02, 0.0),
            line_widths_px: Vec4::new(1.4, 0.95, 0.75, 0.0),
            glow_widths_px: Vec4::new(2.0, 1.5, 1.2, 0.0),
            fog_mask: Handle::default(),
            gravity_well_params: Vec4::new(0.0, 0.12, 0.32, 0.0),
            gravity_well_0: Vec4::ZERO,
            gravity_well_1: Vec4::ZERO,
            gravity_well_2: Vec4::ZERO,
            gravity_well_3: Vec4::ZERO,
        }
    }
}

fn default_planet_body_uniforms() -> PlanetBodyUniforms {
    PlanetBodyUniforms::from_settings(
        &PlanetBodyShaderSettings::default(),
        0.0,
        Vec2::ZERO,
        &super::lighting::WorldLightingState::default(),
        &super::lighting::CameraLocalLightSet::default(),
    )
}

macro_rules! impl_runtime_world_polygon_material {
    ($material_ty:ty, $shader_kind:expr) => {
        impl Material2d for $material_ty {
            fn fragment_shader() -> ShaderRef {
                ShaderRef::Handle(super::shaders::world_polygon_shader_handle($shader_kind))
            }

            fn alpha_mode(&self) -> AlphaMode2d {
                AlphaMode2d::Blend
            }
        }
    };
}

macro_rules! impl_runtime_effect_material {
    ($material_ty:ty, $shader_kind:expr) => {
        impl Material2d for $material_ty {
            fn fragment_shader() -> ShaderRef {
                ShaderRef::Handle(super::shaders::runtime_effect_shader_handle($shader_kind))
            }

            fn alpha_mode(&self) -> AlphaMode2d {
                AlphaMode2d::Blend
            }
        }
    };
}

impl_runtime_world_polygon_material!(
    PlanetVisualMaterial,
    super::shaders::RuntimeWorldPolygonShaderKind::PlanetVisual
);
impl_runtime_world_polygon_material!(
    StarVisualMaterial,
    super::shaders::RuntimeWorldPolygonShaderKind::StarVisual
);
impl_runtime_effect_material!(
    RuntimeEffectMaterial,
    super::shaders::RuntimeEffectShaderKind::RuntimeEffect
);
impl_runtime_effect_material!(
    TacticalMapOverlayMaterial,
    super::shaders::RuntimeEffectShaderKind::TacticalMapOverlay
);
