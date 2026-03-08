#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod client_core;
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub fn run_native() {
    native::run();
}

#[cfg(target_arch = "wasm32")]
pub fn run_wasm() {
    wasm::run();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn boot_sidereal_client() {
    wasm::run();
}
