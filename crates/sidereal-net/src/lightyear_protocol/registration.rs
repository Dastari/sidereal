use avian2d::prelude::{
    AngularDamping, AngularVelocity, LinearDamping, LinearVelocity, Position, Rotation,
};
use bevy::prelude::App;
use core::time::Duration;
use lightyear::prediction::prelude::PredictionRegistrationExt;
use lightyear::prelude::input::native::InputPlugin as NativeInputPlugin;
use lightyear::prelude::{
    AppChannelExt, AppComponentExt, AppMessageExt, ChannelMode, ChannelSettings, NetworkDirection,
    ReliableSettings,
};
use sidereal_game::component_meta::SiderealComponentRegistration;

use super::{
    AssetAckMessage, AssetRequestMessage, AssetStreamChunkMessage, AssetStreamManifestMessage,
    ClientAuthMessage, ClientControlRequestMessage, ClientDisconnectNotifyMessage,
    ClientRealtimeInputMessage, ControlChannel, PlayerInput, ServerControlAckMessage,
    ServerControlRejectMessage, ServerSessionDeniedMessage, ServerSessionReadyMessage,
};

pub fn register_lightyear_protocol(app: &mut App) {
    app.register_message::<ClientAuthMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<ClientControlRequestMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<ClientRealtimeInputMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<ServerSessionReadyMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<ServerSessionDeniedMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<ClientDisconnectNotifyMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<ServerControlAckMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<ServerControlRejectMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<AssetRequestMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<AssetAckMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<AssetStreamManifestMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<AssetStreamChunkMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.add_plugins(NativeInputPlugin::<PlayerInput>::default());
    register_lightyear_replication_components(app);

    app.add_channel::<ControlChannel>(ChannelSettings {
        mode: ChannelMode::UnorderedReliable(ReliableSettings::default()),
        send_frequency: Duration::default(),
        priority: 8.0,
    })
    .add_direction(NetworkDirection::Bidirectional);
}

fn register_lightyear_replication_components(app: &mut App) {
    // Avian physics components — not managed by the sidereal_component macro,
    // registered manually with prediction for client-side rollback/resimulation.
    app.register_component::<Position>().add_prediction();
    app.register_component::<Rotation>().add_prediction();
    app.register_component::<LinearVelocity>().add_prediction();
    app.register_component::<AngularVelocity>().add_prediction();
    app.register_component::<LinearDamping>().add_prediction();
    app.register_component::<AngularDamping>().add_prediction();

    // All sidereal_component-annotated types: the proc macro generates a
    // register_lightyear function per component that calls
    // register_component (+ add_prediction when predict = true).
    // Sort by component_kind to ensure deterministic registration order
    // across server and client binaries (inventory iteration order depends
    // on link order, which differs between binaries).
    let mut registrations: Vec<&SiderealComponentRegistration> =
        inventory::iter::<SiderealComponentRegistration>
            .into_iter()
            .filter(|r| r.meta.replicate)
            .collect();
    registrations.sort_by_key(|r| r.meta.kind);
    for registration in registrations {
        (registration.register_lightyear)(app);
    }
}
