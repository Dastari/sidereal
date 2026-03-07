//! Fullscreen backdrop materials (starfield, space background, streamed sprite) and their update systems.

use avian2d::prelude::*;
use bevy::camera::ScalingMode;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, MeshMaterial2d};
use sidereal_runtime_sync::RuntimeEntityHierarchy;

use sidereal_game::{
    FullscreenLayer, PlanetBodyShaderSettings, RENDER_DOMAIN_FULLSCREEN,
    RENDER_PHASE_FULLSCREEN_BACKGROUND, RENDER_PHASE_FULLSCREEN_FOREGROUND,
    RuntimePostProcessStack, RuntimeRenderLayerDefinition, SpaceBackgroundShaderSettings,
    StarfieldShaderSettings,
};

use super::app_state::ClientAppState;
use super::assets;
use super::components::{
    BackdropCamera, ClientSceneEntity, ControlledEntity, DebugBlueBackdrop,
    FullscreenForegroundCamera, GameplayCamera, PostProcessCamera,
    RuntimeFullscreenMaterialBinding, RuntimeFullscreenRenderable, SpaceBackdropFallback,
};
use super::platform::{
    self, BACKDROP_RENDER_LAYER, FULLSCREEN_FOREGROUND_RENDER_LAYER, POST_PROCESS_RENDER_LAYER,
};
use super::resources::AssetRootPath;
use super::resources::{
    FullscreenExternalWorldData, FullscreenLayerCache, FullscreenLayerCacheEntry,
    StarfieldMotionState,
};
use bevy::state::state_scoped::DespawnOnExit;

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(super) fn sync_fullscreen_layer_renderables_system(
    mut commands: Commands<'_, '_>,
    cache: Res<'_, FullscreenLayerCache>,
    existing: Query<'_, '_, (Entity, &'_ RuntimeFullscreenRenderable)>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut starfield_materials: ResMut<'_, Assets<StarfieldMaterial>>,
    mut space_background_materials: ResMut<'_, Assets<SpaceBackgroundMaterial>>,
    mut space_background_nebula_materials: ResMut<'_, Assets<SpaceBackgroundNebulaMaterial>>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, assets::LocalAssetManager>,
) {
    let shader_materials_enabled = super::shaders::shader_materials_enabled();
    if !shader_materials_enabled {
        for (entity, renderable) in &existing {
            if renderable.pass_id.is_none() && renderable.layer_id.is_some() {
                commands.entity(entity).despawn();
            }
        }
        return;
    }

    for (entity, renderable) in &existing {
        if renderable.pass_id.is_some() || renderable.layer_id.is_none() {
            continue;
        }
        let Some(layer_id) = renderable.layer_id.as_ref() else {
            continue;
        };
        let Some(selection) = cache.entries_by_layer_id.get(layer_id) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !selection.enabled {
            commands.entity(entity).despawn();
            continue;
        }
        let has_streamed_shader = super::shaders::fullscreen_layer_shader_ready(
            &asset_root.0,
            &asset_manager,
            &selection.shader_asset_id,
        );
        if fullscreen_material_kind_from_cache(selection).is_none() || !has_streamed_shader {
            warn!(
                "fullscreen layer renderable unavailable layer_id={} phase={} shader_asset_id={}",
                selection.layer_id, selection.phase, selection.shader_asset_id
            );
            continue;
        }
        let render_layer = render_layer_for_phase(&selection.phase);
        let mesh = meshes.add(Rectangle::new(1.0, 1.0));
        let mut entity_commands = commands.entity(entity);
        entity_commands.insert((
            RuntimeFullscreenRenderable {
                layer_id: Some(selection.layer_id.clone()),
                owner_entity: None,
                pass_id: None,
            },
            Mesh2d(mesh),
            Transform::from_xyz(0.0, 0.0, selection.order as f32),
            RenderLayers::layer(render_layer),
            NoFrustumCulling,
            Visibility::Visible,
            ClientSceneEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ));
        attach_runtime_fullscreen_material(
            &mut entity_commands,
            fullscreen_material_kind_from_cache(selection),
            &mut starfield_materials,
            &mut space_background_materials,
            &mut space_background_nebula_materials,
        );
    }

    for selection in cache.entries_by_layer_id.values() {
        if !selection.enabled {
            continue;
        }
        if existing.iter().any(|(_, renderable)| {
            renderable.layer_id.as_deref() == Some(selection.layer_id.as_str())
                && renderable.pass_id.is_none()
        }) {
            continue;
        }
        let has_streamed_shader = super::shaders::fullscreen_layer_shader_ready(
            &asset_root.0,
            &asset_manager,
            &selection.shader_asset_id,
        );
        if fullscreen_material_kind_from_cache(selection).is_none() || !has_streamed_shader {
            warn!(
                "fullscreen layer renderable unavailable layer_id={} phase={} shader_asset_id={}",
                selection.layer_id, selection.phase, selection.shader_asset_id
            );
            continue;
        }
        let render_layer = render_layer_for_phase(&selection.phase);
        let mesh = meshes.add(Rectangle::new(1.0, 1.0));
        let mut entity_commands = commands.spawn((
            RuntimeFullscreenRenderable {
                layer_id: Some(selection.layer_id.clone()),
                owner_entity: None,
                pass_id: None,
            },
            Mesh2d(mesh),
            Transform::from_xyz(0.0, 0.0, selection.order as f32),
            RenderLayers::layer(render_layer),
            NoFrustumCulling,
            Visibility::Visible,
            ClientSceneEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ));
        attach_runtime_fullscreen_material(
            &mut entity_commands,
            fullscreen_material_kind_from_cache(selection),
            &mut starfield_materials,
            &mut space_background_materials,
            &mut space_background_nebula_materials,
        );
        info!(
            "fullscreen layer renderable ready phase={} order={} shader_asset_id={}",
            selection.phase, selection.order, selection.shader_asset_id
        );
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn refresh_fullscreen_layer_cache_system(
    mut cache: ResMut<'_, FullscreenLayerCache>,
    layers: Query<
        '_,
        '_,
        (
            Entity,
            Option<&FullscreenLayer>,
            Option<&RuntimeRenderLayerDefinition>,
            Option<&StarfieldShaderSettings>,
            Option<&SpaceBackgroundShaderSettings>,
        ),
    >,
) {
    let mut seen_layer_ids = std::collections::HashSet::new();
    for (entity, legacy_layer, runtime_layer, starfield_settings, space_background_settings) in
        &layers
    {
        let Some(selection) = resolve_fullscreen_layer_selection(
            legacy_layer,
            runtime_layer,
            starfield_settings,
            space_background_settings,
        ) else {
            continue;
        };
        let layer_id = runtime_layer
            .map(|layer| layer.layer_id.clone())
            .unwrap_or_else(|| format!("legacy:{}", entity.to_bits()));
        seen_layer_ids.insert(layer_id.clone());
        cache.entries_by_layer_id.insert(
            layer_id.clone(),
            FullscreenLayerCacheEntry {
                layer_id,
                enabled: runtime_layer.map(|layer| layer.enabled).unwrap_or(true),
                phase: selection.phase.to_string(),
                shader_asset_id: selection.shader_asset_id.to_string(),
                order: selection.order,
                starfield_settings: starfield_settings.cloned(),
                space_background_settings: space_background_settings.cloned(),
            },
        );
    }
    cache
        .entries_by_layer_id
        .retain(|layer_id, entry| entry.enabled || seen_layer_ids.contains(layer_id));
}

#[derive(Clone, Copy)]
enum FullscreenMaterialKind {
    Starfield,
    SpaceBackgroundBase,
    SpaceBackgroundNebula,
}

struct FullscreenLayerSelection<'a> {
    phase: &'a str,
    shader_asset_id: &'a str,
    order: i32,
}

fn fullscreen_material_kind_from_cache(
    entry: &FullscreenLayerCacheEntry,
) -> Option<FullscreenMaterialKind> {
    if let Some(kind) = fullscreen_material_kind_for_shader(&entry.shader_asset_id) {
        return Some(kind);
    }
    if entry.starfield_settings.is_some() {
        Some(FullscreenMaterialKind::Starfield)
    } else if entry.space_background_settings.is_some() {
        Some(FullscreenMaterialKind::SpaceBackgroundBase)
    } else {
        None
    }
}

fn resolve_fullscreen_layer_selection<'a>(
    legacy_layer: Option<&'a FullscreenLayer>,
    runtime_layer: Option<&'a RuntimeRenderLayerDefinition>,
    _starfield_settings: Option<&'a StarfieldShaderSettings>,
    _space_background_settings: Option<&'a SpaceBackgroundShaderSettings>,
) -> Option<FullscreenLayerSelection<'a>> {
    if let Some(layer) = runtime_layer
        && layer.enabled
        && matches!(
            layer.phase.as_str(),
            RENDER_PHASE_FULLSCREEN_BACKGROUND | RENDER_PHASE_FULLSCREEN_FOREGROUND
        )
        && layer.material_domain == RENDER_DOMAIN_FULLSCREEN
    {
        return Some(FullscreenLayerSelection {
            phase: layer.phase.as_str(),
            shader_asset_id: layer.shader_asset_id.as_str(),
            order: layer.order,
        });
    }

    legacy_layer.map(|layer| FullscreenLayerSelection {
        phase: RENDER_PHASE_FULLSCREEN_BACKGROUND,
        shader_asset_id: layer.shader_asset_id.as_str(),
        order: layer.layer_order,
    })
}

fn render_layer_for_phase(phase: &str) -> usize {
    match phase {
        RENDER_PHASE_FULLSCREEN_FOREGROUND => FULLSCREEN_FOREGROUND_RENDER_LAYER,
        _ => BACKDROP_RENDER_LAYER,
    }
}

fn fullscreen_material_kind_for_shader(shader_asset_id: &str) -> Option<FullscreenMaterialKind> {
    match super::shaders::fullscreen_shader_kind(shader_asset_id) {
        Some(super::shaders::RuntimeFullscreenShaderKind::Starfield) => {
            Some(FullscreenMaterialKind::Starfield)
        }
        Some(super::shaders::RuntimeFullscreenShaderKind::SpaceBackgroundBase) => {
            Some(FullscreenMaterialKind::SpaceBackgroundBase)
        }
        Some(super::shaders::RuntimeFullscreenShaderKind::SpaceBackgroundNebula) => {
            Some(FullscreenMaterialKind::SpaceBackgroundNebula)
        }
        None => None,
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(super) fn sync_runtime_post_process_renderables_system(
    mut commands: Commands<'_, '_>,
    stacks: Query<'_, '_, (Entity, &'_ RuntimePostProcessStack)>,
    existing: Query<'_, '_, (Entity, &'_ RuntimeFullscreenRenderable)>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut starfield_materials: ResMut<'_, Assets<StarfieldMaterial>>,
    mut space_background_materials: ResMut<'_, Assets<SpaceBackgroundMaterial>>,
    mut space_background_nebula_materials: ResMut<'_, Assets<SpaceBackgroundNebulaMaterial>>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, assets::LocalAssetManager>,
) {
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

    for (entity, renderable) in &existing {
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
        let Some(material_kind) = fullscreen_material_kind_for_shader(shader_asset_id) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !super::shaders::fullscreen_layer_shader_ready(
            &asset_root.0,
            &asset_manager,
            shader_asset_id,
        ) {
            continue;
        }
        let mesh = meshes.add(Rectangle::new(1.0, 1.0));
        let mut entity_commands = commands.entity(entity);
        entity_commands.insert((
            RuntimeFullscreenRenderable {
                layer_id: None,
                owner_entity: Some(existing_owner_entity),
                pass_id: Some(existing_pass_id.clone()),
            },
            Mesh2d(mesh),
            Transform::from_xyz(0.0, 0.0, *order as f32),
            RenderLayers::layer(POST_PROCESS_RENDER_LAYER),
            NoFrustumCulling,
            Visibility::Visible,
            ClientSceneEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ));
        attach_runtime_fullscreen_material(
            &mut entity_commands,
            Some(material_kind),
            &mut starfield_materials,
            &mut space_background_materials,
            &mut space_background_nebula_materials,
        );
    }

    for ((owner_entity, pass_id), (shader_asset_id, order)) in desired {
        if existing.iter().any(|(_, renderable)| {
            renderable.owner_entity == Some(owner_entity)
                && renderable.pass_id.as_deref() == Some(pass_id.as_str())
        }) {
            continue;
        }
        let Some(material_kind) = fullscreen_material_kind_for_shader(&shader_asset_id) else {
            continue;
        };
        if !super::shaders::fullscreen_layer_shader_ready(
            &asset_root.0,
            &asset_manager,
            &shader_asset_id,
        ) {
            continue;
        }
        let mesh = meshes.add(Rectangle::new(1.0, 1.0));
        let mut entity_commands = commands.spawn((
            RuntimeFullscreenRenderable {
                layer_id: None,
                owner_entity: Some(owner_entity),
                pass_id: Some(pass_id),
            },
            Mesh2d(mesh),
            Transform::from_xyz(0.0, 0.0, order as f32),
            RenderLayers::layer(POST_PROCESS_RENDER_LAYER),
            NoFrustumCulling,
            Visibility::Visible,
            ClientSceneEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ));
        attach_runtime_fullscreen_material(
            &mut entity_commands,
            Some(material_kind),
            &mut starfield_materials,
            &mut space_background_materials,
            &mut space_background_nebula_materials,
        );
    }
}

fn clear_runtime_fullscreen_material(entity_commands: &mut EntityCommands<'_>) {
    entity_commands
        .remove::<RuntimeFullscreenMaterialBinding>()
        .remove::<MeshMaterial2d<StarfieldMaterial>>()
        .remove::<MeshMaterial2d<SpaceBackgroundMaterial>>()
        .remove::<MeshMaterial2d<SpaceBackgroundNebulaMaterial>>();
}

fn attach_runtime_fullscreen_material(
    entity_commands: &mut EntityCommands<'_>,
    material_kind: Option<FullscreenMaterialKind>,
    starfield_materials: &mut Assets<StarfieldMaterial>,
    space_background_materials: &mut Assets<SpaceBackgroundMaterial>,
    space_background_nebula_materials: &mut Assets<SpaceBackgroundNebulaMaterial>,
) {
    clear_runtime_fullscreen_material(entity_commands);
    match material_kind {
        Some(FullscreenMaterialKind::Starfield) => {
            let material = starfield_materials.add(StarfieldMaterial::default());
            entity_commands.insert((
                RuntimeFullscreenMaterialBinding::Starfield,
                MeshMaterial2d(material),
            ));
        }
        Some(FullscreenMaterialKind::SpaceBackgroundBase) => {
            let material = space_background_materials.add(SpaceBackgroundMaterial::default());
            entity_commands.insert((
                RuntimeFullscreenMaterialBinding::SpaceBackgroundBase,
                MeshMaterial2d(material),
            ));
        }
        Some(FullscreenMaterialKind::SpaceBackgroundNebula) => {
            let material =
                space_background_nebula_materials.add(SpaceBackgroundNebulaMaterial::default());
            entity_commands.insert((
                RuntimeFullscreenMaterialBinding::SpaceBackgroundNebula,
                MeshMaterial2d(material),
            ));
        }
        None => {}
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
                With<DebugBlueBackdrop>,
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
        ShaderRef::Handle(
            super::shaders::runtime_shader_handle("starfield_wgsl")
                .expect("starfield shader handle must be registered"),
        )
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
    if let Some(asset_id) = settings.flare_texture_asset_id.as_deref() {
        return Some(asset_id.to_string());
    }
    match settings.flare_texture_set {
        0 => Some("space_bg_flare_white_png".to_string()),
        1 => Some("space_bg_flare_blue_png".to_string()),
        2 => Some("space_bg_flare_red_png".to_string()),
        3 => Some("space_bg_flare_sun_png".to_string()),
        _ => None,
    }
}

impl Material2d for SpaceBackgroundMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(
            super::shaders::runtime_shader_handle("space_background_base_wgsl")
                .expect("space background base shader handle must be registered"),
        )
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
        ShaderRef::Handle(
            super::shaders::runtime_shader_handle("space_background_nebula_wgsl")
                .expect("space background nebula shader handle must be registered"),
        )
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
}

impl Material2d for StreamedSpriteShaderMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(
            super::shaders::runtime_shader_handle("sprite_pixel_shader_wgsl")
                .expect("sprite pixel shader handle must be registered"),
        )
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
}

impl Material2d for AsteroidSpriteShaderMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(
            super::shaders::runtime_shader_handle("asteroid_wgsl")
                .expect("asteroid shader handle must be registered"),
        )
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
    pub world_light_primary_dir_intensity: Vec4,
    pub world_light_primary_color_elevation: Vec4,
    pub world_light_ambient: Vec4,
    pub world_light_backlight: Vec4,
    pub world_light_flash: Vec4,
    pub world_light_local_dir_intensity: Vec4,
    pub world_light_local_color_radius: Vec4,
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
    pub primary_dir_intensity: Vec4,
    pub primary_color_elevation: Vec4,
    pub ambient: Vec4,
    pub backlight: Vec4,
    pub flash: Vec4,
    pub local_dir_intensity: Vec4,
    pub local_color_radius: Vec4,
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
        let primary_direction = state.resolved_primary_direction(world_position);
        let local_light =
            super::lighting::resolve_local_light_for_position(camera_local_lights, world_position);
        Self {
            primary_dir_intensity: primary_direction.extend(state.primary_intensity),
            primary_color_elevation: Vec4::new(
                state.primary_color.x,
                state.primary_color.y,
                state.primary_color.z,
                primary_direction.z,
            ),
            ambient: state.ambient_color.extend(state.ambient_intensity),
            backlight: state.backlight_color.extend(state.backlight_intensity),
            flash: state.event_flash_color.extend(state.event_flash_intensity),
            local_dir_intensity: local_light.direction.extend(local_light.intensity),
            local_color_radius: local_light.color.extend(local_light.radius_m),
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
            world_light_primary_dir_intensity: world_uniforms.primary_dir_intensity,
            world_light_primary_color_elevation: world_uniforms.primary_color_elevation,
            world_light_ambient: world_uniforms.ambient,
            world_light_backlight: world_uniforms.backlight,
            world_light_flash: world_uniforms.flash,
            world_light_local_dir_intensity: world_uniforms.local_dir_intensity,
            world_light_local_color_radius: world_uniforms.local_color_radius,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeEffectKind {
    BillboardThruster = 1,
    BillboardImpactSpark = 2,
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
impl_runtime_effect_material!(
    RuntimeEffectMaterial,
    super::shaders::RuntimeEffectShaderKind::RuntimeEffect
);
impl_runtime_effect_material!(
    TacticalMapOverlayMaterial,
    super::shaders::RuntimeEffectShaderKind::TacticalMapOverlay
);

/// Computes fullscreen world-space values used by fullscreen shaders. Runs in Last.
#[allow(clippy::too_many_arguments)]
pub fn compute_fullscreen_external_world_system(
    time: Res<'_, Time>,
    player_view_state: Res<'_, super::app_state::LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    controlled_vel_query: Query<'_, '_, &'static LinearVelocity, With<ControlledEntity>>,
    gameplay_camera_projection: Query<'_, '_, &'static Projection, With<GameplayCamera>>,
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
    mut motion: ResMut<'_, StarfieldMotionState>,
    mut world_data: ResMut<'_, FullscreenExternalWorldData>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(render_size) = platform::safe_render_target_size(window) else {
        return;
    };

    let velocity_vector = if let Some(controlled_id) = &player_view_state.controlled_entity_id {
        if let Some(&entity) = entity_registry.by_entity_id.get(controlled_id.as_str()) {
            controlled_vel_query
                .get(entity)
                .ok()
                .map(|v| v.0)
                .unwrap_or(Vec2::ZERO)
        } else {
            Vec2::ZERO
        }
    } else {
        Vec2::ZERO
    };

    let magnitude = velocity_vector.length();
    let heading = if magnitude > 0.01 {
        velocity_vector / magnitude
    } else {
        Vec2::Y
    };

    let dt = time.delta_secs().max(0.0);
    let zoom_scale = gameplay_camera_projection
        .single()
        .ok()
        .and_then(|projection| match projection {
            Projection::Orthographic(ortho) => Some(ortho.scale.max(0.01)),
            _ => None,
        })
        .unwrap_or(1.0);

    if !motion.initialized {
        motion.initialized = true;
        motion.prev_speed = magnitude;
        motion.smoothed_warp = 0.0;
    }

    // Starfield from controlled entity: vector = velocity, magnitude = speed, heading = unit direction.
    // Parallax is distance-over-time: we need the accumulator so scroll reflects integrated displacement (continual smooth motion).
    // Do not wrap at 1.0 (caused visible reset). Shader uses fract() so pattern is periodic. Wrap at large period to avoid f32 precision loss over long sessions.
    const STARFIELD_WORLD_TO_UV: f32 = 0.024;
    const SCROLL_WRAP_PERIOD: f32 = 4096.0;

    let frame_displacement = velocity_vector * dt;
    let delta_uv = frame_displacement * STARFIELD_WORLD_TO_UV;
    motion.accumulated_scroll_uv += delta_uv;
    if motion.accumulated_scroll_uv.x.abs() >= SCROLL_WRAP_PERIOD {
        motion.accumulated_scroll_uv.x -=
            motion.accumulated_scroll_uv.x.signum() * SCROLL_WRAP_PERIOD;
    }
    if motion.accumulated_scroll_uv.y.abs() >= SCROLL_WRAP_PERIOD {
        motion.accumulated_scroll_uv.y -=
            motion.accumulated_scroll_uv.y.signum() * SCROLL_WRAP_PERIOD;
    }

    let travel_uv = motion.accumulated_scroll_uv;
    motion.starfield_drift_uv = travel_uv;
    motion.background_drift_uv = travel_uv * 0.32;

    let target_warp = ((magnitude - 480.0) / 1650.0).clamp(0.0, 1.25);
    let warp_alpha = 1.0 - (-7.5 * dt).exp();
    motion.smoothed_warp = motion.smoothed_warp.lerp(target_warp, warp_alpha);

    let warp = motion.smoothed_warp;

    world_data.viewport_time = Vec4::new(render_size.x, render_size.y, time.elapsed_secs(), warp);
    // Y-flip so world Y-up matches screen: stars stream opposite travel (e.g. 223° -> 43°).
    world_data.drift_intensity = Vec4::new(travel_uv.x, -travel_uv.y, 1.0, 1.0);
    world_data.velocity_dir = Vec4::new(heading.x, heading.y, zoom_scale, 0.0);
}

#[allow(clippy::type_complexity)]
pub fn update_starfield_material_system(
    world_data: Res<'_, FullscreenExternalWorldData>,
    cache: Res<'_, FullscreenLayerCache>,
    starfield_query: Query<
        '_,
        '_,
        (
            &'_ MeshMaterial2d<StarfieldMaterial>,
            &'_ RuntimeFullscreenRenderable,
            Option<&'_ mut Visibility>,
        ),
        With<RuntimeFullscreenMaterialBinding>,
    >,
    mut materials: ResMut<'_, Assets<StarfieldMaterial>>,
) {
    for (material_handle, renderable, maybe_visibility) in starfield_query {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.viewport_time = world_data.viewport_time;
            material.drift_intensity = world_data.drift_intensity;
            material.velocity_dir = world_data.velocity_dir;
            let Some(layer_id) = renderable.layer_id.as_ref() else {
                continue;
            };
            let Some(entry) = cache.entries_by_layer_id.get(layer_id) else {
                continue;
            };
            let Some(settings) = entry.starfield_settings.as_ref() else {
                continue;
            };
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

#[allow(clippy::type_complexity)]
pub fn update_space_background_material_system(
    world_data: Res<'_, FullscreenExternalWorldData>,
    asset_server: Res<'_, AssetServer>,
    asset_manager: Res<'_, assets::LocalAssetManager>,
    cache: Res<'_, FullscreenLayerCache>,
    bg_query: Query<
        '_,
        '_,
        (
            &'_ MeshMaterial2d<SpaceBackgroundMaterial>,
            &'_ RuntimeFullscreenRenderable,
            Option<&'_ mut Visibility>,
        ),
        With<RuntimeFullscreenMaterialBinding>,
    >,
    mut materials: ResMut<'_, Assets<SpaceBackgroundMaterial>>,
) {
    for (material_handle, renderable, maybe_visibility) in bg_query {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.params.viewport_time = world_data.viewport_time;
            material.params.drift_intensity = world_data.drift_intensity;
            material.params.velocity_dir = world_data.velocity_dir;
            let Some(layer_id) = renderable.layer_id.as_ref() else {
                continue;
            };
            let Some(entry) = cache.entries_by_layer_id.get(layer_id) else {
                continue;
            };
            let Some(settings) = entry.space_background_settings.as_ref() else {
                continue;
            };
            if let Some(mut visibility) = maybe_visibility {
                *visibility = if settings.enabled {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
            material.params.space_bg_params = Vec4::new(
                settings.intensity.max(0.0),
                settings.drift_scale.max(0.0),
                settings.velocity_glow.max(0.0),
                settings.nebula_strength.max(0.0),
            );
            material.params.space_bg_tint = settings.tint_rgb.extend(settings.seed);
            material.params.space_bg_background = settings.background_rgb.extend(1.0);
            let mut flare_enabled = settings.flare_enabled;
            if let Some(flare_asset_id) = resolve_space_background_flare_asset_id(settings) {
                if let Some(path) =
                    assets::streamed_visual_asset_path(&flare_asset_id, &asset_manager)
                {
                    material.flare_texture = asset_server.load(path);
                } else {
                    flare_enabled = false;
                }
            } else {
                flare_enabled = false;
            }
            material.params.space_bg_flare = Vec4::new(
                if flare_enabled { 1.0 } else { 0.0 },
                settings.flare_intensity.max(0.0),
                settings.flare_density.clamp(0.0, 1.0),
                settings.flare_size.max(0.01),
            );
            material.params.space_bg_noise_a = Vec4::new(
                settings.nebula_noise_mode.clamp(0, 1) as f32,
                settings.nebula_octaves.clamp(1, 8) as f32,
                settings.nebula_gain.clamp(0.1, 0.95),
                settings.nebula_lacunarity.clamp(1.1, 4.0),
            );
            material.params.space_bg_noise_b = Vec4::new(
                settings.nebula_power.clamp(0.2, 4.0),
                settings.nebula_shelf.clamp(0.0, 0.95),
                settings.nebula_ridge_offset.clamp(0.5, 2.5),
                0.0,
            );
            material.params.space_bg_star_mask_a = Vec4::new(
                if settings.star_mask_enabled { 1.0 } else { 0.0 },
                settings.star_mask_mode.clamp(0, 1) as f32,
                settings.star_mask_octaves.clamp(1, 8) as f32,
                settings.star_mask_scale.clamp(0.2, 8.0),
            );
            material.params.space_bg_star_mask_b = Vec4::new(
                settings.star_mask_threshold.clamp(0.0, 0.99),
                settings.star_mask_power.clamp(0.2, 4.0),
                settings.star_mask_gain.clamp(0.1, 0.95),
                settings.star_mask_lacunarity.clamp(1.1, 4.0),
            );
            material.params.space_bg_star_mask_c = Vec4::new(
                settings.star_mask_ridge_offset.clamp(0.5, 2.5),
                settings.star_count.clamp(0.0, 5.0),
                settings.star_size_min.clamp(0.01, 0.35),
                settings.star_size_max.clamp(0.01, 0.35),
            );
            material.params.space_bg_blend_a = Vec4::new(
                settings.nebula_blend_mode.clamp(0, 26) as f32,
                settings.nebula_opacity.clamp(0.0, 1.0),
                settings.stars_blend_mode.clamp(0, 26) as f32,
                settings.stars_opacity.clamp(0.0, 1.0),
            );
            material.params.space_bg_blend_b = Vec4::new(
                settings.flares_blend_mode.clamp(0, 26) as f32,
                settings.flares_opacity.clamp(0.0, 1.0),
                settings.zoom_rate.clamp(0.0, 4.0),
                0.0,
            );
            material.params.space_bg_section_flags = Vec4::new(
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
            material.params.space_bg_nebula_color_a = settings
                .nebula_color_primary_rgb
                .clamp(Vec3::ZERO, Vec3::splat(2.0))
                .extend(0.0);
            material.params.space_bg_nebula_color_b = settings
                .nebula_color_secondary_rgb
                .clamp(Vec3::ZERO, Vec3::splat(2.0))
                .extend(0.0);
            material.params.space_bg_nebula_color_c = settings
                .nebula_color_accent_rgb
                .clamp(Vec3::ZERO, Vec3::splat(2.0))
                .extend(0.0);
            material.params.space_bg_star_color = settings
                .star_color_rgb
                .clamp(Vec3::ZERO, Vec3::splat(2.0))
                .extend(1.0);
            material.params.space_bg_flare_tint = settings
                .flare_tint_rgb
                .clamp(Vec3::ZERO, Vec3::splat(2.0))
                .extend(1.0);
            material.params.space_bg_depth_a = Vec4::new(
                settings.depth_layer_separation.clamp(0.0, 2.0),
                settings.depth_parallax_scale.clamp(0.0, 2.0),
                settings.depth_haze_strength.clamp(0.0, 2.0),
                settings.depth_occlusion_strength.clamp(0.0, 3.0),
            );
            material.params.space_bg_light_a = Vec4::new(
                settings.backlight_screen_x.clamp(-1.5, 1.5),
                settings.backlight_screen_y.clamp(-1.5, 1.5),
                settings.backlight_intensity.clamp(0.0, 20.0),
                settings.backlight_wrap.clamp(0.0, 2.0),
            );
            material.params.space_bg_light_b = Vec4::new(
                settings.backlight_edge_boost.clamp(0.0, 6.0),
                settings.backlight_bloom_scale.clamp(0.0, 2.0),
                settings.backlight_bloom_threshold.clamp(0.0, 1.0),
                settings.shaft_quality.clamp(0, 2) as f32,
            );
            material.params.space_bg_light_flags = Vec4::new(
                if settings.enable_backlight { 1.0 } else { 0.0 },
                if settings.enable_light_shafts {
                    1.0
                } else {
                    0.0
                },
                if settings.shafts_debug_view { 1.0 } else { 0.0 },
                settings.shaft_blend_mode.clamp(0, 26) as f32,
            );
            material.params.space_bg_shafts_a = Vec4::new(
                settings.shaft_intensity.clamp(0.0, 40.0),
                settings.shaft_length.clamp(0.05, 0.95),
                settings.shaft_falloff.clamp(0.2, 8.0),
                settings.shaft_samples.clamp(4, 24) as f32,
            );
            material.params.space_bg_shafts_b = settings
                .shaft_color_rgb
                .clamp(Vec3::ZERO, Vec3::splat(3.0))
                .extend(settings.shaft_opacity.clamp(0.0, 1.0));
            material.params.space_bg_backlight_color = settings
                .backlight_color_rgb
                .clamp(Vec3::ZERO, Vec3::splat(3.0))
                .extend(1.0);
        }
    }
}
