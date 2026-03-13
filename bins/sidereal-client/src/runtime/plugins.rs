mod bootstrap_plugins;
mod presentation_plugins;
mod replication_plugins;
mod ui_plugins;

pub(super) use bootstrap_plugins::{ClientBootstrapPlugin, ClientTransportPlugin};
pub(super) use presentation_plugins::{ClientLightingPlugin, ClientVisualsPlugin};
pub(super) use replication_plugins::{ClientPredictionPlugin, ClientReplicationPlugin};
pub(super) use ui_plugins::{ClientDiagnosticsPlugin, ClientUiPlugin};
