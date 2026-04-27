mod config;

use anyhow::Context;
use config::{BootstrapMode, CliAction};
use sidereal_core::logging::{RunLogFile, prepare_timestamped_log_file};
use sidereal_gateway::api::app_with_service;
use sidereal_gateway::auth::{
    AuthConfig, AuthService, BootstrapDispatcher, DirectBootstrapDispatcher, EmailDelivery,
    LogEmailDelivery, NoopEmailDelivery, PostgresAuthStore, SmtpEmailDelivery,
    UdpBootstrapDispatcher,
};
use std::sync::Arc;
use tracing::{Level, info};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::MakeWriterExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli_config = match config::apply_process_cli() {
        Ok(CliAction::Run(config)) => *config,
        Ok(CliAction::Help(text)) => {
            println!("{text}");
            return Ok(());
        }
        Err(err) => anyhow::bail!(err),
    };
    cli_config.apply_env();
    let RunLogFile {
        file: log_file,
        path: log_path,
    } = prepare_timestamped_log_file("sidereal-gateway")
        .context("failed to create gateway log file")?;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("info,postgres::config=warn"))
        .with_max_level(Level::INFO)
        .with_target(true)
        .with_writer(std::io::stderr.and(log_file))
        .try_init();
    info!("sidereal-gateway tracing log file: {}", log_path.display());
    info!(
        "sidereal-gateway replication endpoints: control_udp={} native_udp_public={} webtransport_public={} webtransport_cert_configured={}",
        cli_config.replication_control_udp_addr,
        cli_config.replication_udp_public_addr,
        cli_config.replication_webtransport_public_addr,
        cli_config
            .replication_webtransport_cert_sha256
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
    );
    let auth_config = AuthConfig::from_env().context("invalid auth configuration")?;
    let database_url = std::env::var("GATEWAY_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());
    let store = PostgresAuthStore::connect(&database_url)
        .await
        .context("failed to connect gateway postgres")?;
    store
        .ensure_schema()
        .await
        .context("failed to ensure schema")?;
    let bootstrap_dispatcher: Arc<dyn BootstrapDispatcher> = match cli_config.bootstrap_mode {
        BootstrapMode::Udp => Arc::new(
            UdpBootstrapDispatcher::from_env()
                .await
                .context("invalid replication control UDP config")?,
        ),
        BootstrapMode::Direct => Arc::new(DirectBootstrapDispatcher::from_env()),
    };
    let email_delivery = email_delivery_from_env().context("invalid email delivery config")?;
    let service = Arc::new(AuthService::new_with_dependencies(
        auth_config,
        Arc::new(store),
        bootstrap_dispatcher,
        Arc::new(sidereal_gateway::auth::GraphStarterWorldPersister),
        email_delivery,
    ));

    let listener = tokio::net::TcpListener::bind(cli_config.bind_addr)
        .await
        .with_context(|| format!("failed to bind gateway on {}", cli_config.bind_addr))?;
    info!("sidereal-gateway listening on {}", cli_config.bind_addr);
    axum::serve(listener, app_with_service(service))
        .await
        .context("gateway server failed")?;
    Ok(())
}

fn email_delivery_from_env() -> anyhow::Result<Arc<dyn EmailDelivery>> {
    let mode = std::env::var("GATEWAY_EMAIL_DELIVERY")
        .unwrap_or_else(|_| "noop".to_string())
        .trim()
        .to_ascii_lowercase();
    match mode.as_str() {
        "smtp" => SmtpEmailDelivery::from_env()?
            .map(|delivery| Arc::new(delivery) as Arc<dyn EmailDelivery>)
            .ok_or_else(|| anyhow::anyhow!("SMTP email delivery was not created")),
        "log" => Ok(Arc::new(LogEmailDelivery)),
        "noop" | "" => Ok(Arc::new(NoopEmailDelivery)),
        other => {
            anyhow::bail!("GATEWAY_EMAIL_DELIVERY must be one of noop, log, or smtp; got {other}")
        }
    }
}
