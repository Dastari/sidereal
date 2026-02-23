# Bevy + Lightyear + Avian3D (Rust 2024) — Client/Server MMO-style Top-Down Networking
This is a minimal Rust 2024 workspace example for a **top-down action** game movement loop with:

- **Server-authoritative** simulation
- **Client-side prediction** for the locally-owned player
- **Interpolation** for non-owned (remote) players
- Controls are sent as **Lightyear Inputs** so client and server consume the **same input stream**

## Versions (as written)
- bevy = "0.18"
- lightyear = "0.26.4"
- avian3d = "0.5"

> Notes
> - The focus is the networked movement loop: inputs → predicted movement (local) / authoritative (server) → replication → interpolation/correction.
> - For a real MMO you will add interest management, sharding/zones, and server-side validation (speed/cooldowns/etc).

---

## Workspace layout


mmo_net_example/
Cargo.toml
shared/
Cargo.toml
src/lib.rs
server/
Cargo.toml
src/main.rs
client/
Cargo.toml
src/main.rs


---

## Root `Cargo.toml`

```toml
[workspace]
members = ["shared", "server", "client"]
resolver = "2"
shared/Cargo.toml
[package]
name = "shared"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy = "0.18"
serde = { version = "1", features = ["derive"] }

lightyear = { version = "0.26.4", features = ["udp", "netcode", "input_native", "avian3d"] }
avian3d = { version = "0.5", features = ["3d"] }
shared/src/lib.rs
use avian3d::prelude::*;
use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

pub const FIXED_HZ: f64 = 60.0;

#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Player;

#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlayerOwner(pub PeerId);

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct MoveInput {
    pub x: f32,
    pub y: f32,
    pub dash: bool,
}

impl Default for MoveInput {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, dash: false }
    }
}

#[derive(Channel)]
pub struct ReliableOrdered;

#[derive(Channel)]
pub struct Unreliable;

pub fn build_protocol() -> Protocol {
    let mut protocol = Protocol::default();

    protocol.add_channel::<ReliableOrdered>(ChannelSettings {
        mode: ChannelMode::ReliableOrdered,
        ..default()
    });
    protocol.add_channel::<Unreliable>(ChannelSettings {
        mode: ChannelMode::Unreliable,
        ..default()
    });

    protocol.add_input::<MoveInput>(InputSettings {
        send_during_rollback: true,
        ..default()
    });

    protocol.add_component::<Player>(ComponentSettings::default());
    protocol.add_component::<PlayerOwner>(ComponentSettings::default());

    protocol.add_component::<Transform>(ComponentSettings {
        channel: ChannelKind::of::<Unreliable>(),
        ..default()
    });

    protocol.add_component::<LinearVelocity>(ComponentSettings {
        channel: ChannelKind::of::<Unreliable>(),
        ..default()
    });

    protocol
}

pub fn input_to_velocity(i: MoveInput) -> Vec3 {
    let speed = 8.0;
    let dash_mult = 2.2;

    let mut dir = Vec3::new(i.x, 0.0, i.y);
    if dir.length_squared() > 1.0 {
        dir = dir.normalize();
    }

    let mult = if i.dash { dash_mult } else { 1.0 };
    dir * speed * mult
}
server/Cargo.toml
[package]
name = "server"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy = { version = "0.18", default-features = false, features = ["bevy_app", "bevy_ecs", "bevy_time", "bevy_transform"] }
serde = { version = "1", features = ["derive"] }

lightyear = { version = "0.26.4", features = ["udp", "netcode", "input_native", "avian3d"] }
avian3d = { version = "0.5", features = ["3d"] }

shared = { path = "../shared" }
server/src/main.rs
use avian3d::prelude::*;
use bevy::prelude::*;
use lightyear::prelude::*;
use lightyear::prelude::server::*;
use shared::*;

const SERVER_ADDR: &str = "0.0.0.0:5000";

fn main() {
    let mut app = App::new();

    app.add_plugins(MinimalPlugins);
    app.add_plugins(PhysicsPlugins::default());

    let shared_config = SharedConfig {
        tick: TickConfig::new(std::time::Duration::from_secs_f64(1.0 / FIXED_HZ)),
        ..default()
    };

    let server_config = ServerConfig {
        shared: shared_config,
        net: NetConfig::Netcode {
            config: NetcodeConfig::default(),
            io: IoConfig::UdpSocket(UdpSocketConfig {
                local_addr: SERVER_ADDR.parse().unwrap(),
                ..default()
            }),
        },
        ..default()
    };

    app.add_plugins(ServerPlugins::new(server_config));
    app.insert_resource(build_protocol());

    app.add_systems(Startup, start_listening);
    app.add_systems(Update, on_connect_spawn_player);
    app.add_systems(FixedUpdate, server_apply_inputs_to_players);

    app.run();
}

fn start_listening(mut commands: Commands) {
    commands.start_server();
}

fn on_connect_spawn_player(mut commands: Commands, mut ev: EventReader<ServerConnectEvent>) {
    for e in ev.read() {
        let owner = e.client_id;

        commands.spawn((
            Name::new(format!("Player({owner:?})")),
            Player,
            PlayerOwner(owner),
            RigidBody::Dynamic,
            Collider::capsule_y(0.5, 0.35),
            LinearVelocity::default(),
            Transform::from_xyz(0.0, 0.0, 0.0),
            Replicate {
                prediction: PredictionConfig::Predicted,
                replication_target: ReplicationTarget::All,
                ..default()
            },
        ));
    }
}

fn server_apply_inputs_to_players(
    mut q: Query<(&mut LinearVelocity, &PlayerOwner), With<Player>>,
    inputs: Res<Inputs<MoveInput>>,
) {
    for (mut v, owner) in q.iter_mut() {
        let i = inputs
            .input(owner.0)
            .copied()
            .unwrap_or_default();

        let desired = input_to_velocity(i);
        v.0.x = desired.x;
        v.0.z = desired.z;
        v.0.y = 0.0;
    }
}
client/Cargo.toml
[package]
name = "client"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy = "0.18"
serde = { version = "1", features = ["derive"] }

lightyear = { version = "0.26.4", features = ["udp", "netcode", "input_native", "avian3d"] }
avian3d = { version = "0.5", features = ["3d"] }

shared = { path = "../shared" }
client/src/main.rs
use avian3d::prelude::*;
use bevy::prelude::*;
use lightyear::prelude::*;
use lightyear::prelude::client::*;
use shared::*;

const SERVER_ADDR: &str = "127.0.0.1:5000";

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins);
    app.add_plugins(PhysicsPlugins::default());

    let shared_config = SharedConfig {
        tick: TickConfig::new(std::time::Duration::from_secs_f64(1.0 / FIXED_HZ)),
        ..default()
    };

    let client_config = ClientConfig {
        shared: shared_config,
        net: NetConfig::Netcode {
            config: NetcodeConfig::default(),
            io: IoConfig::UdpSocket(UdpSocketConfig {
                server_addr: SERVER_ADDR.parse().unwrap(),
                ..default()
            }),
        },
        prediction: PredictionConfig {
            rollback: RollbackConfig {
                max_rollback_ticks: 120,
                ..default()
            },
            ..default()
        },
        interpolation: InterpolationConfig {
            interpolation_delay_ticks: 2,
            ..default()
        },
        ..default()
    };

    app.add_plugins(ClientPlugins::new(client_config));
    app.insert_resource(build_protocol());

    app.add_systems(Startup, setup_scene);
    app.add_systems(Startup, connect);
    app.add_systems(Update, gather_input);
    app.add_systems(FixedUpdate, client_predicted_apply_inputs);
    app.add_systems(Update, (tag_remote_players_interpolated, follow_local_player_camera));

    app.run();
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 18.0, 18.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(5.0, 12.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn connect(mut commands: Commands) {
    commands.connect_client();
}

fn gather_input(kb: Res<ButtonInput<KeyCode>>, mut input: ResMut<InputBuffer<MoveInput>>) {
    let mut x = 0.0;
    let mut y = 0.0;

    if kb.pressed(KeyCode::KeyA) { x -= 1.0; }
    if kb.pressed(KeyCode::KeyD) { x += 1.0; }
    if kb.pressed(KeyCode::KeyW) { y += 1.0; }
    if kb.pressed(KeyCode::KeyS) { y -= 1.0; }

    let dash = kb.pressed(KeyCode::ShiftLeft) || kb.pressed(KeyCode::ShiftRight);

    input.set_current(MoveInput { x, y, dash });
}

fn client_predicted_apply_inputs(
    mut q: Query<&mut LinearVelocity, (With<Player>, With<Predicted>)>,
    input: Res<InputBuffer<MoveInput>>,
) {
    let i = *input.current();
    let desired = input_to_velocity(i);

    for mut v in q.iter_mut() {
        v.0.x = desired.x;
        v.0.z = desired.z;
        v.0.y = 0.0;
    }
}

fn tag_remote_players_interpolated(
    mut commands: Commands,
    local_id: Res<LocalId>,
    q: Query<(Entity, &PlayerOwner), (With<Player>, Without<Interpolated>, Without<Predicted>)>,
) {
    for (e, owner) in q.iter() {
        if owner.0 != local_id.0 {
            commands.entity(e).insert(Interpolated);
        }
    }
}

fn follow_local_player_camera(
    mut cam: Query<&mut Transform, (With<Camera3d>, Without<Player>)>,
    players: Query<(&Transform, &PlayerOwner), With<Player>>,
    local_id: Res<LocalId>,
) {
    let Ok(mut cam_t) = cam.get_single_mut() else { return; };

    for (pt, owner) in players.iter() {
        if owner.0 == local_id.0 {
            cam_t.translation = pt.translation + Vec3::new(0.0, 18.0, 18.0);
            cam_t.look_at(pt.translation, Vec3::Y);
            break;
        }
    }
}
What “multiple players with interpolation” means here
Locally-owned player

The locally-owned player entity is predicted on the client.

The client runs client_predicted_apply_inputs in FixedUpdate using InputBuffer<MoveInput>.

Lightyear reconciles the predicted entity against authoritative server state (rollback/replay when needed).

Remote players

Remote players are not simulated locally.

They are marked with Interpolated so Lightyear will smoothly interpolate replicated Transform/LinearVelocity updates.

Running

In one terminal:

cargo run -p server

In two (or more) other terminals:

cargo run -p client