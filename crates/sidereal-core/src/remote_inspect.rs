use std::env;
use std::net::{IpAddr, Ipv4Addr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteInspectConfig {
    pub enabled: bool,
    pub bind_addr: IpAddr,
    pub port: u16,
    pub auth_token: Option<String>,
}

impl RemoteInspectConfig {
    pub fn from_env(service_key: &str, default_port: u16) -> Result<Self, String> {
        let service_upper = service_key.to_ascii_uppercase();

        let enabled = parse_bool_env(&[
            format!("SIDEREAL_{service_upper}_BRP_ENABLED"),
            "SIDEREAL_BRP_ENABLED".to_string(),
        ])
        .unwrap_or(false);

        let bind_addr = parse_ip_env(&[
            format!("SIDEREAL_{service_upper}_BRP_BIND_ADDR"),
            "SIDEREAL_BRP_BIND_ADDR".to_string(),
        ])
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));

        let port = parse_u16_env(&[
            format!("SIDEREAL_{service_upper}_BRP_PORT"),
            "SIDEREAL_BRP_PORT".to_string(),
        ])
        .unwrap_or(default_port);

        let auth_token = first_present_env(&[
            format!("SIDEREAL_{service_upper}_BRP_AUTH_TOKEN"),
            "SIDEREAL_BRP_AUTH_TOKEN".to_string(),
        ]);

        let config = Self {
            enabled,
            bind_addr,
            port,
            auth_token,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        if !self.bind_addr.is_loopback() {
            return Err(
                "BRP currently supports loopback binds only until a real authenticated HTTP gate exists"
                    .into(),
            );
        }
        // Token presence is still required so BRP enablement stays explicit and the env contract
        // remains stable until a real auth layer replaces the loopback-only stopgap.
        match self.auth_token.as_ref() {
            Some(token) if token.len() >= 16 => Ok(()),
            Some(_) => {
                Err("BRP auth token must be at least 16 characters when BRP is enabled".into())
            }
            None => Err("BRP auth token is required when BRP is enabled".into()),
        }
    }
}

fn first_present_env(keys: &[String]) -> Option<String> {
    for key in keys {
        if let Ok(value) = env::var(key)
            && !value.trim().is_empty()
        {
            return Some(value);
        }
    }
    None
}

fn parse_bool_env(keys: &[String]) -> Option<bool> {
    first_present_env(keys).and_then(|raw| {
        let lowered = raw.to_ascii_lowercase();
        match lowered.as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    })
}

fn parse_u16_env(keys: &[String]) -> Option<u16> {
    first_present_env(keys).and_then(|raw| raw.parse::<u16>().ok())
}

fn parse_ip_env(keys: &[String]) -> Option<IpAddr> {
    first_present_env(keys).and_then(|raw| raw.parse::<IpAddr>().ok())
}
