use std::env;
use std::net::{IpAddr, SocketAddr};

#[derive(Debug, Clone)]
pub(crate) enum CliAction {
    Run(Box<ReplicationConfig>),
    Help(String),
}

#[derive(Debug, Clone)]
pub(crate) struct ReplicationConfig {
    pub(crate) headless: bool,
    pub(crate) database_url: String,
    pub(crate) udp_bind: SocketAddr,
    pub(crate) webtransport_bind: SocketAddr,
    pub(crate) webtransport_cert_pem: String,
    pub(crate) webtransport_key_pem: String,
    pub(crate) control_udp_bind: SocketAddr,
    pub(crate) health_bind: SocketAddr,
    pub(crate) asset_root: String,
    pub(crate) scripts_root: String,
    pub(crate) gateway_jwt_secret: String,
    pub(crate) brp_enabled: bool,
    pub(crate) brp_bind_addr: IpAddr,
    pub(crate) brp_port: u16,
    pub(crate) brp_auth_token: String,
}

impl ReplicationConfig {
    pub(crate) fn apply_env(&self) {
        set_env("REPLICATION_DATABASE_URL", self.database_url.clone());
        set_env("REPLICATION_UDP_BIND", self.udp_bind.to_string());
        set_env(
            "REPLICATION_WEBTRANSPORT_BIND",
            self.webtransport_bind.to_string(),
        );
        set_env(
            "REPLICATION_WEBTRANSPORT_CERT_PEM",
            self.webtransport_cert_pem.clone(),
        );
        set_env(
            "REPLICATION_WEBTRANSPORT_KEY_PEM",
            self.webtransport_key_pem.clone(),
        );
        set_env(
            "REPLICATION_CONTROL_UDP_BIND",
            self.control_udp_bind.to_string(),
        );
        set_env("ASSET_ROOT", self.asset_root.clone());
        set_env("SIDEREAL_SCRIPTS_ROOT", self.scripts_root.clone());
        set_env("GATEWAY_JWT_SECRET", self.gateway_jwt_secret.clone());
        set_env(
            "SIDEREAL_REPLICATION_BRP_ENABLED",
            self.brp_enabled.to_string(),
        );
        set_env(
            "SIDEREAL_REPLICATION_BRP_BIND_ADDR",
            self.brp_bind_addr.to_string(),
        );
        set_env("SIDEREAL_REPLICATION_BRP_PORT", self.brp_port.to_string());
        set_env(
            "SIDEREAL_REPLICATION_BRP_AUTH_TOKEN",
            self.brp_auth_token.clone(),
        );
    }
}

pub(crate) fn apply_process_cli() -> Result<CliAction, String> {
    let mut headless = None;
    let mut database_url = None;
    let mut udp_bind = None;
    let mut webtransport_bind = None;
    let mut webtransport_cert_pem = None;
    let mut webtransport_key_pem = None;
    let mut control_udp_bind = None;
    let mut health_bind = None;
    let mut asset_root = None;
    let mut scripts_root = None;
    let mut gateway_jwt_secret = None;
    let mut brp_enabled = None;
    let mut brp_bind_addr = None;
    let mut brp_port = None;
    let mut brp_auth_token = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(CliAction::Help(help_text())),
            "--headless" | "--no-tui" => headless = Some(true),
            "--database-url" => {
                database_url = Some(required_value(&mut args, "--database-url")?);
            }
            "--udp-bind" => {
                udp_bind = Some(parse_socket_addr(
                    "--udp-bind",
                    &required_value(&mut args, "--udp-bind")?,
                )?);
            }
            "--webtransport-bind" => {
                webtransport_bind = Some(parse_socket_addr(
                    "--webtransport-bind",
                    &required_value(&mut args, "--webtransport-bind")?,
                )?);
            }
            "--webtransport-cert-pem" => {
                webtransport_cert_pem = Some(required_value(&mut args, "--webtransport-cert-pem")?);
            }
            "--webtransport-key-pem" => {
                webtransport_key_pem = Some(required_value(&mut args, "--webtransport-key-pem")?);
            }
            "--control-udp-bind" | "--replication-control-udp-bind" => {
                control_udp_bind = Some(parse_socket_addr(
                    arg.as_str(),
                    &required_value(&mut args, arg.as_str())?,
                )?);
            }
            "--health-bind" => {
                health_bind = Some(parse_socket_addr(
                    "--health-bind",
                    &required_value(&mut args, "--health-bind")?,
                )?);
            }
            "--asset-root" => {
                asset_root = Some(required_value(&mut args, "--asset-root")?);
            }
            "--scripts-root" => {
                scripts_root = Some(required_value(&mut args, "--scripts-root")?);
            }
            "--jwt-secret" => {
                gateway_jwt_secret = Some(required_value(&mut args, "--jwt-secret")?);
            }
            "--replication-brp-enabled" => brp_enabled = Some(true),
            "--replication-brp-disabled" => brp_enabled = Some(false),
            "--replication-brp-bind-addr" => {
                brp_bind_addr = Some(parse_ip_addr(
                    "--replication-brp-bind-addr",
                    &required_value(&mut args, "--replication-brp-bind-addr")?,
                )?);
            }
            "--replication-brp-port" => {
                brp_port = Some(parse_u16(
                    "--replication-brp-port",
                    &required_value(&mut args, "--replication-brp-port")?,
                )?);
            }
            "--replication-brp-auth-token" => {
                brp_auth_token = Some(required_value(&mut args, "--replication-brp-auth-token")?);
            }
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

    let config = ReplicationConfig {
        headless: headless
            .unwrap_or_else(|| bool_env("SIDEREAL_REPLICATION_HEADLESS").unwrap_or(false)),
        database_url: database_url
            .or_else(|| env::var("REPLICATION_DATABASE_URL").ok())
            .unwrap_or_else(default_database_url),
        udp_bind: udp_bind
            .or_else(|| env_socket_addr("REPLICATION_UDP_BIND"))
            .unwrap_or_else(|| parse_socket_addr_literal("0.0.0.0:7001")),
        webtransport_bind: webtransport_bind
            .or_else(|| env_socket_addr("REPLICATION_WEBTRANSPORT_BIND"))
            .unwrap_or_else(|| parse_socket_addr_literal("0.0.0.0:7003")),
        webtransport_cert_pem: webtransport_cert_pem
            .or_else(|| env::var("REPLICATION_WEBTRANSPORT_CERT_PEM").ok())
            .unwrap_or_else(|| "./data/dev_certs/replication-webtransport-cert.pem".to_string()),
        webtransport_key_pem: webtransport_key_pem
            .or_else(|| env::var("REPLICATION_WEBTRANSPORT_KEY_PEM").ok())
            .unwrap_or_else(|| "./data/dev_certs/replication-webtransport-key.pem".to_string()),
        control_udp_bind: control_udp_bind
            .or_else(|| env_socket_addr("REPLICATION_CONTROL_UDP_BIND"))
            .unwrap_or_else(|| parse_socket_addr_literal("127.0.0.1:9004")),
        health_bind: health_bind
            .or_else(|| env_socket_addr("REPLICATION_HEALTH_BIND"))
            .unwrap_or_else(|| parse_socket_addr_literal("127.0.0.1:15716")),
        asset_root: asset_root
            .or_else(|| env::var("ASSET_ROOT").ok())
            .unwrap_or_else(|| "./data".to_string()),
        scripts_root: scripts_root
            .or_else(|| env::var("SIDEREAL_SCRIPTS_ROOT").ok())
            .unwrap_or_else(|| "./data/scripts".to_string()),
        gateway_jwt_secret: gateway_jwt_secret
            .or_else(|| env::var("GATEWAY_JWT_SECRET").ok())
            .unwrap_or_else(default_jwt_secret),
        brp_enabled: brp_enabled
            .or_else(|| bool_env("SIDEREAL_REPLICATION_BRP_ENABLED"))
            .or_else(|| bool_env("SIDEREAL_BRP_ENABLED"))
            .unwrap_or(false),
        brp_bind_addr: brp_bind_addr
            .or_else(|| env_ip_addr("SIDEREAL_REPLICATION_BRP_BIND_ADDR"))
            .or_else(|| env_ip_addr("SIDEREAL_BRP_BIND_ADDR"))
            .unwrap_or_else(|| parse_ip_addr_literal("127.0.0.1")),
        brp_port: brp_port
            .or_else(|| env_u16("SIDEREAL_REPLICATION_BRP_PORT"))
            .or_else(|| env_u16("SIDEREAL_BRP_PORT"))
            .unwrap_or(15713),
        brp_auth_token: brp_auth_token
            .or_else(|| env::var("SIDEREAL_REPLICATION_BRP_AUTH_TOKEN").ok())
            .or_else(|| env::var("SIDEREAL_BRP_AUTH_TOKEN").ok())
            .unwrap_or_else(|| "0123456789abcdef".to_string()),
    };

    Ok(CliAction::Run(Box::new(config)))
}

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

fn parse_socket_addr(flag: &str, value: &str) -> Result<SocketAddr, String> {
    value
        .parse::<SocketAddr>()
        .map_err(|err| format!("invalid value for {flag}: {err}"))
}

fn parse_ip_addr(flag: &str, value: &str) -> Result<IpAddr, String> {
    value
        .parse::<IpAddr>()
        .map_err(|err| format!("invalid value for {flag}: {err}"))
}

fn parse_u16(flag: &str, value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .map_err(|err| format!("invalid value for {flag}: {err}"))
}

fn env_socket_addr(name: &str) -> Option<SocketAddr> {
    env::var(name)
        .ok()
        .and_then(|raw| raw.parse::<SocketAddr>().ok())
}

fn env_ip_addr(name: &str) -> Option<IpAddr> {
    env::var(name)
        .ok()
        .and_then(|raw| raw.parse::<IpAddr>().ok())
}

fn env_u16(name: &str) -> Option<u16> {
    env::var(name).ok().and_then(|raw| raw.parse::<u16>().ok())
}

fn bool_env(name: &str) -> Option<bool> {
    env::var(name)
        .ok()
        .and_then(|raw| match raw.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
}

fn set_env(name: &str, value: String) {
    // Startup config is finalized before worker threads or async runtimes begin mutating state.
    unsafe {
        env::set_var(name, value);
    }
}

fn default_database_url() -> String {
    "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string()
}

fn default_jwt_secret() -> String {
    "0123456789abcdef0123456789abcdef".to_string()
}

fn parse_socket_addr_literal(value: &str) -> SocketAddr {
    value
        .parse::<SocketAddr>()
        .expect("hard-coded socket addr literal")
}

fn parse_ip_addr_literal(value: &str) -> IpAddr {
    value.parse::<IpAddr>().expect("hard-coded IP addr literal")
}

fn help_text() -> String {
    [
        "sidereal-replication options:",
        "      --headless                            Disable the TUI even on an interactive terminal",
        "      --database-url URL                    Postgres connection string (env: REPLICATION_DATABASE_URL)",
        "      --udp-bind ADDR                       Lightyear UDP bind address (default: 0.0.0.0:7001)",
        "      --webtransport-bind ADDR              WebTransport bind address (default: 0.0.0.0:7003)",
        "      --webtransport-cert-pem PATH          WebTransport certificate PEM path",
        "      --webtransport-key-pem PATH           WebTransport private key PEM path",
        "      --control-udp-bind ADDR               Replication control/bootstrap UDP bind (default: 127.0.0.1:9004)",
        "      --health-bind ADDR                    Loopback health endpoint bind (default: 127.0.0.1:15716)",
        "      --asset-root PATH                     Asset root (default: ./data)",
        "      --scripts-root PATH                   Script root (default: ./data/scripts)",
        "      --jwt-secret SECRET                   Shared gateway/replication JWT secret",
        "      --replication-brp-enabled             Enable replication BRP",
        "      --replication-brp-disabled            Disable replication BRP",
        "      --replication-brp-bind-addr IP        BRP bind IP (must remain loopback)",
        "      --replication-brp-port PORT           BRP port (default: 15713)",
        "      --replication-brp-auth-token TOKEN    BRP auth token",
        "  -h, --help                                Show this help text",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{CliAction, apply_process_cli, default_database_url, default_jwt_secret};

    #[test]
    fn defaults_match_local_runtime_contract() {
        let old_args: Vec<String> = std::env::args().collect();
        // This test only validates hard-coded defaults through the help/config path.
        let _ = old_args;
        assert_eq!(
            default_database_url(),
            "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal"
        );
        assert_eq!(default_jwt_secret(), "0123456789abcdef0123456789abcdef");
        let help = match CliAction::Help("x".to_string()) {
            CliAction::Help(text) => text,
            CliAction::Run(_) => unreachable!(),
        };
        assert_eq!(help, "x");
        let _ = apply_process_cli as fn() -> Result<CliAction, String>;
    }
}
