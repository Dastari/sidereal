# Entity bundles and bootstrap: scaling to many ship types

## Can bootstrap (starter ship) be done from the gateway?

**Yes.** The gateway already has `DirectBootstrapDispatcher`, which writes a minimal starter world (Player + Ship with **properties only**, `components: Vec::new()`). Replication’s hydration expects **full** graph records: Ship and Module entities with `components` (e.g. `flight_computer`, `health_pool`, `mounted_on`, `engine`, `fuel_tank`) so it can hydrate into ECS. So today:

- **UDP bootstrap (replication)**: `bootstrap_runtime::bootstrap_starter_ship()` builds the full `Vec<GraphEntityRecord>` (Player, Ship with components, Modules with components) and persists; then sends a command so the sim spawns the ship.
- **Direct bootstrap (gateway)**: writes Player + Ship with **no** component records, so hydration gets a ship with no flight_computer, health_pool, and **no** module entities.

To do it all from the gateway:

1. **Single source of “starter corvette as graph records”**  
   Move the logic that builds `Vec<GraphEntityRecord>` for the starter ship (Player + Ship hull + modules) into a place both can use:
   - **Option A**: New crate or module, e.g. `sidereal-game::entity_templates::corvette::starter_world_graph_records(account_id, player_entity_id, position)` → `Vec<GraphEntityRecord>`.
   - **Option B**: Keep it in replication but expose a function the gateway can call (gateway would depend on sidereal-replication or a thin shared lib).

2. **Gateway**  
   In `DirectBootstrapDispatcher::dispatch`, call that function to get the records, then `persistence.persist_graph_records(&records, 0)`. Optionally use `sidereal-game::corvette::CorvetteSpawnConfig::get_spawn_position()` (or a shared “random coords” helper) for position so behaviour matches replication.

3. **Replication**  
   No longer needs to create the ship in the DB for bootstrap; it only hydrates from DB and spawns the controlled entity in the sim when the client connects (or on startup from hydration). You can remove or simplify the UDP “create ship in DB” path.

So: **yes, it can all be done from the gateway**, as long as the gateway (or a shared dependency) produces the **same** full graph records (with components) that replication’s hydration expects.

---

## Why not “just a bundle” and why so much boilerplate?

You already have `CorvetteBundle` in `sidereal-game`. The mess comes from:

1. **Three different “corvette” definitions**
   - **Game**: `CorvetteBundle` + `spawn_corvette()` + `spawn_corvette_modules()` (no Avian physics, no `SimulatedControlledEntity`).
   - **Replication**: `spawn_simulation_entity()` in `simulation_entities.rs` – manually spawns hull + modules with Avian physics, `SimulatedControlledEntity`, `ActionQueue`, etc., using lots of `default_corvette_*()`.
   - **Bootstrap (graph)**: `bootstrap_starter_ship()` in replication – builds `GraphEntityRecord`s by hand with `component_record(...)` and default values again.

   So the “one corvette” is defined in three places with duplicated defaults and structure.

2. **CorvetteSpawnConfig**  
   It’s really “runtime overrides”: owner, player_entity_id, position, velocity, shard_id, display_name. That’s useful, but it doesn’t need to be a big config struct; it could be a small overrides struct or builder on the bundle.

3. **Bevy’s normal pattern**  
   Bevy expects:
   - A **Bundle** = default component set for one entity.
   - To spawn: `commands.spawn(MyBundle::default())` or `commands.spawn(MyBundle { owner_id: x, ..default() })`.
   - For a “prefab” (ship + modules = multiple entities), you either:
     - Have a function that spawns root + children (e.g. `spawn_corvette(commands, overrides)`), or
     - Use Bevy scenes/prefabs.

   So the pattern “one function that spawns hull + modules” is fine; the problem is that **the default loadout is duplicated** across game, replication, and graph, instead of being defined once and reused (or derived) everywhere.

---

## Is this pattern scalable for hundreds of ships?

**Not as-is.** For many ship types (corvette, frigate, debris, missiles, etc.) you’d be repeating the same boilerplate in three places. What scales is:

- **One definition per entity “archetype”**: default components (and for ships, default modules) live in one place.
- **Spawning** = “spawn this archetype with these overrides” (owner, position, etc.), not a new 100-line function per type.

---

## Proposed structure: `entities/` with bundles and minimal overrides

Goal: an **entities** folder (e.g. under `sidereal-game` or a dedicated crate) where each archetype is a **bundle + optional spawn helper**, and you “spawn” by name or type with minimal overrides.

### 1. Folder layout (example under `crates/sidereal-game/src/entities/`)

```text
entities/
  mod.rs
  ship/
    mod.rs
    corvette.rs   # CorvetteBundle + CorvetteDefaults, spawn_corvette(commands, overrides)
    frigate.rs    # (future)
  debris/
    mod.rs
    small_debris.rs
  missiles/
    mod.rs
    light_missile.rs
```

- **ship/corvette.rs**: Defines the **canonical** corvette:
  - `CorvetteBundle` (already exists; can add `Default` and/or `FlightComputer` etc. if needed for game-only use).
  - A small **overrides** struct, e.g. `CorvetteOverrides { owner_id: Option<OwnerId>, shard_id: Option<i32>, position: Option<Vec3>, display_name: Option<String> }` or similar.
  - `spawn_corvette(commands, overrides)` that spawns `CorvetteBundle` (with overrides applied) + children (hardpoints + modules). All defaults live here (or in `default_corvette_*()` used only from this file).
- Same idea for **debris**, **missiles**: each has a bundle and a `spawn_*(commands, overrides)`.

### 2. Single source for “graph records” (for persistence/bootstrap)

For replication and gateway we need **graph records**, not ECS entities. So add a single place that turns “corvette” into records:

- In `entities/ship/corvette.rs` (or a shared `entity_templates` module), add something like:
  - `corvette_starter_graph_records(account_id, player_entity_id, position)` → `Vec<GraphEntityRecord>`
- This uses the **same** defaults as the bundle (same mass, health, engine stats, etc.) and builds Player + Ship + Module records with proper `components`.
- **Gateway** (DirectBootstrapDispatcher): call `corvette_starter_graph_records(...)`, then `persist_graph_records`.
- **Replication** (bootstrap_runtime): remove `bootstrap_starter_ship`’s inline record building; call the same `corvette_starter_graph_records` and persist. Optionally remove UDP bootstrap once gateway is the only entrypoint.

So: **one** “corvette definition” (defaults + structure) used for:
- Bevy bundle + spawn (game / client).
- Graph records (gateway + replication bootstrap).

### 3. Why keep a small “spawn” function instead of only a bundle?

Because a corvette is **multiple entities** (hull + hardpoints + modules). A single `CorvetteBundle` can only describe the hull. So you either:

- **Option A**: One function per archetype, e.g. `spawn_corvette(commands, overrides)`, that spawns the bundle + children. Clean and scalable: add `spawn_frigate`, `spawn_light_missile`, etc., each in its own file.
- **Option B**: Bevy prefabs/scenes – define the tree in a scene file and spawn the scene. Possible, but your current pipeline is code-driven and graph-based, so a code-defined “spawn this bundle + these children” is likely simpler.

So: **yes, use bundles**, but for “ship with modules” you still have a small **spawn_* (commands, overrides)** per archetype; the key is that **defaults and structure live only there** (and in one graph-record builder if you need persistence).

### 4. CorvetteSpawnConfig → minimal overrides

Replace the big config with something like:

```rust
#[derive(Default)]
pub struct CorvetteOverrides {
    pub owner_id: Option<OwnerId>,
    pub player_entity_id: Option<String>,
    pub shard_id: Option<i32>,
    pub position: Option<Vec3>,
    pub velocity: Option<Vec3>,
    pub display_name: Option<String>,
}
```

Then `spawn_corvette(commands, overrides)` uses `CorvetteBundle::default()` and applies overrides. Same for “random position”: a small helper (e.g. `random_spawn_position(account_id)`) used by both gateway and replication. No need for a 6-field config struct unless you want it for serialization (e.g. from a config file).

---

## Summary

| Question | Answer |
|----------|--------|
| Can bootstrap be done from the gateway? | **Yes.** Have a single “starter corvette as graph records” (and optional random position) used by the gateway’s DirectBootstrapDispatcher; replication only hydrates and spawns in sim. |
| Why not just a bundle? | You **should** use a bundle for the hull; “just a bundle” doesn’t cover “hull + modules” (multiple entities). Use one spawn function per archetype that spawns the bundle + children. |
| Why CorvetteSpawnConfig / spawn_corvette_modules? | SpawnConfig is just overrides; reduce to a small overrides struct. Modules exist because a ship is multiple entities; keep a single `spawn_corvette(commands, overrides)` that uses the bundle + spawns modules. |
| Is this scalable? | **Not** with three separate definitions per ship. It **is** scalable with an `entities/` layout: one folder per kind (ship, debris, missiles), one bundle + one spawn function (and one graph-record builder if needed) per archetype, defaults in one place. |
| What does Bevy expect? | Bundles for component set; for multi-entity “prefabs”, a spawn function or scenes. Your desired “entities folder with bundles and spawn anything quickly” matches that. |

Next concrete steps could be: (1) Add `entities/ship/corvette.rs` (and `entities/mod.rs`) and move `CorvetteBundle` + defaults + `spawn_corvette` there, with `CorvetteOverrides` instead of `CorvetteSpawnConfig`. (2) Add `corvette_starter_graph_records(...)` in the same module (or a shared place used by gateway + replication) and switch gateway and replication bootstrap to use it.
