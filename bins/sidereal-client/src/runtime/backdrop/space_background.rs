#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn update_space_background_material_system(
    world_data: Res<'_, FullscreenExternalWorldData>,
    asset_manager: Res<'_, assets::LocalAssetManager>,
    cache_adapter: Res<'_, super::resources::AssetCacheAdapter>,
    asset_root: Res<'_, AssetRootPath>,
    mut images: ResMut<'_, Assets<Image>>,
    mut last_reload_generation: Local<'_, u64>,
    mut flare_cache: Local<'_, std::collections::HashMap<String, Handle<Image>>>,
    bg_query: Query<
        '_,
        '_,
        (
            &'_ MeshMaterial2d<SpaceBackgroundMaterial>,
            &'_ SpaceBackgroundShaderSettings,
            Option<&'_ mut Visibility>,
        ),
        (
            With<RuntimeFullscreenMaterialBinding>,
            Without<MeshMaterial2d<SpaceBackgroundNebulaMaterial>>,
        ),
    >,
    nebula_query: Query<
        '_,
        '_,
        (
            &'_ MeshMaterial2d<SpaceBackgroundNebulaMaterial>,
            &'_ SpaceBackgroundShaderSettings,
            Option<&'_ mut Visibility>,
        ),
        (
            With<RuntimeFullscreenMaterialBinding>,
            Without<MeshMaterial2d<SpaceBackgroundMaterial>>,
        ),
    >,
    mut materials: ResMut<'_, Assets<SpaceBackgroundMaterial>>,
    mut nebula_materials: ResMut<'_, Assets<SpaceBackgroundNebulaMaterial>>,
) {
    if *last_reload_generation != asset_manager.reload_generation {
        flare_cache.clear();
        *last_reload_generation = asset_manager.reload_generation;
    }
    for (material_handle, settings, maybe_visibility) in bg_query {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            if let Some(mut visibility) = maybe_visibility {
                *visibility = if settings.enabled {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
            let (flare_handle, flare_enabled) = resolve_space_background_flare_handle(
                settings,
                &mut flare_cache,
                &asset_manager,
                &asset_root.0,
                *cache_adapter,
                &mut images,
            );
            if let Some(handle) = flare_handle {
                material.flare_texture = handle;
            }
            populate_space_background_uniforms(
                &mut material.params,
                &world_data,
                settings,
                flare_enabled,
            );
        }
    }
    for (material_handle, settings, maybe_visibility) in nebula_query {
        if let Some(material) = nebula_materials.get_mut(&material_handle.0) {
            if let Some(mut visibility) = maybe_visibility {
                *visibility = if settings.enabled {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
            let (flare_handle, flare_enabled) = resolve_space_background_flare_handle(
                settings,
                &mut flare_cache,
                &asset_manager,
                &asset_root.0,
                *cache_adapter,
                &mut images,
            );
            if let Some(handle) = flare_handle {
                material.flare_texture = handle;
            }
            populate_space_background_uniforms(
                &mut material.params,
                &world_data,
                settings,
                flare_enabled,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BackdropRenderPerfCounters, FullscreenExternalWorldData, FullscreenRenderCache,
        SpaceBackgroundMaterial, SpaceBackgroundNebulaMaterial, StarfieldMaterial,
        StarfieldMotionState, compute_fullscreen_external_world_system,
        sync_runtime_post_process_renderables_system, update_space_background_material_system,
    };
    use crate::runtime::app_state::LocalPlayerViewState;
    use crate::runtime::assets::LocalAssetManager;
    use crate::runtime::components::{
        ClientSceneEntity, ControlledEntity, GameplayCamera, RuntimeFullscreenMaterialBinding,
        RuntimeFullscreenRenderable,
    };
    use crate::runtime::resources::{AssetCacheAdapter, AssetRootPath, CacheFuture};
    use crate::runtime::shaders::{
        self, RuntimeShaderAssignmentSyncState, RuntimeShaderAssignments,
    };
    use avian2d::prelude::LinearVelocity;
    use bevy::prelude::*;
    use bevy::sprite_render::MeshMaterial2d;
    use bevy::window::PrimaryWindow;
    use lightyear::prelude::Predicted;
    use sidereal_asset_runtime::AssetCacheIndex;
    use sidereal_game::{
        RENDER_DOMAIN_FULLSCREEN, RENDER_PHASE_FULLSCREEN_BACKGROUND, RuntimePostProcessPass,
        RuntimePostProcessStack, RuntimeRenderLayerDefinition, SpaceBackgroundShaderSettings,
    };
    use sidereal_runtime_sync::RuntimeEntityHierarchy;
    use std::time::Duration;
    #[test]
    fn post_process_sync_reuses_mesh_and_material_for_unchanged_pass() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StarfieldMaterial>>();
        app.init_resource::<Assets<bevy::shader::Shader>>();
        app.init_resource::<FullscreenRenderCache>();
        app.init_resource::<BackdropRenderPerfCounters>();
        app.insert_resource(AssetRootPath(".".to_string()));
        app.insert_resource(LocalAssetManager::default());
        app.insert_resource(dummy_cache_adapter());
        app.insert_resource(RuntimeShaderAssignments::default());
        app.insert_resource(RuntimeShaderAssignmentSyncState::default());
        app.add_systems(
            Update,
            (
                shaders::sync_runtime_shader_assignments_system,
                sync_runtime_post_process_renderables_system
                    .after(shaders::sync_runtime_shader_assignments_system),
            ),
        );

        app.world_mut().spawn(RuntimeRenderLayerDefinition {
            layer_id: "bg_starfield".to_string(),
            phase: RENDER_PHASE_FULLSCREEN_BACKGROUND.to_string(),
            material_domain: RENDER_DOMAIN_FULLSCREEN.to_string(),
            shader_asset_id: "shader.test.starfield".to_string(),
            ..Default::default()
        });
        app.world_mut().spawn(RuntimePostProcessStack {
            passes: vec![RuntimePostProcessPass {
                pass_id: "warp".to_string(),
                shader_asset_id: "shader.test.starfield".to_string(),
                order: 3,
                enabled: true,
                ..Default::default()
            }],
        });

        app.update();
        let (entity, first_mesh, first_material) = post_process_handles(app.world_mut());

        app.update();
        let (same_entity, second_mesh, second_material) = post_process_handles(app.world_mut());

        assert_eq!(
            entity, same_entity,
            "existing post-process entity should be reused"
        );
        assert_eq!(
            first_mesh, second_mesh,
            "post-process quad handle should be stable"
        );
        assert_eq!(
            first_material, second_material,
            "post-process material handle should be stable when authored state is unchanged"
        );

        let perf = app.world().resource::<BackdropRenderPerfCounters>();
        assert_eq!(perf.shared_quad_allocations, 1);
        assert_eq!(perf.post_process_material_allocations, 1);
        assert_eq!(perf.post_process_material_rebinds, 1);
    }

    fn post_process_handles(
        world: &mut World,
    ) -> (Entity, AssetId<Mesh>, AssetId<StarfieldMaterial>) {
        let mut query = world.query_filtered::<(
            Entity,
            &Mesh2d,
            &MeshMaterial2d<StarfieldMaterial>,
            &RuntimeFullscreenRenderable,
        ), With<ClientSceneEntity>>();
        let (entity, mesh, material, renderable) = query
            .single(world)
            .expect("one post-process renderable should exist");
        assert!(renderable.owner_entity.is_some());
        assert!(renderable.pass_id.is_some());
        (entity, mesh.0.id(), material.0.id())
    }

    fn dummy_cache_adapter() -> AssetCacheAdapter {
        fn prepare_root(_: String) -> CacheFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn load_index(_: String) -> CacheFuture<AssetCacheIndex> {
            Box::pin(async { Ok(AssetCacheIndex::default()) })
        }
        fn save_index(_: String, _: AssetCacheIndex) -> CacheFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn read_valid_asset(_: String, _: String, _: String) -> CacheFuture<Option<Vec<u8>>> {
            Box::pin(async { Ok(None) })
        }
        fn write_asset(_: String, _: String, _: Vec<u8>) -> CacheFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn read_valid_asset_sync(_: &str, _: &str, _: &str) -> Option<Vec<u8>> {
            None
        }

        AssetCacheAdapter {
            prepare_root,
            load_index,
            save_index,
            read_valid_asset,
            write_asset,
            read_valid_asset_sync,
        }
    }

    #[test]
    fn fullscreen_motion_prefers_controlled_runtime_velocity_over_registry_lane() {
        let mut app = App::new();
        app.insert_resource({
            let mut time = Time::<()>::default();
            time.advance_by(Duration::from_secs_f32(1.0));
            time
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("controlled".to_string()),
            ..Default::default()
        });
        app.insert_resource(RuntimeEntityHierarchy::default());
        app.insert_resource(StarfieldMotionState::default());
        app.insert_resource(FullscreenExternalWorldData::default());
        app.add_systems(Update, compute_fullscreen_external_world_system);

        app.world_mut().spawn((
            Window {
                resolution: (1280, 720).into(),
                ..Default::default()
            },
            PrimaryWindow,
        ));
        app.world_mut().spawn((
            GameplayCamera,
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));

        let confirmed_entity = app
            .world_mut()
            .spawn(LinearVelocity(Vec2::ZERO.into()))
            .id();
        let predicted_entity = app
            .world_mut()
            .spawn((
                ControlledEntity {
                    entity_id: "controlled".to_string(),
                    player_entity_id: "player".to_string(),
                },
                LinearVelocity(Vec2::new(24.0, 0.0).into()),
                Predicted,
            ))
            .id();

        app.world_mut()
            .resource_mut::<RuntimeEntityHierarchy>()
            .by_entity_id
            .insert("controlled".to_string(), confirmed_entity);

        app.update();

        let world_data = app.world().resource::<FullscreenExternalWorldData>();
        assert_eq!(
            world_data.velocity_dir.xy(),
            Vec2::X,
            "fullscreen shader motion should follow the controlled predicted runtime entity"
        );
        assert!(
            world_data.drift_intensity.x > 0.0,
            "controlled predicted velocity should produce starfield drift"
        );
        assert_ne!(
            predicted_entity, confirmed_entity,
            "test requires distinct runtime lanes"
        );
    }

    #[test]
    fn space_background_motion_updates_base_and_nebula_materials() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>();
        app.init_resource::<Assets<SpaceBackgroundMaterial>>();
        app.init_resource::<Assets<SpaceBackgroundNebulaMaterial>>();
        app.insert_resource(LocalAssetManager::default());
        app.insert_resource(AssetRootPath(".".to_string()));
        app.insert_resource(dummy_cache_adapter());
        app.insert_resource(FullscreenExternalWorldData {
            viewport_time: Vec4::new(1280.0, 720.0, 4.0, 0.2),
            drift_intensity: Vec4::new(3.0, -2.0, 1.0, 1.0),
            velocity_dir: Vec4::new(1.0, 0.0, 1.5, 0.0),
        });
        app.add_systems(Update, update_space_background_material_system);

        let base_handle = app
            .world_mut()
            .resource_mut::<Assets<SpaceBackgroundMaterial>>()
            .add(SpaceBackgroundMaterial::default());
        let nebula_handle = app
            .world_mut()
            .resource_mut::<Assets<SpaceBackgroundNebulaMaterial>>()
            .add(SpaceBackgroundNebulaMaterial::default());

        app.world_mut().spawn((
            MeshMaterial2d(base_handle.clone()),
            SpaceBackgroundShaderSettings::default(),
            RuntimeFullscreenMaterialBinding::SpaceBackgroundBase,
        ));
        app.world_mut().spawn((
            MeshMaterial2d(nebula_handle.clone()),
            SpaceBackgroundShaderSettings::default(),
            RuntimeFullscreenMaterialBinding::SpaceBackgroundNebula,
        ));

        app.update();

        let base_material = app
            .world()
            .resource::<Assets<SpaceBackgroundMaterial>>()
            .get(&base_handle)
            .expect("base material should exist")
            .clone();
        let nebula_material = app
            .world()
            .resource::<Assets<SpaceBackgroundNebulaMaterial>>()
            .get(&nebula_handle)
            .expect("nebula material should exist")
            .clone();

        assert_eq!(
            base_material.params.velocity_dir,
            Vec4::new(1.0, 0.0, 1.5, 0.0)
        );
        assert_eq!(
            nebula_material.params.velocity_dir,
            Vec4::new(1.0, 0.0, 1.5, 0.0)
        );
        assert_eq!(
            base_material.params.drift_intensity,
            Vec4::new(3.0, -2.0, 1.0, 1.0)
        );
        assert_eq!(
            nebula_material.params.drift_intensity,
            Vec4::new(3.0, -2.0, 1.0, 1.0)
        );
    }
}
