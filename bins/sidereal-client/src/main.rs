#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod client_core;
mod platform;
mod runtime;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    platform::native::run();
}

#[cfg(target_arch = "wasm32")]
fn main() {
    platform::wasm::run();
}
