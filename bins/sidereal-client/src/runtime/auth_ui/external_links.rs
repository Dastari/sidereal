fn forgot_password_url() -> String {
    let base = dashboard_base_url();
    format!("{}/forgot-password", base.trim_end_matches('/'))
}

#[cfg(not(target_arch = "wasm32"))]
fn dashboard_base_url() -> String {
    std::env::var("SIDEREAL_DASHBOARD_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string())
}

#[cfg(target_arch = "wasm32")]
fn dashboard_base_url() -> String {
    "/".to_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn open_external_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        command.arg(url);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(url);
        command
    };

    command.spawn().map(|_| ()).map_err(|err| err.to_string())
}

#[cfg(target_arch = "wasm32")]
fn open_external_url(url: &str) -> Result<(), String> {
    web_sys::window()
        .ok_or_else(|| "browser window is unavailable".to_string())?
        .open_with_url(url)
        .map(|_| ())
        .map_err(|err| format!("{err:?}"))
}

fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);
    !is_in_private_use_area && !chr.is_ascii_control()
}
