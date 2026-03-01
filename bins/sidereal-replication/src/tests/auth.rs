use bevy::prelude::*;
use lightyear::prelude::PeerId;

use crate::replication::assets::AssetStreamServerState;
use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::auth::cleanup_client_auth_bindings;
use crate::replication::control::ClientControlRequestOrder;
use crate::replication::input::{
    ClientInputTickTracker, InputRateLimitState, LatestRealtimeInputsByPlayer,
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
    app.init_resource::<AssetStreamServerState>();
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
            .insert(client, "player:test".to_string());
        bindings
            .by_remote_id
            .insert(PeerId::Netcode(42), "player:test".to_string());
    }
    app.world_mut()
        .resource_mut::<ClientVisibilityRegistry>()
        .register_client(client, "player:test".to_string());
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
