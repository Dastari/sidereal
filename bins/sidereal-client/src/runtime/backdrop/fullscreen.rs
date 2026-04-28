// Fullscreen backdrop materials (starfield, space background, streamed sprite) and their update systems.

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
    BackdropCamera, ClientSceneEntity, ControlledEntity, FullscreenForegroundCamera,
    GameplayCamera, PostProcessCamera, RuntimeFullscreenMaterialBinding,
    RuntimeFullscreenRenderable, SpaceBackdropFallback,
};
use super::platform::{
    self, BACKDROP_RENDER_LAYER, FULLSCREEN_FOREGROUND_RENDER_LAYER, POST_PROCESS_RENDER_LAYER,
};
use super::resources::AssetRootPath;
use super::resources::{FullscreenExternalWorldData, StarfieldMotionState};
use bevy::state::state_scoped::DespawnOnExit;

#[derive(Resource, Default)]
pub(crate) struct FullscreenRenderCache {
    fullscreen_quad: Option<Handle<Mesh>>,
}

#[derive(Debug, Default, Resource)]
pub(crate) struct BackdropRenderPerfCounters {
    pub fullscreen_sync_runs: u64,
    pub post_process_sync_runs: u64,
    pub shared_quad_allocations: u64,
    pub fullscreen_material_allocations: u64,
    pub post_process_material_allocations: u64,
    pub fullscreen_material_rebinds: u64,
    pub post_process_material_rebinds: u64,
}

#[derive(Clone, Copy)]
enum BackdropSyncPhase {
    Fullscreen,
    PostProcess,
}

struct FullscreenMaterialAssets<'a> {
    starfield_materials: Option<&'a mut Assets<StarfieldMaterial>>,
    space_background_materials: Option<&'a mut Assets<SpaceBackgroundMaterial>>,
    space_background_nebula_materials: Option<&'a mut Assets<SpaceBackgroundNebulaMaterial>>,
}

struct FullscreenRenderableComponents<'a> {
    existing_renderable: Option<&'a RuntimeFullscreenRenderable>,
    mesh: Option<&'a Mesh2d>,
    transform: Option<&'a Transform>,
    render_layers: Option<&'a RenderLayers>,
    visibility: Option<&'a Visibility>,
    has_no_frustum_culling: bool,
    material_components: (bool, bool, bool),
    current_binding: Option<RuntimeFullscreenMaterialBinding>,
}

struct FullscreenRenderableRequest<'a> {
    desired_renderable: RuntimeFullscreenRenderable,
    fullscreen_mesh: &'a Handle<Mesh>,
    render_layer: usize,
    z_order: f32,
    phase: BackdropSyncPhase,
    material_kind: Option<FullscreenMaterialKind>,
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(super) fn sync_fullscreen_layer_renderables_system(
    mut commands: Commands<'_, '_>,
    layers: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ FullscreenLayer>,
            Option<&'_ RuntimeRenderLayerDefinition>,
            Option<&'_ StarfieldShaderSettings>,
            Option<&'_ SpaceBackgroundShaderSettings>,
            Option<&'_ RuntimeFullscreenRenderable>,
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
    stale_runtime_copies: Query<
        '_,
        '_,
        Entity,
        (
            With<RuntimeFullscreenRenderable>,
            Without<FullscreenLayer>,
            Without<RuntimeRenderLayerDefinition>,
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
    perf.fullscreen_sync_runs = perf.fullscreen_sync_runs.saturating_add(1);
    let fullscreen_mesh = fullscreen_quad_handle(&mut render_cache, &mut meshes, &mut perf);
    let shader_materials_enabled = super::shaders::shader_materials_enabled();
    for entity in &stale_runtime_copies {
        commands.entity(entity).despawn();
    }

    for (
        entity,
        legacy_layer,
        runtime_layer,
        starfield_settings,
        space_background_settings,
        existing_renderable,
        binding,
        mesh,
        transform,
        render_layers,
        visibility,
        has_no_frustum_culling,
        has_starfield_material,
        has_space_background_material,
        has_space_background_nebula_material,
    ) in &layers
    {
        let Some(selection) = resolve_fullscreen_layer_selection(
            entity,
            legacy_layer,
            runtime_layer,
            starfield_settings,
            space_background_settings,
        ) else {
            if existing_renderable.is_some_and(|renderable| {
                renderable.pass_id.is_none() && renderable.layer_id.is_some()
            }) {
                let mut entity_commands = commands.entity(entity);
                clear_runtime_fullscreen_material(&mut entity_commands);
                entity_commands.remove::<(
                    RuntimeFullscreenRenderable,
                    Mesh2d,
                    RenderLayers,
                    NoFrustumCulling,
                )>();
            }
            continue;
        };

        let has_streamed_shader = shader_materials_enabled
            && super::shaders::fullscreen_layer_shader_ready(
                &asset_root.0,
                &asset_manager,
                *cache_adapter,
                selection.shader_asset_id,
            );
        let material_kind = fullscreen_material_kind_for_selection(
            &selection,
            starfield_settings,
            space_background_settings,
            &shader_assignments,
        );
        if material_kind.is_none() || !has_streamed_shader {
            let mut entity_commands = commands.entity(entity);
            clear_runtime_fullscreen_material(&mut entity_commands);
            entity_commands.remove::<(
                RuntimeFullscreenRenderable,
                Mesh2d,
                RenderLayers,
                NoFrustumCulling,
            )>();
            if shader_materials_enabled {
                warn!(
                    "fullscreen layer renderable unavailable layer_id={} phase={} shader_asset_id={}",
                    selection.layer_id, selection.phase, selection.shader_asset_id
                );
            }
            continue;
        }

        let render_layer = render_layer_for_phase(selection.phase);
        let mut entity_commands = commands.entity(entity);
        let desired_renderable = RuntimeFullscreenRenderable {
            layer_id: Some(selection.layer_id.to_string()),
            owner_entity: None,
            pass_id: None,
        };
        ensure_runtime_fullscreen_renderable(
            &mut entity_commands,
            FullscreenRenderableComponents {
                existing_renderable,
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
                desired_renderable,
                fullscreen_mesh: &fullscreen_mesh,
                render_layer,
                z_order: selection.order as f32,
                phase: BackdropSyncPhase::Fullscreen,
                material_kind,
            },
            &mut perf,
            FullscreenMaterialAssets {
                starfield_materials: starfield_materials.as_deref_mut(),
                space_background_materials: space_background_materials.as_deref_mut(),
                space_background_nebula_materials: space_background_nebula_materials.as_deref_mut(),
            },
        );
        if existing_renderable.is_none() {
            info!(
                "fullscreen layer renderable ready phase={} order={} shader_asset_id={}",
                selection.phase, selection.order, selection.shader_asset_id
            );
        }
    }
}

#[derive(Clone, Copy)]
enum FullscreenMaterialKind {
    Starfield,
    SpaceBackgroundBase,
    SpaceBackgroundNebula,
}

struct FullscreenLayerSelection<'a> {
    layer_id: String,
    phase: &'a str,
    shader_asset_id: &'a str,
    order: i32,
}

fn fullscreen_material_kind_for_selection(
    selection: &FullscreenLayerSelection<'_>,
    starfield_settings: Option<&StarfieldShaderSettings>,
    space_background_settings: Option<&SpaceBackgroundShaderSettings>,
    shader_assignments: &super::shaders::RuntimeShaderAssignments,
) -> Option<FullscreenMaterialKind> {
    if let Some(kind) =
        fullscreen_material_kind_for_shader(shader_assignments, selection.shader_asset_id)
    {
        return Some(kind);
    }
    if starfield_settings.is_some() {
        Some(FullscreenMaterialKind::Starfield)
    } else if space_background_settings.is_some() {
        Some(FullscreenMaterialKind::SpaceBackgroundBase)
    } else {
        None
    }
}

fn resolve_fullscreen_layer_selection<'a>(
    entity: Entity,
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
            layer_id: layer.layer_id.clone(),
            phase: layer.phase.as_str(),
            shader_asset_id: layer.shader_asset_id.as_str(),
            order: layer.order,
        });
    }

    legacy_layer.map(|layer| FullscreenLayerSelection {
        layer_id: format!("legacy:{}", entity.to_bits()),
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

fn fullscreen_material_kind_for_shader(
    shader_assignments: &super::shaders::RuntimeShaderAssignments,
    shader_asset_id: &str,
) -> Option<FullscreenMaterialKind> {
    match super::shaders::fullscreen_shader_kind(shader_assignments, shader_asset_id) {
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

