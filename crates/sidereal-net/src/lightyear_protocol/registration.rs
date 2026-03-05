use avian2d::prelude::{
    AngularDamping, AngularVelocity, LinearDamping, LinearVelocity, Position, Rotation,
};
use bevy::prelude::App;
use core::time::Duration;
use lightyear::prediction::prelude::PredictionRegistrationExt;
use lightyear::prelude::InterpolationRegistrationExt;
use lightyear::prelude::input::native::InputPlugin as NativeInputPlugin;
use lightyear::prelude::{
    AppChannelExt, AppComponentExt, AppMessageExt, ChannelMode, ChannelSettings, NetworkDirection,
    ReliableSettings,
};
use sidereal_game::component_meta::SiderealComponentRegistration;

use super::{
    AssetAckMessage, AssetChannel, AssetRequestMessage, AssetStreamChunkMessage,
    AssetStreamManifestMessage, ClientAuthMessage, ClientControlRequestMessage,
    ClientDisconnectNotifyMessage, ClientLocalViewModeMessage, ClientRealtimeInputMessage,
    ControlChannel, InputChannel, PlayerInput, ServerControlAckMessage, ServerControlRejectMessage,
    ServerSessionDeniedMessage, ServerSessionReadyMessage, ServerWeaponFiredMessage,
};

fn lerp_position(start: Position, other: Position, t: f32) -> Position {
    lightyear::avian2d::types::position::lerp(&start, &other, t)
}

fn lerp_rotation(start: Rotation, other: Rotation, t: f32) -> Rotation {
    lightyear::avian2d::types::rotation::lerp(&start, &other, t)
}

fn lerp_linear_velocity(start: LinearVelocity, other: LinearVelocity, t: f32) -> LinearVelocity {
    lightyear::avian2d::types::linear_velocity::lerp(&start, &other, t)
}

fn lerp_angular_velocity(
    start: AngularVelocity,
    other: AngularVelocity,
    t: f32,
) -> AngularVelocity {
    lightyear::avian2d::types::angular_velocity::lerp(&start, &other, t)
}

fn position_should_rollback(this: &Position, that: &Position) -> bool {
    (this.0 - that.0).length() >= 0.03
}

fn rotation_should_rollback(this: &Rotation, that: &Rotation) -> bool {
    this.angle_between(*that).abs() >= 0.003
}

fn linear_velocity_should_rollback(this: &LinearVelocity, that: &LinearVelocity) -> bool {
    (this.0 - that.0).length() >= 0.05
}

fn angular_velocity_should_rollback(this: &AngularVelocity, that: &AngularVelocity) -> bool {
    (this.0 - that.0).abs() >= 0.01
}

pub fn register_lightyear_protocol(app: &mut App) {
    app.register_message::<ClientAuthMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<ClientControlRequestMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<ClientRealtimeInputMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<ClientLocalViewModeMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<ServerWeaponFiredMessage>()
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

    app.add_channel::<InputChannel>(ChannelSettings {
        mode: ChannelMode::SequencedUnreliable,
        send_frequency: Duration::default(),
        priority: 10.0,
    })
    .add_direction(NetworkDirection::Bidirectional);

    app.add_channel::<AssetChannel>(ChannelSettings {
        mode: ChannelMode::UnorderedReliable(ReliableSettings::default()),
        send_frequency: Duration::default(),
        priority: 4.0,
    })
    .add_direction(NetworkDirection::Bidirectional);
}

fn register_lightyear_replication_components(app: &mut App) {
    // Avian physics components — not managed by the sidereal_component macro,
    // registered manually with prediction for client-side rollback/resimulation.
    app.register_component::<Position>()
        .add_prediction()
        .add_should_rollback(position_should_rollback)
        .add_correction_fn(lerp_position)
        .add_interpolation_with(lerp_position);
    app.register_component::<Rotation>()
        .add_prediction()
        .add_should_rollback(rotation_should_rollback)
        .add_correction_fn(lerp_rotation)
        .add_interpolation_with(lerp_rotation);
    app.register_component::<LinearVelocity>()
        .add_prediction()
        .add_should_rollback(linear_velocity_should_rollback)
        .add_interpolation_with(lerp_linear_velocity);
    app.register_component::<AngularVelocity>()
        .add_prediction()
        .add_should_rollback(angular_velocity_should_rollback)
        .add_interpolation_with(lerp_angular_velocity);
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
