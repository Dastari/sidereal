#[allow(clippy::type_complexity)]
pub(super) fn update_streamed_visual_layer_transforms_system(
    camera_motion: Res<'_, CameraMotionState>,
    parents: Query<'_, '_, &'_ ResolvedRuntimeRenderLayer>,
    mut children: Query<'_, '_, (&'_ ChildOf, &'_ mut Transform), With<StreamedVisualChild>>,
) {
    for (parent, mut transform) in &mut children {
        let Ok(layer) = parents.get(parent.parent()) else {
            continue;
        };
        let (x, y, z) =
            streamed_visual_layer_transform(Some(layer), camera_motion.parallax_position_xy);
        transform.translation.x = x;
        transform.translation.y = y;
        transform.translation.z = z;
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn cleanup_planet_body_visual_children_system(
    mut commands: Commands<'_, '_>,
    parents: Query<
        '_,
        '_,
        (
            Entity,
            &'_ Children,
            Option<&'_ PlanetBodyShaderSettings>,
            Option<&'_ RuntimeWorldVisualStack>,
            Option<&'_ RuntimeWorldVisualPassSet>,
            Has<SuppressedPredictedDuplicateVisual>,
        ),
        With<WorldEntity>,
    >,
    visual_children: Query<'_, '_, &'_ RuntimeWorldVisualPass>,
) {
    for (parent_entity, children, planet_settings, visual_stack, pass_set, is_suppressed) in
        &parents
    {
        let should_clear_all_visuals = planet_settings.is_none()
            || is_suppressed
            || !planet_settings.is_some_and(|v| v.enabled);
        let mut removed_any_child = false;
        let desired_pass_set =
            desired_world_visual_pass_set(visual_stack, RuntimeWorldVisualFamily::Planet);
        let mut next_pass_set = pass_set.copied().unwrap_or_default();
        for child in children.iter() {
            let Ok(pass) = visual_children.get(child) else {
                continue;
            };
            if pass.family != RuntimeWorldVisualFamily::Planet {
                continue;
            }
            let remove_child = should_clear_all_visuals || !desired_pass_set.contains(pass.kind);
            if remove_child {
                queue_despawn_if_exists(&mut commands, child);
                next_pass_set.remove(pass.kind);
                removed_any_child = true;
            }
        }
        if (pass_set.is_some() || removed_any_child)
            && let Ok(mut parent_commands) = commands.get_entity(parent_entity)
        {
            if should_clear_all_visuals || desired_pass_set.is_empty() {
                parent_commands.remove::<RuntimeWorldVisualPassSet>();
            } else {
                parent_commands.insert(desired_pass_set);
            }
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn attach_planet_visual_stack_system(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut quad_mesh: ResMut<'_, RuntimeSharedQuadMesh>,
    mut planet_materials: ResMut<'_, Assets<PlanetVisualMaterial>>,
    mut star_materials: ResMut<'_, Assets<StarVisualMaterial>>,
    time: Res<'_, Time>,
    camera_motion: Res<'_, CameraMotionState>,
    shader_assignments: Res<'_, shaders::RuntimeShaderAssignments>,
    world_lighting: Res<'_, WorldLightingState>,
    camera_local_lights: Res<'_, CameraLocalLightSet>,
    mut candidates: Query<
        '_,
        '_,
        (
            Entity,
            &'_ PlanetBodyShaderSettings,
            Option<&'_ RuntimeWorldVisualStack>,
            Option<&'_ SizeM>,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ WorldPosition>,
            Option<&'_ WorldRotation>,
            Option<&'_ ResolvedRuntimeRenderLayer>,
            &'_ mut Visibility,
            Option<&'_ RuntimeWorldVisualPassSet>,
        ),
        (
            With<WorldEntity>,
            Without<PlayerTag>,
            Without<ControlledEntityGuid>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
) {
    if !shader_materials_enabled() {
        return;
    }
    let camera_world_position_xy = camera_motion.parallax_position_xy;
    for (
        entity,
        settings,
        visual_stack,
        size_m,
        position,
        rotation,
        world_position,
        world_rotation,
        resolved_render_layer,
        mut visibility,
        pass_set,
    ) in &mut candidates
    {
        if !settings.enabled {
            continue;
        }
        let Ok(mut entity_commands) = commands.get_entity(entity) else {
            continue;
        };
        ensure_visual_parent_spatial_components(&mut entity_commands);
        let time_s = time.elapsed_secs();
        let world_position = resolve_world_position(position, world_position)
            .unwrap_or(DVec2::ZERO)
            .as_vec2();
        let root_rotation_rad =
            resolve_world_rotation_rad(rotation, world_rotation).unwrap_or(0.0) as f32;
        let diameter_m = size_m
            .map(|v| v.length.max(v.width).max(1.0))
            .unwrap_or(256.0);
        let layer_base_z = planet_layer_base_z(resolved_render_layer);
        let projected_center_world = planet_camera_relative_translation(
            resolved_render_layer,
            world_position,
            camera_world_position_xy,
        );
        let child_translation_xy = planet_visual_child_translation(
            projected_center_world,
            world_position,
            root_rotation_rad,
        );
        let child_rotation = planet_visual_child_rotation(root_rotation_rad);
        let layer_screen_scale = resolved_render_layer
            .map(|layer| runtime_layer_screen_scale_factor(&layer.definition))
            .unwrap_or(1.0);
        let mut next_pass_set = pass_set.copied().unwrap_or_default();
        let Some(body_pass) =
            find_world_visual_pass(visual_stack, RuntimeWorldVisualPassKind::PlanetBody)
        else {
            continue;
        };
        if !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetBody) {
            let Some(body_shader_kind) =
                shaders::world_polygon_shader_kind(&shader_assignments, &body_pass.shader_asset_id)
            else {
                continue;
            };
            let mesh = shared_unit_quad_handle(&mut quad_mesh, &mut meshes);
            let params = PlanetBodyUniforms::from_settings(
                settings,
                time_s,
                world_position,
                &world_lighting,
                &camera_local_lights,
            );
            let scale_multiplier = visual_pass_scale_multiplier(Some(body_pass), 1.0);
            let depth_bias_z = visual_pass_depth_bias_z(Some(body_pass), 0.0);
            entity_commands.with_children(|child| {
                let transform = Transform::from_xyz(
                    child_translation_xy.x,
                    child_translation_xy.y,
                    layer_base_z + PLANET_BODY_LAYER_Z_OFFSET + depth_bias_z,
                )
                .with_rotation(child_rotation)
                .with_scale(Vec3::new(
                    diameter_m * scale_multiplier * layer_screen_scale,
                    diameter_m * scale_multiplier * layer_screen_scale,
                    1.0,
                ));
                match body_shader_kind {
                    shaders::RuntimeWorldPolygonShaderKind::PlanetVisual => {
                        child.spawn((
                            pass_tag(
                                RuntimeWorldVisualFamily::Planet,
                                RuntimeWorldVisualPassKind::PlanetBody,
                            ),
                            NoFrustumCulling,
                            Mesh2d(mesh.clone()),
                            RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                            PlanetProjectedCullRetention::default(),
                            transform,
                            MeshMaterial2d(planet_materials.add(PlanetVisualMaterial {
                                params: params.clone(),
                            })),
                        ));
                    }
                    shaders::RuntimeWorldPolygonShaderKind::StarVisual => {
                        child.spawn((
                            pass_tag(
                                RuntimeWorldVisualFamily::Planet,
                                RuntimeWorldVisualPassKind::PlanetBody,
                            ),
                            NoFrustumCulling,
                            Mesh2d(mesh),
                            RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                            PlanetProjectedCullRetention::default(),
                            transform,
                            MeshMaterial2d(star_materials.add(StarVisualMaterial { params })),
                        ));
                    }
                }
            });
            next_pass_set.insert(RuntimeWorldVisualPassKind::PlanetBody);
        }
        if let (Some(back_pass), Some(front_pass)) = (
            find_world_visual_pass(visual_stack, RuntimeWorldVisualPassKind::PlanetCloudBack),
            find_world_visual_pass(visual_stack, RuntimeWorldVisualPassKind::PlanetCloudFront),
        ) && (!next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetCloudBack)
            || !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetCloudFront))
            && shaders::world_polygon_shader_kind(&shader_assignments, &back_pass.shader_asset_id)
                == Some(shaders::RuntimeWorldPolygonShaderKind::PlanetVisual)
            && shaders::world_polygon_shader_kind(&shader_assignments, &front_pass.shader_asset_id)
                == Some(shaders::RuntimeWorldPolygonShaderKind::PlanetVisual)
        {
            let back_material = planet_materials.add(PlanetVisualMaterial {
                params: PlanetBodyUniforms::from_settings_with_pass(
                    settings,
                    time_s,
                    world_position,
                    Vec4::new(1.0, 0.0, 0.0, 0.0),
                    &world_lighting,
                    &camera_local_lights,
                ),
            });
            let front_material = planet_materials.add(PlanetVisualMaterial {
                params: PlanetBodyUniforms::from_settings_with_pass(
                    settings,
                    time_s,
                    world_position,
                    Vec4::new(2.0, 0.0, 0.0, 0.0),
                    &world_lighting,
                    &camera_local_lights,
                ),
            });
            let back_scale = visual_pass_scale_multiplier(Some(back_pass), 1.035);
            let front_scale = visual_pass_scale_multiplier(Some(front_pass), 1.035);
            let back_depth = visual_pass_depth_bias_z(Some(back_pass), -0.2);
            let front_depth = visual_pass_depth_bias_z(Some(front_pass), 0.5);
            entity_commands.with_children(|child| {
                if !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetCloudBack) {
                    child.spawn((
                        pass_tag(
                            RuntimeWorldVisualFamily::Planet,
                            RuntimeWorldVisualPassKind::PlanetCloudBack,
                        ),
                        NoFrustumCulling,
                        Mesh2d(shared_unit_quad_handle(&mut quad_mesh, &mut meshes)),
                        MeshMaterial2d(back_material),
                        RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                        PlanetProjectedCullRetention::default(),
                        Transform::from_xyz(
                            child_translation_xy.x,
                            child_translation_xy.y,
                            layer_base_z + PLANET_CLOUD_BACK_LAYER_Z_OFFSET + back_depth,
                        )
                        .with_rotation(child_rotation)
                        .with_scale(Vec3::new(
                            diameter_m * back_scale * layer_screen_scale,
                            diameter_m * back_scale * layer_screen_scale,
                            1.0,
                        )),
                    ));
                }
                if !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetCloudFront) {
                    child.spawn((
                        pass_tag(
                            RuntimeWorldVisualFamily::Planet,
                            RuntimeWorldVisualPassKind::PlanetCloudFront,
                        ),
                        NoFrustumCulling,
                        Mesh2d(shared_unit_quad_handle(&mut quad_mesh, &mut meshes)),
                        MeshMaterial2d(front_material),
                        RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                        PlanetProjectedCullRetention::default(),
                        Transform::from_xyz(
                            child_translation_xy.x,
                            child_translation_xy.y,
                            layer_base_z + PLANET_CLOUD_FRONT_LAYER_Z_OFFSET + front_depth,
                        )
                        .with_rotation(child_rotation)
                        .with_scale(Vec3::new(
                            diameter_m * front_scale * layer_screen_scale,
                            diameter_m * front_scale * layer_screen_scale,
                            1.0,
                        )),
                    ));
                }
            });
            next_pass_set.insert(RuntimeWorldVisualPassKind::PlanetCloudBack);
            next_pass_set.insert(RuntimeWorldVisualPassKind::PlanetCloudFront);
        }
        if let (Some(back_pass), Some(front_pass)) = (
            find_world_visual_pass(visual_stack, RuntimeWorldVisualPassKind::PlanetRingBack),
            find_world_visual_pass(visual_stack, RuntimeWorldVisualPassKind::PlanetRingFront),
        ) && (!next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetRingBack)
            || !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetRingFront))
            && shaders::world_polygon_shader_kind(&shader_assignments, &back_pass.shader_asset_id)
                == Some(shaders::RuntimeWorldPolygonShaderKind::PlanetVisual)
            && shaders::world_polygon_shader_kind(&shader_assignments, &front_pass.shader_asset_id)
                == Some(shaders::RuntimeWorldPolygonShaderKind::PlanetVisual)
        {
            let back_material = planet_materials.add(PlanetVisualMaterial {
                params: PlanetBodyUniforms::from_settings_with_pass(
                    settings,
                    time_s,
                    world_position,
                    Vec4::new(0.0, 1.0, 0.0, 0.0),
                    &world_lighting,
                    &camera_local_lights,
                ),
            });
            let front_material = planet_materials.add(PlanetVisualMaterial {
                params: PlanetBodyUniforms::from_settings_with_pass(
                    settings,
                    time_s,
                    world_position,
                    Vec4::new(0.0, 2.0, 0.0, 0.0),
                    &world_lighting,
                    &camera_local_lights,
                ),
            });
            let back_scale = visual_pass_scale_multiplier(Some(back_pass), 1.85);
            let front_scale = visual_pass_scale_multiplier(Some(front_pass), 1.85);
            let back_depth = visual_pass_depth_bias_z(Some(back_pass), -0.45);
            let front_depth = visual_pass_depth_bias_z(Some(front_pass), 0.65);
            entity_commands.with_children(|child| {
                if !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetRingBack) {
                    child.spawn((
                        pass_tag(
                            RuntimeWorldVisualFamily::Planet,
                            RuntimeWorldVisualPassKind::PlanetRingBack,
                        ),
                        NoFrustumCulling,
                        Mesh2d(shared_unit_quad_handle(&mut quad_mesh, &mut meshes)),
                        MeshMaterial2d(back_material),
                        RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                        PlanetProjectedCullRetention::default(),
                        Transform::from_xyz(
                            child_translation_xy.x,
                            child_translation_xy.y,
                            layer_base_z + PLANET_RING_BACK_LAYER_Z_OFFSET + back_depth,
                        )
                        .with_rotation(child_rotation)
                        .with_scale(Vec3::new(
                            diameter_m * back_scale * layer_screen_scale,
                            diameter_m * back_scale * layer_screen_scale,
                            1.0,
                        )),
                    ));
                }
                if !next_pass_set.contains(RuntimeWorldVisualPassKind::PlanetRingFront) {
                    child.spawn((
                        pass_tag(
                            RuntimeWorldVisualFamily::Planet,
                            RuntimeWorldVisualPassKind::PlanetRingFront,
                        ),
                        NoFrustumCulling,
                        Mesh2d(shared_unit_quad_handle(&mut quad_mesh, &mut meshes)),
                        MeshMaterial2d(front_material),
                        RenderLayers::layer(PLANET_BODY_RENDER_LAYER),
                        PlanetProjectedCullRetention::default(),
                        Transform::from_xyz(
                            child_translation_xy.x,
                            child_translation_xy.y,
                            layer_base_z + PLANET_RING_FRONT_LAYER_Z_OFFSET + front_depth,
                        )
                        .with_rotation(child_rotation)
                        .with_scale(Vec3::new(
                            diameter_m * front_scale * layer_screen_scale,
                            diameter_m * front_scale * layer_screen_scale,
                            1.0,
                        )),
                    ));
                }
            });
            next_pass_set.insert(RuntimeWorldVisualPassKind::PlanetRingBack);
            next_pass_set.insert(RuntimeWorldVisualPassKind::PlanetRingFront);
        }
        *visibility = Visibility::Visible;
        entity_commands.insert(next_pass_set);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn ensure_planet_body_root_visibility_system(
    mut planets: Query<
        '_,
        '_,
        (
            &'_ PlanetBodyShaderSettings,
            &'_ mut Visibility,
            Option<&'_ PendingInitialVisualReady>,
        ),
        (
            With<WorldEntity>,
            Without<SuppressedPredictedDuplicateVisual>,
            Without<PlayerTag>,
            Without<ControlledEntityGuid>,
        ),
    >,
) {
    for (settings, mut visibility, pending_initial_visual_ready) in &mut planets {
        if !settings.enabled {
            continue;
        }
        if pending_initial_visual_ready.is_some() {
            *visibility = Visibility::Hidden;
            continue;
        }
        if *visibility != Visibility::Visible {
            *visibility = Visibility::Visible;
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn update_planet_body_visuals_system(
    time: Res<'_, Time>,
    camera_motion: Res<'_, CameraMotionState>,
    world_lighting: Res<'_, WorldLightingState>,
    camera_local_lights: Res<'_, CameraLocalLightSet>,
    mut materials: ResMut<'_, Assets<PlanetVisualMaterial>>,
    mut star_materials: ResMut<'_, Assets<StarVisualMaterial>>,
    planet_camera: Query<
        '_,
        '_,
        (&'_ Camera, &'_ Projection, &'_ GlobalTransform),
        With<PlanetBodyCamera>,
    >,
    planets: Query<
        '_,
        '_,
        (
            &'_ Children,
            &'_ PlanetBodyShaderSettings,
            Option<&'_ RuntimeWorldVisualStack>,
            Option<&'_ SizeM>,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ WorldPosition>,
            Option<&'_ WorldRotation>,
            Option<&'_ ResolvedRuntimeRenderLayer>,
        ),
    >,
    mut planet_visuals: Query<
        '_,
        '_,
        (
            &'_ RuntimeWorldVisualPass,
            Option<&'_ MeshMaterial2d<PlanetVisualMaterial>>,
            Option<&'_ MeshMaterial2d<StarVisualMaterial>>,
            &'_ mut Transform,
            &'_ mut Visibility,
            Option<&'_ mut PlanetProjectedCullRetention>,
        ),
        (),
    >,
    mut cull_state: Local<'_, PlanetProjectedCullRuntimeState>,
) {
    let time_s = time.elapsed_secs();
    let now_s = time.elapsed_secs_f64();
    let camera_view = planet_camera.single().ok();
    let rapid_zoom_out = cull_state.update(
        now_s,
        camera_view.and_then(|(_, projection, _)| match projection {
            Projection::Orthographic(orthographic) => Some(orthographic.scale),
            _ => None,
        }),
    );
    let camera_world_position_xy = camera_motion.parallax_position_xy;
    for (
        children,
        settings,
        visual_stack,
        size_m,
        position,
        rotation,
        world_position,
        world_rotation,
        resolved_render_layer,
    ) in &planets
    {
        if !settings.enabled {
            continue;
        }
        let world_position = resolve_world_position(position, world_position)
            .unwrap_or(DVec2::ZERO)
            .as_vec2();
        let root_rotation_rad =
            resolve_world_rotation_rad(rotation, world_rotation).unwrap_or(0.0) as f32;
        let diameter_m = size_m
            .map(|v| v.length.max(v.width).max(1.0))
            .unwrap_or(256.0);
        let layer_base_z = planet_layer_base_z(resolved_render_layer);
        let projected_center_world = planet_camera_relative_translation(
            resolved_render_layer,
            world_position,
            camera_world_position_xy,
        );
        let child_translation_xy = planet_visual_child_translation(
            projected_center_world,
            world_position,
            root_rotation_rad,
        );
        let child_rotation = planet_visual_child_rotation(root_rotation_rad);
        let layer_screen_scale = resolved_render_layer
            .map(|layer| runtime_layer_screen_scale_factor(&layer.definition))
            .unwrap_or(1.0);
        for child in children.iter() {
            if let Ok((pass, planet_material, star_material, mut transform, mut visibility, retention)) =
                planet_visuals.get_mut(child)
            {
                if pass.family != RuntimeWorldVisualFamily::Planet {
                    continue;
                }
                let mut projected_radius_m = 0.0;
                if planet_material.is_some() || star_material.is_some() {
                    let pass_definition = find_world_visual_pass(visual_stack, pass.kind);
                    let (pass_flags, base_z, base_scale) = match pass.kind {
                        RuntimeWorldVisualPassKind::PlanetBody => {
                            (Vec4::ZERO, layer_base_z + PLANET_BODY_LAYER_Z_OFFSET, 1.0)
                        }
                        RuntimeWorldVisualPassKind::PlanetCloudBack => (
                            Vec4::new(1.0, 0.0, 0.0, 0.0),
                            layer_base_z + PLANET_CLOUD_BACK_LAYER_Z_OFFSET,
                            1.035,
                        ),
                        RuntimeWorldVisualPassKind::PlanetCloudFront => (
                            Vec4::new(2.0, 0.0, 0.0, 0.0),
                            layer_base_z + PLANET_CLOUD_FRONT_LAYER_Z_OFFSET,
                            1.035,
                        ),
                        RuntimeWorldVisualPassKind::PlanetRingBack => (
                            Vec4::new(0.0, 1.0, 0.0, 0.0),
                            layer_base_z + PLANET_RING_BACK_LAYER_Z_OFFSET,
                            1.85,
                        ),
                        RuntimeWorldVisualPassKind::PlanetRingFront => (
                            Vec4::new(0.0, 2.0, 0.0, 0.0),
                            layer_base_z + PLANET_RING_FRONT_LAYER_Z_OFFSET,
                            1.85,
                        ),
                        _ => continue,
                    };
                    let material_params = PlanetBodyUniforms::from_settings_with_pass(
                        settings,
                        time_s,
                        world_position,
                        pass_flags,
                        &world_lighting,
                        &camera_local_lights,
                    );
                    if let Some(material_handle) = planet_material
                        && let Some(material) = materials.get_mut(&material_handle.0)
                    {
                        material.params = material_params.clone();
                    }
                    if let Some(material_handle) = star_material
                        && let Some(material) = star_materials.get_mut(&material_handle.0)
                    {
                        material.params = material_params;
                    }
                    transform.translation.z =
                        base_z + visual_pass_depth_bias_z(pass_definition, 0.0);
                    let scale_multiplier =
                        visual_pass_scale_multiplier(pass_definition, base_scale);
                    let projected_diameter_m = diameter_m * scale_multiplier * layer_screen_scale;
                    transform.scale = Vec3::new(projected_diameter_m, projected_diameter_m, 1.0);
                    projected_radius_m = projected_diameter_m * 0.5;
                }
                transform.translation.x = child_translation_xy.x;
                transform.translation.y = child_translation_xy.y;
                transform.rotation = child_rotation;
                let in_projected_view =
                    camera_view.is_none_or(|(camera, projection, camera_transform)| {
                        projected_planet_intersects_camera_view(
                            projected_center_world,
                            projected_radius_m,
                            rapid_zoom_out,
                            camera,
                            projection,
                            camera_transform,
                        )
                    });
                let is_visible =
                    planet_projected_visibility_with_retention(in_projected_view, retention, now_s);
                *visibility = if is_visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

fn runtime_layer_parallax_factor(definition: &RuntimeRenderLayerDefinition) -> f32 {
    definition.parallax_factor.unwrap_or(1.0).clamp(0.01, 4.0)
}

fn runtime_layer_z_bias(definition: &RuntimeRenderLayerDefinition) -> f32 {
    definition.depth_bias_z.unwrap_or(definition.order as f32)
}

pub(super) fn runtime_layer_screen_scale_factor(definition: &RuntimeRenderLayerDefinition) -> f32 {
    definition
        .screen_scale_factor
        .unwrap_or(1.0)
        .clamp(0.01, 64.0)
}

fn planet_layer_base_z(resolved_render_layer: Option<&ResolvedRuntimeRenderLayer>) -> f32 {
    resolved_render_layer
        .map(|layer| runtime_layer_z_bias(&layer.definition))
        .unwrap_or(-60.0)
}

fn planet_visual_child_translation(
    projected_center_world: Vec2,
    parent_world_position: Vec2,
    parent_rotation_rad: f32,
) -> Vec2 {
    Mat2::from_angle(-parent_rotation_rad) * (projected_center_world - parent_world_position)
}

fn planet_visual_child_rotation(parent_rotation_rad: f32) -> Quat {
    Quat::from_rotation_z(-parent_rotation_rad)
}

pub(super) fn planet_camera_relative_translation(
    resolved_render_layer: Option<&ResolvedRuntimeRenderLayer>,
    planet_world_position: Vec2,
    camera_world_position_xy: Vec2,
) -> Vec2 {
    let parallax_factor = resolved_render_layer
        .map(|layer| runtime_layer_parallax_factor(&layer.definition))
        .unwrap_or(1.0);
    (planet_world_position - camera_world_position_xy) * parallax_factor
}

fn projected_planet_intersects_camera_view(
    projected_center_world: Vec2,
    projected_radius_m: f32,
    rapid_zoom_out: bool,
    camera: &Camera,
    projection: &Projection,
    camera_transform: &GlobalTransform,
) -> bool {
    let Some(viewport_size) = camera.logical_viewport_size() else {
        return false;
    };
    let Projection::Orthographic(orthographic) = projection else {
        return true;
    };
    let half_extents_world = viewport_size * orthographic.scale * 0.5;
    let radius_with_buffer = projected_radius_m.max(0.0)
        + planet_projected_cull_buffer_m(
            viewport_size,
            orthographic.scale,
            projected_radius_m,
            rapid_zoom_out,
        );
    let delta = projected_center_world - camera_transform.translation().truncate();
    delta.x >= -half_extents_world.x - radius_with_buffer
        && delta.x <= half_extents_world.x + radius_with_buffer
        && delta.y >= -half_extents_world.y - radius_with_buffer
        && delta.y <= half_extents_world.y + radius_with_buffer
}

fn planet_projected_cull_buffer_m(
    viewport_size: Vec2,
    orthographic_scale: f32,
    projected_radius_m: f32,
    rapid_zoom_out: bool,
) -> f32 {
    if !viewport_size.is_finite() || !orthographic_scale.is_finite() || orthographic_scale <= 0.0 {
        return projected_radius_m.max(0.0);
    }
    let viewport_margin_ratio = if rapid_zoom_out {
        PLANET_PROJECTED_CULL_ZOOM_OUT_VIEWPORT_MARGIN
    } else {
        PLANET_PROJECTED_CULL_STATIC_VIEWPORT_MARGIN
    };
    let viewport_margin_m =
        viewport_size.max_element() * orthographic_scale * viewport_margin_ratio;
    let minimum_margin_m =
        (PLANET_PROJECTED_CULL_MIN_MARGIN_PX * orthographic_scale).max(projected_radius_m.max(0.0));
    viewport_margin_m.max(minimum_margin_m)
}

fn planet_projected_visibility_with_retention(
    in_projected_view: bool,
    retention: Option<Mut<'_, PlanetProjectedCullRetention>>,
    now_s: f64,
) -> bool {
    let Some(mut retention) = retention else {
        return in_projected_view;
    };
    if in_projected_view {
        retention.visible_until_s = now_s + PLANET_PROJECTED_CULL_RETENTION_GRACE_S;
        return true;
    }
    retention.visible_until_s >= now_s
}

fn streamed_visual_layer_transform(
    resolved_render_layer: Option<&ResolvedRuntimeRenderLayer>,
    camera_world_position_xy: Vec2,
) -> (f32, f32, f32) {
    let Some(layer) = resolved_render_layer else {
        return (0.0, 0.0, STREAMED_VISUAL_BASE_LAYER_Z);
    };
    let parallax_factor = runtime_layer_parallax_factor(&layer.definition);
    let parallax_offset = -camera_world_position_xy * (1.0 - parallax_factor);
    (
        parallax_offset.x,
        parallax_offset.y,
        STREAMED_VISUAL_BASE_LAYER_Z + runtime_layer_z_bias(&layer.definition),
    )
}
