#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(super) fn sync_runtime_post_process_renderables_system(
    mut commands: Commands<'_, '_>,
    stacks: Query<'_, '_, (Entity, &'_ RuntimePostProcessStack)>,
    existing: Query<
        '_,
        '_,
        (
            Entity,
            &'_ RuntimeFullscreenRenderable,
            Option<&'_ RuntimeFullscreenMaterialBinding>,
            Option<&'_ Mesh2d>,
            Option<&'_ Transform>,
            Option<&'_ RenderLayers>,
            Option<&'_ Visibility>,
            Has<NoFrustumCulling>,
            Has<MeshMaterial2d<StarfieldMaterial>>,
            Has<MeshMaterial2d<SpaceBackgroundMaterial>>,
            Has<MeshMaterial2d<SpaceBackgroundNebulaMaterial>>,
        ),
    >,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut render_cache: ResMut<'_, FullscreenRenderCache>,
    mut perf: ResMut<'_, BackdropRenderPerfCounters>,
    starfield_materials: Option<ResMut<'_, Assets<StarfieldMaterial>>>,
    space_background_materials: Option<ResMut<'_, Assets<SpaceBackgroundMaterial>>>,
    space_background_nebula_materials: Option<ResMut<'_, Assets<SpaceBackgroundNebulaMaterial>>>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, assets::LocalAssetManager>,
    cache_adapter: Res<'_, super::resources::AssetCacheAdapter>,
    shader_assignments: Res<'_, super::shaders::RuntimeShaderAssignments>,
) {
    let mut starfield_materials = starfield_materials;
    let mut space_background_materials = space_background_materials;
    let mut space_background_nebula_materials = space_background_nebula_materials;
    perf.post_process_sync_runs = perf.post_process_sync_runs.saturating_add(1);
    let fullscreen_mesh = fullscreen_quad_handle(&mut render_cache, &mut meshes, &mut perf);
    let mut desired = std::collections::HashMap::<(Entity, String), (String, i32)>::new();
    for (owner_entity, stack) in &stacks {
        for pass in &stack.passes {
            if !pass.enabled {
                continue;
            }
            desired.insert(
                (owner_entity, pass.pass_id.clone()),
                (pass.shader_asset_id.clone(), pass.order),
            );
        }
    }

    let existing_keys = existing
        .iter()
        .filter_map(|(_, renderable, ..)| {
            Some((
                renderable.owner_entity?,
                renderable.pass_id.as_ref()?.clone(),
            ))
        })
        .collect::<std::collections::HashSet<_>>();

    for (
        entity,
        renderable,
        binding,
        mesh,
        transform,
        render_layers,
        visibility,
        has_no_frustum_culling,
        has_starfield_material,
        has_space_background_material,
        has_space_background_nebula_material,
    ) in &existing
    {
        let (Some(existing_owner_entity), Some(existing_pass_id)) =
            (renderable.owner_entity, renderable.pass_id.as_ref())
        else {
            continue;
        };
        let key = (existing_owner_entity, existing_pass_id.clone());
        let Some((shader_asset_id, order)) = desired.get(&key) else {
            commands.entity(entity).despawn();
            continue;
        };
        let Some(material_kind) =
            fullscreen_material_kind_for_shader(&shader_assignments, shader_asset_id)
        else {
            commands.entity(entity).despawn();
            continue;
        };
        if !super::shaders::fullscreen_layer_shader_ready(
            &asset_root.0,
            &asset_manager,
            *cache_adapter,
            shader_asset_id,
        ) {
            continue;
        }
        let mut entity_commands = commands.entity(entity);
        ensure_runtime_fullscreen_renderable(
            &mut entity_commands,
            FullscreenRenderableComponents {
                existing_renderable: Some(renderable),
                mesh,
                transform,
                render_layers,
                visibility,
                has_no_frustum_culling,
                material_components: (
                    has_starfield_material,
                    has_space_background_material,
                    has_space_background_nebula_material,
                ),
                current_binding: binding.copied(),
            },
            FullscreenRenderableRequest {
                desired_renderable: RuntimeFullscreenRenderable {
                    layer_id: None,
                    owner_entity: Some(existing_owner_entity),
                    pass_id: Some(existing_pass_id.clone()),
                },
                fullscreen_mesh: &fullscreen_mesh,
                render_layer: POST_PROCESS_RENDER_LAYER,
                z_order: *order as f32,
                phase: BackdropSyncPhase::PostProcess,
                material_kind: Some(material_kind),
            },
            &mut perf,
            FullscreenMaterialAssets {
                starfield_materials: starfield_materials.as_deref_mut(),
                space_background_materials: space_background_materials.as_deref_mut(),
                space_background_nebula_materials: space_background_nebula_materials.as_deref_mut(),
            },
        );
        entity_commands.insert((ClientSceneEntity, DespawnOnExit(ClientAppState::InWorld)));
    }

    for ((owner_entity, pass_id), (shader_asset_id, order)) in desired {
        if existing_keys.contains(&(owner_entity, pass_id.clone())) {
            continue;
        }
        let Some(material_kind) =
            fullscreen_material_kind_for_shader(&shader_assignments, &shader_asset_id)
        else {
            continue;
        };
        if !super::shaders::fullscreen_layer_shader_ready(
            &asset_root.0,
            &asset_manager,
            *cache_adapter,
            &shader_asset_id,
        ) {
            continue;
        }
        let mut entity_commands =
            commands.spawn((ClientSceneEntity, DespawnOnExit(ClientAppState::InWorld)));
        ensure_runtime_fullscreen_renderable(
            &mut entity_commands,
            FullscreenRenderableComponents {
                existing_renderable: None,
                mesh: None,
                transform: None,
                render_layers: None,
                visibility: None,
                has_no_frustum_culling: false,
                material_components: (false, false, false),
                current_binding: None,
            },
            FullscreenRenderableRequest {
                desired_renderable: RuntimeFullscreenRenderable {
                    layer_id: None,
                    owner_entity: Some(owner_entity),
                    pass_id: Some(pass_id),
                },
                fullscreen_mesh: &fullscreen_mesh,
                render_layer: POST_PROCESS_RENDER_LAYER,
                z_order: order as f32,
                phase: BackdropSyncPhase::PostProcess,
                material_kind: Some(material_kind),
            },
            &mut perf,
            FullscreenMaterialAssets {
                starfield_materials: starfield_materials.as_deref_mut(),
                space_background_materials: space_background_materials.as_deref_mut(),
                space_background_nebula_materials: space_background_nebula_materials.as_deref_mut(),
            },
        );
    }
}

