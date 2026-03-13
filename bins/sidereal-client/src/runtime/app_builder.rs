use super::app_setup::configure_client_runtime;
use super::backdrop::{
    AsteroidSpriteShaderMaterial, PlanetVisualMaterial, RuntimeEffectMaterial,
    SpaceBackgroundMaterial, SpaceBackgroundNebulaMaterial, StarfieldMaterial,
    StreamedSpriteShaderMaterial, TacticalMapOverlayMaterial,
};
use super::platform::DEBUG_OVERLAY_RENDER_LAYER;
use crate::runtime::{AssetCacheAdapter, GatewayHttpAdapter};

use bevy::app::PluginGroupBuilder;
use bevy::camera::visibility::RenderLayers;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::gizmos::config::GizmoConfigStore;
use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;
use bevy_svg::prelude::SvgPlugin;

pub(crate) fn build_windowed_client_app(
    default_plugins: PluginGroupBuilder,
    asset_root: String,
    gateway_http_adapter: GatewayHttpAdapter,
    asset_cache_adapter: AssetCacheAdapter,
) -> App {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::BLACK));
    app.add_plugins(default_plugins);
    app.add_plugins(Material2dPlugin::<StarfieldMaterial>::default());
    app.add_plugins(Material2dPlugin::<SpaceBackgroundMaterial>::default());
    app.add_plugins(Material2dPlugin::<SpaceBackgroundNebulaMaterial>::default());
    app.add_plugins(Material2dPlugin::<StreamedSpriteShaderMaterial>::default());
    app.add_plugins(Material2dPlugin::<AsteroidSpriteShaderMaterial>::default());
    app.add_plugins(Material2dPlugin::<PlanetVisualMaterial>::default());
    app.add_plugins(Material2dPlugin::<RuntimeEffectMaterial>::default());
    app.add_plugins(Material2dPlugin::<TacticalMapOverlayMaterial>::default());
    app.add_plugins(SvgPlugin);
    app.add_plugins(FrameTimeDiagnosticsPlugin::default());
    if let Some(mut gizmo_config_store) = app.world_mut().get_resource_mut::<GizmoConfigStore>() {
        let (config, _) =
            gizmo_config_store.config_mut::<bevy::gizmos::config::DefaultGizmoConfigGroup>();
        config.render_layers = RenderLayers::layer(DEBUG_OVERLAY_RENDER_LAYER);
    }
    configure_client_runtime(
        &mut app,
        asset_root,
        false,
        gateway_http_adapter,
        asset_cache_adapter,
    );
    app
}
