use bevy::log::info;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};

pub(crate) fn run() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(RenderPlugin {
        render_creation: RenderCreation::Automatic(WgpuSettings {
            backends: Some(preferred_backends()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    app.add_systems(Startup, || {
        info!("sidereal-client wasm scaffold booted (WebGPU-capable)");
    });
    app.run();
}

fn preferred_backends() -> Backends {
    Backends::BROWSER_WEBGPU | Backends::GL
}
