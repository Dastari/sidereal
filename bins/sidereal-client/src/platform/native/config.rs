#[cfg(not(target_arch = "wasm32"))]
use bevy::render::settings::Backends;
#[cfg(not(target_arch = "wasm32"))]
use std::env;
#[cfg(not(target_arch = "wasm32"))]
use std::net::{IpAddr, SocketAddr};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) enum CliAction {
    Run,
    Help(String),
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn apply_process_cli() -> Result<CliAction, String> {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(CliAction::Help(help_text())),
            "--gateway-url" => set_env("GATEWAY_URL", required_value(&mut args, "--gateway-url")?),
            "--asset-root" => set_env(
                "SIDEREAL_ASSET_ROOT",
                required_value(&mut args, "--asset-root")?,
            ),
            "--headless" => set_env("SIDEREAL_CLIENT_HEADLESS", "true".to_string()),
            "--replication-udp-addr" => {
                let value = required_value(&mut args, "--replication-udp-addr")?;
                parse_socket_addr("--replication-udp-addr", &value)?;
                set_env("REPLICATION_UDP_ADDR", value);
            }
            "--client-udp-bind" => {
                let value = required_value(&mut args, "--client-udp-bind")?;
                parse_socket_addr("--client-udp-bind", &value)?;
                set_env("CLIENT_UDP_BIND", value);
            }
            "--wgpu-backends" => {
                let value = required_value(&mut args, "--wgpu-backends")?;
                validate_backends(&value)?;
                set_env("SIDEREAL_CLIENT_WGPU_BACKENDS", value);
            }
            "--force-software-adapter" => {
                set_env("SIDEREAL_CLIENT_FORCE_SOFTWARE_ADAPTER", "true".to_string())
            }
            "--enable-shader-materials" => {
                set_env("SIDEREAL_ENABLE_SHADER_MATERIALS", "true".to_string())
            }
            "--disable-shader-materials" => {
                set_env("SIDEREAL_ENABLE_SHADER_MATERIALS", "false".to_string())
            }
            "--enable-streamed-shader-overrides" => set_env(
                "SIDEREAL_CLIENT_ENABLE_STREAMED_SHADER_OVERRIDES",
                "true".to_string(),
            ),
            "--disable-streamed-shader-overrides" => set_env(
                "SIDEREAL_CLIENT_ENABLE_STREAMED_SHADER_OVERRIDES",
                "false".to_string(),
            ),
            "--session-ready-timeout-s" => {
                set_env(
                    "SIDEREAL_CLIENT_SESSION_READY_TIMEOUT_S",
                    parse_f64(
                        "--session-ready-timeout-s",
                        &required_value(&mut args, "--session-ready-timeout-s")?,
                    )?
                    .to_string(),
                );
            }
            "--defer-warn-after-s" => {
                set_env(
                    "SIDEREAL_CLIENT_DEFER_WARN_AFTER_S",
                    parse_f64(
                        "--defer-warn-after-s",
                        &required_value(&mut args, "--defer-warn-after-s")?,
                    )?
                    .to_string(),
                );
            }
            "--defer-warn-interval-s" => {
                set_env(
                    "SIDEREAL_CLIENT_DEFER_WARN_INTERVAL_S",
                    parse_f64(
                        "--defer-warn-interval-s",
                        &required_value(&mut args, "--defer-warn-interval-s")?,
                    )?
                    .to_string(),
                );
            }
            "--defer-dialog-after-s" => {
                set_env(
                    "SIDEREAL_CLIENT_DEFER_DIALOG_AFTER_S",
                    parse_f64(
                        "--defer-dialog-after-s",
                        &required_value(&mut args, "--defer-dialog-after-s")?,
                    )?
                    .to_string(),
                );
            }
            "--defer-summary-interval-s" => {
                set_env(
                    "SIDEREAL_CLIENT_DEFER_SUMMARY_INTERVAL_S",
                    parse_f64(
                        "--defer-summary-interval-s",
                        &required_value(&mut args, "--defer-summary-interval-s")?,
                    )?
                    .to_string(),
                );
            }
            "--rollback-state" => {
                let value = required_value(&mut args, "--rollback-state")?;
                validate_enum("--rollback-state", &value, &["always", "check", "disabled"])?;
                set_env("SIDEREAL_CLIENT_ROLLBACK_STATE", value);
            }
            "--max-rollback-ticks" => {
                set_env(
                    "SIDEREAL_CLIENT_MAX_ROLLBACK_TICKS",
                    parse_u16(
                        "--max-rollback-ticks",
                        &required_value(&mut args, "--max-rollback-ticks")?,
                    )?
                    .to_string(),
                );
            }
            "--input-delay-ticks" => {
                set_env(
                    "SIDEREAL_CLIENT_INPUT_DELAY_TICKS",
                    parse_u16(
                        "--input-delay-ticks",
                        &required_value(&mut args, "--input-delay-ticks")?,
                    )?
                    .to_string(),
                );
            }
            "--input-max-predicted-ticks" => {
                set_env(
                    "SIDEREAL_CLIENT_MAX_PREDICTED_TICKS",
                    parse_u16(
                        "--input-max-predicted-ticks",
                        &required_value(&mut args, "--input-max-predicted-ticks")?,
                    )?
                    .to_string(),
                );
            }
            "--input-unfocused-max-predicted-ticks" => {
                set_env(
                    "SIDEREAL_CLIENT_UNFOCUSED_MAX_PREDICTED_TICKS",
                    parse_u16(
                        "--input-unfocused-max-predicted-ticks",
                        &required_value(&mut args, "--input-unfocused-max-predicted-ticks")?,
                    )?
                    .to_string(),
                );
            }
            "--interpolation-min-delay-ms" => {
                set_env(
                    "SIDEREAL_CLIENT_INTERPOLATION_MIN_DELAY_MS",
                    parse_u64(
                        "--interpolation-min-delay-ms",
                        &required_value(&mut args, "--interpolation-min-delay-ms")?,
                    )?
                    .to_string(),
                );
            }
            "--interpolation-send-interval-ratio" => {
                set_env(
                    "SIDEREAL_CLIENT_INTERPOLATION_SEND_INTERVAL_RATIO",
                    parse_f32(
                        "--interpolation-send-interval-ratio",
                        &required_value(&mut args, "--interpolation-send-interval-ratio")?,
                    )?
                    .to_string(),
                );
            }
            "--instant-correction" => {
                set_env("SIDEREAL_CLIENT_INSTANT_CORRECTION", "true".to_string())
            }
            "--nearby-collision-proxy-radius-m" => {
                set_env(
                    "SIDEREAL_CLIENT_NEARBY_COLLISION_PROXY_RADIUS_M",
                    parse_f32(
                        "--nearby-collision-proxy-radius-m",
                        &required_value(&mut args, "--nearby-collision-proxy-radius-m")?,
                    )?
                    .to_string(),
                );
            }
            "--nearby-collision-proxy-max" => {
                set_env(
                    "SIDEREAL_CLIENT_NEARBY_COLLISION_PROXY_MAX",
                    parse_usize(
                        "--nearby-collision-proxy-max",
                        &required_value(&mut args, "--nearby-collision-proxy-max")?,
                    )?
                    .to_string(),
                );
            }
            "--motion-ownership-reconcile-interval-s" => {
                set_env(
                    "SIDEREAL_CLIENT_MOTION_OWNERSHIP_RECONCILE_INTERVAL_S",
                    parse_f64(
                        "--motion-ownership-reconcile-interval-s",
                        &required_value(&mut args, "--motion-ownership-reconcile-interval-s")?,
                    )?
                    .to_string(),
                );
            }
            "--headless-player-entity-id" => set_env(
                "SIDEREAL_CLIENT_HEADLESS_PLAYER_ENTITY_ID",
                required_value(&mut args, "--headless-player-entity-id")?,
            ),
            "--headless-access-token" => set_env(
                "SIDEREAL_CLIENT_HEADLESS_ACCESS_TOKEN",
                required_value(&mut args, "--headless-access-token")?,
            ),
            "--headless-switch-player-entity-id" => set_env(
                "SIDEREAL_CLIENT_HEADLESS_SWITCH_PLAYER_ENTITY_ID",
                required_value(&mut args, "--headless-switch-player-entity-id")?,
            ),
            "--headless-switch-access-token" => set_env(
                "SIDEREAL_CLIENT_HEADLESS_SWITCH_ACCESS_TOKEN",
                required_value(&mut args, "--headless-switch-access-token")?,
            ),
            "--headless-switch-after-s" => {
                set_env(
                    "SIDEREAL_CLIENT_HEADLESS_SWITCH_AFTER_S",
                    parse_f64(
                        "--headless-switch-after-s",
                        &required_value(&mut args, "--headless-switch-after-s")?,
                    )?
                    .to_string(),
                );
            }
            "--client-brp-enabled" => set_env("SIDEREAL_CLIENT_BRP_ENABLED", "true".to_string()),
            "--client-brp-bind-addr" => {
                let value = required_value(&mut args, "--client-brp-bind-addr")?;
                parse_ip_addr("--client-brp-bind-addr", &value)?;
                set_env("SIDEREAL_CLIENT_BRP_BIND_ADDR", value);
            }
            "--client-brp-port" => {
                set_env(
                    "SIDEREAL_CLIENT_BRP_PORT",
                    parse_u16(
                        "--client-brp-port",
                        &required_value(&mut args, "--client-brp-port")?,
                    )?
                    .to_string(),
                );
            }
            "--client-brp-auth-token" => set_env(
                "SIDEREAL_CLIENT_BRP_AUTH_TOKEN",
                required_value(&mut args, "--client-brp-auth-token")?,
            ),
            "--log-file" => set_env(
                "SIDEREAL_CLIENT_LOG_FILE",
                required_value(&mut args, "--log-file")?,
            ),
            other if other.starts_with('-') => {
                return Err(format!("unrecognized option: {other}\n\n{}", help_text()));
            }
            other => {
                return Err(format!(
                    "unexpected positional argument: {other}\n\n{}",
                    help_text()
                ));
            }
        }
    }
    apply_startup_defaults();
    Ok(CliAction::Run)
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_startup_defaults() {
    set_default_env("GATEWAY_URL", "http://127.0.0.1:8080");
    set_default_env("SIDEREAL_ASSET_ROOT", ".");
    set_default_env("REPLICATION_UDP_ADDR", "127.0.0.1:7001");
    set_default_env("CLIENT_UDP_BIND", "127.0.0.1:0");
}

#[cfg(not(target_arch = "wasm32"))]
fn required_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value"))
        .and_then(|value| {
            if value.starts_with('-') {
                Err(format!("{flag} requires a value"))
            } else {
                Ok(value)
            }
        })
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_socket_addr(flag: &str, value: &str) -> Result<SocketAddr, String> {
    value
        .parse::<SocketAddr>()
        .map_err(|err| format!("invalid value for {flag}: {err}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_ip_addr(flag: &str, value: &str) -> Result<IpAddr, String> {
    value
        .parse::<IpAddr>()
        .map_err(|err| format!("invalid value for {flag}: {err}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_u16(flag: &str, value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .map_err(|err| format!("invalid value for {flag}: {err}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_u64(flag: &str, value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|err| format!("invalid value for {flag}: {err}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_usize(flag: &str, value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|err| format!("invalid value for {flag}: {err}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_f64(flag: &str, value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("invalid value for {flag}: {value}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_f32(flag: &str, value: &str) -> Result<f32, String> {
    value
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("invalid value for {flag}: {value}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_enum(flag: &str, value: &str, allowed: &[&str]) -> Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "invalid value for {flag}: {value} (expected one of: {})",
            allowed.join(", ")
        ))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_backends(value: &str) -> Result<(), String> {
    let parsed = Backends::from_comma_list(value);
    if parsed.is_empty() {
        Err(format!(
            "invalid value for --wgpu-backends: {value} (expected comma-separated backends such as vulkan,dx12,metal,gl,webgpu)"
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn set_env(key: &str, value: String) {
    // SAFETY: CLI processing happens once during startup, before the app builds worker threads.
    unsafe {
        env::set_var(key, value);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn set_default_env(key: &str, value: &str) {
    if env::var_os(key).is_some() {
        return;
    }
    // SAFETY: CLI processing happens once during startup, before the app builds worker threads.
    unsafe {
        env::set_var(key, value);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn help_text() -> String {
    [
        "Sidereal native client",
        "",
        "Usage:",
        "  sidereal-client [OPTIONS]",
        "",
        "General options:",
        "  -h, --help                               Show this help screen and exit",
        "      --gateway-url URL                   Gateway base URL (default/env: http://127.0.0.1:8080 / GATEWAY_URL)",
        "      --asset-root PATH                   Asset/cache root path (default/env: . / SIDEREAL_ASSET_ROOT)",
        "      --headless                          Run the native client in headless transport mode (env: SIDEREAL_CLIENT_HEADLESS=1)",
        "      --log-file PATH                     Override panic/startup log file path (env: SIDEREAL_CLIENT_LOG_FILE)",
        "",
        "Native transport options:",
        "      --replication-udp-addr ADDR         Fallback replication UDP address host:port when gateway bootstrap does not supply one",
        "                                          default/env: 127.0.0.1:7001 / REPLICATION_UDP_ADDR",
        "      --client-udp-bind ADDR              Local UDP bind address host:port for the native client socket",
        "                                          default/env: 127.0.0.1:0 / CLIENT_UDP_BIND",
        "",
        "Rendering options:",
        "      --wgpu-backends LIST                Comma-separated backend preference list, for example 'vulkan' or 'vulkan,dx12'",
        "                                          env: SIDEREAL_CLIENT_WGPU_BACKENDS",
        "      --force-software-adapter            Force the WGPU fallback/software adapter",
        "                                          env: SIDEREAL_CLIENT_FORCE_SOFTWARE_ADAPTER=1",
        "      --enable-shader-materials           Enable shader-material rendering paths",
        "      --disable-shader-materials          Disable shader-material rendering paths",
        "                                          env: SIDEREAL_ENABLE_SHADER_MATERIALS=(true|false)",
        "      --enable-streamed-shader-overrides  Use streamed cached shader overrides when present",
        "      --disable-streamed-shader-overrides Use built-in fallback shader sources only",
        "                                          env: SIDEREAL_CLIENT_ENABLE_STREAMED_SHADER_OVERRIDES=(true|false)",
        "",
        "Session and prediction options:",
        "      --session-ready-timeout-s SECONDS   Timeout before session-ready bootstrap is treated as stalled",
        "                                          env: SIDEREAL_CLIENT_SESSION_READY_TIMEOUT_S",
        "      --defer-warn-after-s SECONDS        Delay before logging predicted-adoption warnings",
        "                                          env: SIDEREAL_CLIENT_DEFER_WARN_AFTER_S",
        "      --defer-warn-interval-s SECONDS     Minimum interval between predicted-adoption warnings",
        "                                          env: SIDEREAL_CLIENT_DEFER_WARN_INTERVAL_S",
        "      --defer-dialog-after-s SECONDS      Delay before surfacing predicted-adoption warning dialog",
        "                                          env: SIDEREAL_CLIENT_DEFER_DIALOG_AFTER_S",
        "      --defer-summary-interval-s SECONDS  Interval between prediction runtime summaries",
        "                                          env: SIDEREAL_CLIENT_DEFER_SUMMARY_INTERVAL_S",
        "      --rollback-state MODE               Prediction rollback mode: always, check, disabled",
        "                                          env: SIDEREAL_CLIENT_ROLLBACK_STATE",
        "      --max-rollback-ticks TICKS          Maximum rollback window for the prediction manager",
        "                                          env: SIDEREAL_CLIENT_MAX_ROLLBACK_TICKS",
        "      --input-delay-ticks TICKS           Fixed client input delay applied to Lightyear timeline sync",
        "                                          env: SIDEREAL_CLIENT_INPUT_DELAY_TICKS",
        "      --input-max-predicted-ticks TICKS   Maximum client prediction lead allowed while focused",
        "                                          env: SIDEREAL_CLIENT_MAX_PREDICTED_TICKS",
        "      --input-unfocused-max-predicted-ticks TICKS",
        "                                          Maximum client prediction lead allowed while unfocused",
        "                                          defaults to focused max; set 0 for strict focus-stall testing",
        "                                          env: SIDEREAL_CLIENT_UNFOCUSED_MAX_PREDICTED_TICKS",
        "      --interpolation-min-delay-ms MS     Minimum interpolation delay applied to remote entities",
        "                                          env: SIDEREAL_CLIENT_INTERPOLATION_MIN_DELAY_MS",
        "      --interpolation-send-interval-ratio RATIO",
        "                                          Interpolation delay as a multiple of sender update interval",
        "                                          env: SIDEREAL_CLIENT_INTERPOLATION_SEND_INTERVAL_RATIO",
        "      --instant-correction                Use instant correction instead of smooth correction",
        "                                          env: SIDEREAL_CLIENT_INSTANT_CORRECTION=1",
        "      --nearby-collision-proxy-radius-m M Radius around the controlled entity for local collision proxies",
        "                                          env: SIDEREAL_CLIENT_NEARBY_COLLISION_PROXY_RADIUS_M",
        "      --nearby-collision-proxy-max N      Maximum nearby collision proxies kept active",
        "                                          env: SIDEREAL_CLIENT_NEARBY_COLLISION_PROXY_MAX",
        "      --motion-ownership-reconcile-interval-s SECONDS",
        "                                          Interval between motion-ownership proxy reconciliation passes",
        "                                          env: SIDEREAL_CLIENT_MOTION_OWNERSHIP_RECONCILE_INTERVAL_S",
        "",
        "Headless auth/bootstrap options:",
        "      --headless-player-entity-id ID      Seed headless session player entity id",
        "                                          env: SIDEREAL_CLIENT_HEADLESS_PLAYER_ENTITY_ID",
        "      --headless-access-token TOKEN       Seed headless session access token",
        "                                          env: SIDEREAL_CLIENT_HEADLESS_ACCESS_TOKEN",
        "      --headless-switch-player-entity-id ID",
        "                                          Schedule one headless account switch to the given player entity id",
        "                                          env: SIDEREAL_CLIENT_HEADLESS_SWITCH_PLAYER_ENTITY_ID",
        "      --headless-switch-access-token TOKEN",
        "                                          Access token paired with --headless-switch-player-entity-id",
        "                                          env: SIDEREAL_CLIENT_HEADLESS_SWITCH_ACCESS_TOKEN",
        "      --headless-switch-after-s SECONDS   Delay before applying the scripted headless account switch",
        "                                          env: SIDEREAL_CLIENT_HEADLESS_SWITCH_AFTER_S",
        "",
        "Client BRP options:",
        "      --client-brp-enabled                Enable Bevy Remote inspection on the client",
        "                                          env: SIDEREAL_CLIENT_BRP_ENABLED=1",
        "      --client-brp-bind-addr IP           BRP bind address; loopback only",
        "                                          env: SIDEREAL_CLIENT_BRP_BIND_ADDR",
        "      --client-brp-port PORT              BRP port",
        "                                          env: SIDEREAL_CLIENT_BRP_PORT",
        "      --client-brp-auth-token TOKEN       BRP auth token; required when BRP is enabled",
        "                                          env: SIDEREAL_CLIENT_BRP_AUTH_TOKEN",
        "",
        "Notes:",
        "  Command-line options override environment variables for the current process.",
        "  Debug-only environment toggles and diagnostic kill switches have been removed from the native client startup surface.",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::apply_startup_defaults;

    #[test]
    fn startup_defaults_seed_native_runtime_endpoints() {
        // SAFETY: test mutates process env in a narrow single-threaded assertion block.
        unsafe {
            std::env::remove_var("GATEWAY_URL");
            std::env::remove_var("SIDEREAL_ASSET_ROOT");
            std::env::remove_var("REPLICATION_UDP_ADDR");
            std::env::remove_var("CLIENT_UDP_BIND");
        }
        apply_startup_defaults();
        assert_eq!(
            std::env::var("GATEWAY_URL").as_deref(),
            Ok("http://127.0.0.1:8080")
        );
        assert_eq!(std::env::var("SIDEREAL_ASSET_ROOT").as_deref(), Ok("."));
        assert_eq!(
            std::env::var("REPLICATION_UDP_ADDR").as_deref(),
            Ok("127.0.0.1:7001")
        );
        assert_eq!(
            std::env::var("CLIENT_UDP_BIND").as_deref(),
            Ok("127.0.0.1:0")
        );
    }
}
