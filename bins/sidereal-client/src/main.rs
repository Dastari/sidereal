#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod client_core;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    native::run();
}

#[cfg(target_arch = "wasm32")]
fn main() {
    wasm::run();
}
