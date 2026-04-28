fn clear_runtime_fullscreen_material(entity_commands: &mut EntityCommands<'_>) {
    entity_commands
        .remove::<RuntimeFullscreenMaterialBinding>()
        .remove::<MeshMaterial2d<StarfieldMaterial>>()
        .remove::<MeshMaterial2d<SpaceBackgroundMaterial>>()
        .remove::<MeshMaterial2d<SpaceBackgroundNebulaMaterial>>();
}

fn attach_runtime_fullscreen_material(
    entity_commands: &mut EntityCommands<'_>,
    request: &FullscreenRenderableRequest<'_>,
    perf: &mut BackdropRenderPerfCounters,
    current_binding: Option<RuntimeFullscreenMaterialBinding>,
    materials: FullscreenMaterialAssets<'_>,
) {
    let material_kind = request.material_kind;
    if material_kind.is_none() {
        clear_runtime_fullscreen_material(entity_commands);
        return;
    }

    let desired_binding = material_kind.map(RuntimeFullscreenMaterialBinding::from);
    if current_binding != desired_binding {
        match request.phase {
            BackdropSyncPhase::Fullscreen => {
                perf.fullscreen_material_rebinds =
                    perf.fullscreen_material_rebinds.saturating_add(1);
            }
            BackdropSyncPhase::PostProcess => {
                perf.post_process_material_rebinds =
                    perf.post_process_material_rebinds.saturating_add(1);
            }
        }
    } else {
        return;
    }

    clear_runtime_fullscreen_material(entity_commands);
    match material_kind {
        Some(FullscreenMaterialKind::Starfield) => {
            let Some(starfield_materials) = materials.starfield_materials else {
                warn!("fullscreen starfield material resource missing; skipping renderable attach");
                return;
            };
            let material = starfield_materials.add(StarfieldMaterial::default());
            record_material_allocation(perf, request.phase);
            entity_commands.insert((
                RuntimeFullscreenMaterialBinding::Starfield,
                MeshMaterial2d(material),
            ));
        }
        Some(FullscreenMaterialKind::SpaceBackgroundBase) => {
            let Some(space_background_materials) = materials.space_background_materials else {
                warn!(
                    "fullscreen space background base material resource missing; skipping renderable attach"
                );
                return;
            };
            let material = space_background_materials.add(SpaceBackgroundMaterial::default());
            record_material_allocation(perf, request.phase);
            entity_commands.insert((
                RuntimeFullscreenMaterialBinding::SpaceBackgroundBase,
                MeshMaterial2d(material),
            ));
        }
        Some(FullscreenMaterialKind::SpaceBackgroundNebula) => {
            let Some(space_background_nebula_materials) =
                materials.space_background_nebula_materials
            else {
                warn!(
                    "fullscreen space background nebula material resource missing; skipping renderable attach"
                );
                return;
            };
            let material =
                space_background_nebula_materials.add(SpaceBackgroundNebulaMaterial::default());
            record_material_allocation(perf, request.phase);
            entity_commands.insert((
                RuntimeFullscreenMaterialBinding::SpaceBackgroundNebula,
                MeshMaterial2d(material),
            ));
        }
        None => {}
    }
}

fn ensure_runtime_fullscreen_renderable(
    entity_commands: &mut EntityCommands<'_>,
    components: FullscreenRenderableComponents<'_>,
    request: FullscreenRenderableRequest<'_>,
    perf: &mut BackdropRenderPerfCounters,
    materials: FullscreenMaterialAssets<'_>,
) {
    if components.existing_renderable != Some(&request.desired_renderable) {
        entity_commands.insert(request.desired_renderable.clone());
    }
    if components
        .mesh
        .is_none_or(|existing| existing.0 != *request.fullscreen_mesh)
    {
        entity_commands.insert(Mesh2d(request.fullscreen_mesh.clone()));
    }
    let desired_transform = Transform::from_xyz(0.0, 0.0, request.z_order);
    if components.transform != Some(&desired_transform) {
        entity_commands.insert(desired_transform);
    }
    let desired_layers = RenderLayers::layer(request.render_layer);
    if components.render_layers != Some(&desired_layers) {
        entity_commands.insert(desired_layers);
    }
    if !components.has_no_frustum_culling {
        entity_commands.insert(NoFrustumCulling);
    }
    if components.visibility != Some(&Visibility::Visible) {
        entity_commands.insert(Visibility::Visible);
    }

    let has_expected_material_component = match request.material_kind {
        Some(FullscreenMaterialKind::Starfield) => components.material_components.0,
        Some(FullscreenMaterialKind::SpaceBackgroundBase) => components.material_components.1,
        Some(FullscreenMaterialKind::SpaceBackgroundNebula) => components.material_components.2,
        None => false,
    };
    let effective_binding = if has_expected_material_component {
        components.current_binding
    } else {
        None
    };
    attach_runtime_fullscreen_material(
        entity_commands,
        &request,
        perf,
        effective_binding,
        materials,
    );
}

fn fullscreen_quad_handle(
    render_cache: &mut FullscreenRenderCache,
    meshes: &mut Assets<Mesh>,
    perf: &mut BackdropRenderPerfCounters,
) -> Handle<Mesh> {
    render_cache
        .fullscreen_quad
        .get_or_insert_with(|| {
            perf.shared_quad_allocations = perf.shared_quad_allocations.saturating_add(1);
            meshes.add(Rectangle::new(1.0, 1.0))
        })
        .clone()
}

fn record_material_allocation(perf: &mut BackdropRenderPerfCounters, phase: BackdropSyncPhase) {
    match phase {
        BackdropSyncPhase::Fullscreen => {
            perf.fullscreen_material_allocations =
                perf.fullscreen_material_allocations.saturating_add(1);
        }
        BackdropSyncPhase::PostProcess => {
            perf.post_process_material_allocations =
                perf.post_process_material_allocations.saturating_add(1);
        }
    }
}

impl From<FullscreenMaterialKind> for RuntimeFullscreenMaterialBinding {
    fn from(value: FullscreenMaterialKind) -> Self {
        match value {
            FullscreenMaterialKind::Starfield => Self::Starfield,
            FullscreenMaterialKind::SpaceBackgroundBase => Self::SpaceBackgroundBase,
            FullscreenMaterialKind::SpaceBackgroundNebula => Self::SpaceBackgroundNebula,
        }
    }
}

pub(super) fn detach_fullscreen_layer_hierarchy_links_system(
    mut commands: Commands<'_, '_>,
    fullscreen_layers: Query<'_, '_, (Entity, &'_ ChildOf), With<FullscreenLayer>>,
) {
    for (entity, _) in &fullscreen_layers {
        commands.entity(entity).remove::<ChildOf>();
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn sync_backdrop_fullscreen_system(
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
    mut backdrop_query: Query<
        '_,
        '_,
        &mut Transform,
        (
            Or<(
                With<RuntimeFullscreenMaterialBinding>,
                With<SpaceBackdropFallback>,
            )>,
        ),
    >,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(viewport_size) = platform::safe_viewport_size(window) else {
        return;
    };
    let width = viewport_size.x;
    let height = viewport_size.y;
    for mut transform in &mut backdrop_query {
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
        transform.scale = Vec3::new(width, height, 1.0);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn sync_backdrop_camera_system(
    mut cameras: Query<
        '_,
        '_,
        (&'_ mut Camera, &'_ mut Transform, &'_ mut Projection),
        Or<(
            With<BackdropCamera>,
            With<FullscreenForegroundCamera>,
            With<PostProcessCamera>,
        )>,
    >,
) {
    for (mut camera, mut transform, mut projection) in &mut cameras {
        camera.is_active = true;
        transform.translation = Vec3::ZERO;
        transform.rotation = Quat::IDENTITY;
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scaling_mode = ScalingMode::WindowSize;
            ortho.scale = 1.0;
        }
    }
}

