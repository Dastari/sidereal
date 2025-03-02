use bevy::{log::LogPlugin, prelude::*};
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::client::RepliconRenetClientPlugin;
use renet2::{ChannelConfig, ConnectionConfig, RenetClient, SendType};
use renet2_netcode::{
    ClientAuthentication, NativeSocket, NetcodeClientTransport, NetcodeTransportError,
};
use std::{
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    time::SystemTime,
};

// Define a test component for replication
#[derive(Component, Default, Reflect)]
struct TestComponent {
    value: u32,
}

fn main() {
    // Set log environment variables before creating the app
    std::env::set_var("RUST_LOG", "info,renet2=debug,renetcode2=debug");

    App::new()
        // Use minimal plugins
        .add_plugins(MinimalPlugins)
        .add_plugins(LogPlugin::default())
        // Register the test component for replication
        .register_type::<TestComponent>()
        // Add all the replicon plugins first - this is important!
        .add_plugins((RepliconPlugins, RepliconRenetClientPlugin))
        // Add our systems
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                update_connection,
                handle_connection_state,
                send_test_messages,
                log_connection_status,
            ),
        )
        .run();
}

fn setup(mut commands: Commands) {
    info!("Setting up test client to connect to replication server...");

    // Server information
    let server_addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 5000);
    let protocol_id = 0; // Must match server's protocol_id
    let client_id = 3000; // Unique ID for this client

    // Create socket with any available port
    let socket = match UdpSocket::bind((Ipv4Addr::new(0, 0, 0, 0), 0)) {
        Ok(socket) => {
            match socket.local_addr() {
                Ok(addr) => info!("Socket bound to {}", addr),
                Err(_) => info!("Socket bound successfully but couldn't get local address"),
            }
            socket
        }
        Err(e) => {
            error!("Failed to bind socket: {}", e);
            return;
        }
    };

    // Create NativeSocket
    let native_socket = match NativeSocket::new(socket) {
        Ok(socket) => socket,
        Err(e) => {
            error!("Failed to create native socket: {:?}", e);
            return;
        }
    };

    // Get current time - IMPORTANT: Use current time without offset!
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();

    info!("Current time for token: {:.6}s", current_time.as_secs_f64());

    // Authentication
    let auth = ClientAuthentication::Unsecure {
        client_id,
        protocol_id,
        server_addr,
        user_data: None,
        socket_id: 0,
    };

    // Create transport
    let transport = match NetcodeClientTransport::new(current_time, auth, native_socket) {
        Ok(transport) => {
            info!("Transport created successfully");
            transport
        }
        Err(e) => {
            error!("Failed to create transport: {:?}", e);
            return;
        }
    };

    // Create channel configuration matching the server
    let config = ConnectionConfig::from_shared_channels(vec![ChannelConfig {
        channel_id: 0,
        max_memory_usage_bytes: 5 * 1024 * 1024,
        send_type: SendType::ReliableOrdered {
            resend_time: std::time::Duration::from_millis(100),
        },
    }]);

    // Create client
    let client = RenetClient::new(config, false);

    // Insert resources
    commands.insert_resource(transport);
    commands.insert_resource(client);

    // Create some test entities with our test component
    commands.spawn(TestComponent { value: 42 });

    info!("Client setup complete, waiting for connection...");
}

// Update the connection with latest time
fn update_connection(
    mut client: ResMut<RenetClient>,
    mut transport: ResMut<NetcodeClientTransport>,
) {
    // Get current time for update
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();

    // Update transport with current time
    match transport.update(current_time, &mut client) {
        Ok(_) => {
            // Successfully updated
        }
        Err(e) => {
            // Only log serious errors, not just the expected disconnect ones
            if !matches!(e, NetcodeTransportError::Netcode(_)) {
                error!("Transport update error: {:?}", e);
            }
        }
    }
}

fn handle_connection_state(replicon_client: Res<RepliconClient>, time: Res<Time>) {
    // Track and handle the connection state
    match replicon_client.status() {
        RepliconClientStatus::Connected { client_id } => {
            // We're connected
            if time.elapsed_secs_f64() % 5.0 < 0.01 {
                info!("Connected to server with client ID: {:?}", client_id);
            }
        }
        RepliconClientStatus::Connecting => {
            // We're connecting
            if time.elapsed_secs_f64() % 2.0 < 0.01 {
                info!("Connecting to server...");
            }
        }
        RepliconClientStatus::Disconnected => {
            // We're disconnected
            if time.elapsed_secs_f64() % 5.0 < 0.01 {
                info!("Disconnected from server, will automatically reconnect");
            }
        }
    }
}

fn log_connection_status(
    client: Res<RenetClient>,
    transport: Res<NetcodeClientTransport>,
    time: Res<Time>,
) {
    // Log detailed connection status periodically
    if time.elapsed_secs_f64() % 5.0 < 0.01 {
        info!(
            "RenetClient details: connected={}, connecting={}",
            client.is_connected(),
            transport.is_connecting()
        );
    }
}

fn send_test_messages(mut replicon_client: ResMut<RepliconClient>, time: Res<Time>) {
    // Only send messages if the client is connected
    if replicon_client.is_connected() {
        // Send a test message every 3 seconds
        if time.elapsed_secs_f64() % 3.0 < 0.01 {
            let message = format!("Test message at time {:.2}s", time.elapsed_secs_f64());

            // Use the send method from RepliconClient to queue the message
            replicon_client.send(0, message.clone().into_bytes());
            info!("Sent message: {}", message);
        }
    }
}
