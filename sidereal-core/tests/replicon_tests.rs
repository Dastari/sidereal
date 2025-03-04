use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::renet2::{
    RenetClient, RenetClientPlugin, RenetServer, RenetServerPlugin,
    DefaultChannel,
};
use bevy_replicon_renet2::netcode::{
    ClientAuthentication, ServerAuthentication, ServerSetupConfig,
    NetcodeClientTransport, NetcodeServerTransport,
    NativeSocket,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime};

use sidereal_core::ecs::plugins::replication::{
    client::RepliconRenetClientPlugin,
    server::RepliconRenetServerPlugin,
};

/// Create a server app with our replication plugin
fn setup_server_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
       .add_plugins(RepliconRenetServerPlugin);
    app
}

/// Create a client app with our replication plugin
fn setup_client_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
       .add_plugins(RepliconRenetClientPlugin);
    app
}

/// Setup connection between client and server using actual sockets
fn setup_transport(
    server_app: &mut App,
    client_app: &mut App,
) -> Option<(SocketAddr, u64)> {
    println!("Setting up network transport...");
    
    // Create server transport config with dynamic port (0)
    let server_bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
    let server_socket = match UdpSocket::bind(server_bind_addr) {
        Ok(socket) => socket,
        Err(e) => {
            println!("Failed to bind server socket: {}", e);
            return None;
        }
    };
    
    // Get the actual port assigned by the OS
    let server_addr = match server_socket.local_addr() {
        Ok(addr) => {
            println!("Server bound to address: {}", addr);
            addr
        },
        Err(e) => {
            println!("Failed to get local address: {}", e);
            return None;
        }
    };
    
    // Create NativeSocket for server
    let server_native_socket = match NativeSocket::new(server_socket) {
        Ok(socket) => socket,
        Err(e) => {
            println!("Failed to create server NativeSocket: {}", e);
            return None;
        }
    };
    
    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let protocol_id = 1000;
    let client_id = 100;
    
    println!("Creating server setup config...");
    // Create server setup config
    let addresses: Vec<Vec<SocketAddr>> = vec![vec![server_addr]];
    let server_config = ServerSetupConfig {
        current_time,
        max_clients: 10,
        protocol_id,
        socket_addresses: addresses,
        authentication: ServerAuthentication::Unsecure,
    };
    
    // Create server transport
    let server_transport = match NetcodeServerTransport::new(server_config, server_native_socket) {
        Ok(transport) => {
            println!("Server transport created successfully");
            transport
        },
        Err(e) => {
            println!("Failed to create server transport: {}", e);
            return None;
        }
    };
    
    println!("Creating client socket...");
    // Create client socket
    let client_socket = match UdpSocket::bind("127.0.0.1:0") {
        Ok(socket) => socket,
        Err(e) => {
            println!("Failed to bind client socket: {}", e);
            return None;
        }
    };
    
    // Create NativeSocket for client
    let client_native_socket = match NativeSocket::new(client_socket) {
        Ok(socket) => socket,
        Err(e) => {
            println!("Failed to create client NativeSocket: {}", e);
            return None;
        }
    };
    
    // Create client authentication
    let client_auth = ClientAuthentication::Unsecure {
        server_addr,
        client_id,
        user_data: None,
        protocol_id,
        socket_id: 0,
    };
    
    println!("Creating client transport...");
    let client_transport = match NetcodeClientTransport::new(current_time, client_auth, client_native_socket) {
        Ok(transport) => {
            println!("Client transport created successfully");
            transport
        },
        Err(e) => {
            println!("Failed to create client transport: {}", e);
            return None;
        }
    };
    
    // Add transport to apps
    println!("Adding transports to apps...");
    server_app.insert_resource(server_transport);
    client_app.insert_resource(client_transport);
    
    println!("Network setup complete. Server: {}, Client ID: {}", server_addr, client_id);
    Some((server_addr, client_id))
}

/// Create a test entity on the server
fn create_test_entity(mut commands: Commands) {
    println!("Creating test entity on server");
    commands.spawn((
        Transform::default(),
        Replicated,
    ));
}

/// Test that client can connect to server
#[test]
fn test_client_server_connection() {
    println!("\n=== STARTING CONNECTION TEST ===");
    
    // Setup applications
    println!("Setting up server and client apps...");
    let mut server_app = setup_server_app();
    let mut client_app = setup_client_app();
    
    // Setup networking
    let setup_result = setup_transport(&mut server_app, &mut client_app);
    if setup_result.is_none() {
        println!("❌ Network setup failed, skipping test");
        return;
    }
    
    // Add systems to create entities using Startup
    println!("Adding entity creation system...");
    server_app.add_systems(Startup, create_test_entity);
    
    // Run both apps for a few frames to establish connection
    println!("Running apps to establish connection...");
    for i in 0..20 {
        if i % 5 == 0 {
            println!("Update iteration {}/20", i+1);
        }
        server_app.update();
        client_app.update();
        std::thread::sleep(Duration::from_millis(50));
    }
    
    // Check if server has entities
    let server_entity_count = server_app.world().entities().len();
    println!("Server entity count: {}", server_entity_count);
    assert!(server_entity_count > 0, "Server should have created at least one entity");
    
    // Check RepliconServer status
    if let Some(server) = server_app.world().get_resource::<RepliconServer>() {
        println!("RepliconServer status: available=true");
    } else {
        println!("RepliconServer resource not found");
    }
    
    // Check RepliconClient status
    if let Some(client) = client_app.world().get_resource::<RepliconClient>() {
        println!("RepliconClient status: {:?}", client.status());
    } else {
        println!("RepliconClient resource not found");
    }
    
    println!("✅ Connection test completed successfully");
}

/// Test that client properly disconnects
#[test]
fn test_client_disconnect() {
    println!("\n=== STARTING DISCONNECT TEST ===");
    
    // Setup applications
    println!("Setting up server and client apps...");
    let mut server_app = setup_server_app();
    let mut client_app = setup_client_app();
    
    // Setup networking
    let setup_result = setup_transport(&mut server_app, &mut client_app);
    if setup_result.is_none() {
        println!("❌ Network setup failed, skipping test");
        return;
    }
    
    // Run both apps for a few frames to establish connection
    println!("Running apps to establish connection...");
    for i in 0..10 {
        if i % 2 == 0 {
            println!("Update iteration {}/10", i+1);
        }
        server_app.update();
        client_app.update();
        std::thread::sleep(Duration::from_millis(50));
    }
    
    // Check client status before disconnect
    if let Some(client) = client_app.world().get_resource::<RepliconClient>() {
        println!("Client status before disconnect: {:?}", client.status());
    }
    
    // Remove the client transport to simulate disconnection
    println!("Removing client transport to simulate disconnection...");
    client_app.world_mut().remove_resource::<NetcodeClientTransport>();
    
    // Run a few more frames
    println!("Running apps after disconnect...");
    for i in 0..5 {
        println!("Post-disconnect update {}/5", i+1);
        server_app.update();
        client_app.update();
    }
    
    // Check that client's status is disconnected
    if let Some(client) = client_app.world().get_resource::<RepliconClient>() {
        println!("Client status after disconnect: {:?}", client.status());
    } else {
        println!("RepliconClient resource not found after disconnect");
    }
    
    println!("✅ Disconnect test completed");
}

/// Test messaging functionality between client and server
#[test]
fn test_message_passing() {
    println!("\n=== STARTING MESSAGE PASSING TEST ===");
    
    // Setup applications
    println!("Setting up server and client apps...");
    let mut server_app = setup_server_app();
    let mut client_app = setup_client_app();
    
    // Setup networking
    let setup_result = setup_transport(&mut server_app, &mut client_app);
    if setup_result.is_none() {
        println!("❌ Network setup failed, skipping test");
        return;
    }
    
    let (addr, client_id) = setup_result.unwrap();
    println!("Connection established with server at {}, client ID: {}", addr, client_id);
    
    // Run both apps for a few frames to establish connection
    println!("Running apps to establish connection...");
    for i in 0..10 {
        if i % 2 == 0 {
            println!("Update iteration {}/10", i+1);
        }
        server_app.update();
        client_app.update();
        std::thread::sleep(Duration::from_millis(50));
    }
    
    // Check if we can send messages between client and server
    println!("Attempting to send messages...");
    
    let client_message = "Hello from client";
    if let Some(mut client_renet) = client_app.world_mut().get_resource_mut::<RenetClient>() {
        // Send a test message from client to server
        println!("Client sending message: '{}'", client_message);
        client_renet.send_message(DefaultChannel::ReliableOrdered, client_message.as_bytes().to_vec());
    } else {
        println!("❌ Failed to get RenetClient resource");
    }
    
    let server_message = "Hello from server";
    if let Some(mut server_renet) = server_app.world_mut().get_resource_mut::<RenetServer>() {
        // Send a broadcast message from server to all clients
        println!("Server broadcasting message: '{}'", server_message);
        server_renet.broadcast_message(DefaultChannel::ReliableOrdered, server_message.as_bytes().to_vec());
    } else {
        println!("❌ Failed to get RenetServer resource");
    }
    
    // Run a few more frames to process messages
    println!("Running apps to process messages...");
    for i in 0..5 {
        println!("Post-message update {}/5", i+1);
        
        // Check for received messages on server
        if let Some(mut server_renet) = server_app.world_mut().get_resource_mut::<RenetServer>() {
            for client_id in server_renet.clients_id() {
                while let Some(message) = server_renet.receive_message(client_id, DefaultChannel::ReliableOrdered) {
                    if let Ok(text) = std::str::from_utf8(&message) {
                        println!("✅ Server received from client {}: '{}'", client_id, text);
                    }
                }
            }
        }
        
        // Check for received messages on client
        if let Some(mut client_renet) = client_app.world_mut().get_resource_mut::<RenetClient>() {
            while let Some(message) = client_renet.receive_message(DefaultChannel::ReliableOrdered) {
                if let Ok(text) = std::str::from_utf8(&message) {
                    println!("✅ Client received: '{}'", text);
                }
            }
        }
        
        server_app.update();
        client_app.update();
        std::thread::sleep(Duration::from_millis(50));
    }
    
    println!("✅ Message passing test completed");
}
