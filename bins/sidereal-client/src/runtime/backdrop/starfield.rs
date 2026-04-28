#[allow(clippy::type_complexity)]
pub fn update_starfield_material_system(
    world_data: Res<'_, FullscreenExternalWorldData>,
    starfield_query: Query<
        '_,
        '_,
        (
            &'_ MeshMaterial2d<StarfieldMaterial>,
            &'_ StarfieldShaderSettings,
            Option<&'_ mut Visibility>,
        ),
        With<RuntimeFullscreenMaterialBinding>,
    >,
    mut materials: ResMut<'_, Assets<StarfieldMaterial>>,
) {
    for (material_handle, settings, maybe_visibility) in starfield_query {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.viewport_time = world_data.viewport_time;
            material.drift_intensity = world_data.drift_intensity;
            material.velocity_dir = world_data.velocity_dir;
            if let Some(mut visibility) = maybe_visibility {
                *visibility = if settings.enabled {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
            material.starfield_params = Vec4::new(
                settings.density.clamp(0.0, 1.0),
                settings.layer_count.clamp(1, 8) as f32,
                settings.initial_z_offset.clamp(0.0, 1.0),
                settings.alpha.clamp(0.0, 1.0),
            );
            material.starfield_tint = settings.tint_rgb.extend(settings.intensity.max(0.0));
            material.star_core_params = Vec4::new(
                settings.star_size.clamp(0.1, 10.0),
                settings.star_intensity.clamp(0.0, 10.0),
                settings.star_alpha.clamp(0.0, 1.0),
                0.0,
            );
            material.star_core_color = settings.star_color_rgb.extend(1.0);
            material.corona_params = Vec4::new(
                settings.corona_size.clamp(0.1, 10.0),
                settings.corona_intensity.clamp(0.0, 10.0),
                settings.corona_alpha.clamp(0.0, 1.0),
                0.0,
            );
            material.corona_color = settings.corona_color_rgb.extend(1.0);
        }
    }
}

