use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::renet2::{
    client_connected, client_connecting, client_just_connected, client_just_disconnected,
};
use bevy_replicon_renet2::renet2::{RenetClient, RenetClientPlugin, RenetReceive, RenetSend};

pub struct RepliconRenetClientPlugin;

impl Plugin for RepliconRenetClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RenetClientPlugin)
            .configure_sets(PreUpdate, ClientSet::ReceivePackets.after(RenetReceive))
            .configure_sets(PostUpdate, ClientSet::SendPackets.before(RenetSend))
            .add_systems(
                PreUpdate,
                (
                    Self::set_connecting.run_if(client_connecting),
                    Self::set_disconnected.run_if(client_just_disconnected),
                    Self::set_connected.run_if(client_just_connected),
                    Self::receive_packets.run_if(client_connected),
                )
                    .chain()
                    .in_set(ClientSet::ReceivePackets),
            )
            .add_systems(
                PostUpdate,
                Self::send_packets
                    .in_set(ClientSet::SendPackets)
                    .run_if(client_connected),
            );
    }
}

impl RepliconRenetClientPlugin {
    fn set_disconnected(mut client: ResMut<RepliconClient>) {
        client.set_status(RepliconClientStatus::Disconnected);
    }

    fn set_connecting(mut client: ResMut<RepliconClient>) {
        if client.status() != RepliconClientStatus::Connecting {
            client.set_status(RepliconClientStatus::Connecting);
        }
    }

    fn set_connected(mut client: ResMut<RepliconClient>) {
        let client_id = None;

        client.set_status(RepliconClientStatus::Connected { client_id });
    }

    fn receive_packets(
        channels: Res<RepliconChannels>,
        mut renet_client: ResMut<RenetClient>,
        mut replicon_client: ResMut<RepliconClient>,
    ) {
        for channel_id in 0..channels.server_channels().len() as u8 {
            while let Some(message) = renet_client.receive_message(channel_id) {
                replicon_client.insert_received(channel_id, message);
            }
        }
    }

    fn send_packets(
        mut renet_client: ResMut<RenetClient>,
        mut replicon_client: ResMut<RepliconClient>,
    ) {
        for (channel_id, message) in replicon_client.drain_sent() {
            renet_client.send_message(channel_id, message)
        }
    }
}
