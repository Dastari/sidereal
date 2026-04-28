//! Tests for the replication binary (remote inspect, auth, control, input, visibility).

mod auth;
mod control;
mod input;
mod runtime_scripting;
mod visibility;

use bevy::prelude::*;
use sidereal_core::remote_inspect::RemoteInspectConfig;
use std::net::{IpAddr, Ipv4Addr};

use crate::replication::lifecycle::BrpAuthToken;
use crate::replication::lifecycle::configure_remote;

#[test]
fn remote_endpoint_registers_when_enabled() {
    let cfg = RemoteInspectConfig {
        enabled: true,
        bind_addr: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port: 15713,
        auth_token: Some("0123456789abcdef".to_string()),
    };
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    configure_remote(&mut app, &cfg);

    assert!(
        app.world()
            .contains_resource::<bevy_remote::http::HostPort>()
    );
    assert!(app.world().contains_resource::<BrpAuthToken>());
}
