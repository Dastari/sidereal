use bevy::prelude::*;
use lightyear::prelude::MessageReceiver;
use lightyear::prelude::server::ClientOf;

use sidereal_net::ClientViewUpdateMessage;
use sidereal_persistence::PlayerRuntimeViewState;

use crate::{
    AuthenticatedClientBindings, PlayerRuntimeViewDirtySet, PlayerRuntimeViewRegistry,
    unix_epoch_now_i64,
};

pub fn receive_client_view_updates(
    mut receivers: Query<
        '_,
        '_,
        (Entity, &'_ mut MessageReceiver<ClientViewUpdateMessage>),
        With<ClientOf>,
    >,
    bindings: Res<'_, AuthenticatedClientBindings>,
    mut view_registry: ResMut<'_, PlayerRuntimeViewRegistry>,
    mut dirty_view_states: ResMut<'_, PlayerRuntimeViewDirtySet>,
) {
    for (client_entity, mut receiver) in &mut receivers {
        for message in receiver.receive() {
            let Some(bound_player) = bindings.by_client_entity.get(&client_entity) else {
                continue;
            };
            if bound_player != &message.player_entity_id {
                eprintln!(
                    "replication dropped client view update from {:?}: player mismatch {} != {}",
                    client_entity, message.player_entity_id, bound_player
                );
                continue;
            }
            let entry = view_registry
                .by_player_entity_id
                .entry(bound_player.clone())
                .or_insert_with(|| PlayerRuntimeViewState {
                    player_entity_id: bound_player.clone(),
                    ..Default::default()
                });
            entry.last_focused_entity_id = message.focused_entity_id.clone();
            entry.last_controlled_entity_id = message.controlled_entity_id.clone();
            entry.last_camera_position_m = Some(message.camera_position_m);
            entry.updated_at_epoch_s = unix_epoch_now_i64();
            dirty_view_states
                .player_entity_ids
                .insert(bound_player.clone());
        }
    }
}
