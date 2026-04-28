#[allow(clippy::type_complexity)]
pub(super) fn attach_thruster_plume_visuals_system(
    mut commands: Commands<'_, '_>,
    mut assets: ThrusterPlumeAttachAssets<'_>,
    visual_children: Query<'_, '_, &'_ RuntimeWorldVisualPass>,
    engines: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityLabels,
            Option<&'_ Children>,
            Option<&'_ RuntimeWorldVisualPassSet>,
        ),
        (
            With<WorldEntity>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
) {
    if !shader_materials_enabled() {
        return;
    }
    for (entity, labels, children, pass_set) in &engines {
        if !has_engine_label(labels) {
            continue;
        }
        let has_existing_plume_child = children.is_some_and(|children| {
            children.iter().any(|child| {
                visual_children.get(child).is_ok_and(|pass| {
                    pass.family == RuntimeWorldVisualFamily::Thruster
                        && pass.kind == RuntimeWorldVisualPassKind::ThrusterPlume
                })
            })
        });
        if has_existing_plume_child {
            continue;
        }
        let Ok(mut entity_commands) = commands.get_entity(entity) else {
            continue;
        };
        let plume_mesh = shared_unit_quad_handle(&mut assets.quad_mesh, &mut assets.meshes);
        let plume_material = assets.plume_materials.add(RuntimeEffectMaterial {
            lighting: SharedWorldLightingUniforms::from_state_for_world_position(
                &assets.world_lighting,
                Vec2::ZERO,
                &assets.camera_local_lights,
            ),
            ..RuntimeEffectMaterial::default()
        });
        entity_commands.with_children(|child| {
            child.spawn((
                pass_tag(
                    RuntimeWorldVisualFamily::Thruster,
                    RuntimeWorldVisualPassKind::ThrusterPlume,
                ),
                Mesh2d(plume_mesh),
                MeshMaterial2d(plume_material),
                Transform::from_xyz(0.0, -0.2, 0.1).with_scale(Vec3::new(1.0, 0.02, 1.0)),
                Visibility::Visible,
            ));
        });
        let mut next_pass_set = pass_set.copied().unwrap_or_default();
        next_pass_set.insert(RuntimeWorldVisualPassKind::ThrusterPlume);
        entity_commands.insert(next_pass_set);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn update_thruster_plume_visuals_system(
    time: Res<'_, Time>,
    world_lighting: Res<'_, WorldLightingState>,
    camera_local_lights: Res<'_, CameraLocalLightSet>,
    mut plume_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut plume_children: Query<
        '_,
        '_,
        (
            &'_ RuntimeWorldVisualPass,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Transform,
            &'_ GlobalTransform,
            &'_ mut Visibility,
        ),
        (),
    >,
    engines: Query<
        '_,
        '_,
        (
            &'_ EntityLabels,
            Option<&'_ Children>,
            &'_ MountedOn,
            Option<&'_ AfterburnerCapability>,
            Option<&'_ ThrusterPlumeShaderSettings>,
        ),
    >,
    hulls: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            Option<&'_ MountedOn>,
            &'_ FlightComputer,
            Option<&'_ AfterburnerState>,
        ),
    >,
) {
    let mut hull_state = HashMap::<uuid::Uuid, (f32, bool)>::new();
    for (guid, mounted_on, computer, afterburner_state) in &hulls {
        let thrust_alpha = computer.throttle.max(0.0).clamp(0.0, 1.0);
        let afterburner_active = afterburner_state.is_some_and(|state| state.active);
        let hull_guid = mounted_on
            .map(|mounted_on| mounted_on.parent_entity_id)
            .unwrap_or(guid.0);
        hull_state.insert(hull_guid, (thrust_alpha, afterburner_active));
    }

    for (labels, children, mounted_on, afterburner_capability, plume_settings) in &engines {
        if !has_engine_label(labels) {
            continue;
        }
        let Some((thrust_alpha, afterburner_active)) =
            hull_state.get(&mounted_on.parent_entity_id).copied()
        else {
            continue;
        };
        let settings = plume_settings.cloned().unwrap_or_default();
        if !settings.enabled {
            if let Some(children) = children {
                for child in children.iter() {
                    if let Ok((_, _, _, _, mut visibility)) = plume_children.get_mut(child) {
                        *visibility = Visibility::Hidden;
                    }
                }
            }
            continue;
        }
        let live_afterburner =
            afterburner_active && afterburner_capability.is_some_and(|cap| cap.enabled);
        let thrust_alpha = if settings.debug_override_enabled {
            settings.debug_forced_thrust_alpha.clamp(0.0, 1.0)
        } else {
            thrust_alpha
        };
        let can_afterburn = if settings.debug_override_enabled {
            settings.debug_force_afterburner
        } else {
            live_afterburner
        };
        let base_length = settings.base_length_m.max(0.0);
        let max_length = settings.max_length_m.max(base_length);
        let reactive_length = (thrust_alpha * settings.reactive_length_scale).clamp(0.0, 1.0);
        let mut plume_length = base_length + (max_length - base_length) * reactive_length;
        if can_afterburn {
            plume_length *= settings.afterburner_length_scale.max(1.0);
        }
        plume_length = plume_length.max(0.02);

        let base_width = settings.base_width_m.max(0.02);
        let max_width = settings.max_width_m.max(base_width);
        let plume_width = base_width + (max_width - base_width) * reactive_length;

        let mut plume_alpha = settings.idle_core_alpha
            + (settings.max_alpha - settings.idle_core_alpha).max(0.0)
                * (thrust_alpha * settings.reactive_alpha_scale).clamp(0.0, 1.0);
        if can_afterburn {
            plume_alpha += settings.afterburner_alpha_boost.max(0.0);
        }
        plume_alpha = plume_alpha.clamp(0.0, 1.0);
        let afterburner_alpha = if can_afterburn { 1.0 } else { 0.0 };

        let Some(children) = children else {
            continue;
        };
        for child in children.iter() {
            let Ok((pass, material_handle, mut transform, global_transform, mut visibility)) =
                plume_children.get_mut(child)
            else {
                continue;
            };
            if pass.kind != RuntimeWorldVisualPassKind::ThrusterPlume {
                continue;
            }
            if let Some(material) = plume_materials.get_mut(&material_handle.0) {
                material.lighting = SharedWorldLightingUniforms::from_state_for_world_position(
                    &world_lighting,
                    global_transform.translation().truncate(),
                    &camera_local_lights,
                );
                material.params = RuntimeEffectUniforms::thruster_plume(
                    thrust_alpha.clamp(0.0, 1.0),
                    afterburner_alpha,
                    time.elapsed_secs(),
                    plume_alpha,
                    settings.falloff.max(0.05),
                    settings.edge_softness.max(0.1),
                    settings.noise_strength.max(0.0),
                    settings.flicker_hz.max(0.0),
                    settings.base_color_rgb.extend(1.0),
                    settings.hot_color_rgb.extend(1.0),
                    settings.afterburner_color_rgb.extend(1.0),
                );
            }
            transform.translation = Vec3::new(0.0, -(plume_length * 0.5 + plume_width * 0.18), 0.1);
            transform.scale = Vec3::new(plume_width, plume_length, 1.0);
            *visibility = if plume_alpha > 0.001 {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn update_world_sprite_shader_lighting_system(
    world_lighting: Res<'_, WorldLightingState>,
    camera_local_lights: Res<'_, CameraLocalLightSet>,
    mut generic_materials: ResMut<'_, Assets<StreamedSpriteShaderMaterial>>,
    mut asteroid_materials: ResMut<'_, Assets<AsteroidSpriteShaderMaterial>>,
    parents: Query<
        '_,
        '_,
        (
            &'_ Children,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ WorldPosition>,
            Option<&'_ WorldRotation>,
        ),
        (
            With<WorldEntity>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
    generic_children: Query<
        '_,
        '_,
        &'_ MeshMaterial2d<StreamedSpriteShaderMaterial>,
        With<StreamedVisualChild>,
    >,
    asteroid_children: Query<
        '_,
        '_,
        &'_ MeshMaterial2d<AsteroidSpriteShaderMaterial>,
        With<StreamedVisualChild>,
    >,
) {
    for (entity_children, position, rotation, world_position, world_rotation) in &parents {
        let lighting = SharedWorldLightingUniforms::from_state_for_world_position(
            &world_lighting,
            resolve_world_position(position, world_position)
                .unwrap_or(DVec2::ZERO)
                .as_vec2(),
            &camera_local_lights,
        );
        let local_rotation = shader_rotation_uniform(
            resolve_world_rotation_rad(rotation, world_rotation).unwrap_or(0.0),
        );
        for child in entity_children.iter() {
            if let Ok(material_handle) = generic_children.get(child)
                && let Some(material) = generic_materials.get_mut(&material_handle.0)
            {
                material.lighting = lighting.clone();
                material.local_rotation = local_rotation;
            }
            if let Ok(material_handle) = asteroid_children.get(child)
                && let Some(material) = asteroid_materials.get_mut(&material_handle.0)
            {
                material.lighting = lighting.clone();
                material.local_rotation = local_rotation;
            }
        }
    }
}
