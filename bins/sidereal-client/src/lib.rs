#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod client_core;
mod platform;
mod runtime;

#[cfg(not(target_arch = "wasm32"))]
pub fn run_native() {
    platform::native::run();
}

#[cfg(target_arch = "wasm32")]
pub fn run_wasm() {
    platform::wasm::run();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn boot_sidereal_client() {
    platform::wasm::run();
}
