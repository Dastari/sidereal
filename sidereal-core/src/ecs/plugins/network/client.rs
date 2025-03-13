use crate::ecs::systems::network::{receive_client_message_system, NetworkMessageEvent};
use bevy::prelude::*;
use bevy_renet::netcode::*;
use bevy_renet::renet::*;
use bevy_renet::*;
use std::net::UdpSocket;
use std::time::SystemTime;

pub struct NetworkClientPlugin;

pub const SERVER_ADDR: &str = "127.0.0.1:5000";

impl Plugin for NetworkClientPlugin {
    fn build(&self, app: &mut App) {
        info!("Initializing network client");
        let client = RenetClient::new(ConnectionConfig::default());
        app.add_plugins(RenetClientPlugin);
        app.add_event::<NetworkMessageEvent>();
        app.insert_resource(client);

        // Setup the transport layer
        app.add_plugins(NetcodeClientPlugin);

        let client_id = uuid::Uuid::new_v4();
        let client_id_str = client_id.to_string();
        info!("Generated client ID: {}", client_id_str);

        let authentication = ClientAuthentication::Unsecure {
            server_addr: SERVER_ADDR.parse().unwrap(),
            client_id: client_id.as_u128() as u64,
            user_data: Some({
                let mut user_data = [0; NETCODE_USER_DATA_BYTES];

                // Copy the UUID string bytes into user_data
                let uuid_bytes = client_id_str.as_bytes();
                user_data[..uuid_bytes.len()].copy_from_slice(uuid_bytes);
                user_data
            }),
            protocol_id: 0,
        };
        info!("Configured authentication for server at {}", SERVER_ADDR);
        
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        info!("Bound UDP socket locally to {}", socket.local_addr().unwrap());
        
        let current_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let transport = NetcodeClientTransport::new(current_time, authentication, socket).unwrap();
        info!("Network transport initialized, attempting to connect");

        app.insert_resource(transport);
        app.add_systems(Update, receive_client_message_system);
        
        // Add system to monitor connection status
        app.add_systems(Update, log_connection_status);
    }
}

// New system to monitor connection status changes and handle reconnection
fn log_connection_status(
    mut client: ResMut<RenetClient>,
    mut transport: ResMut<NetcodeClientTransport>,
    mut connection_state: Local<ConnectionState>,
    time: Res<Time>,
) {
    // Always check the actual client status first, overriding any stored state
    let mut current_state = if client.is_connected() {
        ConnectionStateType::Connected
    } else if client.is_connecting() {
        ConnectionStateType::Connecting
    } else if client.is_disconnected() {
        // Only use the reconnecting state if that's what we're currently doing,
        // otherwise consider ourselves disconnected
        if connection_state.state_type == ConnectionStateType::Reconnecting {
            ConnectionStateType::Reconnecting
        } else {
            ConnectionStateType::Disconnected
        }
    } else {
        ConnectionStateType::Unknown
    };

    // Update elapsed time for reconnection attempts
    if current_state == ConnectionStateType::Disconnected || current_state == ConnectionStateType::Reconnecting {
        connection_state.disconnected_time += time.delta_secs();
    } else {
        connection_state.disconnected_time = 0.0;
    }

    // Check if we need to attempt reconnection (every 5 seconds)
    if current_state == ConnectionStateType::Disconnected && 
       connection_state.disconnected_time >= 5.0 {
        info!("Attempting to reconnect to server at {}", SERVER_ADDR);
        
        // Reset timer and change state to reconnecting
        connection_state.disconnected_time = 0.0;
        current_state = ConnectionStateType::Reconnecting;
        
        // Create new client ID
        let client_id = uuid::Uuid::new_v4();
        let client_id_str = client_id.to_string();
        
        // Configure authentication
        let authentication = ClientAuthentication::Unsecure {
            server_addr: SERVER_ADDR.parse().unwrap(),
            client_id: client_id.as_u128() as u64,
            user_data: Some({
                let mut user_data = [0; NETCODE_USER_DATA_BYTES];
                let uuid_bytes = client_id_str.as_bytes();
                user_data[..uuid_bytes.len()].copy_from_slice(uuid_bytes);
                user_data
            }),
            protocol_id: 0,
        };
        
        // Create a new transport with the updated authentication
        if let Ok(socket) = UdpSocket::bind("127.0.0.1:0") {
            let current_time = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap();
                
            if let Ok(new_transport) = NetcodeClientTransport::new(current_time, authentication, socket) {
                *transport = new_transport;
                
                // Reset the client as well to ensure a fresh connection attempt
                *client = RenetClient::new(ConnectionConfig::default());
                
                info!("New transport created with client ID: {}", client_id_str);
            }
        }
    }
    
    // Check for reconnection attempts that have been going on too long - reset them
    if current_state == ConnectionStateType::Reconnecting && 
       connection_state.last_spinner_time >= 30.0 {  // After 30 seconds of trying
        info!("Reconnection attempt timed out, resetting...");
        current_state = ConnectionStateType::Disconnected;
        connection_state.disconnected_time = 5.0;  // Force immediate retry with new connection
        connection_state.last_spinner_time = 0.0;
    }

    // Only log when state changes or on timed intervals for ongoing states
    if connection_state.state_type != current_state {
        // Log state transitions
        match current_state {
            ConnectionStateType::Connected => info!("Client successfully connected to server"),
            ConnectionStateType::Connecting => info!("Client attempting to connect to server..."),
            ConnectionStateType::Reconnecting => info!("Client attempting to reconnect to server..."),
            ConnectionStateType::Disconnected => info!("Client disconnected from server"),
            ConnectionStateType::Unknown => info!("Client in unknown state"),
        }
        
        // Update the state
        connection_state.state_type = current_state;
        connection_state.spinner_count = 0;
        connection_state.last_spinner_time = 0.0;
    } else if (current_state == ConnectionStateType::Connecting || current_state == ConnectionStateType::Reconnecting) 
             && connection_state.last_spinner_time >= 10.0 {  // Only update every 10 seconds
        
        // Calculate elapsed time
        let total_elapsed = if current_state == ConnectionStateType::Connecting {
            connection_state.last_spinner_time
        } else {
            connection_state.disconnected_time
        };
        
        let elapsed = total_elapsed.round() as u32;
        let minutes = elapsed / 60;
        let seconds = elapsed % 60;
        
        let status = if current_state == ConnectionStateType::Connecting {
            "connecting"
        } else {
            "reconnecting"
        };
        
        info!("Still {} ({}m:{}s)", status, minutes, seconds);
        
        // Reset the timer for spinner updates only
        connection_state.last_spinner_time = 0.0;
    } else if current_state == ConnectionStateType::Connecting || current_state == ConnectionStateType::Reconnecting {
        // Just update the time and spinner without logging
        connection_state.last_spinner_time += time.delta_secs();
        connection_state.spinner_count += 1;
    }
}

// Update the enum to include the reconnection state
#[derive(PartialEq, Eq, Clone, Copy)]
enum ConnectionStateType {
    Connected,
    Connecting,
    Reconnecting,
    Disconnected,
    Unknown,
}

// Update the struct to track connection state and disconnection time
#[derive(Default)]
struct ConnectionState {
    state_type: ConnectionStateType,
    spinner_count: u32,
    disconnected_time: f32,
    last_spinner_time: f32,
}

impl Default for ConnectionStateType {
    fn default() -> Self {
        ConnectionStateType::Unknown
    }
}
