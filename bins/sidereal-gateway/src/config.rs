use std::env;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub(crate) enum CliAction {
    Run(Box<GatewayConfig>),
    Help(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BootstrapMode {
    Udp,
    Direct,
}

impl BootstrapMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Udp => "udp",
            Self::Direct => "direct",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GatewayConfig {
    pub(crate) bind_addr: SocketAddr,
    pub(crate) database_url: String,
    pub(crate) bootstrap_mode: BootstrapMode,
    pub(crate) jwt_secret: String,
    pub(crate) access_token_ttl_s: u64,
    pub(crate) refresh_token_ttl_s: u64,
    pub(crate) reset_token_ttl_s: u64,
    pub(crate) allowed_origins: String,
    pub(crate) asset_root: String,
    pub(crate) scripts_root: String,
    pub(crate) replication_control_udp_bind: SocketAddr,
    pub(crate) replication_control_udp_addr: SocketAddr,
    pub(crate) replication_udp_public_addr: SocketAddr,
    pub(crate) replication_webtransport_public_addr: SocketAddr,
    pub(crate) replication_webtransport_cert_sha256: Option<String>,
}

impl GatewayConfig {
    pub(crate) fn apply_env(&self) {
        set_env("GATEWAY_BIND", self.bind_addr.to_string());
        set_env("GATEWAY_DATABASE_URL", self.database_url.clone());
        set_env(
            "GATEWAY_BOOTSTRAP_MODE",
            self.bootstrap_mode.as_str().to_string(),
        );
        set_env("GATEWAY_JWT_SECRET", self.jwt_secret.clone());
        set_env(
            "GATEWAY_ACCESS_TOKEN_TTL_S",
            self.access_token_ttl_s.to_string(),
        );
        set_env(
            "GATEWAY_REFRESH_TOKEN_TTL_S",
            self.refresh_token_ttl_s.to_string(),
        );
        set_env(
            "GATEWAY_RESET_TOKEN_TTL_S",
            self.reset_token_ttl_s.to_string(),
        );
        set_env("GATEWAY_ALLOWED_ORIGINS", self.allowed_origins.clone());
        set_env("ASSET_ROOT", self.asset_root.clone());
        set_env("SIDEREAL_SCRIPTS_ROOT", self.scripts_root.clone());
        set_env(
            "GATEWAY_REPLICATION_CONTROL_UDP_BIND",
            self.replication_control_udp_bind.to_string(),
        );
        set_env(
            "REPLICATION_CONTROL_UDP_ADDR",
            self.replication_control_udp_addr.to_string(),
        );
        set_env(
            "REPLICATION_UDP_PUBLIC_ADDR",
            self.replication_udp_public_addr.to_string(),
        );
        set_env(
            "REPLICATION_WEBTRANSPORT_PUBLIC_ADDR",
            self.replication_webtransport_public_addr.to_string(),
        );
        if let Some(sha256) = &self.replication_webtransport_cert_sha256 {
            set_env("REPLICATION_WEBTRANSPORT_CERT_SHA256", sha256.clone());
        }
    }
}

pub(crate) fn apply_process_cli() -> Result<CliAction, String> {
    let mut bind_addr = None;
    let mut database_url = None;
    let mut bootstrap_mode = None;
    let mut jwt_secret = None;
    let mut access_token_ttl_s = None;
    let mut refresh_token_ttl_s = None;
    let mut reset_token_ttl_s = None;
    let mut allowed_origins = None;
    let mut asset_root = None;
    let mut scripts_root = None;
    let mut replication_control_udp_bind = None;
    let mut replication_control_udp_addr = None;
    let mut replication_udp_public_addr = None;
    let mut replication_webtransport_public_addr = None;
    let mut replication_webtransport_cert_sha256 = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(CliAction::Help(help_text())),
            "--bind" => {
                bind_addr = Some(parse_socket_addr(
                    "--bind",
                    &required_value(&mut args, "--bind")?,
                )?);
            }
            "--database-url" => database_url = Some(required_value(&mut args, "--database-url")?),
            "--bootstrap-mode" => {
                bootstrap_mode = Some(parse_bootstrap_mode(
                    "--bootstrap-mode",
                    &required_value(&mut args, "--bootstrap-mode")?,
                )?);
            }
            "--jwt-secret" => jwt_secret = Some(required_value(&mut args, "--jwt-secret")?),
            "--access-token-ttl-s" => {
                access_token_ttl_s = Some(parse_u64(
                    "--access-token-ttl-s",
                    &required_value(&mut args, "--access-token-ttl-s")?,
                )?);
            }
            "--refresh-token-ttl-s" => {
                refresh_token_ttl_s = Some(parse_u64(
                    "--refresh-token-ttl-s",
                    &required_value(&mut args, "--refresh-token-ttl-s")?,
                )?);
            }
            "--reset-token-ttl-s" => {
                reset_token_ttl_s = Some(parse_u64(
                    "--reset-token-ttl-s",
                    &required_value(&mut args, "--reset-token-ttl-s")?,
                )?);
            }
            "--allowed-origins" => {
                allowed_origins = Some(required_value(&mut args, "--allowed-origins")?);
            }
            "--asset-root" => asset_root = Some(required_value(&mut args, "--asset-root")?),
            "--scripts-root" => scripts_root = Some(required_value(&mut args, "--scripts-root")?),
            "--replication-control-udp-bind" => {
                replication_control_udp_bind = Some(parse_socket_addr(
                    "--replication-control-udp-bind",
                    &required_value(&mut args, "--replication-control-udp-bind")?,
                )?);
            }
            "--replication-control-udp-addr" => {
                replication_control_udp_addr = Some(parse_socket_addr(
                    "--replication-control-udp-addr",
                    &required_value(&mut args, "--replication-control-udp-addr")?,
                )?);
            }
            "--replication-udp-public-addr" => {
                replication_udp_public_addr = Some(parse_socket_addr(
                    "--replication-udp-public-addr",
                    &required_value(&mut args, "--replication-udp-public-addr")?,
                )?);
            }
            "--replication-webtransport-public-addr" => {
                replication_webtransport_public_addr = Some(parse_socket_addr(
                    "--replication-webtransport-public-addr",
                    &required_value(&mut args, "--replication-webtransport-public-addr")?,
                )?);
            }
            "--replication-webtransport-cert-sha256" => {
                replication_webtransport_cert_sha256 = Some(required_value(
                    &mut args,
                    "--replication-webtransport-cert-sha256",
                )?);
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

    let config = GatewayConfig {
        bind_addr: bind_addr
            .or_else(|| env_socket_addr("GATEWAY_BIND"))
            .unwrap_or_else(|| parse_socket_addr_literal("0.0.0.0:8080")),
        database_url: database_url
            .or_else(|| env::var("GATEWAY_DATABASE_URL").ok())
            .unwrap_or_else(default_database_url),
        bootstrap_mode: bootstrap_mode
            .or_else(|| {
                env::var("GATEWAY_BOOTSTRAP_MODE")
                    .ok()
                    .and_then(|raw| parse_bootstrap_mode("GATEWAY_BOOTSTRAP_MODE", &raw).ok())
            })
            .unwrap_or(BootstrapMode::Udp),
        jwt_secret: jwt_secret
            .or_else(|| env::var("GATEWAY_JWT_SECRET").ok())
            .unwrap_or_else(default_jwt_secret),
        access_token_ttl_s: access_token_ttl_s
            .or_else(|| env_u64("GATEWAY_ACCESS_TOKEN_TTL_S"))
            .unwrap_or(900),
        refresh_token_ttl_s: refresh_token_ttl_s
            .or_else(|| env_u64("GATEWAY_REFRESH_TOKEN_TTL_S"))
            .unwrap_or(2_592_000),
        reset_token_ttl_s: reset_token_ttl_s
            .or_else(|| env_u64("GATEWAY_RESET_TOKEN_TTL_S"))
            .unwrap_or(3_600),
        allowed_origins: allowed_origins
            .or_else(|| env::var("GATEWAY_ALLOWED_ORIGINS").ok())
            .unwrap_or_else(|| "http://localhost:3000,http://127.0.0.1:3000".to_string()),
        asset_root: asset_root
            .or_else(|| env::var("ASSET_ROOT").ok())
            .unwrap_or_else(|| "./data".to_string()),
        scripts_root: scripts_root
            .or_else(|| env::var("SIDEREAL_SCRIPTS_ROOT").ok())
            .unwrap_or_else(|| "./data/scripts".to_string()),
        replication_control_udp_bind: replication_control_udp_bind
            .or_else(|| env_socket_addr("GATEWAY_REPLICATION_CONTROL_UDP_BIND"))
            .unwrap_or_else(|| parse_socket_addr_literal("0.0.0.0:0")),
        replication_control_udp_addr: replication_control_udp_addr
            .or_else(|| env_socket_addr("REPLICATION_CONTROL_UDP_ADDR"))
            .unwrap_or_else(|| parse_socket_addr_literal("127.0.0.1:9004")),
        replication_udp_public_addr: replication_udp_public_addr
            .or_else(|| env_socket_addr("REPLICATION_UDP_PUBLIC_ADDR"))
            .or_else(|| env_socket_addr("REPLICATION_UDP_ADDR"))
            .unwrap_or_else(|| parse_socket_addr_literal("127.0.0.1:7001")),
        replication_webtransport_public_addr: replication_webtransport_public_addr
            .or_else(|| env_socket_addr("REPLICATION_WEBTRANSPORT_PUBLIC_ADDR"))
            .unwrap_or_else(|| parse_socket_addr_literal("127.0.0.1:7003")),
        replication_webtransport_cert_sha256: replication_webtransport_cert_sha256
            .or_else(|| env::var("REPLICATION_WEBTRANSPORT_CERT_SHA256").ok()),
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

fn parse_u64(flag: &str, value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|err| format!("invalid value for {flag}: {err}"))
}

fn parse_bootstrap_mode(flag: &str, value: &str) -> Result<BootstrapMode, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "udp" => Ok(BootstrapMode::Udp),
        "direct" => Ok(BootstrapMode::Direct),
        _ => Err(format!(
            "invalid value for {flag}: expected one of udp, direct"
        )),
    }
}

fn env_socket_addr(name: &str) -> Option<SocketAddr> {
    env::var(name)
        .ok()
        .and_then(|raw| raw.parse::<SocketAddr>().ok())
}

fn env_u64(name: &str) -> Option<u64> {
    env::var(name).ok().and_then(|raw| raw.parse::<u64>().ok())
}

fn set_env(name: &str, value: String) {
    // Startup config is finalized before the async runtime begins serving requests.
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

fn help_text() -> String {
    [
        "sidereal-gateway options:",
        "      --bind ADDR                              HTTP bind address (default: 0.0.0.0:8080)",
        "      --database-url URL                       Postgres connection string (env: GATEWAY_DATABASE_URL)",
        "      --bootstrap-mode MODE                    Bootstrap handoff mode: udp or direct",
        "      --jwt-secret SECRET                      Gateway JWT secret",
        "      --access-token-ttl-s SECONDS             Access token TTL (default: 900)",
        "      --refresh-token-ttl-s SECONDS            Refresh token TTL (default: 2592000)",
        "      --reset-token-ttl-s SECONDS              Password-reset token TTL (default: 3600)",
        "      --allowed-origins CSV                    Comma-separated browser origins",
        "      --asset-root PATH                        Asset root (default: ./data)",
        "      --scripts-root PATH                      Script root (default: ./data/scripts)",
        "      --replication-control-udp-bind ADDR      Local UDP bind for gateway bootstrap dispatch (default: 0.0.0.0:0)",
        "      --replication-control-udp-addr ADDR      Replication control/bootstrap target (default: 127.0.0.1:9004)",
        "      --replication-udp-public-addr ADDR       Advertised replication UDP endpoint (default: 127.0.0.1:7001)",
        "      --replication-webtransport-public-addr ADDR",
        "                                               Advertised replication WebTransport endpoint (default: 127.0.0.1:7003)",
        "      --replication-webtransport-cert-sha256 HEX",
        "                                               Advertised WebTransport certificate digest",
        "  -h, --help                                   Show this help text",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{BootstrapMode, default_database_url, default_jwt_secret};

    #[test]
    fn defaults_match_local_runtime_contract() {
        assert_eq!(
            default_database_url(),
            "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal"
        );
        assert_eq!(default_jwt_secret(), "0123456789abcdef0123456789abcdef");
        assert_eq!(BootstrapMode::Udp.as_str(), "udp");
        assert_eq!(BootstrapMode::Direct.as_str(), "direct");
    }
}
