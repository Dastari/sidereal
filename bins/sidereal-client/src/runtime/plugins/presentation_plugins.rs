use bevy::prelude::*;

use crate::runtime::app_state::ClientAppState;
use crate::runtime::components::{
    WeaponImpactExplosionPool, WeaponImpactSparkPool, WeaponTracerCooldowns, WeaponTracerPool,
};
use crate::runtime::resources::{
    DuplicateVisualResolutionState, RenderLayerPerfCounters, RuntimeRenderLayerAssignmentCache,
    RuntimeRenderLayerRegistry, RuntimeRenderLayerRegistryState, RuntimeSharedQuadMesh,
};
use crate::runtime::{assets, backdrop, lighting, render_layers, replication, shaders, visuals};

pub(crate) struct ClientVisualsPlugin;

impl Plugin for ClientVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponTracerPool>();
        app.init_resource::<WeaponTracerCooldowns>();
        app.init_resource::<WeaponImpactSparkPool>();
        app.init_resource::<WeaponImpactExplosionPool>();
        app.init_resource::<RuntimeRenderLayerRegistry>();
        app.init_resource::<RuntimeRenderLayerRegistryState>();
        app.init_resource::<RuntimeRenderLayerAssignmentCache>();
        app.init_resource::<RenderLayerPerfCounters>();
        app.init_resource::<RuntimeSharedQuadMesh>();
        app.init_resource::<DuplicateVisualResolutionState>();
        app.init_resource::<backdrop::FullscreenRenderCache>();
        app.init_resource::<backdrop::BackdropRenderPerfCounters>();
        let in_world_visuals_core = (
            shaders::mark_runtime_shader_assignments_dirty_system
                .after(replication::adopt_native_lightyear_replicated_entities),
            shaders::sync_runtime_shader_assignments_system
                .after(shaders::mark_runtime_shader_assignments_dirty_system),
            render_layers::sync_runtime_render_layer_registry_system
                .after(replication::adopt_native_lightyear_replicated_entities),
            render_layers::resolve_runtime_render_layer_assignments_system
                .after(render_layers::sync_runtime_render_layer_registry_system),
            visuals::suppress_duplicate_predicted_interpolated_visuals_system
                .after(replication::adopt_native_lightyear_replicated_entities),
            visuals::cleanup_streamed_visual_children_system
                .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
            visuals::cleanup_planet_body_visual_children_system
                .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
            visuals::attach_planet_visual_stack_system
                .after(visuals::cleanup_planet_body_visual_children_system),
            visuals::ensure_planet_body_root_visibility_system
                .after(visuals::attach_planet_visual_stack_system),
            visuals::bootstrap_local_ballistic_projectile_visual_roots_system
                .after(replication::adopt_native_lightyear_replicated_entities),
            visuals::sync_unadopted_ballistic_projectile_visual_roots_system
                .after(visuals::bootstrap_local_ballistic_projectile_visual_roots_system),
            visuals::attach_ballistic_projectile_visuals_system
                .after(visuals::sync_unadopted_ballistic_projectile_visual_roots_system)
                .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
            visuals::attach_thruster_plume_visuals_system
                .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
        );
        let in_world_visuals_effects = (
            visuals::update_thruster_plume_visuals_system
                .after(visuals::attach_thruster_plume_visuals_system),
            visuals::ensure_weapon_tracer_pool_system
                .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
            visuals::ensure_weapon_impact_spark_pool_system
                .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
            visuals::ensure_weapon_impact_explosion_pool_system
                .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
            visuals::emit_weapon_tracer_visuals_system
                .after(visuals::ensure_weapon_tracer_pool_system),
            visuals::receive_remote_weapon_tracer_messages_system
                .after(visuals::ensure_weapon_tracer_pool_system),
            visuals::update_weapon_tracer_visuals_system
                .after(visuals::emit_weapon_tracer_visuals_system)
                .after(visuals::receive_remote_weapon_tracer_messages_system)
                .after(visuals::ensure_weapon_impact_spark_pool_system)
                .after(visuals::ensure_weapon_impact_explosion_pool_system),
            visuals::update_weapon_impact_sparks_system
                .after(visuals::update_weapon_tracer_visuals_system),
            visuals::update_weapon_impact_explosions_system
                .after(visuals::update_weapon_impact_sparks_system),
            visuals::attach_streamed_visual_assets_system
                .after(assets::poll_runtime_asset_http_fetches_system)
                .after(render_layers::resolve_runtime_render_layer_assignments_system),
            visuals::update_entity_visibility_fade_in_system
                .after(visuals::attach_streamed_visual_assets_system)
                .after(visuals::ensure_planet_body_root_visibility_system),
        );
        let in_world_backdrop = (
            backdrop::sync_fullscreen_layer_renderables_system
                .after(replication::adopt_native_lightyear_replicated_entities),
            backdrop::sync_runtime_post_process_renderables_system
                .after(replication::adopt_native_lightyear_replicated_entities),
            backdrop::sync_backdrop_camera_system
                .after(backdrop::sync_fullscreen_layer_renderables_system)
                .after(backdrop::sync_runtime_post_process_renderables_system),
            backdrop::sync_backdrop_fullscreen_system.after(backdrop::sync_backdrop_camera_system),
        );
        app.add_systems(
            Update,
            in_world_visuals_core.run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Update,
            in_world_visuals_effects.run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Update,
            in_world_backdrop.run_if(in_state(ClientAppState::InWorld)),
        );
    }
}

pub(crate) struct ClientLightingPlugin;

impl Plugin for ClientLightingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<lighting::WorldLightingState>();
        app.init_resource::<lighting::CameraLocalLightSet>();
        let in_world_lighting = (
            lighting::sync_world_lighting_state_system
                .after(replication::adopt_native_lightyear_replicated_entities),
            lighting::collect_thruster_local_light_emitters_system
                .after(visuals::update_thruster_plume_visuals_system),
            visuals::update_asteroid_shader_lighting_system
                .after(lighting::sync_world_lighting_state_system),
        );
        app.add_systems(
            Update,
            in_world_lighting.run_if(in_state(ClientAppState::InWorld)),
        );
    }
}
