use bevy::prelude::*;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{ChannelRegistry, Transport};

use sidereal_net::ControlChannel;

pub fn ensure_server_transport_channels(
    mut transports: Query<'_, '_, &'_ mut Transport, With<ClientOf>>,
    registry: Res<'_, ChannelRegistry>,
) {
    for mut transport in &mut transports {
        if !transport.has_receiver::<ControlChannel>() {
            transport.add_receiver_from_registry::<ControlChannel>(&registry);
        }
        if !transport.has_sender::<ControlChannel>() {
            transport.add_sender_from_registry::<ControlChannel>(&registry);
        }
    }
}
