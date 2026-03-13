use bevy::prelude::*;
use lightyear::prelude::MessageReceiver;
use sidereal_net::{ServerEntityDestructionMessage, ServerWeaponFiredMessage};

#[derive(Debug, Clone, Message)]
pub(crate) struct RemoteWeaponFiredRuntimeMessage {
    pub message: ServerWeaponFiredMessage,
}

#[derive(Debug, Clone, Message)]
pub(crate) struct RemoteEntityDestructionRuntimeMessage {
    pub message: ServerEntityDestructionMessage,
}

pub(crate) fn fanout_remote_weapon_fired_messages_system(
    mut weapon_fired_writer: MessageWriter<'_, RemoteWeaponFiredRuntimeMessage>,
    mut receivers: Query<
        '_,
        '_,
        &'_ mut MessageReceiver<ServerWeaponFiredMessage>,
        (
            With<lightyear::prelude::client::Client>,
            With<lightyear::prelude::client::Connected>,
        ),
    >,
) {
    for mut receiver in &mut receivers {
        for message in receiver.receive() {
            weapon_fired_writer.write(RemoteWeaponFiredRuntimeMessage {
                message: message.clone(),
            });
        }
    }
}

pub(crate) fn fanout_remote_destruction_messages_system(
    mut destruction_writer: MessageWriter<'_, RemoteEntityDestructionRuntimeMessage>,
    mut receivers: Query<
        '_,
        '_,
        &'_ mut MessageReceiver<ServerEntityDestructionMessage>,
        (
            With<lightyear::prelude::client::Client>,
            With<lightyear::prelude::client::Connected>,
        ),
    >,
) {
    for mut receiver in &mut receivers {
        for message in receiver.receive() {
            destruction_writer.write(RemoteEntityDestructionRuntimeMessage {
                message: message.clone(),
            });
        }
    }
}
