[← Back to README](../README.md)

# Server Architecture and Networking Plan for Sidereal

## Overview of the Architecture

Sidereal's backend is split into two main server roles: a **Replication Server** and multiple **Shard Servers**. This design supports a vast, unbounded 2D world by partitioning simulation work (physics and game logic) and centralizing networking and persistence. Below is a summary of each role:

- **Replication Server:** The authoritative gateway for clients. It manages all client connections (authentication, messages), maintains a **global view** of the world state (especially the subset visible to each client), and synchronizes that state out to players. It also handles **persistence**, applying world updates to a Supabase (PostgreSQL) database for saving game state. The replication server does minimal or no physics; it primarily routes data between shard servers and clients and enforces game rules (to prevent cheating). It knows which entities are in which sector and which clients should receive which updates.
    
- **Shard Servers:** Each shard server is responsible for a region of the game world (one or more **sectors** of size 1000×1000 units). Shards run the **Bevy ECS** and **Avian2D** physics simulation for their sector, computing all game logic (movement, collisions, combat, AI, etc.) within that area. Shard servers are authoritative over the entities in their sector: they advance the state of those entities and detect events (like destruction or crossing sector boundaries). After each simulation tick, shards report state changes to the replication server. Shard servers connect to the replication server on startup, receive their assigned sector(s) and initial entities (from the database or replication server), then continually send updates (position, velocity, health, etc.) to the replication server. They might also receive control commands or new entity spawn instructions forwarded from the replication server (e.g., a player action or a missile entering from a neighboring sector).
    
This architecture allows the system to **scale horizontally**. More shard servers can be added to cover additional sectors as the world (or player population) grows. The replication server ensures clients have a consistent view of the world by merging updates from all shards and sending each player only the relevant subset of entities (those in proximity or in the same sector). The Supabase database ties into this by storing persistent data (e.g. entity definitions, last known positions, player inventories) so that shards or the replication server can load state on startup or when needed.

**Key challenges addressed by this plan:**

- **High-frequency Entity Synchronization:** Ships, missiles, asteroids, and other entities move and change rapidly. We need a networking solution that can sync hundreds or thousands of entity updates per second to many clients with minimal latency. We'll use **UDP** for these updates, as it allows sending frequent, lightweight packets without handshake overhead. Loss of an occasional packet is acceptable for positional updates (the next update will correct the state).
    
- **Reliable Commands & Persistence:** Certain messages (player inputs, chat, or events like "spawn this entity" or **persist this item to DB**) must arrive reliably and in order. For these we use **TCP** or reliability layers on top of UDP. This ensures critical commands or state (like a ship purchase or a sector transfer event) are not lost. We will design a hybrid protocol that leverages both UDP and TCP (or reliable UDP channels) to get the benefits of each.
    
- **Modularity and ECS Compatibility:** We want to integrate networking seamlessly with the Bevy ECS (Entity-Component-System) framework. Systems should send and receive network messages as part of the Bevy schedule, and possibly leverage existing Bevy networking plugins. We will choose Rust crates that are Bevy-friendly and allow treating network messages as just another event or component in the ECS. The solution should allow dropping in networking as a plugin without heavily modifying game logic code.
    
- **Scalability:** The design must accommodate growth in player count and world size. This means supporting multiple shard servers, load balancing their work, and possibly running servers on separate machines or containers. It also means the networking layer should handle dozens or hundreds of connections efficiently and be able to broadcast updates to many clients. We will discuss how to containerize these servers (e.g. using Docker) and manage them (orchestrating shards, possibly via Kubernetes), and strategies like dynamic shard allocation or load-based sector splitting in the future.

## Networking Stack: UDP/TCP Hybrid with Rust

To meet Sidereal's networking requirements, we propose using a **hybrid UDP/TCP strategy** implemented with Rust networking libraries specialized for game development. Below we outline the approach and recommend specific crates to use:

- **UDP for Real-Time State (Unreliable):** Use UDP for the _high-speed, continuous replication_ of entity states (positions, velocities, etc.). UDP allows sending packets at a high frequency (e.g. 20-60 times per second) with low overhead and does not stall if a packet is lost. We acknowledge that UDP packets may be dropped or arrive out of order; the game will tolerate this for transient state updates (the next update will correct any divergence). The system will not spend time retransmitting lost position updates – instead, newer data always supersedes older data. This is crucial for fast-paced action where outdated updates are not useful.
    
- **TCP (or Reliable UDP channels) for Critical Data:** Simultaneously, use reliable channels for important messages: player input commands, game events (like "fire weapon" or "take damage"), chat messages, or any transaction that must be persisted. These can go over a TCP connection or using a reliability layer on top of UDP. The advantage of TCP is simplicity – it guarantees delivery and ordering – which is important for database writes or game logic triggers. The downside is potential latency from head-of-line blocking, so we will keep the TCP channel usage minimal (only for infrequent or non-time-critical events). Many modern game networking libraries implement reliability on UDP to avoid opening a separate TCP socket; we can leverage that for an integrated solution.
    
- **Unified Networking Library (Recommended):** We recommend using **Renet** (with the Bevy plugin **bevy_renet**) as the core networking library. Renet is a Rust network library designed for fast-paced games. It operates over UDP and provides multiple message channels with different delivery guarantees (unreliable, reliable ordered, reliable unordered). This fits perfectly with the hybrid approach: we can define an unreliable channel for state updates and a reliable channel for critical events, all on a single UDP port. Renet also handles packet fragmentation (so large messages can be sent if needed) and encryption/authentication (important for security in a large-scale game). By using bevy_renet, we get an easy integration into Bevy's ECS: the plugin will give us `RenetServer` and `RenetClient` resources for the replication server and clients/shards respectively, and we can add systems to send/receive messages through those. **Renet is built for performance**, supporting hundreds of clients with low overhead, and is battle-tested for FPS-style games.
    
- **Alternative Networking Crates:** For completeness, consider other crates:
    
    - **bevy_quinnet:** A networking plugin using QUIC (an UDP-based reliable protocol). QUIC can be beneficial if Web clients are a target (since QUIC is web-friendly via WebTransport). Using QUIC means we get reliable delivery by default, but it also supports _unreliable datagrams_ for real-time data. `bevy_quinnet` provides a Bevy plugin wrapper over the pure Rust QUIC implementation (Quinn). This is an option if we wanted to consolidate on a single protocol (QUIC) for both reliable and unreliable needs, or to ease browser support. However, Renet with WebTransport (via its renet2/steam integration) can also handle web, so QUIC is not strictly necessary unless we prefer its standardized nature.
        
    - **Laminar:** A legacy crate offering a "semi-reliable" UDP protocol (used by the Amethyst engine). It allows sending unreliable, reliable, or ordered messages over UDP. While Laminar was pioneering for Rust game networking, Renet largely supersedes it in functionality and performance. If one were not using Renet, Laminar could be used with Bevy's old networking plugin (`bevy_prototype_networking_laminar`), but that is outdated compared to the Renet ecosystem.
        
    - **NAIA:** A high-level networking framework for interactive applications, with Bevy support. NAIA is cross-platform (native and web) and aims to make multiplayer networking "dead-simple & lightning-fast". It provides its own architecture: you define a **protocol** of messages and **replicated component** types, and NAIA syncs those between client and server with options for reliability. NAIA could be an alternative to using bevy_replicon (discussed below) for ECS replication. It has Bevy adapters (`naia_bevy_client`, `naia_bevy_server`, etc.) that integrate with Bevy's `World`. NAIA handles tick rates, component replication, etc., but it requires a specific setup (e.g. deriving `Replicate` on components and using a shared protocol spec). We might consider NAIA if we want an all-in-one solution, but given that the user is already working with Supabase and custom serialization, it might be more flexible to proceed with our own replication logic or Replicon.
        
    - **Aeronet:** A newer suite of Bevy-native networking crates (by aecsocket). Aeronet provides low-level building blocks (connections as entities, sending/receiving by mutating components) and supports multiple transports (WebSockets, WebTransport/QUIC, Steam, etc.). It doesn't provide high-level replication itself (explicitly leaving replication/prediction as user responsibility), but it does integrate with bevy_replicon (via `aeronet_replicon`). Aeronet could be useful if we wanted fine control or to support Web clients with WebTransport seamlessly, but it's a bit lower-level than Renet. For now, Renet's out-of-the-box features suffice, but Aeronet is something to watch for future networking improvements.

## Entity State Replication Protocol

A key part of the plan is designing the **messaging protocol** for sending entity updates between the shard servers, replication server, and clients. The goal is to **batch and compactly encode** the game state changes each tick to minimize bandwidth while keeping all parties in sync.

### Batching and Update Messages

Rather than sending one network message per entity, the servers will batch updates for many entities into a single message when possible. For example, each shard server tick will produce an **"Update Packet"** containing the new state of all relevant entities in its sector that changed. Likewise, the replication server will bundle multiple entities' updates destined for a client into a single packet per tick. Batching reduces overhead (each packet has IP/UDP headers, so fewer packets means less header overhead) and improves throughput.

We will define **structured message types** for these updates. Using Rust's type system and serialization, we can create structs like:

```rust
use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct EntityUpdate {
    id: Uuid,                     // Unique entity identifier (same across shard, server, client)
    position: (f32, f32),         // Could use a Vec2; sending as two f32
    velocity: (f32, f32),
    rotation: f32,                // orientation or angular velocity if needed
    health: Option<u16>,          // example of other component data (Option means only present if changed)
    // ... other components of interest ...
}

#[derive(Serialize, Deserialize)]
struct SectorUpdate {
    sector: (i32, i32),           // The sector coordinates this update is for
    entities: Vec<EntityUpdate>,  // All entity states in this sector for this tick
}
```

Each `EntityUpdate` carries the minimal state needed to represent an entity's dynamic state. We include an `id` (using a UUID or similar globally unique ID for the entity) so the replication server and clients know _which entity_ this update refers to. The other fields are components that often change: here position and velocity are included for movement. We can extend this with any other replicated components (e.g., maybe shield level, current action, etc.), or use separate message types for different kinds of entities.

A `SectorUpdate` groups all `EntityUpdate` from one shard's region for a tick. If a shard manages multiple sectors, it could send one combined update (with multiple sectors in one message, or separate messages per sector). Since the architecture currently suggests one shard = one sector, we might not need the `sector` field in every message (the shard identity could imply it), but including it can help verification and flexibility (e.g., if we allow shards to take on extra sectors dynamically).

**Serialization:** We will serialize these structs into binary data for sending. **Bincode** is a great choice for quick, compact serialization of Rust structures; it has low overhead and is easy to use (just `bincode::serialize(&packet)`). Bincode will encode the floats and UUID to bytes without extra fluff (unlike JSON which would be text). Alternatively, we could consider a more specialized format (like bit-packing certain fields, or using Cap'n Proto or FlatBuffers for schema-defined binary encoding). To keep it simple at first, bincode (or Serde's postcard) is sufficient and performant for our needs. For large messages, we should be mindful of UDP packet size – by batching we might approach or exceed the typical MTU (~1200 bytes for internet-safe UDP). Renet will fragment and reassemble larger packets if needed, but it's still wise to keep updates lean. We can refine the data (e.g., send smaller types or delta-compress values) if bandwidth becomes an issue.

**Message Frequency:** We will likely send these `SectorUpdate` messages at a fixed rate (the replication **tick rate**). It's inefficient and unnecessary to send updates every single engine frame if the frame rate is very high. A typical approach is to run a network update at, say, 10, 20, or 30 times per second. Replicon (if used) supports setting a tick policy for replication (e.g., fixed ticks). We can similarly schedule our manual send system with Bevy's `FixedUpdate` or a timer. For instance, we might target 20 Hz updates (50 ms interval) for network sync – this is a compromise between smoothness and bandwidth use. If the game is very fast (bullets etc.), 20 Hz might be low, so perhaps 30-60 Hz for critical nearby entities. We can adjust as needed or even use LOD: high-frequency for nearby, low-frequency for distant.

### Reliable Messages for Events/Commands

In parallel to the streaming state updates, we will define message types for events and commands that require reliable delivery. For example:

- **Control commands:** When a player presses a key or triggers an action, the client sends a command (e.g., "thrust on", "fire weapon at X angle") to the server. These should be reliable so that the server definitely receives the player's intention. We might have a message struct like `PlayerCommand { player_id, action: ActionType, ... }`. These go from client to replication server (which then routes to the appropriate shard that the player's ship is on).
    
- **Spawn/despawn events:** When an entity is created or removed in the world, we want clients to eventually know about it. If a missile is fired or a ship explodes, a `SpawnEntity` or `DespawnEntity` event could be sent. We can handle this via state (the entity will appear/disappear in the next state update anyway), but sending an explicit event might make it faster or ensure none are missed. These events should be reliable (especially despawn, so an entity doesn't "ghost" on the client).
    
- **Persistence triggers:** If the game logic decides to save something to the database (e.g., player docked at a station and we save their inventory), the replication server might send an internal message to a DB handler system. We can treat DB writes as reliable tasks (though they're not really part of client networking). The result of a DB write (success/failure) might be communicated back to the client reliably (maybe via the same TCP channel or a separate ack message).

## Efficient ECS Replication Between Servers

Now we describe how to efficiently propagate changes in the ECS from the shard servers to the replication server (and then out to clients), using the networking structures above. The guiding principle is **server-authoritative state**: shard servers are the source of truth for their sector's entities, and the replication server is the source of truth for what clients see.

### Change Detection and Sending Deltas

Each shard server will run the game simulation for its sector. After each physics tick (or a group of ticks), the shard needs to determine what has changed and send that to the replication server. We can leverage Bevy's ECS change detection to avoid sending unchanged data:

- Bevy marks components as "changed" if they were mutated since the last time they were checked. We can use a query like `query.iter_changed()` for components like `Transform` (for position) or any relevant component to gather only entities that moved or updated.
    
- Alternatively, we can store the last sent state of each entity and diff against the current state, but using Bevy's built-in change tracking is simpler to start.
    

For example, on a shard we might have a system:

```rust
fn collect_entity_updates(
    mut net: ResMut<RenetClient>, // Renet client connected to replication server
    query: Query<(&sidereal::ecs::components::id::Id, &Transform, &LinearVelocity), Changed<Transform>>,
) {
    let mut updates = Vec::new();
    for (id, transform, vel) in query.iter() {
        let pos = (transform.translation.x, transform.translation.y);
        let vel = (vel.0.x, vel.0.y);
        updates.push(EntityUpdate {
            id: id.0, 
            position: pos, 
            velocity: vel,
            rotation: 0.0,    // if we had rotation component, include it
            health: None,     // health not changed here, so skip
        });
    }
    if updates.is_empty() {
        return;
    }
    let sector_update = SectorUpdate {
        sector: CURRENT_SECTOR, 
        entities: updates,
    };
    // Serialize and send via unreliable channel
    let packet = bincode::serialize(&sector_update).unwrap();
    net.send_message(CHANNEL_UNRELIABLE, packet);
}
```

On the **Replication Server** side, a corresponding system receives these updates:

```rust
fn handle_shard_updates(
    mut net: ResMut<RenetServer>, 
    mut commands: Commands,
    mut world: ResMut<WorldMap>, // hypothetical mapping from Uuid to Bevy Entity
    mut query: Query<(&sidereal::ecs::components::id::Id, &mut Transform, &mut LinearVelocity)>,
) {
    // Iterate over all connected shard servers (identified by their client id in RenetServer)
    for shard_client_id in net.clients_id() {
        // We use a while loop to drain all pending messages from this shard this tick
        while let Some(message) = net.receive_message(shard_client_id, CHANNEL_UNRELIABLE) {
            if let Ok(sector_update) = bincode::deserialize::<SectorUpdate>(&message) {
                for ent_update in sector_update.entities {
                    let eid = ent_update.id;
                    // Look up if this entity already exists in our replication world
                    if let Some(entity) = world.entities.get(&eid) {
                        // Update existing entity components
                        if let Ok((_, mut transform, mut velocity)) = query.get_mut(*entity) {
                            transform.translation.x = ent_update.position.0;
                            transform.translation.y = ent_update.position.1;
                            velocity.0.x = ent_update.velocity.0;
                            velocity.0.y = ent_update.velocity.1;
                            // (If other components like health present and changed, update them too)
                        }
                    } else {
                        // If not existing, this is a new entity (perhaps created on shard)
                        // Spawn it in the replication server ECS:
                        let new_entity = commands.spawn((
                            sidereal::ecs::components::id::Id(eid), 
                            sidereal::ecs::components::sector::Sector{ x: sector_update.sector.0, y: sector_update.sector.1 },
                            Transform::from_xyz(ent_update.position.0, ent_update.position.1, 0.0),
                            LinearVelocity(Vec2::new(ent_update.velocity.0, ent_update.velocity.1)),
                            bevy_replicon::prelude::Replicated,  // marker to replicate to clients
                        )).id();
                        world.entities.insert(eid, new_entity);
                    }
                }
            }
        }
    }
}
```

This design ensures the replication server maintains an **ECS mirror** of the active game world. It does not run physics or game logic on these entities; it simply holds their latest state as reported by shards. Because it's an ECS, we can use Bevy queries and systems (or Replicon) to efficiently manage and send data to clients. Each entity has the same `Id` as on the shard, so even if the Bevy `Entity` indices differ, we treat the UUID as the primary identifier for consistency across network and DB.

### Using bevy_replicon for Client Sync

We should strongly consider using **bevy_replicon** on the replication server to automate sending the world state to clients. `bevy_replicon` is a server-authoritative networking framework that hooks into Bevy ECS. It can monitor entities with a `Replicated` component and send their components to connected clients, with configurable tick rates and even per-client visibility control.

How this would work:

- We add `RepliconPlugins` to the replication server app, along with a messaging backend integration (there is `bevy_replicon_renet` maintained by replicon's authors that works with bevy_renet). This setup will create a `RepliconServer` resource and allow us to register which components to replicate.
    
- We tag all entities that should be networked with the `Replicated` component (as done in the spawn above). We also ensure all their relevant components (Transform, etc.) are either `Reflect` or serializable. Replicon will take snapshots of these and send to clients at each tick.
    
- We can configure replicon's tick to perhaps 20 Hz (or matching our network tick).
    
- **Visibility filtering:** Not every client should receive every entity. Sidereal's universe is huge, and a player in sector (10,10) doesn't care about an asteroid in sector (-5,-8). Replicon provides a low-level API for per-client entity visibility, and an extension crate `bevy_replicon_attributes` that makes it easier to manage conditions for what each client sees. We can use this to implement interest management: essentially, tie each client to a "current sector" or view range, and mark entities as visible to that client only if within some range (e.g., the same sector or neighboring sectors). For example, if a client's ship is in sector (X,Y), we could make all entities with a `Sector` component in [X±1, Y±1] visible to that client. The replicon plugin would then only replicate those to that client. This prevents wasting bandwidth on distant objects.
    
- Replicon will handle sending only diffs of changes if configured, to minimize data. It likely uses a similar delta mechanism internally (only changed components since last tick are sent, not the entire entity state every time, unless configured otherwise).
    
- On the client side, if we use replicon, we'd have `RepliconClient` which automatically applies the updates to the client's ECS world, spawning entities or updating components as needed. This saves us from writing a lot of manual client sync code.

## ECS System Layout for a Minimal Implementation

To implement this architecture, we'll structure our Rust project into at least two binaries (server and shard, and possibly a separate client binary for testing). Here's a suggested breakdown and system layout:

### Shared Code and Components

First, define a **shared library crate** (e.g. `sidereal_shared`) that both server and shards (and client) will use. This crate will contain:

- **Component definitions:** Position/Transform, Velocity, Sector, etc., so that they are consistent. For example, using Bevy's `Transform` for position is fine, or define our own lightweight `Position(f32, f32)` component for network clarity. We'll also define the `Id` component (likely wrapping a `Uuid`) and any other gameplay components that need to be known across network boundaries.
    
- **Network message structs:** `EntityUpdate`, `SectorUpdate`, `PlayerCommand`, etc., and implement `Serialize, Deserialize` (with Serde) for them. We can also include the channel constants (e.g., `const CHANNEL_UNRELIABLE: u8 = 0; const CHANNEL_RELIABLE: u8 = 1;`) so both sides use the same indices.
    
- If using replicon or NAIA, any required trait implementations or protocol definitions would go here as well (for replicon, mostly marking components as Reflect/Serialize).
    

This shared crate ensures the server and shard use the exact same data formats. The presence of this crate aligns with how NAIA or replicon would require a shared definition of components/messages.

### Shard Server Systems

In the **shard server** binary:

```rust
fn main() {
    App::new()
      .add_plugins(MinimalPlugins)
      .add_plugin(Avian2dPhysicsPlugin) // pseudocode for physics
      .add_plugin(GameLogicPlugin)      // your game systems
      .add_plugin(bevy_renet::RenetClientPlugin)
      .insert_resource(RenetClient::new(client_config, socket)) // configured to connect to replication server
      .add_systems(Update, apply_player_commands)    // handle input from server (reliable channel)
      .add_systems(Update, game_logic_systems)       // movement, AI, etc.
      .add_systems(PreUpdate, receive_server_messages) // process incoming network events early
      .add_systems(PostUpdate, collect_entity_updates) // after state updated, send out changes
      .run();
}
```

### Replication Server Systems

In the **replication server** binary:

```rust
fn main() {
    App::new()
      .add_plugins(MinimalPlugins)
      .add_plugin(bevy_renet::RenetServerPlugin)
      .add_plugin(bevy_replicon::RepliconServerPlugin) // hypothetical, plus backend
      .insert_resource(RenetServer::new(server_config)) // bound to UDP socket
      .add_systems(Update, handle_new_connections)   // assign sectors to shards, initialize clients
      .add_systems(Update, handle_shard_updates)     // apply shard messages to ECS
      .add_systems(Update, handle_client_commands)   // forward input from clients to shards
      // Replicon's own systems will run to replicate to clients based on our world state
      .run();
}
```

### Supabase Persistence

While the question focuses on networking, a quick note on how Supabase (Postgres) fits in:

- The **entities** table (as seen by the SQL dump) holds the current state of each entity (position, components serialized, etc.). The replication server can update this table periodically. A straightforward approach: every few seconds, or on certain events, the replication server writes the state of entities to the DB. For example, when a shard sends an update, the replication server could mark those entities as "dirty" and a background thread or timer could batch-update the DB with the new positions. We might not want to write every tick (that would be too slow and unnecessary), but maybe once a second per entity or when it leaves active area.
    
- Alternatively, shards could directly write to Supabase when they finalize an event (like an asteroid's resource count changed). But centralized through replication server ensures consistency and avoids multiple writers.
    
- On startup, the replication server can load static world data from the DB (like all NPC stations, etc.) and then distribute them to shards. Or shards request from replication as needed ("I have no data for sector X, give me everything").
    
- Supabase also offers real-time listeners, but that's more for clients via WebSockets; our architecture doesn't rely on that for the game simulation, since we have our own real-time channel.

## Scalability and Future Improvements

The proposed architecture is inherently scalable. Here are strategies and considerations for future scaling beyond the MVP:

### Docker Deployment

Containerizing the replication server and shard servers is highly recommended. Each shard server is essentially identical code (just parameterized by sector ID). We can bake one Docker image for "sidereal-shard" and run multiple containers with an env variable like `SECTOR_X`, `SECTOR_Y` to specify what to load. The replication server is another service with its own image. Using Docker Compose, we could define the replication service and a few shard services for testing. In a production environment, Kubernetes or another orchestrator can manage these containers, allowing dynamic scaling (launching new shard instances as the world expands).

For example, if a new region of space becomes active (players moved there), a new shard server container can be started to handle that region. The replication server would register it and assign the sector. If a shard goes down or needs to restart, the replication server could detect its disconnect and either hand off its entities to another shard or pause that region until a replacement comes up (using the DB state to restore).

### Load Balancing and Multiple Replication Servers

The replication server, at some point, could become a bottleneck if thousands of players connect to a single instance. Because it's handling all networking to clients, its outgoing bandwidth and CPU usage for processing updates is heavy. To scale further, one could introduce multiple replication servers, each responsible for a subset of sectors or players. This starts to resemble a multi-region MMO: e.g., sectors [(-10,-10) to (0,0)] on replication server A, and (1,1) to (10,10) on server B. Shards connect to the respective replication server that manages their region. If a player crosses regions, you'd transfer them between replication servers (a complex but solvable handoff, similar to shard handoff but at a higher level).

Another approach is to use a UDP proxy or mesh network: projects like **Quilkin** (by Embark) provide a UDP proxy for game servers. Quilkin can sit in front of the replication server to handle things like load balancing or filtering. But in our design, replication is stateful, so a simple proxy is not enough to offload load; splitting the actual server responsibilities is necessary for true scaling.

### Dynamic Sharding Strategy

The current shard strategy is by fixed sectors. This is easy to reason about, but note that load may not be evenly distributed. One sector might have 100 players in a big battle (very heavy load on that shard), while others are empty. In the future, consider **dynamic sharding**: the ability to split a hot sector into two shard processes (perhaps dividing the sector spatially or by entity type). This is a hard problem (it's essentially dynamic load balancing in space), but some games use "sub-shards" or spawn temporary instances for crowded areas.

Also, if a sector is empty, the shard could shut down to save resources. The replication server can note which sectors are active. Shards could be started on demand (and load state from DB) when a player enters an empty sector. Serverless or on-demand container platforms could make this automatic, though with some spin-up latency.

### State Consistency and Network Optimization

As we add more shards and possibly more layers, keeping state consistent is important. The design is server-authoritative, so consistency is easier (no conflicting edits from two sources). However, network latency will mean that clients are always slightly behind the true state and commands take time to propagate. We might need to implement:

- **Client-side prediction:** When a player presses forward, the client could immediately move their ship locally (prediction) while the command goes to server and comes back. If the server corrects the position, the client smoothly interpolates.
    
- **Lag compensation:** The replicon ecosystem has a crate `bevy_replicon_snap` for snapshot interpolation and client prediction, which could be explored to improve the feeling of responsiveness.
    
- **Entity interpolation:** If updates are 20Hz, the client can interpolate or extrapolate positions between updates to make motion smooth. Libraries like Glam or CGMath can help with simple vector interpolation.
    
- **Interest management optimization:** Further refine how we determine what entities each client needs to know about. Perhaps use spatial indexing or more sophisticated visibility algorithms.

### Security Considerations

With a large-scale game, cheat prevention is important:

- The server-authoritative model means clients cannot directly modify world state except via allowed commands.
    
- Validate all inputs: ensure a player's fire command comes at a valid rate, or a movement command does not teleport the ship.
    
- Use encryption (Renet with Netcode) to prevent packet tampering or snooping.
    
- Implement proper authentication (Supabase for user accounts, and the replication server only accepts connections with a valid session token).

### Monitoring and Testing

In a deployed environment:

- Monitor network usage and performance metrics (updates/sec, packet sizes, etc.).
    
- Use logging and metrics (Prometheus, etc.) to detect overloaded shards or replication server lag.
    
- Implement comprehensive testing:
  - Simulate numerous clients to test system load
  - Run multiple shard processes to test replication merging
  - Unit test the ECS components and systems
  - Integration test the handoff mechanisms

### Future Extensibility

The system is modular and can be extended:

- Networking layer can be swapped (e.g., replace Renet if needed)
- Physics is encapsulated in Avian2D
- Database layer is independent
- Could add caching (e.g., Redis for quick shard spawning)
- Might add matchmaking or server-discovery services
- Consider WebSocket/WebTransport support for web clients

By following this plan, we achieve a robust starting point: a server architecture that splits responsibilities between shards (computation) and a replication server (network fan-out and persistence), using proven Rust crates for networking. The modular design makes it easier to expand and maintain. As Sidereal grows, this architecture can evolve to meet higher demands, ensuring the universe and its battles remain seamless and responsive for all players.
