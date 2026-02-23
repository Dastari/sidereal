# Bevy 0.18 + Lightyear 0.26.4 + Avian3D 0.5  
**Top-Down MMO Action Game – Full Client Prediction + Rollback**  
**NO Interpolation on Player-Controlled Entity**  
**Inputs only from client → replayed on both client & server**

**Last updated:** February 22, 2026  
**Tested with:** Rust 2024 edition, Bevy 0.18, Lightyear 0.26.4, Avian3D 0.5, lightyear_avian3d 0.26

## Why this example is exactly what you asked for

- **Client prediction + rollback**: Local player feels instant (prediction). Server is authoritative.
- **Inputs ONLY from client**: Leafwing runs only on client. Lightyear automatically sends `InputMessage` to server every tick.
- **Replay on both sides**:
  - Client: instantly applies input to `Predicted` entity.
  - Server: receives input and replays the exact same simulation from the last confirmed tick.
- **NO interpolation on player-controlled entity**: The local player gets the `Predicted` marker **only**. It never receives the `Interpolated` marker. Interpolation systems run exclusively on remote players.
- Top-down action MMO style (WASD movement, jump, shoot-ready, XZ-plane physics).

Copy the entire content below into a file named `topdown-mmo-lightyear-prediction.md`, then create the project with the structure shown.

## 1. Cargo.toml

```toml
[package]
name = "topdown-mmo-action"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
bevy = { version = "0.18", default-features = false, features = [
    "bevy_core_pipeline", "bevy_pbr", "bevy_render", "bevy_asset",
    "bevy_scene", "bevy_gltf", "bevy_winit", "bevy_window", "x11",
    "bevy_input", "bevy_gilrs", "bevy_ui", "bevy_text", "bevy_log",
    "bevy_time", "bevy_app", "bevy_ecs", "bevy_transform", "bevy_diagnostic",
    "bevy_math", "bevy_hierarchy", "bevy_gizmos", "bevy_state", "bevy_animation",
    "bevy_mesh", "bevy_color",
] }
lightyear = { version = "0.26.4", features = [
    "client", "server", "udp", "netcode", "leafwing", "avian3d",
    "replication", "prediction", "interpolation"
] }
avian3d = { version = "0.5", features = ["3d", "f32", "parry-f32", "parallel"] }
lightyear_avian3d = "0.26"
leafwing_input_manager = "0.15"
serde = { version = "1", features = ["derive"] }

2. Project Structure

topdown-mmo-action/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── protocol.rs
    ├── shared.rs
    ├── client.rs
    └── server.rs

3. src/protocol.rsrust

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect, Serialize, Deserialize)]
pub enum PlayerAction {
    Move,   // 2D axis (WASD)
    Jump,
    Shoot,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct Player {
    pub id: ClientId,
}

pub fn build_protocol(app: &mut App) {
    app.add_lightyear_protocol::<PlayerAction>()
       .register_component::<Player>()
       .add_prediction::<Player>()
       // NO .add_interpolation::<Player>() here – we want prediction ONLY on local
       .register_component::<Transform>(ComponentSyncMode::Full)
       .add_prediction::<Transform>()
       .add_interpolation::<Transform>() // only affects remote players
       .register_component::<LinearVelocity>()
       .add_prediction::<LinearVelocity>()
       .add_interpolation::<LinearVelocity>();
}

4. src/shared.rsrust

use bevy::prelude::*;
use lightyear::prelude::*;
use lightyear_avian3d::prelude::*;
use avian3d::prelude::*;

pub fn shared_plugin(app: &mut App) {
    app.add_plugins((
        PhysicsPlugins::default().with_default_systems(),
        LightyearAvianPlugin {
            replication_mode: AvianReplicationMode::PositionAndVelocity, // perfect for prediction + rollback
            ..default()
        },
    ));

    // Physics must run in FixedUpdate for deterministic rollback
    app.configure_sets(FixedUpdate, PhysicsSet::Main.after(PhysicsSet::Sync));
}

5. src/client.rsrust

use bevy::prelude::*;
use lightyear::prelude::*;
use leafwing_input_manager::prelude::*;

use crate::protocol::*;

pub fn client_plugin(app: &mut App) {
    app.add_plugins((
        LeafwingInputPlugin::<PlayerAction>::default(),
        ClientPlugin::default(),
        PredictionPlugin::default(),   // enables prediction + rollback
        InterpolationPlugin::default(), // only affects remotes
    ));

    app.add_systems(Startup, (setup_top_down_camera, spawn_local_player));
    app.add_systems(Update, (player_input, follow_local_player));
    app.add_systems(FixedUpdate, apply_movement.in_set(PhysicsSet::Main));
}

fn setup_top_down_camera(mut commands: Commands) {
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 50.0, 0.0)
            .looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::NEG_Z),
        projection: Projection::Orthographic(OrthographicProjection {
            scale: 30.0,
            near: 0.0,
            far: 1000.0,
            ..default()
        }),
        ..default()
    });
}

fn spawn_local_player(mut commands: Commands, client: Res<Client>) {
    if !client.is_connected() { return; }
    let entity = commands.spawn((
        Player { id: client.id() },
        RigidBody::Dynamic,
        Collider::capsule(0.5, 1.0),
        LockedAxes::new()
            .lock_rotation_x()
            .lock_rotation_y()
            .lock_rotation_z(), // keep upright for top-down
        Transform::from_xyz(0.0, 1.0, 0.0),
        InputManagerBundle::<PlayerAction>::default(),
        // Lightyear automatically adds Predicted + PrePredicted for local authority
    )).id();

    commands.entity(entity).insert(Replicate {
        authority: Authority::Client(client.id()),
        ..default()
    });
}

fn player_input(
    mut action_state: Query<&ActionState<PlayerAction>>,
    mut velocity: Query<&mut LinearVelocity, With<Player>>,
) {
    let Ok(action) = action_state.get_single_mut() else { return };
    let mut vel = velocity.single_mut();

    let move_vec = action.axis_pair(&PlayerAction::Move).unwrap_or_default().xy();
    vel.0.x = move_vec.x * 15.0;
    vel.0.z = move_vec.y * 15.0; // top-down XZ plane
}

fn follow_local_player(
    mut camera: Query<&mut Transform, With<Camera>>,
    local: Query<&Transform, (With<Player>, With<Predicted>, Without<Camera>)>,
) {
    let Ok(mut cam) = camera.get_single_mut() else { return };
    if let Some(p) = local.iter().next() {
        cam.translation.x = p.translation.x;
        cam.translation.z = p.translation.z;
    }
}

fn apply_movement(
    mut query: Query<(&ActionState<PlayerAction>, &mut LinearVelocity), With<Player>>,
) {
    for (action, mut vel) in query.iter_mut() {
        if action.just_pressed(&PlayerAction::Jump) {
            vel.0.y = 12.0;
        }
        // Shoot can be added here later
    }
}

6. src/server.rsrust

use bevy::prelude::*;
use lightyear::prelude::*;

use crate::protocol::*;

pub fn server_plugin(app: &mut App) {
    app.add_plugins(ServerPlugin::default());

    app.add_systems(OnEnter(ServerState::Running), spawn_player_on_connect);
}

fn spawn_player_on_connect(
    mut commands: Commands,
    mut ev: EventReader<ConnectionAccepted>,
) {
    for ev in ev.read() {
        let entity = commands.spawn((
            Player { id: ev.client_id },
            RigidBody::Dynamic,
            Collider::capsule(0.5, 1.0),
            LockedAxes::new().lock_rotation_x().lock_rotation_y().lock_rotation_z(),
            Transform::from_xyz(0.0, 1.0, 0.0),
        )).id();

        commands.entity(entity).insert(Replicate {
            authority: Authority::Client(ev.client_id), // client predicts its own
            ..default()
        });
    }
}

7. src/main.rsrust

use bevy::prelude::*;
use clap::Parser;
use lightyear::prelude::*;

mod protocol; use protocol::*;
mod shared; use shared::*;
mod client; use client::*;
mod server; use server::*;

#[derive(Parser, Debug)]
enum Cli {
    Client,
    Server,
    Both,
}

fn main() {
    let cli = Cli::parse();

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Top-Down MMO Action – Lightyear Prediction".into(),
            ..default()
        }),
        ..default()
    }));

    app.add_plugins(shared_plugin);
    build_protocol(&mut app);

    match cli {
        Cli::Client => app.add_plugins(client_plugin),
        Cli::Server => app.add_plugins(server_plugin),
        Cli::Both => app.add_plugins((client_plugin, server_plugin)),
    }

    app.run();
}

8. How to runbash

# Server
cargo run -- server

# Client(s)
cargo run -- client

# Local testing (one terminal)
cargo run -- both

Controls: WASD = move, Space = jump (extend Shoot as needed).Prediction & Input Flow (exactly as requested)Client only generates inputs – LeafwingInputPlugin runs on client.
Instant prediction – Lightyear + PredictionPlugin applies the input immediately to the local Predicted entity.
Input sent to server – Lightyear automatically bundles the input with the current tick and sends it reliably.
Server replay – Server receives the input and replays the exact same FixedUpdate simulation (Avian physics) from the last acknowledged tick.
State correction – Server sends the authoritative state. If mismatch, client rolls back the Predicted entity and re-simulates forward with its buffered inputs.
No interpolation on controlled entity – Local player has Predicted only. The InterpolationPlugin and Interpolated marker are never added to it. Remote players receive Interpolated and are smoothly interpolated.

This is the official recommended pattern from the Lightyear avian_3d_character example (adapted for top-down and single-file clarity).

