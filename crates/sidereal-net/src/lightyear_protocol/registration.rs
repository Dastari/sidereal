use avian3d::prelude::{
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
use sidereal_game::{
    BaseMassKg, CargoMassKg, Engine, EntityGuid, FactionId, FactionVisibility, FlightComputer,
    FlightTuning, FuelTank, Hardpoint, HealthPool, Inventory, MassKg, MaxVelocityMps, ModuleMassKg,
    MountedOn, OwnerId, PublicVisibility, ScannerComponent, ScannerRangeBuff, ScannerRangeM, SizeM,
    TotalMassKg,
};

use super::{
    AssetAckMessage, AssetRequestMessage, AssetStreamChunkMessage, AssetStreamManifestMessage,
    ClientAuthMessage, ClientViewUpdateMessage, ControlChannel, PlayerInput,
};

pub fn register_lightyear_protocol(app: &mut App) {
    app.register_message::<ClientAuthMessage>()
        .add_direction(NetworkDirection::Bidirectional);
    app.register_message::<ClientViewUpdateMessage>()
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
    // Avian physics components — replicated so the client can run prediction/rollback.
    app.register_component::<Position>().add_prediction();
    app.register_component::<Rotation>().add_prediction();
    app.register_component::<LinearVelocity>().add_prediction();
    app.register_component::<AngularVelocity>().add_prediction();
    app.register_component::<LinearDamping>().add_prediction();
    app.register_component::<AngularDamping>().add_prediction();

    // Gameplay components needed by client-side rollback/resimulation.
    app.register_component::<FlightComputer>().add_prediction();
    app.register_component::<FlightTuning>().add_prediction();
    app.register_component::<MaxVelocityMps>().add_prediction();
    app.register_component::<SizeM>().add_prediction();
    app.register_component::<TotalMassKg>().add_prediction();

    // Entity identity/ownership and world composition.
    app.register_component::<EntityGuid>();
    app.register_component::<OwnerId>();
    app.register_component::<MountedOn>();
    app.register_component::<Hardpoint>();

    // Replicated gameplay state and visibility metadata.
    app.register_component::<HealthPool>();
    app.register_component::<MassKg>();
    app.register_component::<BaseMassKg>();
    app.register_component::<CargoMassKg>();
    app.register_component::<ModuleMassKg>();
    app.register_component::<Inventory>();
    app.register_component::<Engine>();
    app.register_component::<FuelTank>();
    app.register_component::<ScannerRangeM>();
    app.register_component::<ScannerComponent>();
    app.register_component::<ScannerRangeBuff>();
    app.register_component::<FactionId>();
    app.register_component::<FactionVisibility>();
    app.register_component::<PublicVisibility>();
}
