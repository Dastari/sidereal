#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(super) fn cleanup_streamed_visual_children_system(
    mut commands: Commands<'_, '_>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    mut last_reload_generation: Local<'_, u64>,
    cache_adapter: Res<'_, super::resources::AssetCacheAdapter>,
    shader_assignments: Res<'_, shaders::RuntimeShaderAssignments>,
    parents: Query<
        '_,
        '_,
        (
            Entity,
            &'_ Children,
            Option<&'_ StreamedVisualAssetId>,
            Option<&'_ StreamedSpriteShaderAssetId>,
            Option<&'_ ProceduralSprite>,
            Has<PlanetBodyShaderSettings>,
            Has<StreamedVisualAttached>,
            Option<&'_ StreamedVisualAttachmentKind>,
            Option<&'_ StreamedProceduralSpriteVisualFingerprint>,
            Has<SuppressedPredictedDuplicateVisual>,
            Option<&'_ PlayerTag>,
            Has<ControlledEntityGuid>,
        ),
        With<WorldEntity>,
    >,
    visual_children: Query<'_, '_, (), With<StreamedVisualChild>>,
) {
    let catalog_reloaded = *last_reload_generation != asset_manager.reload_generation;
    *last_reload_generation = asset_manager.reload_generation;
    for (
        parent_entity,
        children,
        visual_asset_id,
        sprite_shader_asset_id,
        procedural_sprite,
        has_planet_shader,
        has_visual_attached,
        attached_kind,
        procedural_visual_fingerprint,
        is_suppressed,
        player_tag,
        has_controlled_entity_guid,
    ) in &parents
    {
        let is_procedural_asteroid =
            procedural_sprite.is_some_and(|sprite| sprite.generator_id == "asteroid_rocky_v1");
        let world_sprite_kind = sprite_shader_asset_id
            .and_then(|shader| shaders::world_sprite_shader_kind(&shader_assignments, &shader.0))
            .or_else(|| {
                is_procedural_asteroid.then_some(shaders::RuntimeWorldSpriteShaderKind::Asteroid)
            });
        let has_streamed_sprite_shader_path = sprite_shader_asset_id.is_some_and(|shader| {
            shaders::world_sprite_shader_ready(
                &asset_root.0,
                &asset_manager,
                *cache_adapter,
                &shader.0,
            )
        });
        let desired_kind = if sprite_shader_asset_id.is_some()
            || procedural_sprite.is_some()
            || attached_kind.is_some()
        {
            Some(resolve_streamed_visual_material_kind(
                shader_materials_enabled(),
                world_sprite_kind,
                has_streamed_sprite_shader_path,
            ))
        } else {
            None
        };
        let desired_procedural_fingerprint = procedural_sprite
            .filter(|sprite| sprite.generator_id == "asteroid_rocky_v1")
            .map(procedural_sprite_fingerprint);
        let procedural_image_changed = procedural_visual_fingerprint
            .map(|stored| Some(stored.0) != desired_procedural_fingerprint)
            .unwrap_or_else(|| desired_procedural_fingerprint.is_some());
        let should_clear_visual = visual_asset_id.is_none()
            || catalog_reloaded
            || has_planet_shader
            || is_suppressed
            || player_tag.is_some()
            || has_controlled_entity_guid
            || procedural_image_changed
            || desired_kind.is_some_and(|desired| {
                streamed_visual_needs_rebuild(attached_kind.copied(), desired)
            });
        if !should_clear_visual {
            continue;
        }
        let mut removed_any_child = false;
        for child in children.iter() {
            if visual_children.get(child).is_ok() {
                queue_despawn_if_exists(&mut commands, child);
                removed_any_child = true;
            }
        }
        if (has_visual_attached || removed_any_child)
            && let Ok(mut parent_commands) = commands.get_entity(parent_entity)
        {
            parent_commands.remove::<(
                StreamedVisualAttached,
                StreamedVisualAttachmentKind,
                StreamedProceduralSpriteVisualFingerprint,
            )>();
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn attach_streamed_visual_assets_system(
    mut commands: Commands<'_, '_>,
    mut images: ResMut<'_, Assets<Image>>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    mut cached_assets: Local<'_, StreamedVisualAssetCaches>,
    cache_adapter: Res<'_, super::resources::AssetCacheAdapter>,
    shader_assignments: Res<'_, shaders::RuntimeShaderAssignments>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut quad_mesh: ResMut<'_, RuntimeSharedQuadMesh>,
    mut sprite_shader_materials: ResMut<'_, Assets<StreamedSpriteShaderMaterial>>,
    mut asteroid_shader_materials: ResMut<'_, Assets<AsteroidSpriteShaderMaterial>>,
    world_lighting: Res<'_, WorldLightingState>,
    camera_local_lights: Res<'_, CameraLocalLightSet>,
    candidates: Query<
        '_,
        '_,
        (
            Entity,
            &StreamedVisualAssetId,
            Option<&EntityGuid>,
            Option<&ProceduralSprite>,
            Option<&SizeM>,
            Option<&Position>,
            Option<&Rotation>,
            Option<&WorldPosition>,
            Option<&WorldRotation>,
            Option<&StreamedSpriteShaderAssetId>,
            Option<&ResolvedRuntimeRenderLayer>,
            Has<PlanetBodyShaderSettings>,
            Option<&PendingVisibilityFadeIn>,
        ),
        (
            With<WorldEntity>,
            Without<PlayerTag>,
            Without<ControlledEntityGuid>,
            Without<StreamedVisualAttached>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
) {
    if cached_assets.last_reload_generation != asset_manager.reload_generation {
        cached_assets.streamed_image_cache.clear();
        cached_assets.last_reload_generation = asset_manager.reload_generation;
    }
    let use_shader_materials = shader_materials_enabled();
    for (
        entity,
        asset_id,
        entity_guid,
        procedural_sprite,
        size_m,
        position,
        rotation,
        world_position,
        world_rotation,
        sprite_shader,
        resolved_render_layer,
        has_planet_shader,
        pending_fade_in,
    ) in &candidates
    {
        if has_planet_shader {
            continue;
        }
        let Ok(mut entity_commands) = commands.get_entity(entity) else {
            continue;
        };
        ensure_visual_parent_spatial_components(&mut entity_commands);

        let is_procedural_asteroid =
            procedural_sprite.is_some_and(|sprite| sprite.generator_id == "asteroid_rocky_v1");
        let desired_procedural_fingerprint = procedural_sprite
            .filter(|sprite| sprite.generator_id == "asteroid_rocky_v1")
            .map(procedural_sprite_fingerprint);
        let world_sprite_kind = sprite_shader
            .and_then(|shader| shaders::world_sprite_shader_kind(&shader_assignments, &shader.0))
            .or_else(|| {
                is_procedural_asteroid.then_some(shaders::RuntimeWorldSpriteShaderKind::Asteroid)
            });
        let generated_asteroid_images =
            if is_procedural_asteroid && let Some(procedural_sprite) = procedural_sprite {
                let guid = entity_guid
                    .map(|guid| guid.0)
                    .unwrap_or_else(uuid::Uuid::nil);
                let fingerprint = procedural_sprite_fingerprint(procedural_sprite);
                Some(
                    cached_assets
                        .asteroid_sprite_cache
                        .entry((guid, fingerprint))
                        .or_insert_with(|| {
                            let generated = generate_procedural_sprite_image_set(
                                &guid.to_string(),
                                procedural_sprite,
                            )
                            .expect("procedural asteroid sprite generation must succeed");
                            let albedo = images.add(image_from_rgba(
                                generated.width,
                                generated.height,
                                generated.albedo_rgba,
                            ));
                            let normal = images.add(normal_image_from_rgba(
                                generated.width,
                                generated.height,
                                generated.normal_rgba,
                            ));
                            (albedo, normal)
                        })
                        .clone(),
                )
            } else {
                None
            };

        let image_handle =
            if let Some((albedo_handle, _normal_handle)) = generated_asteroid_images.clone() {
                albedo_handle
            } else if let Some(handle) = cached_assets.streamed_image_cache.get(&asset_id.0) {
                handle.clone()
            } else {
                let Some(handle) = assets::cached_image_handle(
                    &asset_id.0,
                    &asset_manager,
                    &asset_root.0,
                    *cache_adapter,
                    &mut images,
                ) else {
                    continue;
                };
                cached_assets
                    .streamed_image_cache
                    .insert(asset_id.0.clone(), handle.clone());
                handle
            };
        let normal_image_handle = generated_asteroid_images
            .as_ref()
            .map(|(_albedo_handle, normal_handle)| normal_handle.clone())
            .unwrap_or_else(|| flat_normal_image_handle(&mut cached_assets, &mut images));

        let texture_size_px = generated_asteroid_images
            .as_ref()
            .and_then(|(albedo_handle, _normal_handle)| images.get(albedo_handle))
            .map(|image| image.size())
            .or_else(|| images.get(&image_handle).map(|image| image.size()));
        let custom_size = assets::resolved_world_sprite_size(texture_size_px, size_m);

        let has_streamed_sprite_shader_path = sprite_shader.is_some_and(|shader| {
            shaders::world_sprite_shader_ready(
                &asset_root.0,
                &asset_manager,
                *cache_adapter,
                &shader.0,
            )
        });
        let material_kind = resolve_streamed_visual_material_kind(
            use_shader_materials,
            world_sprite_kind,
            has_streamed_sprite_shader_path,
        );
        match material_kind {
            StreamedVisualMaterialKind::AsteroidShader => {
                let shared_quad = shared_unit_quad_handle(&mut quad_mesh, &mut meshes);
                let material = asteroid_shader_materials.add(AsteroidSpriteShaderMaterial {
                    image: image_handle.clone(),
                    normal_image: normal_image_handle,
                    lighting: SharedWorldLightingUniforms::from_state_for_world_position(
                        &world_lighting,
                        resolve_world_position(position, world_position)
                            .map(|value| value.as_vec2())
                            .unwrap_or(Vec2::ZERO),
                        &camera_local_lights,
                    ),
                    local_rotation: shader_rotation_uniform(
                        resolve_world_rotation_rad(rotation, world_rotation).unwrap_or(0.0),
                    ),
                });
                let sprite_size = custom_size.unwrap_or(Vec2::splat(16.0));
                let (x, y, z) = streamed_visual_layer_transform(resolved_render_layer, Vec2::ZERO);
                entity_commands.with_children(|child| {
                    child.spawn((
                        StreamedVisualChild,
                        Mesh2d(shared_quad),
                        MeshMaterial2d(material),
                        Transform::from_xyz(x, y, z).with_scale(Vec3::new(
                            sprite_size.x,
                            sprite_size.y,
                            1.0,
                        )),
                    ));
                });
                entity_commands.try_insert((
                    StreamedVisualAttached,
                    StreamedVisualAttachmentKind::AsteroidShader,
                ));
                if let Some(fingerprint) = desired_procedural_fingerprint {
                    entity_commands
                        .try_insert(StreamedProceduralSpriteVisualFingerprint(fingerprint));
                } else {
                    entity_commands.remove::<StreamedProceduralSpriteVisualFingerprint>();
                }
                continue;
            }
            StreamedVisualMaterialKind::GenericShader => {
                let shared_quad = shared_unit_quad_handle(&mut quad_mesh, &mut meshes);
                let material = sprite_shader_materials.add(StreamedSpriteShaderMaterial {
                    image: image_handle.clone(),
                    lighting: SharedWorldLightingUniforms::from_state_for_world_position(
                        &world_lighting,
                        resolve_world_position(position, world_position)
                            .map(|value| value.as_vec2())
                            .unwrap_or(Vec2::ZERO),
                        &camera_local_lights,
                    ),
                    local_rotation: shader_rotation_uniform(
                        resolve_world_rotation_rad(rotation, world_rotation).unwrap_or(0.0),
                    ),
                });
                let sprite_size = custom_size.unwrap_or(Vec2::splat(16.0));
                let (x, y, z) = streamed_visual_layer_transform(resolved_render_layer, Vec2::ZERO);
                entity_commands.with_children(|child| {
                    child.spawn((
                        StreamedVisualChild,
                        Mesh2d(shared_quad),
                        MeshMaterial2d(material),
                        Transform::from_xyz(x, y, z).with_scale(Vec3::new(
                            sprite_size.x,
                            sprite_size.y,
                            1.0,
                        )),
                    ));
                });
                entity_commands.try_insert((
                    StreamedVisualAttached,
                    StreamedVisualAttachmentKind::GenericShader,
                ));
                entity_commands.remove::<StreamedProceduralSpriteVisualFingerprint>();
                continue;
            }
            StreamedVisualMaterialKind::Plain => {}
        }
        let (x, y, z) = streamed_visual_layer_transform(resolved_render_layer, Vec2::ZERO);
        entity_commands.with_children(|child| {
            child.spawn((
                StreamedVisualChild,
                Sprite {
                    image: image_handle,
                    color: if pending_fade_in.is_some() {
                        Color::srgba(1.0, 1.0, 1.0, 0.0)
                    } else {
                        Color::WHITE
                    },
                    custom_size,
                    ..Default::default()
                },
                Transform::from_xyz(x, y, z),
            ));
        });
        entity_commands.try_insert((StreamedVisualAttached, StreamedVisualAttachmentKind::Plain));
        entity_commands.remove::<StreamedProceduralSpriteVisualFingerprint>();
    }
}
