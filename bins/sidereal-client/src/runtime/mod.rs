mod app_builder;
mod app_setup;
mod auth_ui;
mod dev_console;
mod dialog_ui;
mod ecs_util;

mod app_state;
mod asset_loading_ui;
mod assets;
mod audio;
mod auth_net;
mod backdrop;
mod bootstrap;
mod camera;
mod combat_messages;
mod components;
mod control;
mod debug_overlay;
mod input;
mod lighting;
mod logout;
mod motion;
mod notification_ui;
mod owner_manifest;
mod pause_menu;
mod platform;
mod plugins;
mod post_process;
mod render_layers;
mod replication;
mod resources;
mod scene;
mod scene_world;
mod shaders;
mod startup_assets;
mod startup_loading_ui;
mod tactical;
mod transforms;
mod transport;
mod ui;
mod visuals;
mod world_loading_ui;

pub(crate) use app_builder::build_windowed_client_app;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use app_setup::configure_client_runtime;
pub(crate) use app_state::*;
pub(crate) use auth_net::submit_auth_request;
#[allow(unused_imports)]
pub(crate) use dev_console::build_log_capture_layer;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use dev_console::{build_file_fmt_layer, install_panic_file_hook, log_file_path};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use platform::{MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, configured_wgpu_settings};
pub(crate) use resources::*;
