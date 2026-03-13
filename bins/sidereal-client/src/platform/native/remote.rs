//! Bevy Remote (BRP) inspection for native client.

use bevy::prelude::*;
use sidereal_core::remote_inspect::RemoteInspectConfig;

#[derive(Debug, Resource, Clone)]
#[allow(dead_code)]
pub(crate) struct BrpAuthToken(pub String);

pub(crate) fn configure_remote(app: &mut App, cfg: &RemoteInspectConfig) {
    if !cfg.enabled {
        return;
    }

    app.add_plugins(bevy_remote::RemotePlugin::default());
    app.add_plugins(
        bevy_remote::http::RemoteHttpPlugin::default()
            .with_address(cfg.bind_addr)
            .with_port(cfg.port),
    );
    app.insert_resource(BrpAuthToken(
        cfg.auth_token.clone().expect("validated token"),
    ));
}
