// ==============================================================================
// main.rs
// Bevy + Lightyear + Avian3D - Strict Client Prediction (No Interpolation)
// ==============================================================================

use bevy::prelude::*;
use avian3d::prelude::*;
use lightyear::prelude::*;
use lightyear::prelude::client::*;
use lightyear::prelude::server::*;
use lightyear_avian3d::prelude::*;
use serde::{Deserialize, Serialize};

// ==========================================
// 1. SHARED PROTOCOL & DATA
// ==========================================

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Player(pub ClientId);

// The input payload sent from client to server
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
pub struct PlayerMoveInput {
    pub direction: Vec2,
}
impl UserAction for PlayerMoveInput {}

pub struct SharedPlugin;
impl Plugin for SharedPlugin {
    fn build(&self, app: &mut App) {
        // 1. Avian Physics Setup
        // We MUST disable sleeping and spatial islands for Lightyear rollback to work predictably.
        app.add_plugins(
            PhysicsPlugins::default()
                .build()
                .disable::<IslandsPlugin>()
                .disable::<IslandSleepingPlugin>(),
        );

        // 2. Lightyear-Avian Rollback Integration
        app.add_plugins(LightyearAvianExtPlugin);

        // 3. Register Networked Components & Inputs
        app.register_component::<Player>(ChannelDirection::ServerToClient);
        app.add_plugins(InputPlugin::<PlayerMoveInput>::default());

        // 4. THE CORE PREDICTION/REPLAY SYSTEM
        // Both the client and server run this exact same system during FixedUpdate.
        // - On the server: It applies inputs received from the client for the current tick.
        // - On the client: It applies local predicted inputs, and resimulates them during rollbacks.
        app.add_systems(FixedUpdate, apply_movement_inputs);
    }
}

// The shared deterministic physics logic
fn apply_movement_inputs(
    mut players: Query<(&Player, &mut LinearVelocity)>,
    // The InputManager handles buffering on the client and receiving on the server seamlessly
    mut input_manager: ResMut<InputManager<PlayerMoveInput>>,
) {
    let speed = 15.0;

    for (player, mut velocity) in players.iter_mut() {
        // Fetch the input for this specific client's entity for the current simulation tick
        if let Some(input) = input_manager.get_input(player.0) {
            velocity.x = input.direction.x * speed;
            velocity.z = input.direction.y * speed;
        } else {
            // If no input is present (e.g., packet loss or idle), zero out velocity
            velocity.x = 0.0;
            velocity.z = 0.0;
        }
    }
}

// ==========================================
// 2. SERVER LOGIC
// ==========================================

pub struct ServerGamePlugin;
impl Plugin for ServerGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_connections);
    }
}

fn handle_connections(mut commands: Commands, mut events: EventReader<ConnectEvent>) {
    for event in events.read() {
        let client_id = event.client_id();
        info!("Client connected: {:?}", client_id);

        commands.spawn((
            Player(client_id),
            // Avian3D Physics Components
            RigidBody::Dynamic,
            Collider::capsule(0.5, 1.0),
            Position::default(),
            LinearVelocity::default(),
            LockedAxes::ROTATION_LOCKED, // Essential for Top-Down Action
            
            // Lightyear Replication configuration
            Replicate {
                target: ReplicationTarget::All,
                authority: AuthorityPeer::Server, // Server is the absolute boss
                sync: SyncTarget {
                    // 1. The local client predicts this entity locally
                    prediction: NetworkTarget::Only(vec![client_id]), 
                    // 2. NO INTERPOLATION. Raw snap updates for everyone else.
                    interpolation: NetworkTarget::None, 
                },
                ..default()
            },
        ));
    }
}

// ==========================================
// 3. CLIENT LOGIC
// ==========================================

pub struct ClientGamePlugin;
impl Plugin for ClientGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, buffer_local_inputs);
    }
}

// Read raw keyboard state and package it into the Lightyear InputManager.
// We do this in PreUpdate so the shared `apply_movement_inputs` system 
// has access to it during the FixedUpdate physics steps.
fn buffer_local_inputs(
    mut input_manager: ResMut<InputManager<PlayerMoveInput>>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    let mut direction = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyW) { direction.y -= 1.0; }
    if keyboard.pressed(KeyCode::KeyS) { direction.y += 1.0; }
    if keyboard.pressed(KeyCode::KeyA) { direction.x -= 1.0; }
    if keyboard.pressed(KeyCode::KeyD) { direction.x += 1.0; }

    let direction = direction.normalize_or_zero();
    
    // Send to server AND buffer locally for our prediction loop
    input_manager.add_input(PlayerMoveInput { direction }, Target::Server);
}

// ==========================================
// 4. APP ENTRY POINT
// ==========================================

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    
    // Add our Shared logic (Physics, Components, Shared Systems)
    app.add_plugins(SharedPlugin);

    // Note: In a real environment, you'd use Lightyear's client/server config builders 
    // here to set up your UDP/WebTransport bindings and select which mode to run in.
    
    // For the sake of the example structure, we pretend both are added.
    // app.add_plugins(ServerGamePlugin);
    // app.add_plugins(ClientGamePlugin);

    // app.run();
}