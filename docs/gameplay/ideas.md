This guide covers the architecture and best practices for creating a multiplayer **top-down 2D space MMO** in Rust using **Bevy 0.15**. We will design a server-authoritative system with a central replication server and multiple shard servers, leveraging Bevy’s ECS and community networking crates. Topics include the high-level architecture, networking stack (using **bevy_replicon** and **bevy_replicon_renet2**), serialization strategy (with **Bincode** and Bevy’s Reflect), delta-state synchronization, update loop timing for physics vs networking, shared project structure, and tips for error handling and debugging. Code examples and references to community resources are provided throughout.

## 1. Architecture Design: Replication Server and Shard Servers

In an MMO architecture, a single server cannot handle the entire world if it’s very large or populated. We use a **replication server** as the authoritative coordinator and **shard servers** for scaling the simulation load. The world can be partitioned (for example, by regions of space or zones) and each shard simulates physics and game logic for its region​

. The replication server maintains the unified world state and communicates with shards and clients. Here’s the breakdown:

- **Replication Server (Central Coordinator):** This server holds the _authoritative world state_ and is the one all game clients connect to. It loads the initial world (e.g. star systems, ships, etc.) from persistent storage (such as Supabase, which can serve as a Postgres database) and spawns those entities in its ECS. It handles **persistence** (saving world state back to Supabase periodically or on shutdown) and **replication**: broadcasting state changes to clients. The replication server does minimal physics; instead it trusts shard servers for simulation. Think of it as a world state manager that knows about all entities and their components, ensuring clients see a consistent, authoritative state.
    
- **Shard Servers (Regional Simulators):** Each shard server is responsible for a subset of the world (e.g. a sector of space). Shards run the **physics simulation** and game mechanics for entities in their region. We can use the **avian2d** physics engine on each shard, which integrates with Bevy’s ECS. Shards operate at a fixed tick rate (e.g. 60 Hz) to simulate movement, collisions, etc. The shard servers **send delta updates** (state changes) to the replication server whenever entities move or change in their region. The replication server then applies those updates to its world state and replicates them out to clients. This offloads expensive physics calculations from the central server to multiple shard processes.
    
- **Communication Between Servers:** The replication server and shard servers communicate over the network as well (server-to-server messages). The replication server can instruct shards to spawn or despawn entities (for example, if a player moves into a new region, an entity might transfer to a different shard). Conversely, shards report back component changes (position, velocity, health, etc.) to the replication server. We’ll use an ECS-friendly networking approach here too, reusing our client-server networking stack for inter-server messages. In practice, each shard can connect to the replication server similar to a client (but with special privileges), or we can set up dedicated channels for server-to-server messaging. The important idea is that **all state changes funnel into the replication server**, which is the single source of truth that clients listen to. This ensures consistency even though multiple shards are simulating in parallel.
    

_Illustration of a client-server architecture:_ The replication server (top) processes game logic and broadcasts periodic state updates (“syncs”) to all connected clients. Clients send input (or in our case, shard servers send state changes) back to the server, which resolves the authoritative state.

- **Supabase Integration:** Supabase (backed by PostgreSQL) can be used to persist game data such as the world’s state, player inventories, etc. On startup, the replication server might fetch a saved snapshot of the world from Supabase (for example, stored as a binary blob or as rows of entity components) and reconstruct the ECS state. During runtime, the replication server could periodically save snapshots or specific events to Supabase for persistence. Using Supabase’s realtime capabilities isn’t strictly necessary since our replication server handles realtime sync, but Supabase is useful as a durable storage. For implementation, you could serialize the world state using the techniques in the Serialization section and upload it via Supabase’s REST API or a direct Postgres connection. Ensure that this persistence layer is decoupled from the game loop (e.g., run saves on a separate thread or at intervals to avoid blocking gameplay).

**Bevy ECS Design:** The replication server and shards will each be Bevy apps running without rendering (headless mode). They will share common **component types** and logic (defined in a shared crate, see section 6). Entities in the world (spaceships, projectiles, planets, etc.) will exist on the replication server (for state tracking) and on one shard (for physics), but we will avoid duplicate simulation. Only shards simulate movement; the replication server’s copy of an entity is updated only when a shard sends a message. We might give each entity a component indicating which shard “owns” it (so the replication server knows where to forward certain messages or which shard is authoritative for that entity). The replication server can also enforce rules or do high-level coordination (like matchmaking players to shards, or enforcing game rules globally).

**Summary:** This architecture is effectively a **server-authoritative multi-server** setup. The replication server is authoritative to clients (clients never directly trust shard data; everything comes via the central server), which prevents cheating and ensures a single authority​

. Shard servers are _delegated authority_ for physics – the replication server trusts their computations for movement and collisions in their region. This structure allows horizontal scaling: you can run multiple shard servers (possibly on different machines or containers) to scale with player load, while keeping a single point that clients connect to for a seamless experience. If needed, you can even partition players across multiple replication servers (each with its own shards) for different universes or use cases, but within one game world, one replication server simplifies consistency.
## 2. Networking Stack: bevy_replicon and Renet for Client-Server and Inter-Server Communication

We will use Bevy’s ECS-oriented networking libraries to handle communication. The primary crate is **bevy_replicon**, a high-level server-authoritative networking framework that integrates with Bevy. Bevy Replicon automates a lot of the sync work: it can replicate entities/components from server to clients, and provides an event system for messages (client->server or vice versa)​

. However, Replicon by itself doesn’t handle the low-level transport – it’s transport-agnostic. We’ll pair it with **Renet** via the **bevy_replicon_renet2** crate, which uses UDP (with reliability features) and is compatible with Bevy’s runtime. Renet2 will handle the actual packet sending, while Replicon ensures our world state and events are synchronized.

**Client-Server Networking (Replicon + Renet):** In a server-authoritative model, the server sends world updates to clients, and clients send input or requests to the server. With bevy_replicon, **replication is one-directional** (server → client) for world state, to prevent cheating​

. Any data that needs to go from clients to server (e.g. player commands, or in our case shard updates to the replication server) is sent as **events** or messages. We configure Replicon on the replication server to replicate entities/components, and on the clients to receive those. Replicon will track which entities exist on the server, which components have changed, and construct network packets to send the diffs to each client automatically.

- **Bevy Replicon Setup:** To add Replicon, use the provided plugins. For example, on the **server** (replication server binary), you might do:

    ``use bevy::prelude::*; use bevy_replicon::prelude::*; use bevy_replicon_renet2::RepliconRenetPlugins;  fn main() {     App::new()         .add_plugins(MinimalPlugins) // no default window         .add_plugins(RepliconPlugins)          .add_plugins(RepliconRenetPlugins)          .add_startup_system(setup_network)         .run(); }  fn setup_network(mut commands: Commands, channels: Res<RepliconChannels>) {     // Configure Renet networking (UDP sockets, channels, etc.)     use bevy_replicon_renet2::renet2::{RenetServer, NetcodeServerTransport, ConnectionConfig};     // Generate Renet channel config from Replicon (ensures channels match on client & server)     let connection_config = ConnectionConfig::from_channels(         channels.server_configs(),          channels.client_configs()     );     // Create the Renet server socket (listening on some port)     let transport = NetcodeServerTransport::new(/* ip, port, private key, etc */);     let server = RenetServer::new(connection_config, /* current_time */ 0.0, transport);     // Insert Renet resources so the network will start listening     commands.insert_resource(server); }``
    
    On the **client** (game client binary), you would similarly add `RepliconPlugins` and `RepliconRenetPlugins`, then create a `RenetClient` and `NetcodeClientTransport` resource pointing to the server’s address. The `RepliconRenetPlugins` automatically include the necessary Renet systems and will integrate with Replicon’s **RepliconServer** and **RepliconClient** resources internally​
    
    
    . You should **not** add both server and client resources in the same app, as that causes a loop (one Bevy app should either be a server or a client)​
    
    [docs.rs](https://docs.rs/bevy_replicon_renet2#:~:text=Just%20like%20with%20regular%20,40%20resources%20from%20Renet)
    
    . In our case, the replication server runs the server plugin, shards might run as clients (to connect to the replication server), and the player clients run as clients.
    
- **Renet Channels and Messaging:** Renet (via `bevy_renet2`) allows defining multiple channels for different kinds of data (reliable, unreliable, ordered, etc.). The beauty of `bevy_replicon_renet2` is that it sets up these channels to match Replicon’s needs. After registering all components and events for replication, you obtain a `RepliconChannels` resource which knows how many channels are needed and of what kind (e.g. an unreliable channel for state updates, reliable for events). We use `ConnectionConfig::from_channels(channels.server_configs(), channels.client_configs())` to create the Renet config that both server and client use​
    
    [docs.rs](https://docs.rs/bevy_replicon_renet2#:~:text=fn%20init%28channels%3A%20Res,client_configs%28%29%2C)
    
    . This ensures the server and clients have a consistent channel layout. For example, Replicon might use one channel for “entity update ticks” (unreliable, since newer updates replace older ones) and another for “event messages” (reliable, so none are lost). The code above shows how we pass those configs to the RenetServer.
    
- **Inter-Server Messaging:** We can leverage the same system for shard→replication server communication. Essentially, each shard server can **act as a “client”** connecting to the replication server using Replicon/Renet. The replication server will see shard servers as just another connection (we might identify them by a special client ID or a flag). Because Replicon’s replication is normally server→client, the shard (as a client) won’t receive world data except what we allow. We can configure **visibility** so that shards perhaps receive only a minimal subset (or nothing) from the replication server. For instance, the replication server could hide all entities from shard connections (since shards already have their own copy of their region’s entities), and we use this connection primarily for shards to send events upstream. Using Replicon’s event system, shards can send messages to the server reliably. Replicon allows defining custom event types marked with the `Event` trait that can be sent from client to server​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=,struct%20DummyEvent)
    
    . We would create events for things like “PositionChanged” or “PhysicsState” that include the entity (or an ID) and new component data. On the shard, we write to an `EventWriter<FromClient<PhysicsUpdate>>` (Replicon wraps client->server events in a `FromClient<E>` type) and on the replication server we handle those events to update the world state.
    
    For example, define an event and register it on both client (shard) and server:
    
    rust
    
    CopyEdit
    
    `use bevy_replicon::prelude::*; #[derive(Debug, Deserialize, Serialize, Event)] struct PhysicsUpdate {     entity: u64,       // using u64 for entity ID that both shard and server know     position: Vec2,     velocity: Vec2, } // On shard (client side), send event when a physics tick changes something: fn report_changes(     query: Query<(Entity, &Transform, &Velocity), Changed<Transform>>,     mut events: EventWriter<ToServer<PhysicsUpdate>> ) {     for (entity, transform, velocity) in &query {         events.send(ToServer::new(PhysicsUpdate {             entity: entity.to_bits(), // convert Entity to u64 ID             position: transform.translation.truncate(),             velocity: velocity.0,         }));     } }`
    
    On the replication server, we would listen for `PhysicsUpdate` events and apply them:
    
    rust
    
    CopyEdit
    
    `fn apply_physics_events(     mut events: EventReader<PhysicsUpdate>,     mut query: Query<(&mut Transform, &mut Velocity)> ) {     for update in events.iter() {         if let Some(entity) = Entity::from_bits(update.entity) {             if let Ok((mut transform, mut velocity)) = query.get_mut(entity) {                 transform.translation.x = update.position.x;                 transform.translation.y = update.position.y;                 velocity.0 = update.velocity;             }         }     } }`
    
    Here `ToServer<T>` is a wrapper from Replicon that marks an event to be sent to the server (Replicon will ensure it’s delivered)​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=)
    
    . We convert Entities to a global ID (bits) to identify them across servers – an alternative is to use Replicon’s entity mapping system, but that’s more involved. This approach assumes the replication server assigns a globally unique ID to each entity (which Bevy’s Entities can provide if the server spawns them and communicates the ID to shards). The main idea is that **shards only send events for changes**, and the replication server applies them.
    
- **Server-to-Server sync vs Direct Replication:** Another design could be the replication server not storing the full world, and instead _relaying_ updates between shards and clients. However, maintaining the state on the replication server has advantages: it can validate and filter updates, and persist or inspect them easily. It effectively acts as a **world proxy** or hub (sometimes called an “interest management server” in MMO terms). The networking stack remains mostly the same as standard client-server, except that some “clients” are actually shard servers in disguise.
    
- **Networking Library Choices:** We choose **Renet** because it’s a battle-tested UDP networking library for Rust games, and **bevy_replicon_renet2** because it cleanly integrates with Bevy’s ECS (no manual serialization for each message type, it works with our components and events). Alternatives include **QUIC** (e.g. bevy_quinnet) or WebTransport for web builds, which Replicon also supports via different backend crates​
    
    [github.com](https://github.com/projectharmonia/bevy_replicon/blob/master/README.md#:~:text=)
    
    . The pattern remains similar regardless of transport. For example, if we wanted browser clients, we could use `bevy_replicon_quinnet` similarly. Renet was chosen here as it’s widely used and efficient.
    
- **Code Sample – Spawning and Replicating Entities:** Once networking is set up, using Replicon is straightforward. You mark entities and components to replicate. For instance, when the replication server spawns a new spaceship entity:
    
    rust
    
    CopyEdit
    
    `let entity = commands.spawn((     PlayerShip { /* ... */ },     Transform::default(),     Velocity(Vec2::ZERO),     Replicated,            // marker component from Replicon )).id();`
    
    You would ensure that `PlayerShip`, `Transform`, and `Velocity` components are set to replicate to clients. In Replicon, you do:
    
    rust
    
    CopyEdit
    
    `app.replicate::<Transform>()    .replicate::<Velocity>()    .replicate::<PlayerShip>(); #[derive(Component, Reflect, Serialize, Deserialize)] #[reflect(Component)]  struct PlayerShip { /* fields */ }`
    
    By calling `app.replicate::<T>()` for each component type, Replicon knows to include that component’s data in the sync messages​

    
    . Any entity with the `Replicated` marker will be considered for replication, and only those whitelisted component types will be serialized and sent​

    . On the client side, when a new entity is replicated, it will automatically get spawned with those components and also marked `Replicated` client-side. This drastically simplifies the amount of manual networking code – we don’t have to send position updates ourselves; Replicon handles it as long as we mark the component as replicable and its value changed.
    
- **Ensuring Network Data Flows:** The replication server should broadcast out to clients at a fixed rate (tick rate) rather than every frame (more on this in section 5). Replicon uses an internal **tick counter** and only sends state when the tick increments​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=)
    
    . We can configure this tick to increment on a timer (for example, 20 times per second). This means even if our server runs at 60 FPS, network packets might be sent at ~20 FPS, grouping the changes. We’ll discuss how to configure and balance this soon.
    

In summary, using **bevy_replicon + bevy_replicon_renet2** provides a robust ECS-native networking stack. It covers:

- Automatic replication of entities/components (server → client).
    
- Event-driven messaging (client → server, or server → client triggers) for things like inputs or inter-server updates.
    
- Built-in support for multiple channels (reliable/unreliable) and integration with Renet’s transport layer.
    
- Flexibility to integrate shards as “headless clients” feeding into the system.
    

By following the patterns above, our networking code stays high-level and ties neatly into the ECS. This reduces boilerplate significantly (compared to manually serializing every message and managing sockets). The code samples above demonstrate setting up the plugins and sending/receiving a custom message. Always ensure that **both endpoints register the same components and events in the same order** in their Bevy app, which is easy if you centralize that in the shared crate (see section 6)​

[docs.rs](https://docs.rs/bevy_replicon#:~:text=Make%20sure%20that%20the%20component,code%20in%20your%20%E2%80%9Cshared%E2%80%9D%20crate)

.

## 3. Serialization Strategy: Bincode and Bevy Reflect for ECS Data

Serialization is crucial for sending data over the network and storing it in a database. We aim for a strategy that is **compact, efficient, and easy to maintain** as the game grows. The chosen approach is to use **Bincode** (a binary serialization format) in combination with Bevy’s **Reflect** system to automate as much as possible.

**Why Bincode?** Bincode is a binary format that works with Rust’s Serde. It produces a compact byte representation with very low overhead (no field names or whitespace like JSON) and has good performance. In an MMO, saving bandwidth is important; binary encoding is much smaller than JSON or text. Bincode is widely used for network messages in Rust games and is straightforward to integrate (just `bincode::serialize` and `deserialize`). It’s worth noting that the Bevy Replicon library by default uses **Postcard** for serialization​

[docs.rs](https://docs.rs/bevy_replicon#:~:text=By%20default%20all%20components%20are,feature%20on%20Bevy)

, which is another compact binary format similar to Bincode. Postcard is `#![no_std]` friendly and very minimal, but Bincode is equally viable for our use-case and might be easier to work with if we already use Serde elsewhere. The differences are minor (Postcard might yield slightly smaller output by not encoding sequence lengths the same way). You can actually choose to let Replicon keep using Postcard under the hood for networking (since it’s built-in), but for **our own serialization tasks (like saving to Supabase)**, using Bincode can unify with any custom data we have.

**ECS Component Serialization via Reflect:** One challenge is serializing ECS components, which are often custom types. We want a system where any new component we add can automatically be serialized without writing boilerplate for each. Bevy’s **Reflect** system provides runtime type information and (optionally) Serde support for types. By deriving `Reflect` for a component, and registering it with Bevy’s type registry, we can manipulate it dynamically. More concretely, Bevy’s `bevy_reflect` crate has a module `bevy_reflect::serde` that lets you serialize any `dyn Reflect` value if the type is registered with reflect data​

[docs.rs](https://docs.rs/bevy_reflect/latest/bevy_reflect/serde/index.html#:~:text=A%20general%20purpose%20deserializer%20for,de%29serialization%20of%20a%20type)

​

[docs.rs](https://docs.rs/bevy_reflect/latest/bevy_reflect/serde/index.html#:~:text=Traits%C2%A7)

. This is how Bevy saves scenes to files: it collects components into a `DynamicScene` (which holds `Reflect` representations of each entity’s components) and then serializes that via Ron or binary.

**Best Practice – Derive and Register:** For each component that needs to go over the network or into storage, do the following:

- Derive the Serde traits and Reflect:
    
    rust
    
    CopyEdit
    
    `#[derive(Component, Reflect, Serialize, Deserialize)] #[reflect(Component, Serialize, Deserialize)] struct Velocity(Vec2);`
    
    This ensures the component can be serialized with Serde (for Bincode/Postcard) and also is a reflectable type (which Bevy can work with dynamically). The `#[reflect(Serialize, Deserialize)]` part will generate reflect type data that knows how to (de)serialize the component using Serde if needed.
    
- Register the type in your app’s type registry (usually done in a startup system or plugin):
    
    rust
    
    CopyEdit
    
    `app.register_type::<Velocity>();`
    
    Now Bevy’s `TypeRegistry` knows about `Velocity` and how to serialize/deserialize it if it’s part of a scene or reflect serialization process.
    

Replicon will automatically use Serde (via Postcard) for any component that implements `Serialize`/`Deserialize`​

[docs.rs](https://docs.rs/bevy_replicon#:~:text=By%20default%20all%20components%20are,feature%20on%20Bevy)

. If a component doesn’t implement Serde, Replicon cannot include it by default. In our strategy, we plan to derive Serde for all components that need networking. This means **most of the time, you get Bincode/Postcard support “for free” just by deriving**. For example, if you add a new `Shield` component to ships, you add `Serialize, Deserialize` to its derive and register it, then call `app.replicate::<Shield>()` on the server – now it will sync over the network automatically.

**Automating Future Components:** By following the pattern of deriving the necessary traits on every component, you ensure that any future component is ready to serialize. You generally won’t need to manually implement `serde::Serialize` by hand – the derive does it. One thing to watch: if your component contains non-serializable fields (like a `HashMap` without Serde or a pointer), you’d need to mark those to skip or handle them. In a game, most components are simple data (numbers, vectors, small structs), which Serde can handle easily. Additionally, using Reflect’s dynamic serialization means even components that don’t implement Serde could be handled by custom logic, but that’s an advanced scenario. Replicon does allow customizing serialization per type if needed, using `AppRuleExt::replicate_with` to plug in your own (de)serialization function​

[docs.rs](https://docs.rs/bevy_replicon#:~:text=If%20your%20component%20doesn%E2%80%99t%20implement,you%20can%20use%20AppRuleExt%3A%3Areplicate_with)

– for example, you might compress a `f32` into an `i16` across the network to save bandwidth (quantization). But unless needed, relying on Serde + Bincode is simpler and less error-prone.

**Using Bevy Reflect to Save/Load State:** For persistence (Supabase), you might want to save the entire world state periodically. Bevy can help by capturing a snapshot of the world via `DynamicScene`. The Replicon crate even provides a utility: `bevy_replicon::scene::replicate_into(&world, &mut dynamic_scene)` which will fill a `DynamicScene` with all entities marked for replication and their components​

[docs.rs](https://docs.rs/bevy_replicon#:~:text=This%20pairs%20nicely%20with%20server,restore%20the%20correct%20game%20state)

. You can then serialize that `DynamicScene`. `DynamicScene` implements Serde (it uses reflect internally to serialize all the components). For example:

rust

CopyEdit

``use bevy::scene::DynamicScene; // Suppose `world` is a reference to your Bevy World on the replication server let scene = DynamicScene::from_world(&world, world.query_filtered::<Entity, With<Replicated>>().iter());  // (Bevy also has SceneSpawner, but here we directly make a DynamicScene) let bytes = bincode::serialize(&scene).expect("Failed to serialize scene"); // Now `bytes` can be stored in Supabase (e.g., as a binary column or file)``

When loading, you could do the reverse:

rust

CopyEdit

`let scene: DynamicScene = bincode::deserialize(&bytes_from_db).unwrap(); scene.write_to_world(&mut world);`

This would spawn all entities and components from the scene into your world. Using `replicate_into` from Replicon can filter to only replicated entities, which might be what you want to persist (you might not want to save non-replicated runtime stuff). This approach means **future components are automatically included** in the save, as long as they were marked Replicated and have Reflect/Serde. You don’t have to manually update a save function for each new component type – `DynamicScene` + reflect handles it generically.

- _Can Reflect automate Bincode serialization?_ – Yes, indirectly. If a type is reflectable with Serde data (as shown with `#[reflect(Serialize, Deserialize)]`), you can treat a `&dyn Reflect` as something that implements `serde::Serialize`. The Bevy reflect API provides a type called `ReflectSerializer` which wraps a reflect value and serializes it using any Serde `Serializer`​
    
    [docs.rs](https://docs.rs/bevy_reflect/latest/bevy_reflect/serde/index.html#:~:text=A%20general%20purpose%20deserializer%20for,de%29serialization%20of%20a%20type)
    
    . Bincode works by implementing the Serde `Serializer` trait. So you could, if needed, do something like:
    
    rust
    
    CopyEdit
    
    `use bevy_reflect::serde::ReflectSerializer; let registry = world.resource::<AppTypeRegistry>().clone(); let type_registry = registry.read(); let reflect_value = any_component_value.reflect(reflect::Reflect); let serializable = ReflectSerializer::new(reflect_value, &type_registry); let bytes = bincode::serialize(&serializable).unwrap();`
    
    This is essentially what DynamicScene does internally for each component. Thus, Bevy’s reflect can act as a bridge to Serde without writing specific impls. There are crates like `bevy_reflect` (which is part of Bevy itself) that enable this. In practice, we don’t usually need to call `ReflectSerializer` ourselves if we use the Scene approach or Replicon’s built-in replication, but it’s good to know this exists.
    
- **Existing Tools/Crates:** Apart from Replicon and DynamicScene, there is also **naia** (another networking engine) which has a concept of a “shared” crate for definitions (similar to what we do) and uses auto-generated protocol mappings, but we’ve chosen Replicon for its closer integration with Bevy. There’s also `bevy_net` under development and things like `bevy_client_server_events` (event-based simple networking)​
    
    [crates.io](https://crates.io/crates/bevy_client_server_events#:~:text=bevy_client_server_events%20,about%20serialization%20or%20network)
    
    , but those often require manual handling. Our strategy with reflect and serde is fairly future-proof; as Bevy evolves, reflect/serde support is likely to improve (for example, Bevy might decouple reflection and serialization in the future to make it even more flexible​
    
    [github.com](https://github.com/bevyengine/bevy/issues/3664#:~:text=Decouple%20serialization%20and%20reflection%20%C2%B7,does%20not%20return%20an%20option)
    
    ).
    

**Serializing Custom Data:** Not everything is an ECS component. You might need to serialize player account data, or an inventory which isn’t stored as components (though you could store inventory as components too). For such data, since it likely implements Serde (or you can derive), using Bincode directly is fine. For example:

rust

CopyEdit

`#[derive(Serialize, Deserialize)] struct PlayerProfile { name: String, credits: u32, ships: Vec<ShipBlueprint>, /*...*/ } let profile_bytes = bincode::serialize(&profile).unwrap();`

You can store those in Supabase (perhaps in a separate table). The key is to use Serde consistently so that one serialization format (Bincode) can handle all your data types, whether they’re ECS components or standalone structs.

**Important Considerations:**

- **Endianess and Compatibility:** Bincode (and Postcard) produce data dependent on field order and type definitions. If you update a component’s structure, old serialized data might not deserialize correctly. This is similar to migrating a database – you may need versioning or migration code for old save data. For network, all players and server are running the same code version, so that’s not an issue (just ensure the client and server are built from the same code). For persistence, you might handle version by, for instance, storing a version number along with the blob and migrating as needed.
    
- **Bevy’s `serialize` Feature:** Note that some Bevy types (like `Transform`, `GlobalTransform`, etc.) only implement Serde when a feature flag is enabled. If you want to replicate Bevy’s built-in components like `Transform`, enable the `"serialize"` feature for the Bevy crates. Replicon’s documentation mentions this​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=By%20default%20all%20components%20are,feature%20on%20Bevy)
    
    . In your Cargo.toml, for example:
    
    toml
    
    CopyEdit
    
    `bevy = { version = "0.15", features = ["serialize"] }`
    
    This way `Transform` has `Serialize`/`Deserialize` and can be included in replication or scenes.
    
- **Floating Point Precision:** Bincode will serialize floats in their binary form (f32 as 4 bytes IEEE754). That’s fine, but remember that different architectures may have different endianness. Bincode by default is little-endian (on little-endian machines). If you ever need to send data between different architectures (rare for games, usually all x86-64 or WASM), consider configuring Bincode with a fixed endianness or using Postcard (which is always little-endian by spec). This is a minor detail but good to know.
    

By using Bincode + Serde derive on everything, and leveraging Bevy Reflect for dynamic cases, we achieve a **maintenance-free serialization layer**: you add new game components or data types and just derive the traits, and they become network-ready. This approach emphasizes **data-driven design**: your game’s data structures drive what gets replicated or saved, rather than writing lots of imperative code to pack/unpack fields.

## 4. Delta Synchronization: Sending Only Changes (Diffs) per Entity

In a game with potentially thousands of entities, sending the full state of each entity every tick would be wasteful. We need to send **only the differences (deltas)** – i.e., only the components that have changed since the last update. Our networking stack (Replicon) and Bevy’s ECS change detection will largely handle this for us, but let’s outline how to approach delta sync.

**Bevy Change Detection:** Bevy automatically tracks when a component is mutated using a “change tick.” If a system queries `Changed<Transform>` it will get only entities whose Transform was modified since last time that system ran. We can use this mechanism to know what to send. For example, in the shard server, we had a system reporting `PhysicsUpdate` events only for `Changed<Transform>` entities – that ensures we’re not spamming updates for stationary objects. Similarly, on the replication server, Replicon uses change ticks to decide what to include in each update tick.

**Replicon’s Delta Mechanism:** Bevy Replicon by default replicates component data only on change. It groups changes by tick. Internally, it likely compares the last sent state or uses Bevy’s detection – for efficiency, it doesn’t resend an unchanged component every tick. As evidence, Replicon has a `ConfirmHistory` for entities and notes that it doesn’t update the confirmation if an entity had no changes that tick (to avoid overhead)​

[docs.rs](https://docs.rs/bevy_replicon#:~:text=ConfirmHistory%20%20along%20with%20the,hasn%E2%80%99t%20changed%20for%20performance%20reasons)

. This implies if nothing about an entity changed, that entity might be skipped in that update. Only when you mutate a component or spawn/despawn an entity does it get included in the next network packet. This is exactly what we want: **bandwidth scales with activity, not with total entity count**. Idle objects cost almost nothing to sync beyond initial spawn.

- For example, if a spaceship is drifting with no input, its position might still change due to physics. But if it’s perfectly stationary and nothing about it changes, after the initial replication, no further updates are sent for it until something changes.
    

**Sending only changed components:** Suppose an entity has multiple components that we replicate (say `Transform`, `Velocity`, `Health`). If only `Transform` and `Velocity` change in a tick, but `Health` remains the same, a good system will send only the new Transform and Velocity. Replicon’s component-based replication does that – each component is tracked individually. It doesn’t resend the `Health` component’s value unless it changed. If using your own networking, you would mimic this by e.g. storing previous values or using change flags.

**Ensuring compatibility with new component types:** Because the delta system operates per component type generically, adding a new replicable component type automatically gains the same benefit. As long as you mark it for replication, Replicon will include it when it changes. There is no need to write special-case diff code for each new component. This is a strength of an ECS-based approach – you can iterate over all components of type T that changed and handle them uniformly.

**Server-to-Server Deltas:** In our shard update events, we also only send changes. We explicitly used `Changed<Transform>` in the query. For other data, we could do similar things. Alternatively, the shard might accumulate changes in a list and send them periodically. If using Replicon events as shown, it’s already event-driven (only sending when something changes triggers the event). Another strategy could be to send a **snapshot** of the shard’s world at some interval, but that’s less efficient if many things haven’t changed. So sticking to events on actual changes (or at a coarse interval) is better.

**Diffing Strategies:** There are a few common patterns for delta compression:

- **State-based diff**: Compare current state to last sent state and send differences (Replicon effectively does this via ticks).
    
- **Event-based**: Directly send an event when something happens (like a “took damage” event instead of sending health continuously). In our design, things like a projectile hitting a ship could be sent as an event (“ApplyDamage”) rather than continuous health values. Use events where appropriate to reduce needing to sync a value every frame.
    
- **Thresholds**: Only send a change if it’s significant (to avoid tiny changes flooding updates). E.g., if an entity rotates by 0.1 degrees which is negligible, maybe only send rotation if it changes more than some threshold. This is game-specific; for a space MMO, even small movements matter for accurate physics, so we might not threshold position, but we might throttle less critical data (like only update an AI ship’s target if it changes).
    

**Delta across the network boundaries:** We have two network hops potentially – shard to replication server, and replication server to clients. We should ensure that we’re not duplicating effort or missing updates:

- The shard sends an event for a change once. The replication server, upon receiving that, will mark the component as changed in its world (because we apply the new value). That will trigger Replicon to include that change in the next tick to clients. So the delta propagates naturally.
    
- If multiple changes happen within one server tick, Replicon might batch them into one update. For example, if a shard sends two position updates quickly for the same entity before the replication server’s next network tick, the server’s ECS will have the latest position, and Replicon will probably just send the final state at the tick. This is fine (the intermediate change doesn’t need to be sent if it was overwritten, unless you need intermediate for very high fidelity, which you usually don’t).
    
- Ensure that **component changes that happen only on the replication server** also get out. For instance, the replication server might add a component to an entity (like tagging a player as “in combat” or something); since that’s a change, Replicon will replicate it. If shards need to know about such changes, you’d have to also send them to the shard. That could be done via the visibility or by treating shards as clients that see those components. Replicon does allow per-client visibility filtering​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=)
    
    , so you could configure shards’ connections to _see_ certain admin components if needed.
    

**Best Practices for Delta Sync:**

- **Replicate only necessary components:** Don’t mark components as replicable if they can be derived or are not needed on clients. For example, physics collision shapes might not need to be on the client if the client only needs to render positions. By reducing what is replicated, you implicitly reduce data to send. Replicon supports a concept of “required components” to handle components that are needed to spawn an entity but not updated afterward, etc., which helps keep things clean​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=)
    
    . An example from the docs: you might replicate only a `Player` marker and `Transform`, and have the client insert a default `Mesh` locally (so you don’t send mesh data)​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=%2F%2F%20Replicate%20only%20transform%20and,Player%3E%28%29%20.add_observer%28init_player_mesh)
    
    . This keeps network delta smaller and ensures compatibility when new components are added (if they’re purely cosmetic, you might not replicate them at all).
    
- **Use efficient data types:** Large components (like a big struct of many fields) will send all those fields when changed, even if one sub-field changes (unless you break it into smaller components). Consider splitting data so that independent things are separate components, enabling finer-grained change detection. For instance, instead of one `ShipStatus { position, velocity, health, fuel }` component, use separate `Transform`, `Velocity`, `Health`, `Fuel` components. Then damage doesn’t cause the position to resend, etc.
    
- **Group changes per tick:** Send changes at a fixed tick (say 50ms). This naturally coalesces rapid changes. Replicon’s tick does this. If you roll your own, you’d do similarly: on each tick, gather all changes since last tick and send them in one packet per client.
    
- **Testing new components:** When you add a new component and mark it replicable, test that adding/changing it indeed results in network traffic. You can use logs or debug counters. For example, Replicon offers `NetworkStats` on each client entity which likely tracks bytes sent/received​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=Backends%20manage%20RepliconServer%20%20and,independent%20way)
    
    . Monitoring that can tell you if your changes are causing more data.
    

In summary, **delta synchronization is largely handled by the ECS and Replicon**. Our job as developers is to structure our data to take advantage of it and not to defeat it by doing things like sending redundant events. As long as we replicate components and only mutate them when necessary, the network will only carry the differences. This approach scales well: whether an update involves 10 entities or 1000, the cost is proportional to how many actually changed.

One should also consider a worst-case scenario: e.g., a big battle where hundreds of ships move and fight at once (so many changes). In that case, you may hit bandwidth limits. Techniques like interest management (only send nearby entities to each client) become important. Replicon allows per-client visibility filtering​

[docs.rs](https://docs.rs/bevy_replicon#:~:text=)

, meaning the server can flag which entities each client should know about. For instance, a player on one side of the galaxy doesn’t need updates about a far-away battle; the server can avoid replicating those entities to that client at all. This further reduces delta load per client. Implementing that involves adding a `ClientVisibility` component or using an attribute system (Replicon has an `attributes` extension crate) to define which client sees what. This is a more advanced optimization but crucial in MMO scaling.

## 5. Update Cycles and Performance Tuning

Managing the game’s update loops is crucial in a distributed system. We have a physics simulation running on shards, and network synchronization running on the server (and to some extent on shards for sending events). Balancing when these run, and at what frequency, affects both performance and the “feel” (smoothness, responsiveness) of the game.

**Physics Update Rate (Simulation Step):** We choose to run physics at a **fixed timestep**, e.g. 60 FPS (which is ~16.67 ms per physics frame). Running physics in a fixed timestep ensures the simulation is deterministic and stable, regardless of rendering framerate fluctuations. The Avian physics engine defaults to running in Bevy’s `FixedPostUpdate` schedule with a fixed timestep, precisely to avoid frame-rate dependent behavior​

[docs.rs](https://docs.rs/avian2d#:~:text=To%20produce%20consistent%2C%20frame%20rate,to%20visible%20stutter%20for%20movement)

​

[docs.rs](https://docs.rs/avian2d#:~:text=See%20the%20PhysicsInterpolationPlugin%20for%20more,information)

. This means in the shard server, the physics systems (like Avian’s `PhysicsStep`) will execute 60 times per second. If the machine can’t keep up, Avian can do multiple steps or skip steps to catch up, which might cause a slight stutter if not handled, but Avian provides interpolation components to smooth this​

[docs.rs](https://docs.rs/avian2d#:~:text=To%20produce%20consistent%2C%20frame%20rate,to%20visible%20stutter%20for%20movement)

.

- In practice, you’d add the Avian plugin which likely does:
    
    rust
    
    CopyEdit
    
    `app.add_plugins(PhysicsPlugins::default());`
    
    By default, as of Avian 0.2, it uses Bevy’s fixed timestep scheduling (which under the hood uses a sub-schedule that ticks at a fixed rate)​
    
    [docs.rs](https://docs.rs/avian2d/latest/avian2d/schedule/struct.Physics.html#:~:text=Physics%20in%20avian2d%3A%3Aschedule%20,the%20issue%20is%20by)
    
    . The default timestep might be 1/60s but can usually be configured. If your MMO needs less frequent physics (to save CPU), you could lower this, but typically 60 Hz is a good baseline for smooth movement.
    
- **Why fixed timestep?** Physics engines (and game logic like movement) often produce unstable or non-deterministic results if the delta-time varies. Fixed timestep gives consistent results. Moreover, in a networked context, using fixed ticks makes it easier to reason about synchronization (e.g., you can stamp each physics tick with a number). If you ever do client-side prediction or lag compensation, a fixed step is invaluable.
    

**Network Update Rate:** We generally do **network sends at a lower rate** than physics. Commonly, fast-paced games send state 10 to 20 times per second. Our goal is to reduce bandwidth but still have acceptable latency and smoothness. In our scenario:

- The **shard → replication server** updates (via events) could be sent at the full 60 Hz for critical things, but that might be overkill. We could throttle shard reports to, say, 30 Hz or 20 Hz, without much negative effect, because the replication server doesn’t need every single physics step if they are very fine-grained. However, one must be careful: if we throttle too much, the replication server’s representation could lag behind the shard’s actual state significantly. We could design the shard updates to send at a fixed rate (maybe every 2 or 3 physics steps). This can be done by scheduling the event-sending system in a fixed timer or using Bevy’s `FixedUpdate` with a different period.
    
- The **replication server → clients** updates definitely should be capped (sending every frame (60Hz+) to potentially hundreds of clients would be too much). Replicon’s tick system is perfect for this. By default, you might configure:
    
    rust
    
    CopyEdit
    
    `RepliconPlugins.build().set(ServerPlugin {     tick_policy: TickPolicy::Fixed(std::time::Duration::from_millis(50)),     ..Default::default() })`
    
    (The exact API might differ; in the docs they show `TickPolicy::Manual` and then you manually increment​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=On%20server%20current%20tick%20stored,runs%20when%20this%20resource%20changes)
    
    , but there’s likely a Fixed option too.) The idea is to have the `RepliconServer` tick every 50ms (20 Hz). When the tick changes, Replicon collects all changes and sends out packets​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=)
    
    . This aligns with best practices – many games use 10-20Hz update for state.
    
- **Interpolating on Clients:** With a 20 Hz update rate, clients will receive a new snapshot every 50ms. To render at 60+ FPS smoothly, the client should interpolate or extrapolate between updates. If you use something like **bevy_replicon_snap** (an add-on for snapshot interpolation)​
    
    [github.com](https://github.com/projectharmonia/bevy_replicon/blob/master/README.md#:~:text=)
    
    , it can automate that. Otherwise, you can simply have the client lerp object positions between the last known and current known state over those 50ms. Avian on the client side can help via its `TransformInterpolation` component (which it uses for visual smoothing when physics tick is lower than render FPS)​
    
    [docs.rs](https://docs.rs/avian2d#:~:text=To%20produce%20consistent%2C%20frame%20rate,to%20visible%20stutter%20for%20movement)
    
    ​
    
    [docs.rs](https://docs.rs/avian2d#:~:text=This%20stutter%20can%20be%20resolved,the%20%2057%20by%20default)
    
    . If we run no physics on client (client is just rendering), we can still leverage that concept: basically store previous transform and use interpolation. The main point is, by adjusting the update cadence and using techniques to mask it, we achieve a balance of network load and visual quality.
    

**Scheduling in Bevy:** Bevy 0.15 allows multiple schedules. Typically:

- Physics systems are placed in a fixed-time schedule (e.g., `FixedUpdate` or Avian’s `PhysicsSet::Step`). The Avian docs note they run in `FixedPostUpdate` by default at fixed timestep​
    
    [docs.rs](https://docs.rs/avian2d#:~:text=To%20produce%20consistent%2C%20frame%20rate,to%20visible%20stutter%20for%20movement)
    
    .
    
- Networking send/receive systems run in the normal update or in another fixed schedule. Replicon’s systems (processing incoming packets, sending outgoing) run each frame in `PreUpdate` or `PostUpdate` stages (the Renet integration likely updates in `PreUpdate` to read packets, and Replicon sends in `PostUpdate` after game logic, or on tick).
    

On the **shard server**, one approach:

- Run physics at 60 Hz.
    
- After each physics step (or on some steps), send out events for changes. We might put the event-sending system in the same fixed schedule right after physics step. For example, if using Bevy’s fixed update, you can specify ordering:
    
    rust
    
    CopyEdit
    
    `app.add_systems(     FixedUpdate,     (avian2d::step_physics_system, report_changes.after(avian2d::step_physics_system)) );`
    
    Where `report_changes` is the system that sends `PhysicsUpdate` events as in section 2. This way, every physics tick we immediately emit the changes. If that’s too frequent, you could instead use a counter or run `report_changes` every Nth tick. There’s a utility `run_if` in Bevy to conditionally run a system. e.g.,
    
    rust
    
    CopyEdit
    
    `.add_systems(FixedUpdate, report_changes.run_if(FixedTimer::new(Duration::from_millis(50))));`
    
    which would effectively send at 20Hz even though physics is 60Hz.
    

On the **replication server**, Replicon can internally use a fixed tick. In 0.15, Bevy introduced fixed timestep scheduling officially, so you might integrate Replicon’s tick with it. The docs show an example of using `TickPolicy::Manual` and then adding `bevy_replicon::server::increment_tick` to `FixedUpdate`​

[docs.rs](https://docs.rs/bevy_replicon#:~:text=On%20server%20current%20tick%20stored,runs%20when%20this%20resource%20changes)

. That implies:

rust

CopyEdit

`app.add_systems(FixedUpdate, bevy_replicon::server::increment_tick);`

and configure that FixedUpdate to say 20Hz. This will increment Replicon’s tick at 20Hz, triggering sends. Alternatively, `TickPolicy::Fixed(duration)` might automate it. Either way works.

**Performance Considerations:**

- **Server FPS:** The replication server doesn’t do heavy physics, so it could run at a fairly uncapped framerate (or just tick along with network). It will be mostly bound by how fast it can serialize and send packets. With Replicon, serialization is efficient (especially using postcard/bincode which are binary and fairly fast). If the CPU becomes a bottleneck (say thousands of entities changing, generating large packets), you might consider doing some work in parallel (Bevy can parallelize some systems). Replicon likely uses background threads for Renet (Renet spawns threads for socket by default), so sending doesn’t block the main thread too much.
    
- **Shard Performance:** Each shard simulating physics should be tuned so that 60 Hz is sustainable with the number of entities in that shard. If one shard area becomes too populated (and CPU can’t handle 60 Hz), you might subdivide that area into multiple shards (this ties into dynamic load balancing, possibly splitting regions – a complex topic outside scope, but interesting). Avian physics is ECS-driven and should scale, but monitor the frame times. Avian provides an FAQ on performance (like ensuring not too many contacts, etc.)​
    
    [docs.rs](https://docs.rs/avian2d#:~:text=Check%20out%20the%20GitHub%20repository,engine%E2%80%99s%20features%20and%20their%20documentation)
    
    .
    
- **Network Bandwidth:** At 20 updates per second, if each update per client is, say, 5 KB (just as an estimate for a moderately busy scene), that’s 100 KB/s per client. For 100 clients, that’s 10 MB/s outgoing from the server, which is high but maybe borderline on a server with a good network (80 Mbps). In practice, average update sizes might be smaller (if using interest management, each client only gets what they need). It’s important to measure. **Profiling network**: You can instrument with `NetworkStats` – for example, Replicon attaches a `NetworkStats` component to each `ConnectedClient` on the server which tracks bytes in/out, ping, etc., which you can log​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=Backends%20manage%20RepliconServer%20%20and,independent%20way)
    
    . That helps to tune tick rates or component priorities if needed.
    
- **Packet Coalescing:** Renet will batch all messages for a tick into one packet per client, which is good. It also handles fragmentation if a packet is too large for one UDP datagram. We should still be mindful to avoid extremely large packets. If you find that a single tick sometimes produces a huge amount of data (say a big battle starts and 500 entities suddenly have changes), you might consider splitting updates (e.g., send multiple ticks back-to-back). But ideally, interest management prevents any one client from ever receiving updates for all 500 at once if they’re not nearby.
    

**When to run what:**

- **Input handling:** On the client side (not a big focus of the question, but for completeness), you would capture player input (keyboard/mouse) every frame and send it to the server perhaps also at a fixed rate or immediately. If using Replicon events for input, it can be sent as soon as the event happens, or you can throttle inputs to, say, 60 Hz. Usually input is light data (like “thrust on” or “fire weapon”), so it’s fine to send frequently and handle on server.
    
- **Order of operations:** A typical server tick (for replication server) might be:
    
    1. Receive all incoming events from shards and clients (e.g., commands, physics updates).
        
    2. Apply those to the world state.
        
    3. Run any game logic systems that exist on the server (maybe minimal if shards do most logic).
        
    4. Increment replication tick (if it’s the right time) and let Replicon send out diffs.
        
    5. Loop.
        
    
    For shards:
    
    6. Receive any commands from server (possibly if server needs to instruct shard).
        
    7. Run physics step (which moves ships, etc.).
        
    8. Run game logic (AI, spawning projectiles, etc.) on the shard.
        
    9. Send out events for any results (to server).
        
    10. Loop at fixed rate.
        
- **Frame pacing and jitter:** If your physics tick and network tick are not multiples of each other, you might get minor jitter in updates. For instance, with physics at 16.7ms and network at 50ms, sometimes you’ll send after 3 physics updates, sometimes after 2 (since 3*16.7=50.1ms). Over long run it averages out. This is fine; any jitter can be smoothed with interpolation. If you want a perfectly consistent ratio, you could use 60Hz physics and 20Hz network (3:1 exactly), which 50ms vs 16.67ms is basically 3:1 with a tiny 0.1ms difference. That difference is negligible. So it’s effectively every 3 physics frames, send an update.
    
- **Time synchronization:** All servers should ideally have a concept of time or tick. Replicon likely uses a tick counter (which we drive). Shards might tag events with a timestamp or tick when an action happened. For example, if doing lag compensation, you might want to know at what shard tick an event (like a hit) occurred. However, a simpler approach: you might not need that unless doing client-side prediction/rollback. It’s mentioned that replicon ensures events are applied in order and eventually consistent​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=All%20events%2C%20inserts%2C%20removals%20and,order%20as%20on%20the%20server)
    
    , so we rely on that rather than manual time management.
    

**Performance Tips:**

- Use **profiling tools**: e.g., enable Bevy’s log for system run times (`RUST_LOG=bevy_ecs=trace` shows system durations) to catch any system that’s too slow.
    
- Enable **network logging** to debug performance: Replicon allows setting `RUST_LOG=bevy_replicon=debug` or `trace` to see a lot of info​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=%C2%A7Troubleshooting)
    
    . This can show messages about ticks, serialization errors, etc. It’s very useful when tuning – for instance, you might see a warning if a packet is too large or if a client falls behind.
    
- If CPU usage is high on the replication server, consider moving some calculations off the main thread. Because it’s ECS, you could use Bevy’s tasks for certain workloads. But typically, the replication server just copying data is not heavy; the heavier work is on shards (physics).
    
- If a shard cannot keep up 60Hz, as mentioned, split load or reduce complexity (e.g., lower physics accuracy, use simpler collision shapes, etc.). Avian is fairly new; if performance is a problem, another physics engine like Rapier could be considered, but Avian’s advantage is being ECS-native and possibly easier to integrate with reflect (Rapier has serialization too, though).
    

**Frequency of Supabase writes:** As part of performance, consider how often you persist to the database. Writing the entire world every tick is impossible; instead, maybe snapshot every 5 minutes or on server shutdown. Or log incremental events (like a log of state changes) to reconstruct if needed. Supabase writes should be asynchronous (don’t stall the game loop waiting for DB). Perhaps use a separate thread or an async task to push data, and throttle how often. The replication server can accumulate a list of changes and periodically flush to DB.

To summarize this section: **run physics regularly and frequently, run networking a bit less frequently, and use Bevy’s scheduling to coordinate the two.** By doing physics on shards at 60 Hz and networking at ~20 Hz, we get smooth simulation and efficient network usage. We carefully place systems so that network messages go out after physics updates, and we utilize interpolation on clients to hide the lower update rate. This yields a responsive game without saturating bandwidth. Always test and measure – if the motion appears choppy, consider increasing network rate or improving interpolation; if the bandwidth is too high, consider reducing rate or using more aggressive culling of what is sent.

## 6. Shared Project Structure: Organizing Core Logic and Types

When building a multiplayer game with separate client and server binaries (and in our case, shard server binaries), it’s best to organize the project as a **workspace with multiple crates**. This allows sharing code between client, server, and shards, while keeping each binary lean (e.g., the server doesn’t include graphics code, the client doesn’t include server-specific code). A common pattern is to have:

- a **core (shared) crate**,
    
- a **server crate** (for the replication server),
    
- a **shard crate** (could be combined with server crate if they’re very similar, but often separated for clarity),
    
- a **client crate**.
    

For example, the directory structure might be:

css

CopyEdit

`my_game/ ├─ Cargo.toml (workspace) ├─ core/  │   ├─ Cargo.toml │   └─ src/lib.rs ├─ server/ │   ├─ Cargo.toml │   └─ src/main.rs ├─ shard/ │   ├─ Cargo.toml │   └─ src/main.rs └─ client/     ├─ Cargo.toml     └─ src/main.rs`

**Core Crate:** The `core` crate is a library that contains all the game’s fundamental definitions:

- **Components:** e.g. `struct PlayerShip`, `struct Velocity`, etc. You define them here and derive `Component, Reflect, Serialize, Deserialize` as needed. This way, both server and client use the exact same component types (no duplication).
    
- **Events:** If you have custom event types (like our `PhysicsUpdate` or perhaps a `ChatMessage` event), define them in core as well and derive `Event, Serialize, Deserialize` so they can be used in networking.
    
- **Resources or Constants:** Any constant values (max players, world size) or resource structs that are shared can go here.
    
- **Plugin or Systems for Registration:** Often, you might create a `CorePlugin` in this crate that, when added to an App, registers all the types and maybe adds common systems. For example, `CorePlugin` could contain:
    
    rust
    
    CopyEdit
    
    `pub struct CorePlugin; impl Plugin for CorePlugin {   fn build(&self, app: &mut App) {       // Register reflect types for all components and events       app.register_type::<PlayerShip>()          .register_type::<Velocity>()          .register_type::<Health>();       // Add replication rules (only on server, maybe conditionally compiled)       #[cfg(feature = "server")]       {           app.replicate::<PlayerShip>()              .replicate::<Velocity>()              .replicate::<Health>();           app.add_event::<PhysicsUpdate>();       }   } }`
    
    This is just an example. You might use feature flags (`cfg(feature = "server")`) so that when compiling the server binary, it includes replicate calls, and when compiling the client, maybe not. However, an easier approach is to just include those calls in the server setup code instead (to avoid feature juggling). The key point is the `core` crate can provide functions or plugins to setup common stuff. For instance, if you have any game logic systems that run on both client and server (like maybe an AI system could run on server or single-player), those could live in core and be added by the server.
    
- **Network Protocol Definitions:** If using Replicon, it abstracts a lot, so you don’t have a manual protocol mapping. But if you needed to define message formats (in some networking libraries you define an enum of message types), core would be the place. With our design, core defines events and components, and those effectively are the protocol when combined with Replicon.
    

By putting all this in core, we ensure that the **“component and event registration order is the same on client and server”**, as Replicon recommends​

[docs.rs](https://docs.rs/bevy_replicon#:~:text=Make%20sure%20that%20the%20component,code%20in%20your%20%E2%80%9Cshared%E2%80%9D%20crate)

, since we’ll call the same CorePlugin or registration function in both binaries. This prevents subtle bugs where client and server have mismatched type IDs for serialization.

**Server Crate:** The `server/src/main.rs` is the entry point for the replication server. It will depend on the core crate. In it:

- You build a Bevy App, add `CorePlugin` (from core), add `RepliconPlugins` and the networking plugin (`RepliconRenetPlugins`), and any server-specific plugins (for example, a plugin that loads from Supabase on startup, or spawns AI).
    
- You might also include a `ServerPlugin` in core that only runs on server (via feature flag or separate struct) to add systems like the Supabase persistence system, or server-specific logic (like authoritative AI decisions).
    
- The server crate would be compiled without any of the client’s graphical stuff. By excluding Bevy’s `DefaultPlugins` (which includes windowing, winit, etc.) and only using `MinimalPlugins` plus what you need (Replicon, etc.), the server stays lightweight.
    

**Shard Crate:** The shard server main will be similar to server but with physics. It depends on core as well (to get components, etc.). In it:

- Add `CorePlugin`,
    
- Add physics plugin (Avian),
    
- Add any game logic plugins needed for simulation (maybe an AI plugin, or a spawning system for NPCs in that shard’s region),
    
- Add `RepliconPlugins` _as a client_ and `RepliconRenetPlugins`,
    
- Connect to the replication server (initialize RenetClient in startup).
    
- Run loop.
    

Shard and replication server share a lot in common (both are Bevy apps using ECS), but their configured plugins differ. It may be possible to unify them into one binary that can run in different modes (like a single `server` binary that can run as master or shard based on an argument). But separating can simplify configuration.

**Client Crate:** The client would have all the rendering, input handling, UI, etc. It also depends on core for the shared types. The client adds `DefaultPlugins` (to get a window, input, etc.), plus `CorePlugin`, plus `RepliconPlugins` (with `.disable::<ServerPlugin>()` because it’s a client) and `RepliconRenetPlugins`. It will connect to the replication server via RenetClient as well. It may also include interpolation/prediction plugins (like `bevy_replicon_snap` for client-side interpolation​

[github.com](https://github.com/projectharmonia/bevy_replicon/blob/master/README.md#:~:text=)

). Also, any client-only components or systems (like a component for an explosion visual effect that doesn’t exist on server) can be defined in the client crate if not needed on server.

**Compilation features:** The Replicon crate uses features for client/server, and our core can also use feature flags if needed. As seen in Replicon docs, they provide `client` and `server` cargo features to strip out unused code​

[docs.rs](https://docs.rs/bevy_replicon#:~:text=use%20Cargo%20features%20and%20split,the%20logic%20into%20modules)

. In our setup, we might not need to manually use those if we are compiling separate binaries anyway (e.g., the server binary can compile with `--no-default-features` on certain crates). But it’s something to be aware of. The core crate could be compiled with both features in both cases, but maybe it’s fine since it’s just definitions.

**Advantages of a Shared Core:**

- **Single Source of Truth:** All game data structures are defined once. This avoids errors where client and server think of an entity differently.
    
- **Ease of updates:** If you add a new component or event, you add it in core and both server and client immediately know about it after a redeploy.
    
- **Registration Order:** As mentioned, ensuring the same registration order is important for networking. By calling a single function from core on both sides, you guarantee that. (Under the hood, when serializing, types are usually identified by an ID or by type name strings; if one side has an extra type registered earlier, IDs could mismatch. So always register in consistent order​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=Make%20sure%20that%20the%20component,code%20in%20your%20%E2%80%9Cshared%E2%80%9D%20crate)
    
    .)
    

One potential downside is that the client includes some types it may not use (like the physics components perhaps). But that’s usually fine. The client might not simulate physics, but it still may need the physics component (e.g., `RigidBody` from Avian) to know an entity’s properties (or simply because it gets replicated; though you might not replicate the actual `RigidBody` component type, just its Transform). In any case, including those definitions doesn’t hurt.

**Example of core definitions:**

rust

CopyEdit

`// core/src/lib.rs use bevy::prelude::*; use bevy_replicon::prelude::*; use serde::{Serialize, Deserialize}; use bevy_reflect::TypeUuid;  #[derive(Component, Reflect, Serialize, Deserialize, Debug, Default)] #[reflect(Component)] pub struct Transform2D {     pub position: Vec2,     pub rotation: f32, } #[derive(Component, Reflect, Serialize, Deserialize, Debug, Default)] #[reflect(Component)] pub struct Velocity(pub Vec2);  #[derive(Component, Reflect, Serialize, Deserialize, Debug)] #[reflect(Component)] pub struct Player {     pub name: String, }  #[derive(Event, Serialize, Deserialize, Debug)] pub struct ChatMessage {     pub player_id: u64,     pub message: String, }  // maybe marker for Replicated is just bevy_replicon::server::Replicated, we use that directly pub struct CorePlugin; impl Plugin for CorePlugin {     fn build(&self, app: &mut App) {         app.register_type::<Transform2D>()            .register_type::<Velocity>()            .register_type::<Player>();         // Register events so bevy knows about them on both sides         app.add_event::<ChatMessage>();     } }`

Then in server main:

rust

CopyEdit

`.app.add_plugin(CorePlugin)     .add_plugins(RepliconPlugins) // adds both ClientPlugin & ServerPlugin by default, we might disable ClientPlugin     .add_plugins(RepliconRenetPlugins)     .add_startup_system(setup_replicated_components); ... fn setup_replicated_components(mut app: App) {     app.replicate::<Transform2D>()        .replicate::<Velocity>()        .replicate::<Player>(); }`

Alternatively, you could include the replicate calls in `CorePlugin` behind a `#[cfg(feature="server")]` as earlier, and enable that feature only in server build. There are multiple ways, but clarity and avoiding duplication is the goal.

**Note:** We used a custom `Transform2D` above for example – in practice you might use `Transform` from Bevy for 2D as well (just ignore the z or use 3D transforms in a 2D game).

**Documentation and Clarity:** Keeping the project structured also helps new contributors or your future self. It will be clear where to look for core logic versus platform-specific logic. For instance, rendering systems will be in the client crate, database code in the server crate, etc., while core is mostly pure data and maybe some game logic that is universal.

**Testing in shared context:** You can even have tests in the core crate to test that certain game mechanics work in isolation (like a function that calculates damage). Those can run without needing the full game. And because core doesn’t pull in heavy dependencies like graphics, compile times are reduced when working on that part.

In conclusion, a **shared core crate** fosters consistency and reduces code duplication in a client-server architecture. It aligns with Replicon’s recommendation to keep registration unified​

[docs.rs](https://docs.rs/bevy_replicon#:~:text=Make%20sure%20that%20the%20component,code%20in%20your%20%E2%80%9Cshared%E2%80%9D%20crate)

. By dividing into core, server, shard, client, each part can be built with only the features it needs, improving compile times and runtime performance (no unnecessary systems running). This also sets us up to possibly compile the client to WASM (for web) while keeping server native, since the core is platform-agnostic Rust code.

## 7. Error Handling and Debugging in a Distributed Bevy System

Developing a distributed multiplayer game can be tricky to debug, as issues might arise from network timing, state discrepancies, or simply code bugs that are harder to reproduce when spread across multiple processes. This final section provides best practices for **logging, error handling, and debugging** to help identify and resolve problems in your Bevy MMO.

**Logging and Tracing:** Make liberal use of logging on both server and client. Bevy uses the `log` crate facade (and typically the `tracing` crate under the hood for structured logs). You can include the `LogPlugin` to initialize logging. For example, on server start you might do:

rust

CopyEdit

`.use bevy::log::LogPlugin; ... app.add_plugins(LogPlugin::default()); // by default this reads RUST_LOG env var`

Set the `RUST_LOG` environment variable to configure log levels. For debugging network issues, enable debug logs for the relevant crates:

- `bevy_replicon=debug` (or `trace` for even more) – Replicon’s internal logs can show connection events, tick updates, serialization details, etc.​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=%C2%A7Troubleshooting)
    
    .
    
- `renet=debug` or `bevy_renet2=debug` – Renet may log warnings if a packet is dropped or if a disconnect happens.
    
- Your own crates (core, server, shard) – set to debug to see your log messages.
    

For example:

pgsql

CopyEdit

`RUST_LOG=info,bevy_replicon=debug,bevy_replicon_renet2=debug,my_game=debug`

This will output a lot of info to the console. You might direct server logs to a file for later analysis if running a persistent server.

**Structured Logging:** Consider using `tracing` directly for more structured logs or spans. For instance, you could create spans for each connection or each shard to group log messages. The `tracing-subscriber` crate can then output JSON logs which can be aggregated or filtered. This is more advanced but pays off when debugging complex interactions.

**Error Handling:**

- **Network errors:** Always handle the possibility of disconnects. Renet’s API might return an error if sending fails. Replicon likely abstracts that, but you should handle events like client disconnected or timeout. For example, you might have a system that checks if a shard (as a client) is still connected, and if not, tries to reconnect or alerts an admin.
    
- **Deserialization errors:** If the client and server get out-of-sync in terms of data (say a component was not registered on client, so it can’t deserialize it), Replicon will log an error. Such errors are critical because they often cause the connection to desync. If you see messages like “Deserialize error for entity X component Y”, it means a type wasn’t known or data was corrupted. The fix is usually to ensure the type is registered with `app.register_type` on the client and that the client and server code exactly match versions. Replicon uses `error!` log for client deserialization issues (which will show by default)​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=Alternatively%20you%20can%20configure%20LogPlugin,to%20make%20it%20permanent)
    
    .
    
- **Database errors:** When using Supabase or any DB, handle failures gracefully. If a load from Supabase fails at startup, you might default to an empty world or retry. Use `Result` returns and log errors. For async calls (if using supabase’s REST or something), ensure you `.await` them properly in an async context (Bevy can integrate with async via `bevy_async` or just tokio if you run it outside Bevy’s Schedule).
    
- **Physics errors:** Physics engines might not often “error” per se, but watch for things like unstable simulations. Avian might log warnings if something is wrong (like if you create a rigidbody with NaN coordinates). Keep an eye on its output when debugging physics behavior.
    

**Testing and Simulation:** Debugging distributed issues can be easier if you can **simulate the whole setup on one machine**:

- You can run multiple server instances locally (each on different ports). For example, run one replication server, and two shard server processes connected to it, plus one client, all on localhost. This is great for observing the interactions in real-time. Keep consoles open for each process’s logs.
    
- Introduce artificial latency or packet loss to test robustness. The `bevy_replicon_example_backend` has a `ConditionerConfig` mentioned that can simulate network conditions​
    
    [programming.dev](https://programming.dev/post/27428096?scrollToComments=true#:~:text=Added%20RemoteEventRegistry%20to%20get%20channels,Changed%20Rename%20ChannelKind%20i)
    
    . If using Renet directly, you might not have that readily, but you can use tools like `tc` on Linux or Clumsy on Windows to add latency. This helps reproduce issues that only occur under lag.
    
- Use small-scale scenarios to verify correctness. For instance, move one ship and ensure it replicates properly to the client. Then test many ships. Identify at what load things break.
    

**In-Game Debugging Tools:**

- Consider building a simple debug UI (with egui or Bevy UI) that shows network stats (ping, bytes/sec) on the client. Replicon’s `ConnectedClient` components on the server include a `NetworkStats` with info like RTT, packet counts​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=Backends%20manage%20RepliconServer%20%20and,independent%20way)
    
    . You could send some of that to clients or log it periodically. On the client, you can access `RenetClient` which might have ping info.
    
- A **console command system** can be helpful. For example, listen for chat messages that start with “/” and handle debug commands. You could implement commands like `/tp` (teleport) to move a ship or `/status` to print out number of entities, etc., in server log. This can help inspect state at runtime.
    

**Consistency Checks:** In a distributed simulation, sometimes the server and client might disagree on state (due to a bug). One strategy to catch this is to have the server periodically send a checksum or snapshot of important state to clients (or just to an admin tool) and compare. For example, every 10 seconds, compute a hash of all important components on the server and on the client and compare them (this is heavy and not for production, but for debugging). If they differ, log which entity differs. This can pinpoint desync issues. Replicon ensures eventual consistency by design, so ideally you shouldn’t get long-term desync, but during development, if you misuse it, it could happen.

**Crash Handling:** Make sure to handle panic or crashes gracefully. If the replication server crashes, all clients will disconnect – have the clients detect this (Renet will likely error out) and perhaps attempt to reconnect or at least display a message “Server down”. Similarly, if a shard goes down, the replication server should notice the shard’s connection dropped; it might then mark entities from that shard as unavailable or attempt to reassign them to a backup shard. Designing redundancy is complex (beyond scope), but at minimum log it and avoid total server crash. Using `std::panic::set_hook` to log panics can be useful for capturing stack traces on crash.

**Using Debuggers and Inspection:**

- You can attach a debugger (gdb/lldb or VSCode) to the server or shard process if needed to step through logic, since they are just Rust programs. However, a lot of game issues are timing-related, which logging is usually better for.
    
- Consider using an ECS inspection tool. Bevy has an unofficial `bevy_inspector_egui` which can display entity data in a UI. Running that on the server (with an egui context, even headless you could run inspector in some context) is harder, but on the client you could use it to see component values live. This is more for single-player debugging normally, but you might integrate it into a debug build of the client to confirm that, say, the client’s entity has the same component values the server is sending.
    

**Trace Events:** Sometimes you need to trace a specific entity’s journey. You could instrument an entity with an ID and put its ID in log messages whenever it’s updated or replicated. E.g., log on server: “Ship123 moved to (x,y)” and log on client when it receives: “Received update for Ship123”. This manual tracing helps ensure the sequence is correct. Since our IDs might not match 1:1 (server Entity vs client Entity), use something like a GUID component or the server’s Entity bits (the Replicon `ServerEntityMap` maps server IDs to client IDs​

[docs.rs](https://docs.rs/bevy_replicon#:~:text=Entity%20IDs%20differ%20between%20clients,stored%20in%20the%20ServerEntityMap%20resource)

). You can obtain the server-assigned ID for an entity and log that on both sides for correlation.

**Utilize Replicon’s Debug Features:** The Replicon documentation and community might have tips for debugging. They mention a `Troubleshooting` section with enabling logs​

[docs.rs](https://docs.rs/bevy_replicon#:~:text=%C2%A7Troubleshooting)

and how certain errors are logged at levels (client deserialization at error, server at debug to not spam)​

[docs.rs](https://docs.rs/bevy_replicon#:~:text=Alternatively%20you%20can%20configure%20LogPlugin,to%20make%20it%20permanent)

. If you run into issues, enabling `trace` might output each message in detail, which can be overwhelming but sometimes necessary.

**Testing Error Scenarios:** Proactively test scenarios like:

- A shard server disconnects abruptly: Does the replication server log it? Does it stop receiving updates (it should) and maybe mark those entities? You might implement a timeout that despawns or flags entities from a lost shard after X seconds so they don’t remain frozen.
    
- A client with high latency: Test if your interpolation works by adding an artificial delay.
    
- Packet loss: Ensure the game can handle dropped packets. Using Renet (which has reliability on certain channels), important messages (like spawn events) will eventually arrive due to reliability. But if an unreliable update is lost, the next state update will correct it. That’s fine – just ensure no critical info is only sent unreliably once.
    
- Supabase down: If the DB is unreachable, the server should still run (maybe with an empty world or cached last state) and try saving later. Log the error and perhaps retry in a bit.
    

**Using Diagnostics:** Bevy has a diagnostics system (e.g., `DiagnosticsPlugin`) that can track things like frame time. You might extend this to track custom metrics: number of messages sent, etc. You could then output or even display these on an admin UI.

In conclusion, treat the distributed system as you would any high-reliability server system: use thorough logging, handle errors without crashing, and test under adverse conditions. Bevy and its ecosystem provide good support: from logging utilities to structured data access for debugging. Over the course of development, you’ll likely iteratively refine both the error handling (making the system robust to network issues) and the debugging tools (to pinpoint game logic issues). Leverage the community references and examples – many have gone through similar challenges (for instance, check out the Bevy Discord’s networking channel for advice​

[github.com](https://github.com/projectharmonia/bevy_replicon/blob/master/README.md#:~:text=For%20examples%20navigate%20to%20the,in%20order%20to%20run%20them)

). And remember, **consistency checks and logs are your best friends** when something feels off in the game.

By following the above guidelines, you’ll have a solid foundation to build a scalable, maintainable multiplayer space MMO with Bevy. You have an architecture that can grow (by adding more shards as needed), a networking stack that takes care of the heavy lifting of synchronization, a serialization setup that minimizes manual work, and a plan for debugging and iterating as you develop new features. Good luck with your Bevy multiplayer project!

**References:**

- Bevy Replicon (server-authoritative networking for Bevy) – _Project repository and docs_​
    
    [github.com](https://github.com/projectharmonia/bevy_replicon/blob/master/README.md#:~:text=)
    
    ​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=)
    
- Bevy Replicon Renet2 (Renet integration for Replicon) – _Docs for channels and setup_​
    
    [docs.rs](https://docs.rs/bevy_replicon_renet2#:~:text=fn%20init%28channels%3A%20Res,client_configs%28%29%2C)
    
    ​
    
    [docs.rs](https://docs.rs/bevy_replicon_renet2#:~:text=Just%20like%20with%20regular%20,40%20resources%20from%20Renet)
    
- Bevy Reflect and Serialization – _Bevy dynamic serialization for reflected types_​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=By%20default%20all%20components%20are,feature%20on%20Bevy)
    
    ​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=This%20pairs%20nicely%20with%20server,restore%20the%20correct%20game%20state)
    
- Avian Physics for Bevy – _Fixed timestep physics and interpolation_​
    
    [docs.rs](https://docs.rs/avian2d#:~:text=To%20produce%20consistent%2C%20frame%20rate,to%20visible%20stutter%20for%20movement)
    
    ​
    
    [docs.rs](https://docs.rs/avian2d#:~:text=This%20stutter%20can%20be%20resolved,the%20%2057%20by%20default)
    
- Sharding in MMO servers – _Concept of dividing world into regions handled by separate servers_​
    
    [chipperchickadee.com](https://www.chipperchickadee.com/blog/modern-server-architecture/#:~:text=2,distributed%20across%20the%20shards%204)
    
- Bevy workspace organization – _Recommendation to split into client, server, shared crates for networking_​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=into%20%E2%80%9Cclient%E2%80%9D%2C%20%E2%80%9Cserver%E2%80%9D%2C%20and%20%E2%80%9Cshared%E2%80%9D,split%20the%20logic%20into%20modules)
    
    ​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=Make%20sure%20that%20the%20component,code%20in%20your%20%E2%80%9Cshared%E2%80%9D%20crate)
    
- Replicon tick rate and updates – _Using fixed ticks to send updates at intervals_​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=)
    
- Logging and debugging in Replicon – _Enabling debug logs for troubleshooting_​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=%C2%A7Troubleshooting)
    
    ​
    
    [docs.rs](https://docs.rs/bevy_replicon#:~:text=Alternatively%20you%20can%20configure%20LogPlugin,to%20make%20it%20permanent)