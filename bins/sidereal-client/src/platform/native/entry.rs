use bevy::asset::{AssetApp, AssetPlugin};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::RenderCreation;
use bevy::scene::ScenePlugin;
use bevy::window::{PresentMode, Window, WindowPlugin, WindowResizeConstraints};
use bevy::winit::WinitSettings;
use sidereal_core::remote_inspect::RemoteInspectConfig;
use std::fs::OpenOptions;
use std::io::Write;
use tracing_subscriber::FmtSubscriber;

use crate::runtime;

use super::{
    CliAction, apply_process_cli, configure_remote, native_asset_cache_adapter,
    native_gateway_http_adapter,
};

pub(crate) fn build_headless_client_app(
    asset_root: String,
    gateway_http_adapter: runtime::GatewayHttpAdapter,
    asset_cache_adapter: runtime::AssetCacheAdapter,
) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::log::LogPlugin::default());
    app.add_plugins(bevy::transform::TransformPlugin);
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(ScenePlugin);
    // Avian's collider cache reads mesh asset events even in headless mode.
    app.add_message::<bevy::asset::AssetEvent<Mesh>>();
    app.init_asset::<Mesh>();
    app.init_asset::<Image>();
    app.init_asset::<bevy::shader::Shader>();
    runtime::configure_client_runtime(
        &mut app,
        asset_root,
        true,
        gateway_http_adapter,
        asset_cache_adapter,
    );
    app
}

pub(crate) fn run() {
    match apply_process_cli() {
        Ok(CliAction::Run) => {}
        Ok(CliAction::Help(help)) => {
            println!("{help}");
            return;
        }
        Err(err) => {
            emit_startup_tracing_error(&err.to_string());
            std::process::exit(2);
        }
    }
    runtime::install_panic_file_hook();

    let headless_transport = std::env::var("SIDEREAL_CLIENT_HEADLESS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let remote_cfg = match RemoteInspectConfig::from_env("CLIENT", 15714) {
        Ok(cfg) => cfg,
        Err(err) => {
            emit_startup_tracing_error(&format!("invalid CLIENT BRP config: {err}"));
            std::process::exit(2);
        }
    };

    let asset_root = std::env::var("SIDEREAL_ASSET_ROOT").unwrap_or_else(|_| ".".to_string());
    let mut app = if headless_transport {
        build_headless_client_app(
            asset_root.clone(),
            native_gateway_http_adapter(),
            native_asset_cache_adapter(),
        )
    } else {
        runtime::build_windowed_client_app(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        present_mode: PresentMode::AutoVsync,
                        resizable: true,
                        resize_constraints: WindowResizeConstraints {
                            min_width: runtime::MIN_WINDOW_WIDTH,
                            min_height: runtime::MIN_WINDOW_HEIGHT,
                            ..default()
                        },
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::asset::AssetPlugin {
                    file_path: asset_root.clone(),
                    ..Default::default()
                })
                .set(LogPlugin {
                    custom_layer: runtime::build_log_capture_layer,
                    fmt_layer: runtime::build_file_fmt_layer,
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(runtime::configured_wgpu_settings()),
                    ..Default::default()
                }),
            asset_root.clone(),
            native_gateway_http_adapter(),
            native_asset_cache_adapter(),
        )
    };

    configure_remote(&mut app, &remote_cfg);
    if !headless_transport {
        app.insert_resource(WinitSettings::continuous());
    }
    app.run();
}

fn log_startup_error_line(message: &str) {
    let path = runtime::log_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{message}");
    }
}

fn emit_startup_tracing_error(message: &str) {
    log_startup_error_line(message);
    let subscriber = FmtSubscriber::builder()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .without_time()
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        tracing::error!("{message}");
    });
}
