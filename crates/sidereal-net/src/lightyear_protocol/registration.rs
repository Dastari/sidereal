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
use sidereal_game::{
    BaseMassKg, CargoMassKg, CharacterMovementController, ControlledEntityGuid, Cost, Engine,
    EntityGuid, FactionId, FactionVisibility, FlightComputer, FlightTuning, FuelTank, Hardpoint,
    HealthPool, Inventory, MassKg, MaxVelocityMps, ModuleMassKg, MountedOn, OwnerId,
    PublicVisibility, ScannerComponent, ScannerRangeBuff, ScannerRangeM, SiderealComponentMetadata,
    SizeM, SpriteShaderAssetId, TotalMassKg, VisualAssetId,
};

use super::{
    AssetAckMessage, AssetRequestMessage, AssetStreamChunkMessage, AssetStreamManifestMessage,
    ClientAuthMessage, ClientControlRequestMessage, ClientDisconnectNotifyMessage,
    ClientRealtimeInputMessage, ControlChannel, PlayerInput, ServerControlAckMessage,
    ServerControlRejectMessage, ServerSessionReadyMessage,
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
    macro_rules! register_game_component {
        ($app:expr, $ty:ty) => {
            if <$ty as SiderealComponentMetadata>::META.replicate {
                $app.register_component::<$ty>();
            }
        };
    }
    macro_rules! register_game_component_with_prediction {
        ($app:expr, $ty:ty) => {
            if <$ty as SiderealComponentMetadata>::META.replicate {
                $app.register_component::<$ty>().add_prediction();
            }
        };
    }

    // Avian physics components — replicated so the client can run prediction/rollback.
    app.register_component::<Position>().add_prediction();
    app.register_component::<Rotation>().add_prediction();
    app.register_component::<LinearVelocity>().add_prediction();
    app.register_component::<AngularVelocity>().add_prediction();
    app.register_component::<LinearDamping>().add_prediction();
    app.register_component::<AngularDamping>().add_prediction();

    // Gameplay components needed by client-side rollback/resimulation.
    register_game_component_with_prediction!(app, FlightComputer);
    register_game_component_with_prediction!(app, FlightTuning);
    register_game_component_with_prediction!(app, MaxVelocityMps);
    register_game_component_with_prediction!(app, SizeM);
    register_game_component_with_prediction!(app, TotalMassKg);

    // Entity identity/ownership and world composition.
    register_game_component!(app, EntityGuid);
    register_game_component!(app, ControlledEntityGuid);
    register_game_component!(app, OwnerId);
    register_game_component!(app, MountedOn);
    register_game_component!(app, Hardpoint);

    // Replicated gameplay state and visibility metadata.
    register_game_component!(app, HealthPool);
    register_game_component!(app, MassKg);
    register_game_component!(app, BaseMassKg);
    register_game_component!(app, CargoMassKg);
    register_game_component!(app, ModuleMassKg);
    register_game_component!(app, Inventory);
    register_game_component!(app, Cost);
    register_game_component!(app, Engine);
    register_game_component!(app, FuelTank);
    register_game_component!(app, ScannerRangeM);
    register_game_component!(app, ScannerComponent);
    register_game_component!(app, ScannerRangeBuff);
    register_game_component!(app, FactionId);
    register_game_component!(app, FactionVisibility);
    register_game_component!(app, PublicVisibility);
    register_game_component!(app, CharacterMovementController);
    register_game_component!(app, VisualAssetId);
    register_game_component!(app, SpriteShaderAssetId);
}
