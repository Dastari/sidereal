fn activate_weapon_impact_spark(
    impact_pos: Vec2,
    pool: &mut WeaponImpactSparkPool,
    sparks: &mut Query<
        '_,
        '_,
        (
            &'_ mut WeaponImpactSpark,
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
        ),
        WeaponImpactSparkQueryFilter,
    >,
    effect_materials: &mut Assets<RuntimeEffectMaterial>,
) {
    if pool.sparks.is_empty() {
        return;
    }
    let spark_entity = pool.sparks[pool.next_index % pool.sparks.len()];
    pool.next_index = (pool.next_index + 1) % pool.sparks.len();
    let Ok((mut spark, mut transform, material_handle, mut visibility)) =
        sparks.get_mut(spark_entity)
    else {
        return;
    };
    spark.ttl_s = WEAPON_IMPACT_SPARK_TTL_S;
    spark.max_ttl_s = WEAPON_IMPACT_SPARK_TTL_S;
    transform.translation = Vec3::new(impact_pos.x, impact_pos.y, 0.45);
    transform.scale = Vec3::ONE;
    if let Some(material) = effect_materials.get_mut(&material_handle.0) {
        material.params = RuntimeEffectUniforms::impact_spark(
            0.0,
            1.0,
            1.0,
            0.95,
            Vec4::new(1.0, 0.9, 0.55, 1.0),
        );
    }
    *visibility = Visibility::Visible;
}

fn activate_weapon_impact_explosion(
    impact_pos: Vec2,
    pool: &mut WeaponImpactExplosionPool,
    explosions: &mut Query<
        '_,
        '_,
        (
            &'_ mut WeaponImpactExplosion,
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
        ),
        WeaponImpactExplosionQueryFilter,
    >,
    effect_materials: &mut Assets<RuntimeEffectMaterial>,
) {
    if pool.explosions.is_empty() {
        return;
    }
    let explosion_entity = pool.explosions[pool.next_index % pool.explosions.len()];
    pool.next_index = (pool.next_index + 1) % pool.explosions.len();
    let Ok((mut explosion, mut transform, material_handle, mut visibility)) =
        explosions.get_mut(explosion_entity)
    else {
        return;
    };
    explosion.ttl_s = WEAPON_IMPACT_EXPLOSION_TTL_S;
    explosion.max_ttl_s = WEAPON_IMPACT_EXPLOSION_TTL_S;
    explosion.base_scale = 1.2;
    explosion.growth_scale = 4.4;
    explosion.intensity_scale = 1.0;
    explosion.domain_scale = 1.12;
    explosion.screen_distortion_scale = 0.0;
    transform.translation = Vec3::new(impact_pos.x, impact_pos.y, 0.43);
    transform.scale = Vec3::splat(1.6);
    if let Some(material) = effect_materials.get_mut(&material_handle.0) {
        material.params = RuntimeEffectUniforms::explosion_burst(
            0.0,
            1.0,
            1.0,
            0.92,
            0.35,
            explosion.domain_scale,
            Vec4::new(1.0, 0.92, 0.68, 1.0),
            Vec4::new(1.0, 0.54, 0.16, 1.0),
            Vec4::new(0.24, 0.14, 0.08, 1.0),
        );
    }
    *visibility = Visibility::Visible;
}

fn activate_destruction_effect(
    profile_id: &str,
    impact_pos: Vec2,
    pool: &mut WeaponImpactExplosionPool,
    explosions: &mut Query<
        '_,
        '_,
        (
            &'_ mut WeaponImpactExplosion,
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
        ),
        WeaponImpactExplosionQueryFilter,
    >,
    effect_materials: &mut Assets<RuntimeEffectMaterial>,
) {
    if pool.explosions.is_empty() {
        return;
    }
    let explosion_entity = pool.explosions[pool.next_index % pool.explosions.len()];
    pool.next_index = (pool.next_index + 1) % pool.explosions.len();
    let Ok((mut explosion, mut transform, material_handle, mut visibility)) =
        explosions.get_mut(explosion_entity)
    else {
        return;
    };
    let (core_color, rim_color, smoke_color) = match profile_id {
        "explosion_burst" => (
            Vec4::new(1.0, 0.94, 0.76, 1.0),
            Vec4::new(1.0, 0.58, 0.18, 1.0),
            Vec4::new(0.22, 0.14, 0.10, 1.0),
        ),
        _ => (
            Vec4::new(1.0, 0.94, 0.76, 1.0),
            Vec4::new(1.0, 0.58, 0.18, 1.0),
            Vec4::new(0.22, 0.14, 0.10, 1.0),
        ),
    };
    explosion.ttl_s = DESTRUCTION_EXPLOSION_TTL_S;
    explosion.max_ttl_s = DESTRUCTION_EXPLOSION_TTL_S;
    explosion.base_scale = DESTRUCTION_EXPLOSION_BASE_SCALE;
    explosion.growth_scale = DESTRUCTION_EXPLOSION_GROWTH_SCALE;
    explosion.intensity_scale = DESTRUCTION_EXPLOSION_INTENSITY;
    explosion.domain_scale = 1.45;
    explosion.screen_distortion_scale = 1.0;
    transform.translation = Vec3::new(impact_pos.x, impact_pos.y, 0.52);
    transform.scale = Vec3::splat(DESTRUCTION_EXPLOSION_BASE_SCALE);
    if let Some(material) = effect_materials.get_mut(&material_handle.0) {
        material.params = RuntimeEffectUniforms::explosion_burst(
            0.0,
            DESTRUCTION_EXPLOSION_INTENSITY,
            1.25,
            1.0,
            0.55,
            explosion.domain_scale,
            core_color,
            rim_color,
            smoke_color,
        );
    }
    *visibility = Visibility::Visible;
}

fn has_engine_label(labels: &EntityLabels) -> bool {
    labels
        .0
        .iter()
        .any(|label| label.eq_ignore_ascii_case("engine"))
}

impl StreamedVisualMaterialKind {
    const fn attachment_kind(self) -> StreamedVisualAttachmentKind {
        match self {
            Self::Plain => StreamedVisualAttachmentKind::Plain,
            Self::GenericShader => StreamedVisualAttachmentKind::GenericShader,
            Self::AsteroidShader => StreamedVisualAttachmentKind::AsteroidShader,
        }
    }
}

fn pass_tag(
    family: RuntimeWorldVisualFamily,
    kind: RuntimeWorldVisualPassKind,
) -> RuntimeWorldVisualPass {
    RuntimeWorldVisualPass { family, kind }
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct PlanetProjectedCullRetention {
    visible_until_s: f64,
}

#[derive(Debug, Default)]
pub(super) struct PlanetProjectedCullRuntimeState {
    last_orthographic_scale: Option<f32>,
    rapid_zoom_out_until_s: f64,
}

impl PlanetProjectedCullRuntimeState {
    fn update(&mut self, now_s: f64, orthographic_scale: Option<f32>) -> bool {
        let Some(scale) = orthographic_scale.filter(|value| value.is_finite() && *value > 0.0)
        else {
            self.last_orthographic_scale = None;
            self.rapid_zoom_out_until_s = 0.0;
            return false;
        };
        if let Some(last_scale) = self.last_orthographic_scale
            && scale > last_scale * (1.0 + PLANET_PROJECTED_CULL_ZOOM_OUT_SCALE_THRESHOLD)
        {
            self.rapid_zoom_out_until_s = now_s + PLANET_PROJECTED_CULL_ZOOM_OUT_HOLD_S;
        }
        self.last_orthographic_scale = Some(scale);
        now_s <= self.rapid_zoom_out_until_s
    }
}

fn runtime_world_visual_pass_kind(
    pass: &RuntimeWorldVisualPassDefinition,
) -> Option<RuntimeWorldVisualPassKind> {
    match (pass.visual_family.as_str(), pass.visual_kind.as_str()) {
        ("planet", "body") => Some(RuntimeWorldVisualPassKind::PlanetBody),
        ("planet", "cloud_back") => Some(RuntimeWorldVisualPassKind::PlanetCloudBack),
        ("planet", "cloud_front") => Some(RuntimeWorldVisualPassKind::PlanetCloudFront),
        ("planet", "ring_back") => Some(RuntimeWorldVisualPassKind::PlanetRingBack),
        ("planet", "ring_front") => Some(RuntimeWorldVisualPassKind::PlanetRingFront),
        ("thruster", "plume") => Some(RuntimeWorldVisualPassKind::ThrusterPlume),
        _ => None,
    }
}

fn find_world_visual_pass(
    stack: Option<&RuntimeWorldVisualStack>,
    kind: RuntimeWorldVisualPassKind,
) -> Option<&RuntimeWorldVisualPassDefinition> {
    let stack = stack?;
    stack.passes.iter().find(|pass| {
        pass.enabled && runtime_world_visual_pass_kind(pass).is_some_and(|value| value == kind)
    })
}

fn desired_world_visual_pass_set(
    stack: Option<&RuntimeWorldVisualStack>,
    family: RuntimeWorldVisualFamily,
) -> RuntimeWorldVisualPassSet {
    let mut set = RuntimeWorldVisualPassSet::default();
    let Some(stack) = stack else {
        return set;
    };
    for pass in &stack.passes {
        if !pass.enabled {
            continue;
        }
        let Some(kind) = runtime_world_visual_pass_kind(pass) else {
            continue;
        };
        let expected_family = match kind {
            RuntimeWorldVisualPassKind::PlanetBody
            | RuntimeWorldVisualPassKind::PlanetCloudBack
            | RuntimeWorldVisualPassKind::PlanetCloudFront
            | RuntimeWorldVisualPassKind::PlanetRingBack
            | RuntimeWorldVisualPassKind::PlanetRingFront => RuntimeWorldVisualFamily::Planet,
            RuntimeWorldVisualPassKind::ThrusterPlume => RuntimeWorldVisualFamily::Thruster,
        };
        if expected_family == family {
            set.insert(kind);
        }
    }
    set
}

fn visual_pass_scale_multiplier(
    pass: Option<&RuntimeWorldVisualPassDefinition>,
    fallback: f32,
) -> f32 {
    pass.and_then(|value| value.scale_multiplier)
        .unwrap_or(fallback)
}

fn visual_pass_depth_bias_z(pass: Option<&RuntimeWorldVisualPassDefinition>, fallback: f32) -> f32 {
    pass.and_then(|value| value.depth_bias_z)
        .unwrap_or(fallback)
}

fn shader_materials_enabled() -> bool {
    shaders::shader_materials_enabled()
}

fn procedural_sprite_fingerprint(sprite: &ProceduralSprite) -> u64 {
    let mut seed = 0x517cc1b727220a95u64;
    for byte in sprite.generator_id.as_bytes() {
        seed ^= u64::from(*byte);
        seed = seed.wrapping_mul(0x100000001b3);
    }
    seed ^= u64::from(sprite.resolution_px);
    seed ^= u64::from(sprite.crater_count) << 8;
    seed ^= u64::from(sprite.edge_noise.to_bits()) << 16;
    seed ^= u64::from(sprite.lobe_amplitude.to_bits()) << 32;
    for value in sprite
        .palette_dark_rgb
        .iter()
        .chain(sprite.palette_light_rgb.iter())
        .chain(sprite.mineral_accent_rgb.iter())
    {
        seed ^= u64::from(value.to_bits()).rotate_left(7);
        seed = seed.wrapping_mul(0x100000001b3);
    }
    seed ^= u64::from(sprite.pixel_step_px) << 40;
    seed ^= u64::from(sprite.crack_intensity.to_bits()).rotate_left(11);
    seed ^= u64::from(sprite.mineral_vein_intensity.to_bits()).rotate_left(17);
    seed ^= match sprite.surface_style {
        sidereal_game::ProceduralSpriteSurfaceStyle::Rocky => 0x01,
        sidereal_game::ProceduralSpriteSurfaceStyle::Carbonaceous => 0x02,
        sidereal_game::ProceduralSpriteSurfaceStyle::Metallic => 0x03,
        sidereal_game::ProceduralSpriteSurfaceStyle::Shard => 0x04,
        sidereal_game::ProceduralSpriteSurfaceStyle::GemRich => 0x05,
    };
    if let Some(family_seed_key) = &sprite.family_seed_key {
        for byte in family_seed_key.as_bytes() {
            seed ^= u64::from(*byte);
            seed = seed.wrapping_mul(0x100000001b3);
        }
    }
    seed
}

fn image_from_rgba(width: u32, height: u32, data: Vec<u8>) -> Image {
    image_from_rgba_with_format(width, height, data, TextureFormat::Rgba8UnormSrgb)
}

fn normal_image_from_rgba(width: u32, height: u32, data: Vec<u8>) -> Image {
    image_from_rgba_with_format(width, height, data, TextureFormat::Rgba8Unorm)
}

fn image_from_rgba_with_format(
    width: u32,
    height: u32,
    data: Vec<u8>,
    format: TextureFormat,
) -> Image {
    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
}

fn flat_normal_image_handle(
    cache: &mut StreamedVisualAssetCaches,
    images: &mut Assets<Image>,
) -> Handle<Image> {
    cache
        .flat_normal_image
        .get_or_insert_with(|| images.add(normal_image_from_rgba(1, 1, vec![128, 128, 255, 255])))
        .clone()
}

fn ensure_visual_parent_spatial_components(entity_commands: &mut EntityCommands<'_>) {
    entity_commands.try_insert((
        Transform::default(),
        GlobalTransform::default(),
        Visibility::default(),
    ));
}

fn resolve_streamed_visual_material_kind(
    use_shader_materials: bool,
    world_sprite_kind: Option<shaders::RuntimeWorldSpriteShaderKind>,
    has_streamed_sprite_shader_path: bool,
) -> StreamedVisualMaterialKind {
    if !use_shader_materials {
        return StreamedVisualMaterialKind::Plain;
    }

    match world_sprite_kind {
        Some(shaders::RuntimeWorldSpriteShaderKind::Asteroid) => {
            StreamedVisualMaterialKind::AsteroidShader
        }
        Some(shaders::RuntimeWorldSpriteShaderKind::GenericSprite)
            if has_streamed_sprite_shader_path =>
        {
            StreamedVisualMaterialKind::GenericShader
        }
        _ => StreamedVisualMaterialKind::Plain,
    }
}

fn streamed_visual_needs_rebuild(
    attached_kind: Option<StreamedVisualAttachmentKind>,
    desired_kind: StreamedVisualMaterialKind,
) -> bool {
    attached_kind != Some(desired_kind.attachment_kind())
}

