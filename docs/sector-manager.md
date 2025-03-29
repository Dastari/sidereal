# Sector Manager Design for Sidereal Replication Server

This document outlines the design for the `SectorManager`, a crucial component within the Replication Server responsible for managing the game world's sectors and distributing their simulation load across available Shard Servers.

## 1. Core Responsibilities

The `SectorManager` will:

- Divide the infinite 2D world into a grid of fixed-size sectors (e.g., 1000x1000 units).
- Track the state of each sector (e.g., Unloaded, Loading, Active, Unloading).
- Keep track of which entities reside within which sector (likely via a `Sector` component on entities within the Replication Server's ECS).
- Manage a pool of connected and registered Shard Servers.
- Dynamically assign active or activating sectors to specific Shard Servers for simulation.
- Implement a load balancing strategy to distribute sectors fairly and efficiently among shards, aiming to cluster adjacent sectors on the same shard where possible.
- Handle the lifecycle of sectors, including activation when needed and deactivation when empty.
- Orchestrate the smooth transition of entities moving between sectors, including handoffs between different Shard Servers if necessary.

## 2. Data Structures (Replication Server - Conceptual)

The following data structures (likely implemented using Bevy ECS resources and queries) will be needed within the Replication Server:

- **`ActiveShards: Resource<HashMap<ShardId, ShardInfo>>`**:
  - Maps a unique `ShardId` (assigned upon registration) to information about the shard.
  - `ShardInfo` could contain:
    - Network connection details (e.g., Renet Client ID).
    - Current reported load (`ShardLoadStats`).
    - Set of sectors currently assigned (`HashSet<(i32, i32)>`).
- **`SectorMap: Resource<HashMap<(i32, i32), SectorAssignmentState>>`**:
  - Maps sector coordinates `(x, y)` to their current status.
  - `SectorAssignmentState` could be an enum:
    - `Unloaded`: Not simulated, no shard assigned.
    - `Loading { shard_id: ShardId }`: Being assigned, initial state being sent.
    - `Active { shard_id: ShardId }`: Actively simulated by the specified shard.
    - `Unloading { shard_id: ShardId }`: Being deactivated.
- **Entity Sector Tracking**:
  - This should primarily be handled by querying for entities with the `Sector` component (`sidereal::ecs::components::sector::Sector`) within the Replication Server's ECS world. This component is updated based on messages received from the authoritative shard simulating the entity.

## 3. Shard Lifecycle & Communication Protocol

Assumes Shard Servers connect via a non-Replicon channel (e.g., Renet) for management communication.

- **Registration:**
  1.  Shard connects to Replication Server.
  2.  Shard sends `RegisterShard` message.
  3.  Replication Server adds shard to `ActiveShards`, assigns a unique `ShardId`, and sends back `RegistrationAck { shard_id: ShardId }`.
- **Load Reporting:**
  1.  Shards periodically (e.g., every 5-10 seconds) send `ShardLoadUpdate { stats: ShardLoadStats }` message.
  2.  `ShardLoadStats` should initially contain:
      - `entity_count: u32`
      - `player_count: u32`
      - (Optional Future): `avg_tick_time_ms: f32`, `cpu_load_percent: f32` (Note: CPU/memory can be hard to report reliably).
  3.  Replication Server updates the corresponding `ShardInfo` in `ActiveShards`.
- **Sector Assignment:**
  1.  Replication Server decides to assign Sector `(x, y)` to `shard_id`.
  2.  Updates `SectorMap`: `(x, y) -> Loading { shard_id }`.
  3.  Sends `AssignSector { sector_coords: (i32, i32) }` message to the target shard.
  4.  Sends initial entity data for that sector (see Section 7). Could be a separate `SectorInitialState { sector_coords, entities: Vec<EntityData> }` message over a reliable channel.
  5.  Shard receives `AssignSector`, prepares its internal state.
  6.  Shard receives `SectorInitialState`, spawns entities in its ECS.
  7.  Shard sends `SectorReady { sector_coords: (i32, i32) }` back to Replication Server.
  8.  Replication Server updates `SectorMap`: `(x, y) -> Active { shard_id }`.
- **Sector Unassignment:**
  1.  Replication Server decides to deactivate Sector `(x, y)` currently assigned to `shard_id`.
  2.  Updates `SectorMap`: `(x, y) -> Unloading { shard_id }`.
  3.  Sends `UnassignSector { sector_coords: (i32, i32) }` message to the shard.
  4.  Shard stops simulation for the sector, potentially sends final state updates for persistent entities, cleans up internal state.
  5.  Shard sends `SectorRemoved { sector_coords: (i32, i32) }` back to Replication Server.
  6.  Replication Server updates `SectorMap`: `(x, y) -> Unloaded`.

## 4. Sector Activation & Deactivation Logic

- **Activation Trigger:**
  - An entity (player ship, NPC) is predicted to move into an `Unloaded` sector.
  - An entity's sensor range extends into an `Unloaded` sector.
  - Administrative action.
  - The Replication Server detects this trigger.
- **Activation Process:**
  1.  Identify the target `Unloaded` sector `(x, y)`.
  2.  Select the best shard to assign it to based on the Load Balancing strategy (Section 5).
  3.  Initiate the Sector Assignment protocol (Section 3).
- **Deactivation Trigger:**
  - A sector remains empty (zero players, maybe zero significant NPCs/entities) for a configurable duration (e.g., 5 minutes).
  - This can be detected by the responsible Shard (which sends a `ProposeSectorDeactivation { sector_coords }` message) or by the Replication Server monitoring entity counts per sector based on updates.
- **Deactivation Process:**
  1.  Replication Server confirms the sector `(x, y)` should be unloaded.
  2.  Initiate the Sector Unassignment protocol (Section 3).

## 5. Load Balancing Strategy

- **Goal:** Distribute active sectors among available shards to prevent any single shard from becoming overloaded, while minimizing cross-shard entity transitions by clustering adjacent sectors.
- **Load Metric:** Start with a simple weighted score per shard based on `ShardLoadStats`: e.g., `load_score = entity_count + (player_count * 10)`.
- **Assigning New Sectors:**
  1.  When activating sector `(x, y)`, identify candidate shards.
  2.  Calculate a score for each candidate: `score = base_load_score + proximity_penalty`.
  3.  `proximity_penalty`: Increases score if the shard does _not_ already manage sectors adjacent to `(x, y)`. Decreases score (bonus) if it _does_.
  4.  Choose the shard with the lowest final score.
- **Rebalancing (Periodic):**
  1.  Periodically (e.g., every minute), review shard loads.
  2.  Identify overloaded shards (e.g., `load_score > threshold`).
  3.  Identify underloaded shards.
  4.  For an overloaded shard, select one or more of its sectors (candidates could be those bordering sectors managed by underloaded shards) to migrate.
  5.  Initiate Unassignment/Assignment protocols to move the selected sector(s) to a less loaded shard. (Note: Sector migration is complex as it involves transferring active simulation state, potentially easier to just unassign/reassign if state can be fully reconstructed from Replication Server).

## 6. Entity Transition Handling

- **Detection:** The Shard Server simulating an entity detects its position crossing a sector boundary from `old_sector` to `new_sector`.
- **Shard Action:**
  1.  The shard _immediately_ sends an `EntityTransitionRequest { entity_id, new_sector, current_entity_state }` message to the Replication Server via a reliable channel. `current_entity_state` includes all relevant components (position, velocity, health, etc.).
  2.  The shard continues simulating the entity briefly to avoid visual stutter, but considers it "pending transfer".
- **Replication Server Action:**
  1.  Receives `EntityTransitionRequest`.
  2.  Updates the entity's `Sector` component in its own ECS to `new_sector`.
  3.  Looks up `new_sector` in `SectorMap`:
      - **Case A: `new_sector` is Active and managed by the _same_ shard.**
        - Send `AcknowledgeTransition { entity_id, new_sector }` back to the shard. The shard now fully owns the entity in the new sector.
      - **Case B: `new_sector` is Active and managed by a _different_ shard (`new_shard_id`).**
        - Send `EntityEnterSector { entity_id, entity_state }` to `new_shard_id` (using `current_entity_state` from the request).
        - Send `ConfirmTransitionExit { entity_id }` to the original shard (`old_shard_id`). The old shard can now safely remove the entity from its simulation.
      - **Case C: `new_sector` is Unloaded.**
        - Trigger the Activation process (Section 4) for `new_sector`. Assign it to a shard (`assigned_shard_id`, could be old shard or a new one).
        - Once the sector is Loading/Active:
          - If assigned to the _original_ shard, proceed like Case A (send Ack).
          - If assigned to a _different_ shard, proceed like Case B (send `EntityEnterSector` to new shard, `ConfirmTransitionExit` to old shard). The `EntityEnterSector` message essentially becomes part of the initial state for the newly activated sector.
- **Shard Response:**
  - On `AcknowledgeTransition`: The shard updates its internal entity->sector mapping.
  - On `ConfirmTransitionExit`: The shard removes the entity from its simulation.
  - On `EntityEnterSector`: The shard spawns the entity in its ECS world using the provided state and starts simulating it.

## 7. Initial State Transfer

When a sector is assigned (`AssignSector`), the Replication Server must provide the initial state of all relevant entities within that sector to the Shard Server.

- **Method:** Custom reliable message preferred over using Replicon for this bulk transfer.
- **Process:**
  1.  Replication Server queries its ECS for all entities with `Sector == assigned_sector_coords`.
  2.  Serialize the relevant components for these entities into a list (e.g., `Vec<EntitySpawnData>`).
  3.  Send `SectorInitialState { sector_coords, entities: Vec<EntitySpawnData> }` message to the assigned shard.
  4.  The shard deserializes and spawns these entities locally.

## 8. Neighbor Updates / Cross-Shard Visibility

- **Initial Strategy:** Rely solely on the Replication Server + `bevy_replicon`'s visibility system. The Replication Server has a view of entities across all shards. When sending updates to a game client, it should check entities in neighboring sectors (even if simulated by different shards) and include them if they fall within the client's visibility range/interest area. Shards do _not_ need direct knowledge of entities in neighboring sectors initially.
- **Future Consideration (If Necessary):** If _server-side simulation logic_ on a shard requires awareness of entities just across its border (e.g., AI targeting, physics queries), implement a mechanism where:
  1.  Shards mark entities near their border.
  2.  Replication Server receives updates for these near-border entities.
  3.  Replication Server forwards a filtered/lightweight version of these updates (`NeighborEntityUpdate { entity_id, position, velocity, ... }`) to the shard(s) managing the adjacent sector(s).
  4.  Receiving shards create temporary "ghost" or "proxy" representations of these neighboring entities for use in local simulation queries. Avoid full simulation of ghosts.

This design prioritizes the Replication Server as the central coordinator and source of truth, leveraging Shard Servers for distributed computation. Communication relies on explicit messages for control and state transfer, while `bevy_replicon` remains focused on replicating the consolidated world state from the Replication Server to Game Clients.
