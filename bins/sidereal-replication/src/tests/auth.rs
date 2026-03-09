use bevy::prelude::*;
use lightyear::prelude::PeerId;

use crate::replication::auth::{
    AUTH_CONFIG_DENIED_REASON, AuthenticatedClientBindings, cleanup_client_auth_bindings,
    configured_gateway_jwt_secret,
};
use crate::replication::control::ClientControlRequestOrder;
use crate::replication::input::{
    ClientInputTickTracker, InputRateLimitState, LatestRealtimeInputsByPlayer,
    RealtimeInputActivityByPlayer,
};
use crate::replication::lifecycle::ClientLastActivity;
use crate::replication::visibility::ClientVisibilityRegistry;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{RemoteId, ReplicationState};

#[test]
fn cleanup_drops_visibility_for_disconnected_client() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<AuthenticatedClientBindings>();
    app.init_resource::<ClientInputTickTracker>();
    app.init_resource::<InputRateLimitState>();
    app.init_resource::<LatestRealtimeInputsByPlayer>();
    app.init_resource::<RealtimeInputActivityByPlayer>();
    app.init_resource::<ClientVisibilityRegistry>();
    app.init_resource::<ClientControlRequestOrder>();
    app.init_resource::<ClientLastActivity>();
    app.add_systems(Update, cleanup_client_auth_bindings);

    let client = app
        .world_mut()
        .spawn((ClientOf, RemoteId(PeerId::Netcode(42))))
        .id();
    let replicated = app.world_mut().spawn(ReplicationState::default()).id();

    {
        let mut bindings = app
            .world_mut()
            .resource_mut::<AuthenticatedClientBindings>();
        bindings
            .by_client_entity
            .insert(client, "11111111-1111-1111-1111-111111111111".to_string());
        bindings.by_remote_id.insert(
            PeerId::Netcode(42),
            "11111111-1111-1111-1111-111111111111".to_string(),
        );
    }
    app.world_mut()
        .resource_mut::<ClientVisibilityRegistry>()
        .register_client(client, "11111111-1111-1111-1111-111111111111".to_string());
    app.world_mut()
        .get_mut::<ReplicationState>(replicated)
        .expect("replication state exists")
        .gain_visibility(client);

    app.world_mut().entity_mut(client).despawn();
    app.update();

    // Cleanup removes bindings and registry entries; ReplicationState visibility bits
    // are intentionally left as-is (see cleanup_client_auth_bindings).
    assert!(
        !app.world()
            .resource::<AuthenticatedClientBindings>()
            .by_client_entity
            .contains_key(&client)
    );
    assert!(
        !app.world()
            .resource::<ClientVisibilityRegistry>()
            .player_entity_id_by_client
            .contains_key(&client)
    );
}

#[test]
fn configured_gateway_jwt_secret_rejects_missing_or_short_values() {
    unsafe {
        std::env::remove_var("GATEWAY_JWT_SECRET");
    }
    assert_eq!(
        configured_gateway_jwt_secret().unwrap_err(),
        AUTH_CONFIG_DENIED_REASON
    );

    unsafe {
        std::env::set_var("GATEWAY_JWT_SECRET", "too-short");
    }
    assert_eq!(
        configured_gateway_jwt_secret().unwrap_err(),
        AUTH_CONFIG_DENIED_REASON
    );

    unsafe {
        std::env::set_var("GATEWAY_JWT_SECRET", "0123456789abcdef0123456789abcdef");
    }
    assert_eq!(
        configured_gateway_jwt_secret().as_deref(),
        Ok("0123456789abcdef0123456789abcdef")
    );
}
