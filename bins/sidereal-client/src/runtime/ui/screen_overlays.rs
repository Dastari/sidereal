#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(super) fn update_runtime_screen_overlay_passes_system(
    time: Res<'_, Time>,
    tactical_map_state: Res<'_, TacticalMapUiState>,
    fog_cache: Res<'_, TacticalFogCache>,
    contacts_cache: Res<'_, TacticalContactsCache>,
    camera_motion: Res<'_, CameraMotionState>,
    windows: Query<'_, '_, &'_ Window, With<PrimaryWindow>>,
    mut map_queries: ParamSet<
        '_,
        '_,
        (
            Query<
                '_,
                '_,
                &'_ Transform,
                (With<ControlledEntity>, Without<RuntimeScreenOverlayPass>),
            >,
            Query<
                '_,
                '_,
                (
                    &'_ mut Visibility,
                    &'_ mut Transform,
                    &'_ RuntimeScreenOverlayPass,
                    &'_ MeshMaterial2d<TacticalMapOverlayMaterial>,
                ),
                (With<RuntimeScreenOverlayPass>, Without<ControlledEntity>),
            >,
        ),
    >,
    map_settings_query: Query<'_, '_, &'_ TacticalMapUiSettings>,
    mut fx_materials: ResMut<'_, Assets<TacticalMapOverlayMaterial>>,
    mut images: ResMut<'_, Assets<Image>>,
    mut fog_mask_state: Local<'_, TacticalFogMaskUpdateState>,
) {
    let map_settings = map_settings_query
        .iter()
        .next()
        .cloned()
        .unwrap_or_default();
    let Ok(window) = windows.single() else {
        return;
    };
    let controlled_world_xy = map_queries
        .p0()
        .iter()
        .next()
        .map(|transform| transform.translation.truncate());
    let alpha = tactical_map_state.alpha;
    let width = window.width();
    let height = window.height();
    let world_center_base = controlled_world_xy.unwrap_or(camera_motion.world_position_xy);
    let world_center = world_center_base + tactical_map_state.pan_offset_world;
    let map_zoom = tactical_map_state.map_zoom.max(1e-6);
    let mut fx_overlay = map_queries.p1();
    let Ok((mut fx_visibility, mut fx_transform, fx_pass, fx_material_handle)) =
        fx_overlay.single_mut()
    else {
        return;
    };
    *fx_visibility = if alpha > 0.001 {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if alpha <= 0.001 {
        return;
    }
    fx_transform.translation.x = 0.0;
    fx_transform.translation.y = 0.0;
    fx_transform.translation.z = -10.0;
    fx_transform.scale = Vec3::new(width, height, 1.0);

    if let Some(material) = fx_materials.get_mut(&fx_material_handle.0) {
        match fx_pass.kind {
            RuntimeScreenOverlayPassKind::TacticalMap => {
                update_tactical_runtime_screen_overlay_material(
                    material,
                    &mut images,
                    &fog_cache,
                    &contacts_cache,
                    &map_settings,
                    width,
                    height,
                    time.elapsed_secs(),
                    alpha,
                    world_center,
                    map_zoom,
                    &mut fog_mask_state,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn update_tactical_runtime_screen_overlay_material(
    material: &mut TacticalMapOverlayMaterial,
    images: &mut Assets<Image>,
    fog_cache: &TacticalFogCache,
    contacts_cache: &TacticalContactsCache,
    map_settings: &TacticalMapUiSettings,
    width: f32,
    height: f32,
    time_s: f32,
    alpha: f32,
    world_center: Vec2,
    map_zoom: f32,
    fog_mask_state: &mut TacticalFogMaskUpdateState,
) {
    material.viewport_time = Vec4::new(width, height, time_s, alpha);
    material.map_center_zoom_mode = Vec4::new(
        world_center.x,
        world_center.y,
        map_zoom,
        map_settings.fx_mode as f32,
    );
    material.grid_major = Vec4::new(
        map_settings.grid_major_color_rgb.x,
        map_settings.grid_major_color_rgb.y,
        map_settings.grid_major_color_rgb.z,
        map_settings.grid_major_alpha * alpha,
    );
    material.grid_minor = Vec4::new(
        map_settings.grid_minor_color_rgb.x,
        map_settings.grid_minor_color_rgb.y,
        map_settings.grid_minor_color_rgb.z,
        map_settings.grid_minor_alpha * alpha,
    );
    material.grid_micro = Vec4::new(
        map_settings.grid_micro_color_rgb.x,
        map_settings.grid_micro_color_rgb.y,
        map_settings.grid_micro_color_rgb.z,
        map_settings.grid_micro_alpha * alpha,
    );
    material.grid_glow_alpha = Vec4::new(
        map_settings.grid_major_glow_alpha * alpha,
        map_settings.grid_minor_glow_alpha * alpha,
        map_settings.grid_micro_glow_alpha * alpha,
        0.0,
    );
    material.fx_params = Vec4::new(
        map_settings.fx_opacity,
        map_settings.fx_noise_amount,
        map_settings.fx_scanline_density,
        map_settings.fx_scanline_speed,
    );
    material.fx_params_b = Vec4::new(
        map_settings.fx_crt_distortion,
        map_settings.fx_vignette_strength,
        map_settings.fx_green_tint_mix,
        0.0,
    );
    material.background_color = Vec4::new(
        map_settings.background_color_rgb.x,
        map_settings.background_color_rgb.y,
        map_settings.background_color_rgb.z,
        0.0,
    );
    material.line_widths_px = Vec4::new(
        map_settings.line_width_major_px,
        map_settings.line_width_minor_px,
        map_settings.line_width_micro_px,
        0.0,
    );
    material.glow_widths_px = Vec4::new(
        map_settings.glow_width_major_px,
        map_settings.glow_width_minor_px,
        map_settings.glow_width_micro_px,
        0.0,
    );
    update_tactical_gravity_well_uniforms(material, contacts_cache, world_center);
    update_tactical_fog_mask_texture(
        images,
        material,
        fog_cache,
        width,
        height,
        world_center,
        map_zoom,
        fog_mask_state,
    );
}

fn update_tactical_gravity_well_uniforms(
    material: &mut TacticalMapOverlayMaterial,
    contacts_cache: &TacticalContactsCache,
    world_center: Vec2,
) {
    let mut wells = contacts_cache
        .contacts_by_entity_id
        .values()
        .filter_map(tactical_gravity_well_from_contact)
        .collect::<Vec<_>>();
    wells.sort_by(|left, right| {
        let left_score = left.radius_m * left.mass_scale
            / left.center.distance_squared(world_center).max(1.0).sqrt();
        let right_score = right.radius_m * right.mass_scale
            / right.center.distance_squared(world_center).max(1.0).sqrt();
        right_score.total_cmp(&left_score)
    });

    let mut uniforms = [Vec4::ZERO; TACTICAL_GRAVITY_WELL_COUNT];
    for (index, well) in wells.iter().take(TACTICAL_GRAVITY_WELL_COUNT).enumerate() {
        uniforms[index] = Vec4::new(well.center.x, well.center.y, well.radius_m, well.mass_scale);
    }

    material.gravity_well_params = Vec4::new(
        wells.len().min(TACTICAL_GRAVITY_WELL_COUNT) as f32,
        0.12,
        0.32,
        0.0,
    );
    material.gravity_well_0 = uniforms[0];
    material.gravity_well_1 = uniforms[1];
    material.gravity_well_2 = uniforms[2];
    material.gravity_well_3 = uniforms[3];
}

struct TacticalGravityWell {
    center: Vec2,
    radius_m: f32,
    mass_scale: f32,
}

fn tactical_gravity_well_from_contact(
    contact: &sidereal_net::TacticalContact,
) -> Option<TacticalGravityWell> {
    if !contact.is_live_now || !tactical_contact_has_gravity_well(contact.kind.as_str()) {
        return None;
    }

    let size_radius = contact
        .size_m
        .map(|size| size.into_iter().fold(0.0_f32, f32::max) * 0.5)
        .unwrap_or(0.0);
    let mass_scale = contact
        .mass_kg
        .filter(|mass| *mass > 0.0)
        .map(|mass| (mass.log10() / 12.0).clamp(0.75, 2.5))
        .unwrap_or(1.0);
    let radius_m = (size_radius * (5.0 + mass_scale * 2.0))
        .max(TACTICAL_GRAVITY_WELL_MIN_RADIUS_M)
        .clamp(
            TACTICAL_GRAVITY_WELL_MIN_RADIUS_M,
            TACTICAL_GRAVITY_WELL_MAX_RADIUS_M,
        );

    Some(TacticalGravityWell {
        center: Vec2::new(contact.position_xy[0] as f32, contact.position_xy[1] as f32),
        radius_m,
        mass_scale,
    })
}

fn tactical_contact_has_gravity_well(kind: &str) -> bool {
    matches!(
        kind.to_ascii_lowercase().as_str(),
        "planet" | "star" | "blackhole" | "black_hole"
    )
}

#[allow(clippy::too_many_arguments)]
fn update_tactical_fog_mask_texture(
    images: &mut Assets<Image>,
    material: &TacticalMapOverlayMaterial,
    fog_cache: &TacticalFogCache,
    viewport_width_px: f32,
    viewport_height_px: f32,
    world_center: Vec2,
    map_zoom_px_per_world: f32,
    build_state: &mut TacticalFogMaskUpdateState,
) {
    let Some(image) = images.get_mut(&material.fog_mask) else {
        return;
    };
    let expected_len = (TACTICAL_FOG_MASK_RESOLUTION * TACTICAL_FOG_MASK_RESOLUTION) as usize;
    let needs_rebuild = image.texture_descriptor.size.width != TACTICAL_FOG_MASK_RESOLUTION
        || image.texture_descriptor.size.height != TACTICAL_FOG_MASK_RESOLUTION
        || image.texture_descriptor.format != TextureFormat::R8Unorm
        || image.data.as_ref().map_or(0, Vec::len) != expected_len;
    let viewport_width_u32 = viewport_width_px.max(0.0).round() as u32;
    let viewport_height_u32 = viewport_height_px.max(0.0).round() as u32;
    let params_changed = !build_state.initialized
        || build_state.fog_revision != fog_cache.revision
        || build_state.viewport_width_px != viewport_width_u32
        || build_state.viewport_height_px != viewport_height_u32
        || build_state.world_center.distance_squared(world_center) > 0.0001
        || (build_state.map_zoom - map_zoom_px_per_world).abs() > 0.0001;
    if !needs_rebuild && !params_changed {
        return;
    }
    if needs_rebuild {
        *image = Image::new_fill(
            Extent3d {
                width: TACTICAL_FOG_MASK_RESOLUTION,
                height: TACTICAL_FOG_MASK_RESOLUTION,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[255],
            TextureFormat::R8Unorm,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );
    }
    let Some(mask) = image.data.as_mut() else {
        return;
    };
    let cell_size_m = fog_cache.cell_size_m;
    if cell_size_m <= 0.0
        || map_zoom_px_per_world <= 0.0
        || viewport_width_px <= 0.0
        || viewport_height_px <= 0.0
    {
        mask.fill(255);
        build_state.initialized = true;
        build_state.fog_revision = fog_cache.revision;
        build_state.viewport_width_px = viewport_width_u32;
        build_state.viewport_height_px = viewport_height_u32;
        build_state.world_center = world_center;
        build_state.map_zoom = map_zoom_px_per_world;
        return;
    }

    let width = TACTICAL_FOG_MASK_RESOLUTION as usize;
    let height = TACTICAL_FOG_MASK_RESOLUTION as usize;
    let width_f = TACTICAL_FOG_MASK_RESOLUTION as f32;
    let height_f = TACTICAL_FOG_MASK_RESOLUTION as f32;

    for y in 0..height {
        let sample_screen_y = ((y as f32 + 0.5) / height_f) * viewport_height_px;
        let world_y =
            world_center.y + (viewport_height_px * 0.5 - sample_screen_y) / map_zoom_px_per_world;
        let cell_y = (world_y / cell_size_m).floor() as i32;
        for x in 0..width {
            let sample_screen_x = ((x as f32 + 0.5) / width_f) * viewport_width_px;
            let world_x = world_center.x
                + (sample_screen_x - viewport_width_px * 0.5) / map_zoom_px_per_world;
            let cell_x = (world_x / cell_size_m).floor() as i32;
            let index = y * width + x;
            mask[index] = if fog_cache.revealed_cells.contains(&sidereal_net::GridCell {
                x: cell_x,
                y: cell_y,
            }) {
                255
            } else {
                0
            };
        }
    }
    build_state.initialized = true;
    build_state.fog_revision = fog_cache.revision;
    build_state.viewport_width_px = viewport_width_u32;
    build_state.viewport_height_px = viewport_height_u32;
    build_state.world_center = world_center;
    build_state.map_zoom = map_zoom_px_per_world;
}

fn ids_refer_to_same_guid(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    parse_guid_from_entity_id(left)
        .zip(parse_guid_from_entity_id(right))
        .is_some_and(|(l, r)| l == r)
}

fn format_sector_code(x: f32, y: f32) -> String {
    let sector_size = 1000.0;
    let sector_x = (x / sector_size).floor() as i32;
    let sector_y = (y / sector_size).floor() as i32;
    let east_west = if sector_x >= 0 { 'E' } else { 'W' };
    let north_south = if sector_y >= 0 { 'N' } else { 'S' };
    format!(
        "{east_west}{:02}-{north_south}{:02}",
        sector_x.abs(),
        sector_y.abs()
    )
}

