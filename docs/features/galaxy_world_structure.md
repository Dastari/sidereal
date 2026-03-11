# Galaxy World Structure

**Status:** Proposed  
**Date:** 2026-03-04

## 1. Overview

Sidereal's game world is a galaxy containing solar systems. Each solar system has a central star, planets with moons, space stations, asteroid belts, and other entities. Players and entities can exist anywhere in the galaxy, including the space between solar systems. The galaxy uses a single continuous 2D coordinate space with f64 precision.

This document covers:

- Coordinate precision and world scale.
- Galaxy and solar system entity model.
- Per-solar-system background shader visuals.
- Spatial partitioning at galaxy scale.
- Solar system transitions and boundary behavior.
- Integration with existing visibility, persistence, and replication systems.

## 2. Coordinate Precision

### 2.1 Current State: f32

All coordinates are currently f32. Avian2d `Position` wraps `Vec2` (f32). Gameplay components (`VisibilityRangeM`, `SizeM`, etc.) are f32. Persistence parses positions as f64 then truncates to f32 during hydration.

### 2.2 Why f32 Is Insufficient

With 40 solar systems, even modestly spaced, galaxy diameter will be in the thousands-of-km range. f32 precision degrades unacceptably at these distances.

| Distance from origin | f32 precision | Impact |
|---|---|---|
| 1 km | ~0.06 mm | No issues |
| 50 km (solar system radius) | ~4 mm | Acceptable |
| 500 km (inter-system travel) | ~32 mm | Noticeable jitter on entities |
| 2,000 km | ~0.125 m | Visible jumping; physics instability |
| 5,000 km (galaxy diameter) | ~0.5 m | Broken -- ships snap between half-meter increments |

At 60 Hz with 100 m/s velocity, each tick moves 1.67 m. At large coordinates, position deltas can approach or fall below available f32 precision, causing motion to stall or quantize.

### 2.3 Decision: Enable f64 for World Coordinates

Enable Avian2d's `f64` feature flag workspace-wide:

```toml
avian2d = { version = "0.5.0", features = ["serialize", "f64"] }
```

This changes `Position` to wrap `DVec2`, `LinearVelocity` to `DVec2`, etc. At 5,000 km from origin, f64 precision is better than a nanometer -- effectively unlimited for any game-scale galaxy.

Gameplay components (`VisibilityRangeM`, `SizeM`, `FlightTuning`, `Engine`, `FuelTank`, `MassKg`, etc.) remain f32. They represent small-magnitude values where f32 precision is more than sufficient.

### 2.4 Rendering: Camera-Relative f32

Bevy's `Transform` and `GlobalTransform` are f32 (`Vec3`). This cannot change without forking Bevy. The standard large-world solution:

1. Authoritative physics uses f64 `Position` (Avian).
2. Before rendering, subtract the camera's world position (f64) from each entity's world position (f64).
3. Cast the resulting small relative offset to f32 for `Transform`.
4. Everything near the camera is near f32 zero, so rendering precision is always good regardless of absolute world position.

The project already disables `PhysicsTransformPlugin` on both server and client and manually syncs transforms. The camera-relative conversion slots into that same sync point on the client.

The server does not render and does not need camera-relative transforms. Server-side `Transform` values are only used for Bevy hierarchy propagation (`GlobalTransform` for mounted children). Server transform sync should cast f64 positions to f32 directly -- precision loss on the server side is acceptable because the server does not render, and the authoritative position remains f64 in `Position`.

### 2.5 Persistence

Graph persistence currently stores positions as JSON arrays (`"position_m": [10.0, 0.0, 0.0]`). JSON numbers are f64-capable. The hydration path already parses via `as_f64()` then truncates to f32:

```rust
Some(Vec3::new(
    arr[0].as_f64()? as f32,
    arr[1].as_f64()? as f32,
    arr[2].as_f64()? as f32,
))
```

After the f64 migration, this truncation is removed and the f64 values are used directly for Avian `Position`. Persistence round-trips f64 precision without loss through JSON.

### 2.6 Replication

Lightyear serializes component data via serde. Avian2d `Position(DVec2)` with the `f64` feature serializes as two f64 values. Network bandwidth for position increases from 8 bytes (2x f32) to 16 bytes (2x f64) per entity per update. At typical entity counts this is negligible.

### 2.7 World Scale Guidelines

These are starting-point numbers, tunable via world init scripts and solar system entity data.

| Concept | Approximate Scale |
|---|---|
| Ship length | 20-100 m |
| Station diameter | 200-2,000 m |
| Planet visual radius | 5,000-50,000 m |
| Solar system playfield radius | 50,000-200,000 m (50-200 km) |
| Inter-system spacing | 200,000-1,000,000 m (200 km - 1,000 km) |
| Galaxy diameter (40 systems) | 2,000,000-10,000,000 m (2,000-10,000 km) |
| Scanner range (gameplay) | 2,000-10,000 m |
| Visibility delivery range | 5,000-20,000 m |

All units are meters, consistent with existing component conventions (`_m`, `_mps`, `_kg` suffixes).

## 3. World Structure: Flat Coordinate Space with References

### 3.1 Design Principles

- **Single continuous coordinate space.** All entities exist in one 2D galaxy plane. No coordinate rebasing, no loading screens, no origin shifts at boundaries.
- **Solar systems are entities, not coordinate-space containers.** A solar system is an entity at a position with a radius. It does not define a local coordinate frame.
- **Membership is a reference, not hierarchy.** Entities within a solar system carry a `SolarSystemId(Uuid)` component that references the system. This is a data relationship, not a parent-child transform hierarchy.
- **Entities between solar systems have no `SolarSystemId`.** They exist in the same coordinate space with no special handling. Alternatively, `SolarSystemId` can be derived from position at low frequency (nearest system within radius).

### 3.2 Why Not Hierarchy

The project uses `MountedOn`/`ParentGuid` for things that move together (modules mounted on ships, engines on hardpoints). Solar system membership is categorically different:

- **Planets don't orbit.** They have fixed positions in galaxy coordinates. No transform propagation is needed from a solar system parent.
- **Ships transit between systems.** Reparenting entities on every boundary crossing is expensive and error-prone. Updating or removing a `SolarSystemId` component is trivial.
- **Visibility already works on absolute positions.** The scanner/visibility system checks distances in galaxy coordinates. No hierarchy traversal is needed for spatial queries.
- **Entities between systems need no special case.** In a hierarchy model, entities without a parent system require special handling. In the flat model, they just exist.

### 3.3 Solar System Entity

A solar system is a normal ECS entity with components describing its spatial extent and visual character.

Components:

| Component | Type | Purpose |
|---|---|---|
| `EntityGuid` | `Uuid` | Standard entity identity |
| `DisplayName` | `String` | Human-readable name (e.g. "Arcturus System") |
| `Position` | `DVec2` | Center of the system in galaxy coordinates |
| `SolarSystemRadius` | `f64` | Defines the spatial boundary of the system |
| `SolarSystemVisuals` | struct | Backdrop shader settings for this system |
| `PublicVisibility` | marker | All clients should know about all solar systems |
| `ShardAssignment` | `i32` | Shard mapping for multi-shard (future) |

Solar system entities do not have `RigidBody`, `Collider`, or physics components. They are data entities, not simulation participants.

### 3.4 Celestial Body Entities

Stars, planets, and moons are normal entities at fixed galaxy coordinates. They reference their solar system via `SolarSystemId` but are not children in the Bevy hierarchy sense.

A solar system's central body is not necessarily a single star. It may be a binary or trinary star system, a black hole, a neutron star, or any other massive body. The model handles this naturally: each central body is an independent entity at its own position within the system, tagged with `CelestialBodyKind` to describe what it is. Binary/trinary systems simply have multiple star entities near the system center with appropriate offsets.

**Star / Central Body:**

| Component | Value / Type |
|---|---|
| `EntityGuid` | Uuid |
| `DisplayName` | "Arcturus A" |
| `Position` | Galaxy coords (at or offset from system center) |
| `SolarSystemId` | Uuid of parent system |
| `CelestialBodyKind` | `Star`, `BlackHole`, `NeutronStar`, `WhiteDwarf`, etc. |
| `VisualAssetId` | "star_yellow_giant" / "black_hole_accretion" / etc. |
| `SizeM` | Body radius (or event horizon / visual radius for black holes) |
| `PublicVisibility` | marker |

Binary/trinary example -- a binary star system has two star entities:

- Star A at `system_center + (-500, 0)` with `CelestialBodyKind::Star`
- Star B at `system_center + (500, 0)` with `CelestialBodyKind::Star`

A black hole system has one central body entity:

- Sagittarius at `system_center` with `CelestialBodyKind::BlackHole`

The system's visual character (accretion disk glow, gravitational lensing hints in the backdrop shader, etc.) is driven by the `SolarSystemVisuals` on the solar system entity, not by the central body entity directly. This keeps backdrop rendering decoupled from individual entity rendering.

**Planet:**

| Component | Value / Type |
|---|---|
| `EntityGuid` | Uuid |
| `DisplayName` | "Arcturus IV" |
| `Position` | Fixed galaxy coords (baked orbital position) |
| `SolarSystemId` | Uuid of parent system |
| `CelestialBodyKind` | `Planet` |
| `VisualAssetId` | "planet_terran_01" |
| `SizeM` | Planet radius |
| `PublicVisibility` | marker |

**Moon:** Same pattern. May carry an `OrbitalBodyId(Uuid)` referencing its parent planet for data/UI purposes, but positioned independently in galaxy coordinates.

**Stations, asteroid belts, etc.:** Normal entities at galaxy coordinates with `SolarSystemId` referencing their system and appropriate gameplay components.

### 3.5 Ships and Players

Ships and players exist at galaxy coordinates as they do today. They may optionally carry `SolarSystemId` derived from their position relative to nearby solar system radii. This derivation can run at low frequency (1 Hz) and is used for:

- Client backdrop selection.
- UI indicators ("Currently in: Arcturus System").
- Shard assignment hints (future).

Ships do not need `SolarSystemId` for visibility, physics, or persistence -- those systems work on absolute coordinates.

### 3.6 New Components

| Component | Kind String | Persist | Replicate | Visibility | Purpose |
|---|---|---|---|---|---|
| `SolarSystemId(Uuid)` | `"solar_system_id"` | true | true | Public | References the solar system this entity belongs to |
| `SolarSystemRadius(f64)` | `"solar_system_radius"` | true | true | Public | Spatial boundary of a solar system |
| `SolarSystemVisuals` | `"solar_system_visuals"` | true | true | Public | Backdrop shader parameters for a solar system |
| `StarTag` | `"star_tag"` | true | true | Public | Descriptive label following existing tag pattern |
| `PlanetTag` | `"planet_tag"` | true | true | Public | Descriptive label following existing tag pattern |
| `MoonTag` | `"moon_tag"` | true | true | Public | Descriptive label following existing tag pattern |
| `SolarSystemTag` | `"solar_system_tag"` | true | true | Public | Descriptive label for solar system entities |
| `OrbitalBodyId(Uuid)` | `"orbital_body_id"` | true | true | Public | Reference to parent celestial body (moon -> planet) |
| `CelestialBodyKind` | `"celestial_body_kind"` | true | true | Public | Enum: `Star`, `BlackHole`, `NeutronStar`, `WhiteDwarf`, `Planet`, `Moon`, `AsteroidField`, etc. |

These follow existing conventions: `ShipTag`, `ModuleTag`, `WeaponTag` already exist as descriptive labels. `SolarSystemId` follows the reference-not-hierarchy pattern.

## 4. Per-Solar-System Background Visuals

### 4.1 Current State

Two fullscreen backdrop entities exist with fixed deterministic GUIDs:

- Space background (`layer_order: -200`, shader: `space_background_wgsl`).
- Starfield (`layer_order: -190`, shader: `starfield_wgsl`).

These are persisted and replicated as normal entities. Shader settings (`SpaceBackgroundShaderSettings`, `StarfieldShaderSettings`) are client-local (not persisted, not replicated) -- they use hardcoded JSON defaults.

This model produces one uniform backdrop for the entire game world.

### 4.2 Target Model

Each solar system carries its own visual configuration. The client determines the player's current solar system context and applies that system's visuals to the local backdrop materials.

**Solar system entity carries `SolarSystemVisuals`:**

```rust
pub struct SolarSystemVisuals {
    pub space_background: SpaceBackgroundShaderSettings,
    pub starfield: StarfieldShaderSettings,
}
```

This component is persisted and replicated with `PublicVisibility`, so all clients know every system's visual character.

**Client-side backdrop resolution:**

1. Each frame (or at ~1 Hz for efficiency), the client checks the player's position against all known solar system positions/radii.
2. If the player is within a solar system's radius, apply that system's `SolarSystemVisuals` to the backdrop materials.
3. If the player is between systems, apply a default deep-space visual preset (darker, sparser stars, no nebula).
4. When transitioning across a system boundary, crossfade between the old and new settings over a configurable duration.

**Existing fullscreen layer entities become client-local rendering targets.** They are the quad meshes that actually render the backdrop. Their shader settings are driven by the solar system context, not by their own replicated data. The current `FullscreenLayer` entity creation in `world_init.lua` becomes the default fallback visual set.

### 4.3 Authoring Flow

Solar system visuals are authored via Lua scripts in `world_init.lua`:

```lua
-- Single-star system
ctx.world:spawn_entity("solar_system", {
  display_name = "Arcturus System",
  position = { x = 150000, y = 320000 },
  solar_system_radius = 100000,
  solar_system_visuals = {
    space_background = {
      nebula_density = 0.6,
      nebula_color_1 = { 0.2, 0.05, 0.3 },
      nebula_color_2 = { 0.1, 0.15, 0.4 },
      star_density = 200,
    },
    starfield = {
      density = 800,
      layer_count = 4,
      intensity = 1.2,
      tint = { 0.9, 0.95, 1.0 },
    },
  },
})
ctx.world:spawn_entity("star", {
  display_name = "Arcturus",
  position = { x = 150000, y = 320000 },
  solar_system_id = arcturus_system_id,
  celestial_body_kind = "star",
  visual_asset_id = "star_yellow_giant",
})

-- Binary star system
ctx.world:spawn_entity("solar_system", {
  display_name = "Castor System",
  position = { x = 500000, y = 780000 },
  solar_system_radius = 120000,
  solar_system_visuals = { ... },
})
ctx.world:spawn_entity("star", {
  display_name = "Castor A",
  position = { x = 499500, y = 780000 },
  solar_system_id = castor_system_id,
  celestial_body_kind = "star",
  visual_asset_id = "star_blue_main_sequence",
})
ctx.world:spawn_entity("star", {
  display_name = "Castor B",
  position = { x = 500500, y = 780000 },
  solar_system_id = castor_system_id,
  celestial_body_kind = "star",
  visual_asset_id = "star_red_dwarf",
})

-- Black hole system
ctx.world:spawn_entity("solar_system", {
  display_name = "Maw",
  position = { x = -200000, y = 50000 },
  solar_system_radius = 80000,
  solar_system_visuals = {
    space_background = {
      nebula_density = 0.9,
      nebula_color_1 = { 0.4, 0.0, 0.05 },
      nebula_color_2 = { 0.1, 0.0, 0.15 },
      star_density = 50,
    },
    starfield = {
      density = 300,
      intensity = 0.6,
      tint = { 1.0, 0.7, 0.5 },
    },
  },
})
ctx.world:spawn_entity("star", {
  display_name = "The Maw",
  position = { x = -200000, y = 50000 },
  solar_system_id = maw_system_id,
  celestial_body_kind = "black_hole",
  visual_asset_id = "black_hole_accretion",
})
```

This leverages the existing scripting system for world population.

## 5. Spatial Partitioning at Galaxy Scale

### 5.1 Current State

The visibility system uses a uniform 2D grid with 300 m cells (`VISIBILITY_CELL_SIZE_M = 300.0`). Cell keys are computed as `(floor(x / cell_size), floor(y / cell_size))`. The grid is stored in a `HashMap<(i64, i64), Vec<Entity>>` and rebuilt from scratch every visibility tick.

Two candidate modes exist:

- `spatial_grid` (current default): uniform-grid candidate preselection with policy exception bypass.
- `full_scan` (debug fallback): O(clients x entities).

### 5.2 Why the Current Grid Works at Galaxy Scale

A common concern is that a 5,000 km galaxy with small cells creates too many cells. This is not actually a problem because:

- **Only occupied cells exist.** `HashMap` stores only cells containing entities. 280 million potential cells cost nothing if 200 are occupied.
- **Entity count is modest.** 40 solar systems with 50-200 entities each, plus players and ships, totals ~10,000 entities. This is trivially indexable regardless of cell size.
- **Query radius is bounded.** Scanner range queries iterate cells within `ceil(range / cell_size)` in each axis. With appropriately sized cells, this stays small.

### 5.3 Recommendation: Increase and Configure Cell Size

The current 300 m cell size is too small for galaxy-scale scanner ranges. If scanner ranges increase to 5-10 km for gameplay at this scale, a 300 m cell means iterating 33x33 = ~1,000 cells per query.

Make cell size configurable and scale it to match scanner/delivery range:

| Cell Size | Scanner 5 km | Scanner 10 km | Notes |
|---|---|---|---|
| 300 m | 17x17 = 289 cells | 34x34 = 1,156 cells | Current; too many cells per query |
| 1,000 m (1 km) | 5x5 = 25 cells | 10x10 = 100 cells | Good balance |
| 2,000 m (2 km) | 3x3 = 9 cells | 5x5 = 25 cells | Minimal query cost |
| 5,000 m (5 km) | 1x1 = 1 cell | 2x2 = 4 cells | Very fast but coarse |

**Recommended default: 2,000 m cells**, configurable via `SIDEREAL_VISIBILITY_CELL_SIZE_M`.

At 2 km cells, a 5,000 km galaxy has 2,500 x 2,500 = 6.25 million potential cells. With ~10,000 entities across ~200 occupied cells, the HashMap stays compact and query iteration stays fast.

### 5.4 Solar-System-Aware Preselection (Future Optimization)

As an optional optimization layered on top of the grid:

1. Before running per-entity visibility checks, compute the player's distance to each solar system center.
2. If `distance_to_system_center - system_radius > scanner_range`, no entity in that system can be visible. Skip all entities with that `SolarSystemId`.
3. This reduces the candidate set for players far from most systems.

This is a preselection optimization only, following the existing authorization-first visibility pipeline. It must not bypass ownership/public/faction policy exceptions.

### 5.5 Grid Cell Key Type

With f64 coordinates, cell keys become `i64`:

```rust
fn cell_key(position: DVec2, cell_size: f64) -> (i64, i64) {
    (
        (position.x / cell_size).floor() as i64,
        (position.y / cell_size).floor() as i64,
    )
}
```

This supports galaxy coordinates up to ~9.2 x 10^18 meters from origin at any cell size, which is far beyond any reasonable game scale.

## 6. Solar System Transitions

### 6.1 Determining Current Solar System

For each player, the server (and/or client) determines the current solar system context:

```
for each solar_system in all_solar_systems:
    if distance(player.position, solar_system.position) <= solar_system.radius:
        player.current_system = solar_system
        break
```

This runs at low frequency (~1 Hz). With 40 solar systems, it is 40 distance checks per player per second -- negligible.

If no system matches, the player is in deep space (between systems).

If systems overlap (unlikely but possible), nearest-center wins.

### 6.2 What Happens at Boundaries

**Nothing special for most systems.** The player's position changes continuously. At some point, the distance check flips from inside to outside (or vice versa). The effects:

- **Backdrop:** client crossfades to new system visuals (or deep-space default). Purely visual, client-local.
- **UI:** "Entering Arcturus System" / "Leaving Arcturus System" notification. Client-local.
- **Visibility:** no change. Scanner range and spatial queries operate on absolute positions regardless of system membership.
- **Physics:** no change. Continuous coordinate space.
- **Persistence:** no change. Entity positions are absolute.
- **`SolarSystemId` on player (if used):** updated at next 1 Hz tick. Optional.

### 6.3 No Loading, No Rebasing

There are no loading screens, coordinate system switches, or entity reparenting at solar system boundaries. This is the primary advantage of the flat coordinate model.

## 7. Integration With Existing Systems

### 7.1 Visibility and Replication

No changes to the visibility pipeline architecture. The authorization-first model (section 7 of the design document) works on absolute positions. Solar system membership is irrelevant to visibility evaluation.

Changes needed:

- Cell size becomes configurable.
- Cell key computation uses f64 positions and i64 keys.
- Scanner range values may need tuning for galaxy-scale gameplay.

### 7.2 Persistence and Hydration

Solar system entities persist as normal graph records. Their components (`SolarSystemRadius`, `SolarSystemVisuals`, `SolarSystemId`) are registered through the standard component authoring workflow with `#[sidereal_component(...)]`.

Hydration reconstructs solar system entities the same way as any other entity.

### 7.3 Scripting

Galaxy and solar system content is a primary use case for the Lua scripting system (`docs/features/scripting_support.md`). The scripting architecture already supports world bootstrap orchestration and entity spawning through graph record persistence. Galaxy structure extends this with new entity archetypes, event hooks, and runtime queries.

#### 7.3.1 World Bootstrap (world_init.lua)

Galaxy layout is defined in `world_init.lua` during first-boot world initialization. This is the primary authoring surface for the galaxy. Adding, removing, or repositioning solar systems is a script change, not a Rust code change.

The world init script spawns:

- Solar system entities with positions, radii, and visual configurations.
- Central body entities (stars, black holes) within each system.
- Planets and moons at fixed orbital positions.
- Stations, asteroid fields, and other static content.

All spawned entities flow through the standard graph record persistence path and are subject to the existing component-kind validation against the generated Rust component registry. Solar system components (`SolarSystemTag`, `SolarSystemRadius`, `SolarSystemVisuals`, `CelestialBodyKind`, etc.) must be registered in the Rust component registry before scripts can reference them. The bundle registry (`bundles/bundle_registry.lua`) must declare these component kinds in the `required_component_kinds` allowlist for any bundle that spawns galaxy entities.

The world init idempotency guard (`script_world_init_state` marker in `GraphPersistence`) prevents re-seeding the galaxy on restarts, consistent with existing behavior for fullscreen layer records.

#### 7.3.2 Bundle Registry Extensions

The script-driven bundle registry needs new archetype entries for galaxy entities:

```lua
bundles = {
  solar_system = {
    graph_records_script = "bundles/solar_system.lua",
    required_component_kinds = {
      C.entity_guid, C.display_name, C.solar_system_tag,
      C.solar_system_radius, C.solar_system_visuals,
      C.public_visibility, C.shard_assignment,
    },
  },
  star = {
    graph_records_script = "bundles/star.lua",
    required_component_kinds = {
      C.entity_guid, C.display_name, C.solar_system_id,
      C.celestial_body_kind, C.visual_asset_id, C.size_m,
      C.public_visibility,
    },
  },
  planet = {
    graph_records_script = "bundles/planet.lua",
    required_component_kinds = {
      C.entity_guid, C.display_name, C.solar_system_id,
      C.celestial_body_kind, C.visual_asset_id, C.size_m,
      C.public_visibility,
    },
  },
  -- ...
}
```
Bundles use `graph_records_script` payloads so Lua returns complete `GraphEntityRecord`-compatible data for each entity. This supports procedural generation while preserving component-kind validation against each bundle's allowlist before persistence.

#### 7.3.3 Procedural Galaxy Generation

Galaxy layout is a strong candidate for scripted procedural generation. Rather than hand-placing 40 solar systems, a generation script can:

```lua
function WorldInit.on_boot(ctx)
  if ctx.world:ensure_once("seed:galaxy:v1") then
    local rng = ctx.world:seeded_rng("galaxy_layout")
    for i = 1, 40 do
      local angle = rng:float(0, math.pi * 2)
      local dist = rng:float(200000, 4000000)
      local pos = { x = math.cos(angle) * dist, y = math.sin(angle) * dist }
      local system_id = ctx.world:spawn_entity("solar_system", {
        display_name = "System " .. i,
        position = pos,
        solar_system_radius = rng:float(50000, 200000),
        solar_system_visuals = generate_system_visuals(rng),
      })
      spawn_system_contents(ctx, system_id, pos, rng)
    end
  end
end
```

The seeded RNG ensures deterministic galaxy layout across restarts when combined with the idempotency guard. Modders can replace the generation function to create entirely different galaxy shapes.

#### 7.3.4 Event Bridge Integration (Future)

Once the authoritative event bridge is implemented (scripting Phase C), galaxy structure opens several event-driven scripting surfaces. These follow the hybrid execution model documented in `docs/features/scripting_support.md` section 2.6: handlers are triggered by events, can perform read-only world queries via `ctx.world`, and emit mutations via `ctx:emit_intent()`.

Galaxy-relevant events in the initial allowlist (scripting section 5.4):

| Event | Payload | Script Use Case |
|---|---|---|
| `system_enter` | `{ player_id, solar_system_id }` | Trigger system-specific missions, faction encounters, dialogue |
| `system_exit` | `{ player_id, solar_system_id }` | Clean up system-local mission state, transition escort NPCs |

Additional galaxy events to add as the system matures:

| Event | Payload | Script Use Case |
|---|---|---|
| `deep_space_enter` | `{ player_id, last_system_id }` | Trigger deep-space random encounters, pirate ambushes |
| `approach_body` | `{ player_id, body_id, body_kind, distance }` | Planet-specific content, station docking sequences |

Example handler using read-only queries for context:

```lua
function EconomyHandler.on_system_enter(ctx, event)
  local player = ctx.world:find_entity(event.player_id)
  local system = ctx.world:find_entity(event.solar_system_id)
  local reputation = player:get("ScriptState")

  -- Hostile faction territory warning
  local system_faction = system:get("FactionId")
  if system_faction and reputation then
    local rep = reputation[system_faction.value] or 0
    if rep < -50 then
      ctx:emit_intent("emit_event", {
        event_id = "player_warning",
        payload = {
          player_id = event.player_id,
          message = "You are entering hostile territory.",
        },
      })
      -- Spawn faction patrol to intercept
      ctx:emit_intent("spawn_entity", {
        archetype = "faction_interceptor_squad",
        position = player:position(),
        solar_system_id = event.solar_system_id,
        faction = system_faction.value,
        target = event.player_id,
      })
    end
  end
end
```

All events flow through the standard event bridge with declarative handler registration, per-handler instruction budgets, and error isolation. Scripts emit intent through validated APIs; the server remains authoritative over all state changes.

#### 7.3.5 Runtime Script API Extensions (Future)

The runtime API surface (scripting section 8) needs galaxy-aware query functions:

- `find_solar_system(uuid)` -- look up a solar system entity by GUID.
- `find_system_at(position)` -- find which solar system contains a given position (or nil for deep space).
- `query_entities_in_system(system_uuid, filter)` -- query entities within a specific solar system.
- `spawn_in_system(system_uuid, archetype_id, opts)` -- spawn an entity within a system, automatically setting `SolarSystemId` and validating position is within radius.

These are convenience wrappers over the generic `find_entity` / `query_entities` / `spawn_entity` APIs. They exist to make galaxy-aware scripting ergonomic without adding new authority bypasses.

#### 7.3.6 Modding Surface

The galaxy structure is a high-value modding target. The scripting system enables:

- **Custom galaxies:** mods replace `world_init.lua` with a completely different galaxy layout, system count, and visual theme.
- **New system content:** mods add new solar systems, planets, stations, and asteroid fields by appending to the world init or registering additional bundles.
- **Custom celestial body kinds:** new `CelestialBodyKind` values (e.g. `PulsarStar`, `DysonSphere`) require Rust enum extension, but their visual/gameplay behavior can be largely script-driven through variant overlays and event hooks.
- **Dynamic galaxy events:** scripted faction wars that destroy stations, create debris fields, or shift system control -- all through intent-based script APIs that the server validates and applies authoritatively.

The component extensibility rules from the scripting contract apply: scripts may compose galaxy entities from registered component kinds but may not introduce new authoritative component types without Rust-side registration.

### 7.4 Entity Variant Framework

Solar systems, planets, and stars can use the variant framework (DR-0007) for visual and parameter diversity:

- `VariantFamilyId("solar_system.nebula")` with variants like `"red_nebula"`, `"blue_nebula"`, `"sparse_field"`.
- `VariantFamilyId("planet.terran")` with variants like `"terran_ocean"`, `"terran_desert"`, `"terran_ice"`.

Variant selection is server-authoritative and persisted, consistent with existing framework design.

### 7.5 Hierarchy (MountedOn / ParentGuid)

No changes to the hierarchy model. Solar system membership is a flat reference (`SolarSystemId`), not a parent-child relationship. `MountedOn`/`ParentGuid` hierarchy remains reserved for structural composition (modules on ships, engines on hardpoints).

### 7.6 Sharding (Future)

Solar system boundaries are natural shard boundaries. Each system could map to a shard via `ShardAssignment`. Inter-system space could be assigned to the nearest system's shard or to a dedicated transit shard.

Entity shard handoff at system boundaries is a future concern and is orthogonal to the coordinate model. The flat coordinate space means handoff is a `ShardAssignment` component update, not a coordinate transformation.

## 8. Implementation Plan

### Phase 1: f64 Coordinate Migration

- [ ] Enable `f64` feature on `avian2d` dependency workspace-wide.
- [ ] Propagate type changes through gameplay components that interact with `Position`/`LinearVelocity` (flight system, physics sync, spawn helpers).
- [ ] Update persistence hydration to stop truncating f64 to f32 for position/velocity.
- [ ] Update visibility grid to use `f64` positions and `i64` cell keys.
- [ ] Add camera-relative f32 transform sync on client (subtract camera f64 position, cast to f32).
- [ ] Verify server-side transform sync handles f64-to-f32 cast for Bevy hierarchy propagation.
- [ ] Update entity template graph record generation with f64 position values.
- [ ] Run quality gates: `cargo check --workspace`, clippy, WASM target check.

### Phase 2: Solar System Components

- [ ] Define solar system component family: `SolarSystemTag`, `SolarSystemId`, `SolarSystemRadius`, `SolarSystemVisuals`, `StarTag`, `PlanetTag`, `MoonTag`, `OrbitalBodyId`.
- [ ] Register components through `#[sidereal_component(...)]` macro with persist/replicate/visibility metadata.
- [ ] Add component entries to generated registry.
- [ ] Add roundtrip persistence/hydration tests.

### Phase 3: Galaxy Layout in Scripts

- [ ] Extend `world_init.lua` to spawn solar system entities with positions, radii, and visuals.
- [ ] Extend bundle registry with solar system, star, and planet archetypes.
- [ ] Add spawn helpers for solar system entity graphs (system + star + planets).
- [ ] Add example galaxy layout with 3-5 solar systems for development.

### Phase 4: Client Backdrop Switching

- [ ] Add client system to determine player's current solar system context (1 Hz position vs. radius check).
- [ ] Apply `SolarSystemVisuals` from current system to backdrop materials.
- [ ] Add crossfade/blend transition when changing systems.
- [ ] Add deep-space default visual preset for between-system travel.
- [ ] Add UI notification for system entry/exit.

### Phase 5: Spatial Partitioning Tuning

- [ ] Make visibility cell size configurable via `SIDEREAL_VISIBILITY_CELL_SIZE_M`.
- [ ] Tune default cell size for galaxy-scale scanner ranges.
- [ ] Add solar-system-distance preselection optimization (optional).
- [ ] Add telemetry for visibility query performance at galaxy scale.

## 9. Open Questions

1. **Galaxy shape:** Are systems arranged in a disk, spiral, cluster, or arbitrary layout? This affects visual presentation and travel distance patterns but not the technical model.
2. **Travel speed between systems:** Current max velocity is 100-250 m/s. At 500 km inter-system distance, transit takes 30-80 minutes. Warp/jump mechanics may be needed. This is a gameplay design question, not an architecture question.
3. **System discovery:** Are all 40 solar systems known to all players from the start, or are some hidden/discoverable? This affects whether `PublicVisibility` is appropriate for all systems or whether some should be faction/scanner-gated.
4. **Multiple galaxies:** The document mentions "there might be different galaxies." If each galaxy is a separate coordinate space, this would require instance/world separation at the session level, not coordinate system changes. Deferred until needed.
5. **Asteroid belt representation:** Are asteroids individual entities with positions, or a zone entity with procedural client-side rendering? Individual entities are simpler and consistent with the flat model but may require many entities for dense fields.

## 10. References

### Related Sidereal Docs

- `docs/sidereal_design_document.md` -- architecture principles, visibility pipeline (sections 7.x).
- `docs/features/visibility_replication_contract.md` -- authorization-first visibility.
- `docs/plans/scan_intel_minimap_spatial_plan.md` -- spatial indexing direction.
- `docs/decisions/dr-0007_entity_variant_framework.md` -- variant/archetype framework.
- `docs/features/scripting_support.md` -- scripting architecture and world init.
- `docs/component_authoring_guide.md` -- component registration workflow.

### Code Paths

- `crates/sidereal-game/src/flight.rs` -- flight physics (f32 constants, will need f64 position interaction).
- `bins/sidereal-replication/src/replication/visibility.rs` -- spatial grid and visibility pipeline.
- `data/scripts/bundles/*.lua` -- script-authored entity graph record generation.
- `crates/sidereal-runtime-sync/src/lib.rs` -- position hydration (f64 truncation site).
- `crates/sidereal-game/src/entities/fullscreen_layers.rs` -- backdrop entity bundles.
- `bins/sidereal-client/src/native/backdrop.rs` -- client backdrop rendering and shader materials.
- `data/scripts/world/world_init.lua` -- world bootstrap script.
