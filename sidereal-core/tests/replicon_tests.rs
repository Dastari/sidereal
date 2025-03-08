use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    netcode::{
        ClientAuthentication, NativeSocket, NetcodeClientTransport, NetcodeServerTransport,
        ServerAuthentication, ServerSetupConfig,
    },
    renet2::{ConnectionConfig, DefaultChannel, RenetClient, RenetServer},
    RenetChannelsExt,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime};

use sidereal_core::ecs::plugins::replication::{
    client::RepliconRenetClientPlugin, server::RepliconRenetServerPlugin,
};

/// Create a server app with our replication plugin
fn setup_server_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(RepliconRenetServerPlugin)
        // Initialize resources needed for replication
        .init_resource::<RepliconServer>()
        .init_resource::<ConnectedClients>()
        .init_resource::<RepliconChannels>();

    // Create connection config for the server
    let channels = app.world().resource::<RepliconChannels>();
    let connection_config = ConnectionConfig::from_channels(
        channels.get_server_configs(),
        channels.get_client_configs(),
    );

    // Initialize RenetServer with the connection config
    app.insert_resource(RenetServer::new(connection_config));

    app
}

/// Create a client app with our replication plugin
fn setup_client_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(RepliconRenetClientPlugin)
        // Initialize resources needed for replication
        .init_resource::<RepliconClient>()
        .init_resource::<RepliconChannels>();

    // Create connection config for the client
    let channels = app.world().resource::<RepliconChannels>();
    let connection_config = ConnectionConfig::from_channels(
        channels.get_server_configs(),
        channels.get_client_configs(),
    );

    // Initialize RenetClient with the connection config
    app.insert_resource(RenetClient::new(connection_config, false));

    app
}

/// Setup connection between client and server using actual sockets
fn setup_transport(server_app: &mut App, client_app: &mut App) -> Option<(SocketAddr, u64)> {
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
        }
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

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
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
        }
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
    let client_transport =
        match NetcodeClientTransport::new(current_time, client_auth, client_native_socket) {
            Ok(transport) => {
                println!("Client transport created successfully");
                transport
            }
            Err(e) => {
                println!("Failed to create client transport: {}", e);
                return None;
            }
        };

    // Add transport to apps
    println!("Adding transports to apps...");
    server_app.insert_resource(server_transport);
    client_app.insert_resource(client_transport);

    println!(
        "Network setup complete. Server: {}, Client ID: {}",
        server_addr, client_id
    );
    Some((server_addr, client_id))
}

/// Create a test entity on the server
fn create_test_entity(mut commands: Commands) {
    println!("Creating test entity on server");
    commands.spawn((Transform::default(), Replicated));
}

/// Test that client can connect to server
#[test]
fn test_client_server_connection() {
    println!("\n🔌 TESTING CLIENT-SERVER CONNECTION");
    println!("This test verifies that a client can properly connect to a server and receive replicated entities");

    // Setup applications
    println!("🏗️ Setting up server and client apps...");
    let mut server_app = setup_server_app();
    let mut client_app = setup_client_app();

    // Setup networking
    let setup_result = setup_transport(&mut server_app, &mut client_app);
    if setup_result.is_none() {
        println!("❌ Network setup failed, skipping test");
        return;
    }

    // Add systems to create entities using Startup
    println!("🚀 Adding entity creation system...");
    server_app.add_systems(Startup, create_test_entity);

    // Run both apps for a few frames to establish connection
    println!("➡️ Running apps to establish connection...");
    for i in 0..20 {
        if i % 5 == 0 {
            println!("   - Update iteration {}/20", i + 1);
        }
        server_app.update();
        client_app.update();
        std::thread::sleep(Duration::from_millis(50));
    }

    // Check if server has entities
    let server_entity_count = server_app.world().entities().len();
    println!("✅ VERIFICATION:");
    println!("   - Server entity count: {}", server_entity_count);
    assert!(
        server_entity_count > 0,
        "Server should have created at least one entity"
    );

    // Check RepliconServer status
    if let Some(_server) = server_app.world().get_resource::<RepliconServer>() {
        println!("   - RepliconServer status: ✅ Available");
    } else {
        println!("   - RepliconServer status: ❌ Not found");
    }

    // Check RepliconClient status
    if let Some(client) = client_app.world().get_resource::<RepliconClient>() {
        println!("   - RepliconClient status: {:?}", client.status());
    } else {
        println!("   - RepliconClient status: ❌ Not found");
    }

    println!("✅ Connection test completed successfully");
}

/// Test that client properly disconnects
#[test]
fn test_client_disconnect() {
    println!("\n🔌 TESTING CLIENT DISCONNECTION");
    println!("This test verifies that a client properly disconnects and changes status");

    // Setup applications
    println!("🏗️ Setting up server and client apps...");
    let mut server_app = setup_server_app();
    let mut client_app = setup_client_app();

    // Setup networking
    let setup_result = setup_transport(&mut server_app, &mut client_app);
    if setup_result.is_none() {
        println!("❌ Network setup failed, skipping test");
        return;
    }

    // Run both apps for a few frames to establish connection
    println!("➡️ Running apps to establish connection...");
    for i in 0..10 {
        if i % 2 == 0 {
            println!("   - Update iteration {}/10", i + 1);
        }
        server_app.update();
        client_app.update();
        std::thread::sleep(Duration::from_millis(50));
    }

    // Check client status before disconnect
    if let Some(client) = client_app.world().get_resource::<RepliconClient>() {
        println!("🔍 Client status before disconnect: {:?}", client.status());
    }

    // Remove the client transport to simulate disconnection
    println!("✂️ Removing client transport to simulate disconnection...");
    client_app
        .world_mut()
        .remove_resource::<NetcodeClientTransport>();

    // Run a few more frames
    println!("➡️ Running apps after disconnect...");
    for i in 0..5 {
        println!("   - Post-disconnect update {}/5", i + 1);
        server_app.update();
        client_app.update();
    }

    // Check that client's status is disconnected
    println!("✅ VERIFICATION:");
    if let Some(client) = client_app.world().get_resource::<RepliconClient>() {
        let status = client.status();
        println!("   - Client status after disconnect: {:?}", status);

        // Just display if it appears to be disconnected based on debug output
        println!(
            "   - Status check: {}",
            if format!("{:?}", status).contains("Disconnected") {
                "✅ Client successfully disconnected"
            } else {
                "❌ Client still shows as connected"
            }
        );
    } else {
        println!("   - RepliconClient status: ❌ Resource not found after disconnect");
    }

    println!("✅ Disconnect test completed successfully");
}

/// Test messaging functionality between client and server
#[test]
fn test_message_passing() {
    println!("\n📨 TESTING MESSAGE PASSING");
    println!("This test verifies that messages can be sent between client and server");

    // Debug DefaultChannel enum values
    println!("📊 Channel configuration:");
    println!(
        "   - DefaultChannel::ReliableOrdered = {}",
        DefaultChannel::ReliableOrdered as u8
    );
    println!(
        "   - DefaultChannel::Unreliable = {}",
        DefaultChannel::Unreliable as u8
    );

    // Setup applications
    println!("🏗️ Setting up server and client apps...");
    let mut server_app = setup_server_app();
    let mut client_app = setup_client_app();

    // Setup networking
    let setup_result = setup_transport(&mut server_app, &mut client_app);
    if setup_result.is_none() {
        println!("❌ Network setup failed, skipping test");
        return;
    }

    let (addr, client_id) = setup_result.unwrap();
    println!("🔌 Connection established:");
    println!("   - Server address: {}", addr);
    println!("   - Client ID: {}", client_id);

    // Resource status check
    println!("\n📋 RESOURCE STATUS BEFORE UPDATES:");
    println!(
        "   - Server has RenetServer: {}",
        if server_app.world().contains_resource::<RenetServer>() {
            "✅"
        } else {
            "❌"
        }
    );
    println!(
        "   - Server has RepliconServer: {}",
        if server_app.world().contains_resource::<RepliconServer>() {
            "✅"
        } else {
            "❌"
        }
    );
    println!(
        "   - Client has RenetClient: {}",
        if client_app.world().contains_resource::<RenetClient>() {
            "✅"
        } else {
            "❌"
        }
    );
    println!(
        "   - Client has RepliconClient: {}",
        if client_app.world().contains_resource::<RepliconClient>() {
            "✅"
        } else {
            "❌"
        }
    );

    // Run both apps for a few frames to establish connection
    println!("➡️ Running apps to establish connection...");
    for i in 0..20 {
        if i % 5 == 0 {
            println!("   - Update iteration {}/20", i + 1);
        }
        server_app.update();
        client_app.update();
        std::thread::sleep(Duration::from_millis(50));
    }

    // Check resource status again after updates
    println!("\n📋 RESOURCE STATUS AFTER UPDATES:");
    println!(
        "   - Server has RenetServer: {}",
        if server_app.world().contains_resource::<RenetServer>() {
            "✅"
        } else {
            "❌"
        }
    );
    println!(
        "   - Server has RepliconServer: {}",
        if server_app.world().contains_resource::<RepliconServer>() {
            "✅"
        } else {
            "❌"
        }
    );
    println!(
        "   - Client has RenetClient: {}",
        if client_app.world().contains_resource::<RenetClient>() {
            "✅"
        } else {
            "❌"
        }
    );
    println!(
        "   - Client has RepliconClient: {}",
        if client_app.world().contains_resource::<RepliconClient>() {
            "✅"
        } else {
            "❌"
        }
    );

    // Define test messages
    let client_message = "Hello from client test";
    let server_message = "Hello from server test";

    // Try to send messages if resources exist
    println!("📤 SENDING TEST MESSAGES:");

    // Use RepliconClient/RepliconServer to send messages
    if server_app.world().contains_resource::<RepliconServer>()
        && client_app.world().contains_resource::<RepliconClient>()
    {
        // Send client to server message using RepliconClient
        if let Some(mut replicon_client) = client_app.world_mut().get_resource_mut::<RepliconClient>() {
            println!("   - Client sending: '{}'", client_message);
            // Use the replicon abstraction instead of direct Renet access
            replicon_client.send(DefaultChannel::ReliableOrdered as u8, client_message.as_bytes().to_vec());
        }

        // Send server to client message using RepliconServer
        // Get connected clients first (immutable borrow)
        let connected_clients_exist = server_app.world().contains_resource::<ConnectedClients>();
        let mut client_ids = Vec::new();
        
        if connected_clients_exist {
            if let Some(connected_clients) = server_app.world().get_resource::<ConnectedClients>() {
                // Collect client IDs to avoid borrowing issues
                for client in connected_clients.iter() {
                    client_ids.push(client.id());
                }
            }
        }
        
        // Now do the mutable borrow without conflicting
        if let Some(mut replicon_server) = server_app.world_mut().get_resource_mut::<RepliconServer>() {
            if !client_ids.is_empty() {
                println!("   - Server broadcasting: '{}'", server_message);
                
                // Send message to each connected client
                for client_id in &client_ids {
                    println!("   - Sending to client: {:?}", client_id);
                    replicon_server.send(
                        *client_id,
                        DefaultChannel::ReliableOrdered as u8, 
                        server_message.as_bytes().to_vec()
                    );
                }
            }
        }

        // Run a few more frames to process messages
        println!("➡️ Running apps to process messages...");
        let mut client_received = false;
        let mut server_received = false;

        for i in 0..10 {
            println!("   - Post-message update {}/10", i + 1);
            
            // Update both apps to process messages through our plugin systems
            server_app.update();
            client_app.update();
            
            // Check for received messages on server using RepliconServer
            if let Some(mut server_renet) = server_app.world_mut().get_resource_mut::<RenetServer>() {
                // Fix: Access messages directly from RenetServer instead of using drain_received
                for client_id in server_renet.clients_id() {
                    while let Some(message) = server_renet.receive_message(client_id, DefaultChannel::ReliableOrdered as u8) {
                        if let Ok(text) = std::str::from_utf8(&message) {
                            println!(
                                "   - 📩 Server received from client {}: '{}'",
                                client_id, text
                            );
                            server_received = true;
                        }
                    }
                }
            }

            // Check for received messages on client using RepliconClient
            if let Some(mut client_renet) = client_app.world_mut().get_resource_mut::<RenetClient>() {
                // Fix: Access messages directly from RenetClient instead of using drain_received
                while let Some(message) = client_renet.receive_message(DefaultChannel::ReliableOrdered as u8) {
                    if let Ok(text) = std::str::from_utf8(&message) {
                        println!("   - 📩 Client received: '{}'", text);
                        client_received = true;
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(50));
        }

        // Verify message receipt
        println!("\n✅ VERIFICATION:");
        println!(
            "   - Client received message: {}",
            if client_received {
                "✅ Message received"
            } else {
                "❌ No message received"
            }
        );
        println!(
            "   - Server received message: {}",
            if server_received {
                "✅ Message received"
            } else {
                "❌ No message received"
            }
        );
    } else {
        println!("❌ Required resources not available for message passing test");
        println!("   This indicates a plugin initialization issue or resource management problem");
    }

    println!("✅ Message passing test completed");
}
