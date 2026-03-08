use anyhow::Context;
use sidereal_core::logging::{RunLogFile, prepare_timestamped_log_file};
use sidereal_gateway::api::app_with_service;
use sidereal_gateway::auth::{
    AuthConfig, AuthService, BootstrapDispatcher, DirectBootstrapDispatcher, PostgresAuthStore,
    UdpBootstrapDispatcher,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{Level, info};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::MakeWriterExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
    let config = AuthConfig::from_env().context("invalid auth configuration")?;
    let database_url = std::env::var("GATEWAY_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());
    let store = PostgresAuthStore::connect(&database_url)
        .await
        .context("failed to connect gateway postgres")?;
    store
        .ensure_schema()
        .await
        .context("failed to ensure schema")?;
    let bootstrap_mode =
        std::env::var("GATEWAY_BOOTSTRAP_MODE").unwrap_or_else(|_| "udp".to_string());
    let bootstrap_dispatcher: Arc<dyn BootstrapDispatcher> =
        if bootstrap_mode.eq_ignore_ascii_case("udp") {
            Arc::new(
                UdpBootstrapDispatcher::from_env()
                    .await
                    .context("invalid replication control UDP config")?,
            )
        } else {
            Arc::new(DirectBootstrapDispatcher::from_env())
        };
    let service = Arc::new(AuthService::new(
        config,
        Arc::new(store),
        bootstrap_dispatcher,
    ));

    let bind_addr = std::env::var("GATEWAY_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let socket_addr: SocketAddr = bind_addr
        .parse()
        .with_context(|| format!("invalid GATEWAY_BIND value: {bind_addr}"))?;

    let listener = tokio::net::TcpListener::bind(socket_addr)
        .await
        .with_context(|| format!("failed to bind gateway on {socket_addr}"))?;
    info!("sidereal-gateway listening on {}", socket_addr);
    axum::serve(listener, app_with_service(service))
        .await
        .context("gateway server failed")?;
    Ok(())
}
