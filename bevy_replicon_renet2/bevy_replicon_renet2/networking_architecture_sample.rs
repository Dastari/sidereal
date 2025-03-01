use std::{
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::SystemTime,
};

use bevy::{prelude::*, app::AppExit};
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    netcode::{
        ClientAuthentication, NativeSocket, NetcodeClientTransport, NetcodeServerTransport, 
        ServerAuthentication, ServerSetupConfig,
    },
    renet2::{ConnectionConfig, RenetClient, RenetServer},
    RenetChannelsExt, RepliconRenetPlugins,
};
use serde::{Deserialize, Serialize};

// Protocol constants
const PROTOCOL_ID: u64 = 0x123456789ABCDEF0; // Unique identifier for your protocol
const DEFAULT_PORT: u16 = 5000;

fn main() {
    // This would be your backend server
    if std::env::args().any(|arg| arg == "backend") {
        App::new()
            .add_plugins((DefaultPlugins, RepliconPlugins, RepliconRenetPlugins))
            .add_plugins(BackendServerPlugin)
            .run();
    } 
    // This would be one of your shard servers
    else if std::env::args().any(|arg| arg == "shard") {
        let shard_id = std::env::args()
            .find_map(|arg| arg.strip_prefix("shard-id=").map(|s| s.to_string()))
            .unwrap_or_else(|| "shard-1".to_string());
        
        App::new()
            .insert_resource(ShardId(shard_id))
            .add_plugins((DefaultPlugins, RepliconPlugins, RepliconRenetPlugins))
            .add_plugins(ShardServerPlugin)
            .run();
    } 
    // Client mode - eventually this would use websockets
    else {
        App::new()
            .add_plugins((DefaultPlugins, RepliconPlugins, RepliconRenetPlugins))
            .add_plugins(GameClientPlugin)
            .run();
    }
}

// Resource to store the shard identifier
#[derive(Resource)]
struct ShardId(String);

// Plugin for the central backend server
struct BackendServerPlugin;

impl Plugin for BackendServerPlugin {
    fn build(&self, app: &mut App) {
        app.replicate::<GameEntityPosition>()
           .replicate::<GameEntityState>()
           // Define custom channels for different types of messages
           .add_client_event::<ShardServerConnected>(ChannelKind::Reliable)
           .add_client_event::<EntityStateUpdate>(ChannelKind::Ordered)
           // Systems
           .add_systems(Startup, setup_backend_server)
           .add_systems(Update, (
               handle_shard_connections,
               distribute_entity_updates,
               cleanup_disconnected_shards,
           ));
    }
}

// Plugin for shard servers
struct ShardServerPlugin;

impl Plugin for ShardServerPlugin {
    fn build(&self, app: &mut App) {
        app.replicate::<GameEntityPosition>()
           .replicate::<GameEntityState>()
           // Define channels for different message types
           .add_client_event::<EntitySpawned>(ChannelKind::Reliable)
           .add_client_event::<EntityStateUpdate>(ChannelKind::Ordered)
           .add_client_event::<EntityDespawned>(ChannelKind::Reliable)
           // Systems
           .add_systems(Startup, connect_to_backend)
           .add_systems(Update, (
               process_game_logic,
               sync_entities_with_backend,
               handle_client_connections,
           ));
    }
}

// Plugin for game clients
struct GameClientPlugin;

impl Plugin for GameClientPlugin {
    fn build(&self, app: &mut App) {
        app.replicate::<GameEntityPosition>()
           .replicate::<GameEntityState>()
           .add_client_event::<PlayerInput>(ChannelKind::Ordered)
           // Systems
           .add_systems(Startup, connect_to_shard)
           .add_systems(Update, (
               process_input,
               render_game_state,
               handle_server_disconnect,
           ));
    }
}

// Replicable components
#[derive(Component, Serialize, Deserialize, Clone)]
struct GameEntityPosition(Vec3);

#[derive(Component, Serialize, Deserialize, Clone)]
struct GameEntityState {
    entity_type: EntityType,
    health: f32,
    status_effects: Vec<StatusEffect>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
enum EntityType {
    Player,
    NPC,
    Item,
    Environment,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
enum StatusEffect {
    Stunned,
    Slowed,
    Invulnerable,
    Buffed,
}

// Network events
#[derive(Event, Serialize, Deserialize, Clone)]
struct ShardServerConnected {
    shard_id: String,
    capacity: u32,
    current_load: u32,
}

#[derive(Event, Serialize, Deserialize, Clone)]
struct EntityStateUpdate {
    entity_id: u64,
    position: Option<GameEntityPosition>,
    state: Option<GameEntityState>,
}

#[derive(Event, Serialize, Deserialize, Clone)]
struct EntitySpawned {
    entity_id: u64,
    position: GameEntityPosition,
    state: GameEntityState,
}

#[derive(Event, Serialize, Deserialize, Clone)]
struct EntityDespawned {
    entity_id: u64,
}

#[derive(Event, Serialize, Deserialize, Clone)]
struct PlayerInput {
    action: InputAction,
    direction: Option<Vec2>,
    target_entity: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum InputAction {
    Move,
    Attack,
    UseAbility(u32),
    Interact,
}

// Setup the backend server network
fn setup_backend_server(mut commands: Commands, channels: Res<RepliconChannels>) -> Result<(), Box<dyn Error>> {
    let server = RenetServer::new(ConnectionConfig::from_channels(
        channels.get_server_configs(),
        channels.get_client_configs(),
    ));

    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
    let public_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), DEFAULT_PORT);
    let socket = UdpSocket::bind(public_addr)?;
    
    let server_config = ServerSetupConfig {
        current_time,
        max_clients: 32, // Maximum number of shard servers
        protocol_id: PROTOCOL_ID,
        authentication: ServerAuthentication::Unsecure, // In production, use secure authentication
        socket_addresses: vec![vec![public_addr]],
    };
    
    let transport = NetcodeServerTransport::new(server_config, NativeSocket::new(socket).unwrap())?;

    commands.insert_resource(server);
    commands.insert_resource(transport);
    
    info!("Backend server started on {}", public_addr);
    
    Ok(())
}

// Connect a shard server to the backend
fn connect_to_backend(
    mut commands: Commands, 
    channels: Res<RepliconChannels>,
    shard_id: Res<ShardId>,
) -> Result<(), Box<dyn Error>> {
    let client = RenetClient::new(
        ConnectionConfig::from_channels(channels.get_server_configs(), channels.get_client_configs()),
        false,
    );

    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
    // Use shard_id hash as client_id to ensure uniqueness
    let client_id = shard_id.0.bytes().fold(0u64, |acc, b| acc.wrapping_add(b as u64));
    
    let backend_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), DEFAULT_PORT);
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        socket_id: 0,
        server_addr: backend_addr,
        user_data: None,
    };
    
    let transport = NetcodeClientTransport::new(current_time, authentication, NativeSocket::new(socket).unwrap())?;

    commands.insert_resource(client);
    commands.insert_resource(transport);
    
    info!("Shard server {} connecting to backend at {}", shard_id.0, backend_addr);
    
    Ok(())
}

// Handle connections from shard servers to the backend
fn handle_shard_connections(
    mut server: ResMut<RenetServer>,
    mut shard_events: EventReader<FromClient<ShardServerConnected>>,
) {
    for FromClient { client_id, event } in shard_events.read() {
        info!("Shard server {} connected with capacity: {}", event.shard_id, event.capacity);
        
        // Here you would track the shard in your backend's shard registry
        // This is where you would decide which shards handle which game areas
    }
}

// Sample system for a shard server to process game logic
fn process_game_logic(
    time: Res<Time>,
    mut entity_query: Query<(Entity, &mut GameEntityPosition, &mut GameEntityState)>,
    // Additional resources/queries as needed
) {
    // Handle game physics, AI, and other systems within the shard
    for (entity, mut position, mut state) in entity_query.iter_mut() {
        // Example game logic - move entities, update states, etc.
        // In a real implementation, this would be more complex
    }
}

// Sync entity states from shard servers to the backend
fn sync_entities_with_backend(
    mut client: ResMut<RenetClient>,
    entity_query: Query<(Entity, &GameEntityPosition, &GameEntityState), Changed<GameEntityPosition>>,
    mut state_events: EventWriter<EntityStateUpdate>,
) {
    if !client.is_connected() {
        return;
    }
    
    // Send updated entity states to the backend
    for (entity, position, state) in entity_query.iter() {
        let entity_id = entity.index(); // Using entity index as a network ID
        
        let update = EntityStateUpdate {
            entity_id: entity_id as u64,
            position: Some(position.clone()),
            state: Some(state.clone()),
        };
        
        state_events.send(update);
    }
}

// Distribute entity updates from the backend to relevant shards
fn distribute_entity_updates(
    mut server: ResMut<RenetServer>,
    mut update_events: EventReader<FromClient<EntityStateUpdate>>,
    // You would need additional resources to track which shards are responsible for which areas
) {
    for FromClient { client_id, event } in update_events.read() {
        // Determine which shards need this entity update based on your game's sharding strategy
        // This is a simplified example - you'd need more complex logic for actual zone-based sharding
        
        // Broadcast to all shards for simplicity in this example
        // In a real implementation, you'd only send to relevant shards
        for target_client in server.clients_id() {
            // Skip the sender to avoid echo
            if target_client == *client_id {
                continue;
            }
            
            // Send the update to the other shards
            server.send_message(target_client, ChannelKind::Ordered, bincode::serialize(&event).unwrap());
        }
    }
}

// Connect client to a shard server
fn connect_to_shard(mut commands: Commands, channels: Res<RepliconChannels>) -> Result<(), Box<dyn Error>> {
    // Similar to connect_to_backend but for game clients
    // In a real implementation, this would likely use WebSockets instead of UDP
    // For this example, we're using the same connection method
    
    let client = RenetClient::new(
        ConnectionConfig::from_channels(channels.get_server_configs(), channels.get_client_configs()),
        false,
    );

    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
    let client_id = current_time.as_millis() as u64;
    let shard_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), DEFAULT_PORT + 1); // Shard server port
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        socket_id: 0,
        server_addr: shard_addr,
        user_data: None,
    };
    
    let transport = NetcodeClientTransport::new(current_time, authentication, NativeSocket::new(socket).unwrap())?;

    commands.insert_resource(client);
    commands.insert_resource(transport);
    
    info!("Game client connected to shard at {}", shard_addr);
    
    Ok(())
}

// Simple system to handle client input
fn process_input(
    input: Res<ButtonInput<KeyCode>>,
    mut input_events: EventWriter<PlayerInput>,
) {
    let mut direction = Vec2::ZERO;
    
    if input.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }
    if input.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if input.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if input.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }
    
    if direction != Vec2::ZERO {
        input_events.send(PlayerInput {
            action: InputAction::Move,
            direction: Some(direction.normalize_or_zero()),
            target_entity: None,
        });
    }
    
    // Handle other input actions
    if input.just_pressed(KeyCode::Space) {
        input_events.send(PlayerInput {
            action: InputAction::Attack,
            direction: None,
            target_entity: None,
        });
    }
}

// Cleanup disconnected shards
fn cleanup_disconnected_shards(
    mut server: ResMut<RenetServer>,
    mut exit: EventWriter<AppExit>,
) {
    // Check for disconnected clients
    let disconnected_clients: Vec<_> = server
        .disconnections()
        .map(|(client_id, reason)| {
            info!("Shard disconnected: {client_id:?}, reason: {reason}");
            client_id
        })
        .collect();
    
    // In a production system, you'd need to handle redistribution of game entities
    // when a shard server disconnects
}

// Handle server disconnect events on the client side
fn handle_server_disconnect(
    client: Res<RenetClient>,
    mut exit: EventWriter<AppExit>,
) {
    if !client.is_connected() && client.is_disconnected() {
        error!("Disconnected from server: {:?}", client.disconnect_reason());
        // In a real game, you might want to show a disconnection screen rather than exit
        exit.send(AppExit);
    }
}

// Render game state based on replicated entities
fn render_game_state(
    entity_query: Query<(&GameEntityPosition, &GameEntityState)>,
    // Add renderer-specific resources
) {
    // In a real game, this would use Bevy's rendering capabilities
    // to display the current game state
    for (position, state) in entity_query.iter() {
        // Render each entity based on position and state
    }
}

// Handle client connections to a shard server
fn handle_client_connections(
    mut server: Option<ResMut<RenetServer>>,
    mut commands: Commands,
) {
    let Some(mut server) = server else { return };
    
    // Process new client connections
    for client_id in server.clients_id() {
        // Check if this is a new connection
        if server.is_client_connected(client_id) && !server.has_client_data::<()>(client_id) {
            info!("New client connected: {client_id:?}");
            
            // Mark as processed
            server.insert_client_data(client_id, ());
            
            // Spawn player entity for this client
            let player_entity = commands.spawn((
                GameEntityPosition(Vec3::ZERO),
                GameEntityState {
                    entity_type: EntityType::Player,
                    health: 100.0,
                    status_effects: Vec::new(),
                },
                Replicated,
            )).id();
            
            // Associate client with their entity
            // You'd need a separate component for this in a real implementation
        }
    }
} 