[← Back to README](../README.md)

# Server Architecture and Networking Plan for Sidereal

## Overview of the Architecture

Sidereal's backend is split into two main server roles: a **Replication Server** and multiple **Shard Servers**. This design supports a vast, unbounded 2D world by centralizing authority and distributing simulation work. Below is a summary of each role:

- **Replication Server:** The authoritative server for all game state. It:
    - Maintains the primary Bevy ECS world state
    - Manages all client connections (authentication, messages)
    - Handles persistence to Supabase (PostgreSQL)
    - Validates and processes state change requests from shards
    - Replicates entity state to relevant shards and clients
    - Enforces game rules and prevents cheating
    - Knows which entities are in which sector
    
- **Shard Servers:** Each shard server is responsible for running physics and game logic simulation for a region of the game world (one or more **sectors** of size 1000×1000 units). Shards:
    - Receive replicated copies of entities in their sector from the replication server
    - Run the **Bevy ECS** and **Avian2D** physics simulation
    - Send state change requests back to the replication server (e.g., "entity X should move to position Y")
    - Do not maintain authoritative state - they only simulate and propose changes
    - Can handle multiple sectors if needed for load balancing

This architecture allows the system to **scale horizontally** while maintaining a single source of truth. More shard servers can be added to handle additional sectors as the world (or player population) grows. The replication server ensures consistency by being the sole authority for game state, while shards distribute the computational load of physics and game logic simulation.

**Key challenges addressed by this plan:**

- **High-frequency Entity Synchronization:** Ships, missiles, asteroids, and other entities move and change rapidly. We need a networking solution that can sync hundreds or thousands of entity updates per second to many clients with minimal latency. We'll use **UDP** for these updates, as it allows sending frequent, lightweight packets without handshake overhead. Loss of an occasional packet is acceptable for positional updates (the next update will correct the state).
    
- **Reliable Commands & Persistence:** Certain messages (player inputs, chat, or events like "spawn this entity" or **persist this item to DB**) must arrive reliably and in order. For these we use **TCP** or reliability layers on top of UDP. This ensures critical commands or state (like a ship purchase or a sector transfer event) are not lost. We will design a hybrid protocol that leverages both UDP and TCP (or reliable UDP channels) to get the benefits of each.
    
- **Modularity and ECS Compatibility:** We want to integrate networking seamlessly with the Bevy ECS (Entity-Component-System) framework. Systems should send and receive network messages as part of the Bevy schedule, and possibly leverage existing Bevy networking plugins. We will choose Rust crates that are Bevy-friendly and allow treating network messages as just another event or component in the ECS. The solution should allow dropping in networking as a plugin without heavily modifying game logic code.
    
- **Scalability:** The design must accommodate growth in player count and world size. This means supporting multiple shard servers, load balancing their work, and possibly running servers on separate machines or containers. It also means the networking layer should handle dozens or hundreds of connections efficiently and be able to broadcast updates to many clients. We will discuss how to containerize these servers (e.g. using Docker) and manage them (orchestrating shards, possibly via Kubernetes), and strategies like dynamic shard allocation or load-based sector splitting in the future.

## Networking Stack: Updated Approach with Renet2

To meet Sidereal's networking requirements, we'll use a hybrid approach combining renet2 for raw networking with bevy_replicon_renet2 for client-facing ECS replication:

- **Client-Replication Server Communication:** We'll use **bevy_replicon_renet2** (v0.7.0) to handle all communication between game clients and the replication server. This library provides seamless integration between Replicon's ECS replication framework and renet2's networking capabilities. Using Replicon here gives us automatic entity replication, client prediction, and other game-networking features with minimal code.

- **Replication Server-Shard Server Communication:** For communication between the replication server and shard servers, we'll use **renet2** (v0.7.0) directly with **bevy_renet2** (v0.7.0) for Bevy integration. This approach gives us more control over the exact data being transmitted between servers, without the overhead of Replicon's full entity replication system which isn't needed for server-to-server communication.
    - **Client:** The shard server connects as a client using `RenetClient` and `NetcodeClientTransport`. It requires the `RenetClientPlugin` and `NetcodeClientPlugin` from `bevy_renet2` to be added to the app.
    - **Server:** The replication server listens for shards using a separate `RenetServer` and `NetcodeServerTransport` instance.

- **Replication Server-Shard Server Communication:** For communication between the replication server and shard servers, we'll use **renet2** (v0.7.0) directly with **bevy_renet2** (v0.7.0) for Bevy integration. This approach gives us more control over the exact data being transmitted between servers, without the overhead of Replicon's full entity replication system which isn't needed for server-to-server communication.

- **Client-facing Server:** Uses `bevy_replicon_renet2` for game clients. This server will handle entity replication to clients using Replicon's automatic ECS synchronization.
    - **Important Plugin Interaction:** `bevy_replicon_renet2`'s `RepliconRenetPlugins` internally adds the core `bevy_renet2` server plugins (`RenetServerPlugin`, `NetcodeServerPlugin`). This is crucial because explicitly adding these core plugins again for the shard-facing server will cause a Bevy panic due to duplicate plugin registration.

- **Shard-facing Server:** Uses `renet2`/`bevy_renet2` directly for communication with shard servers. This server will use custom message types for efficient server-to-server communication.
    - **Manual Management:** Because Bevy only allows one instance of each resource type (e.g., `RenetServer`, `NetcodeServerTransport`), and the client-facing server uses the standard resources managed by `RepliconRenetPlugins`, the shard-facing server components *must* be managed manually. This involves:
        1. Defining a custom resource, e.g., `ShardListener { server: RenetServer, transport: NetcodeServerTransport }`.
        2. Creating the `RenetServer` and `NetcodeServerTransport` for shards in a `Startup` system, packaging them into `ShardListener`, and inserting *that* resource.
        3. Adding a custom `Update` system (e.g., `manual_shard_server_update`) that accesses `ResMut<ShardListener>` and manually calls `transport.update(delta, &mut server)` and `server.update(delta)`.
        4. Adapting event/message handling systems (like `handle_server_events`) to access the server via the `ShardListener` resource instead of `ResMut<RenetServer>`.

This dual-server approach is necessary because:
- The client and shard connections require different handling (Replicon ECS replication vs. direct messaging)
- Security and authentication needs differ between client and shard connections
- Using separate servers allows for different network configurations (port numbers, packet rates, etc.)
- It provides cleaner separation of concerns and code organization

Both servers can run within the same Bevy application on the replication server.

### Networking Message Types and Serialization

For shard-replication server communication, we'll define custom message types that are optimized for server-to-server communication:

```rust
use serde::{Serialize, Deserialize};
use uuid::Uuid;

// Channels for renet2 communication between replication and shard servers
const SHARD_CHANNEL_UNRELIABLE: u8 = 0;
const SHARD_CHANNEL_RELIABLE: u8 = 1;

#[derive(Serialize, Deserialize)]
enum ShardToReplicationMessage {
    EntityUpdates(Vec<EntityUpdate>),
    SpawnRequest { entity_type: EntityType, position: (f32, f32) },
    DespawnNotification(Uuid),
    // Other message types as needed
}

#[derive(Serialize, Deserialize)]
enum ReplicationToShardMessage {
    InitializeSector { sector_id: (i32, i32), entities: Vec<EntityInitData> },
    EntityAdded(EntityData),
    EntityRemoved(Uuid),
    PlayerCommand { player_id: Uuid, command_type: CommandType, data: Vec<u8> },
    // Other message types as needed
}

#[derive(Serialize, Deserialize)]
struct EntityUpdate {
    id: Uuid,
    position: (f32, f32),
    velocity: (f32, f32),
    rotation: f32,
    // Other dynamic state fields
}

// ... other struct definitions
```

We'll use `bincode` (v2.x) for efficient binary serialization of these messages. Because our messages include external types like `uuid::Uuid` which provide `serde` compatibility via feature flags (but not direct `bincode::Encode`/`Decode` implementations), we must:
1. Enable the `serde` feature for the `bincode` crate itself in `Cargo.toml`.
2. Enable the `serde` feature for dependencies like `uuid` in `Cargo.toml`.
3. Use the `bincode::serde::*` functions (e.g., `bincode::serde::encode_to_vec`, `bincode::serde::decode_from_slice`) for serialization/deserialization, rather than the main `bincode::encode_*`/`decode_*` functions or the direct `bincode::Encode`/`Decode` derive macros on our message types.
4. Ensure our message types derive `serde::Serialize` and `serde::Deserialize`.

## Entity State Replication Protocol

The updated architecture modifies how we handle entity replication:

### Client-Replication Server Replication

For client-replication server communication, we'll use bevy_replicon_renet2 which handles entity replication automatically:

1. Entities on the replication server that should be visible to clients are marked with Replicon's `Replicated` component
2. Replicon automatically serializes and sends entity state to clients based on visibility rules
3. Client-side, Replicon spawns and updates entities based on this received data

### Replication Server-Shard Server Communication

For replication server-shard server communication, we'll implement a custom messaging protocol using renet2:

1. Shard servers will collect entity updates after each physics/game logic tick
2. These updates will be batched into `ShardToReplicationMessage::EntityUpdates` messages and sent to the replication server
3. The replication server will process these updates and apply them to its ECS world
4. For new entities or entities moving between sectors, the replication server will send appropriate initialization data to shards

This approach gives us more control over exactly what data is sent between servers and allows for more efficient communication than using Replicon's full entity replication system.

## ECS System Layout for the Implementation

### Replication Server Systems

The replication server will need systems for both client and shard communication:

```rust
fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        // Client-facing networking with Replicon
        .add_plugins(RepliconPlugins)
        .add_plugins(RepliconRenetPlugins)
        // Shard-facing networking: Core plugins are added by RepliconRenetPlugins above.
        // We add our custom plugin containing the ShardListener resource and update systems.
        .insert_resource(setup_shard_server_config()) // Custom function to set up shard-facing server
        // Remaining resources and systems...
        .add_systems(Update, handle_shard_messages)
        .add_systems(Update, forward_client_commands_to_shards)
        .add_systems(Update, update_entity_visibility)
        // Replicon handles client entity replication automatically
        .run();
}

fn handle_shard_messages(
    mut shard_listener: ResMut<ShardListener>, // Access manually managed server
    mut commands: Commands,
    mut world_entities: ResMut<WorldEntityRegistry>,
    mut query: Query<(&Id, &mut Transform, &mut Velocity)>,
) {
    // Process messages from all connected shards
    let server = &mut shard_listener.server;
    for client_id in server.clients_id() {
        while let Some(message) = server.receive_message(client_id, SHARD_CHANNEL_UNRELIABLE) {
            if let Ok(ShardToReplicationMessage::EntityUpdates(updates)) = bincode::deserialize(&message) {
                for update in updates {
                    // Apply entity updates to the ECS world
                    // This will automatically propagate to clients via Replicon
                    // ...
                }
            }
            // Handle other message types...
        }
    }
}
```

### Shard Server Systems

The shard server will connect to the replication server as a client:

```rust
fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(Avian2dPhysicsPlugin)
        .add_plugins(GameLogicPlugin)
        // Connect to replication server
        .add_plugins(RenetClientPlugin)
        .add_plugins(NetcodeClientPlugin) // Handles RenetClient updates
        .insert_resource(setup_replication_client_config()) // Custom function to set up client
        // Systems
        .add_systems(PreUpdate, receive_replication_messages)
        .add_systems(Update, game_logic_systems)
        .add_systems(PostUpdate, send_entity_updates_to_replication)
        .run();
}

fn send_entity_updates_to_replication(
    mut replication_client: ResMut<RenetClient>,
    query: Query<(&Id, &Transform, &Velocity), Changed<Transform>>,
) {
    let mut updates = Vec::new();
    
    for (id, transform, velocity) in query.iter() {
        updates.push(EntityUpdate {
            id: id.0,
            position: (transform.translation.x, transform.translation.y),
            velocity: (velocity.0.x, velocity.0.y),
            rotation: transform.rotation.z,
            // Other fields...
        });
    }
    
    if !updates.is_empty() {
        let message = ShardToReplicationMessage::EntityUpdates(updates);
        let bytes = bincode::serde::encode_to_vec(&message, bincode::config::standard()).unwrap(); // Use bincode::serde
        replication_client.send_message(SHARD_CHANNEL_UNRELIABLE, bytes);
    }
}
```

## Scalability and Future Improvements

The updated architecture maintains all the scalability benefits of the original design while providing more efficient server-to-server communication:

1. **Separate Network Stacks:** By using different networking approaches for client and shard communication, we can optimize each for its specific needs.

2. **Efficient Server-to-Server Communication:** Direct renet2 messaging between replication and shard servers allows us to fine-tune exactly what data is transmitted, minimizing bandwidth usage.
    - The use of `bincode` (via its `serde` module) provides efficient binary serialization. Ensuring compatible versions of dependencies like `uuid` (e.g., v1.12 for Bevy 0.15) with the necessary `serde` feature enabled is important.

3. **Flexible Deployment:** The replication server can handle both client and shard connections independently, allowing for separate scaling strategies if needed.

### Docker Deployment and Scaling

The deployment strategy remains unchanged - containerizing both replication and shard servers with appropriate configuration. Since the communication protocol is now custom, we have more flexibility to optimize how sectors are assigned and how entities are transferred between shards.

### Security Considerations

With direct renet2 communication between servers, we should implement proper authentication for shard servers connecting to the replication server. This can be done using renet2's authentication features, potentially with pre-shared keys or certificates for shard servers.

## Conclusion

By using renet2 directly for server-to-server communication while keeping bevy_replicon_renet2 for client-server communication, we achieve a more efficient and flexible networking architecture. This approach gives us fine-grained control over the exact data being transmitted between servers while still providing clients with the benefits of a high-level entity replication system.

The replication server will run two separate renet2 servers - one for clients using Replicon and one for shards using direct messaging. This dual-server approach provides clean separation of concerns and allows each network stack to be optimized for its specific use case.
