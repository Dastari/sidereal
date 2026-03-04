# Scripting Support

**Status:** Active contract and implementation plan
**Last updated:** 2026-03-04

## 1. Decision Summary

Sidereal uses Lua for server-authoritative content scripting. Scripts drive high-level content (missions, NPC AI, economy events, dialogue, procedural generation, world bootstrap) while Rust/ECS owns the simulation kernel (physics, networking, replication, persistence, rendering).

Language: Lua via raw `mlua`.
Shared crate: `crates/sidereal-scripting`.
Script source root: `data/scripts` (override via `SIDEREAL_SCRIPTS_ROOT`).

### 1.1 Why Lua

- Industry-proven for game modding (Factorio, WoW, Roblox, Garry's Mod).
- Fast enough for content-layer logic at 1-10 Hz (20-50x slower than Rust is acceptable outside hot paths).
- Small memory footprint (~200 KB per Lua state).
- Sandboxable: critical for untrusted mod content.
- Simple syntax: accessible to non-Rust content authors and modders.

### 1.2 Why Raw mlua (Not bevy_mod_scripting)

The implementation uses `mlua` directly rather than `bevy_mod_scripting` for:

- Tighter control over authority boundaries and sandbox policy.
- Minimal abstraction between script payloads and graph persistence contracts.
- Deterministic integration with gateway/replication startup and persistence workflows.

This does not block future evaluation of `bevy_mod_scripting` for specific tooling or editor layers, but authoritative runtime scripting remains `mlua`-backed.

### 1.3 Decision Register

The following DR entries should be created as this system matures:

- **DR-020**: Scripting Language Selection (Lua for content) -- accepted.
- **DR-021**: Script-ECS Boundary Definition -- accepted, contract in section 2.1.
- **DR-022**: Mod Security and Sandboxing Policy -- accepted, baseline implemented in section 4.
- **DR-023**: Script Execution Model (event-driven + read-only queries + intent-only writes) -- accepted, contract in section 2.6.

## 2. Architecture Contract

### 2.1 Authority Boundary

Scripts emit intent through Rust APIs. Scripts do not directly mutate transforms, velocities, ownership, or session binding.

Script APIs must resolve to the same authority flow as all other gameplay:

```
client input -> shard sim -> replication/distribution -> persistence
```

- Script-side entity identity uses UUID/entity IDs only. No raw Bevy `Entity` handles cross the script boundary.
- Script runtime must never bypass replication visibility/redaction policy.
- Script-spawned entities pass through normal visibility and replication policy systems.

### 2.2 Runtime Placement

```
┌─────────────────────────────────────────────────────────┐
│                    MOD / CONTENT SCRIPTS                 │
│  world/*.lua, missions/*.lua, ai/*.lua, economy/*.lua   │
│  (Hot-reloadable content authored in Lua)               │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│              SCRIPT API LAYER (Rust)                     │
│  crates/sidereal-scripting                              │
│  Sandboxed VM factory, module loader, table decode,     │
│  component-kind validation, path security               │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                  BEVY ECS CORE (Rust)                    │
│  Physics, Networking, Persistence, Rendering            │
│  30 Hz authoritative simulation                         │
└─────────────────────────────────────────────────────────┘
```

Scripts execute only on the authoritative runtime host:

- Dedicated server for MMO deployments.
- Local host process for offline single-player.
- Local host process when a player opens their session to others.

Browser/native clients do not execute authoritative gameplay scripts; they receive replicated outcomes.

Integration points:

- `bins/sidereal-gateway`: account-registration-time script hooks (starter bundle selection, world init config).
- `bins/sidereal-replication`: authoritative host boot world orchestration, and (future) runtime event-driven script execution.
- Script-driven runtime shader binding for 2D visuals is planned via generic client material types and streamed shader assets (see `docs/features/dynamic_runtime_shader_material_plan.md`).

### 2.3 Content Model

- Base archetypes/components remain in Rust (`sidereal-game`) and graph template paths (`sidereal-runtime-sync`).
- Scripts reference archetype/variant IDs and high-level commands; they do not define low-level physics internals.
- Variant/archetype resolution remains server-side and persistence-backed (aligned with `dr-0007_entity_variant_framework.md`).

### 2.4 Component Extensibility Rules

To keep scripting powerful without breaking persistence/replication contracts:

1. Scripts may define and spawn content bundles/templates composed from existing registered component kinds.
2. Scripts may override allowed fields on allowed component kinds at spawn time.
3. Scripts may not introduce brand-new runtime component types in authoritative simulation.
4. Unknown/unregistered component kinds are rejected by spawn validation.
5. Restricted authority-sensitive fields (identity/ownership/session binding/core motion authority internals) are server-assigned or rejected when provided by script input.

Implication for mods:

- "New entity type" in mod terms means a new data/script archetype built from registered components and variant overlays.
- Truly new authoritative ECS component families require Rust component authoring, registry integration, and persistence/replication coverage through the normal workflow.

### 2.5 Generic Script-State Components (Planned)

Provide generic script-friendly persisted components (e.g. `ScriptState { data: HashMap<String, ScriptValue> }`) so mods can track custom per-entity logic state without requiring new Rust component types. This is the single biggest enabler for mod diversity and should be implemented before the modding phase.

Example policy-aligned pattern:

- A scripted archetype may include `Health` plus `Indestructible`.
- Damage systems in Rust check `Indestructible` and suppress health reduction/destruction.
- Behavior remains deterministic and authoritative because component semantics are implemented in Rust systems.

### 2.6 Script Execution Model: Event-Driven with Read-Only Queries

Scripts follow a hybrid execution model: event-driven execution triggers with read-only world access within handlers. This is the same model used by Factorio, WoW, and Roblox.

Three alternative models were evaluated:

| Model | Description | Verdict |
|---|---|---|
| System-style | Scripts define "systems" that iterate entities with component filters on a schedule | Rejected -- unbounded iteration, hard to budget, blurs authority boundary |
| Purely event-driven | Scripts only receive pre-packaged event payloads, no world access | Rejected -- too limiting; AI/economy logic needs to inspect world state to make decisions |
| **Hybrid (accepted)** | **Event-driven execution + read-only world queries within handlers + intent-only writes** | **Accepted** -- clean authority boundary, expressive enough for real content, budgetable |

#### 2.6.1 Execution: Event-Driven

Rust controls when script code runs. Scripts never poll or run in a loop. Two trigger mechanisms:

1. **Gameplay events**: Rust systems emit events that dispatch to registered Lua handlers (`on_entity_damaged`, `on_system_enter`, `on_mission_objective_complete`, etc.).
2. **Registered intervals**: scripts register recurring timer callbacks via `ctx.events:register_interval(name, seconds, handler)`. The Rust scheduler owns timing and respects instruction budgets.

Handlers are short-lived: a handler runs, reads world state, emits intent, and returns. No persistent coroutines or blocking waits.

#### 2.6.2 World Access: Read-Only Queries Within Handlers

When a handler fires, it can read any component on any entity on the authoritative host. This is safe because:

- Scripts only run on the authoritative server. There is no client-side data leak risk.
- Read-only access cannot break invariants. Only intent emission can change state.
- The instruction budget constrains how many reads a handler can do per invocation. Entity lookups and queries consume instructions like any other operation.
- Scripts receive `ScriptEntity` wrappers with `guid()`, `position()`, `get(component_kind)` methods. No raw Bevy `Entity` handles, `&World` references, or `Query<>` types cross the boundary.

Read API returns point-in-time snapshots. Scripts cannot hold references across ticks.

**Snapshot boundary**: The script world snapshot is built at the start of each FixedUpdate tick, after the previous tick's physics writeback has propagated. Scripts see world state as of the prior tick's physics resolution. Intent application occurs before the current tick's physics step, so script-driven motion is reflected in the same tick's physics. The replication visibility system evaluates positions after the current tick's physics writeback. Net effect: no one-tick visibility lag for script-driven motion. See `docs/features/spatial_partitioning_implementation_plan.md` section 10.3 for the full schedule diagram.

#### 2.6.3 Mutations: Intent-Only

All state changes go through `ctx:emit_intent(action, payload)`. This queues a validated action through the same authority pipeline as player input. The server processes it like any other gameplay action. Scripts never directly mutate components, positions, velocities, or ownership.

Invalid intent is rejected and logged. The script continues executing (intent rejection is not a script error).

#### 2.6.4 Script State Persistence

Scripts read and write per-entity custom state through the `ScriptState` component:

- **Read**: `entity:get("ScriptState")` returns the current state table (or nil if absent).
- **Write**: `ctx:emit_intent("set_script_state", { entity_id = id, key = "patrol_index", value = 3 })`.

`ScriptState` persists across ticks and across server restarts via graph persistence. This allows scripts to maintain stateful logic (patrol waypoint index, mission progress, economy accumulators) without requiring new Rust component types.

#### 2.6.5 Example: NPC AI Patrol/Engage/Flee

```lua
function on_ai_tick(ctx, event)
  local npc = ctx.world:find_entity(event.entity_id)
  local hp = npc:get("HealthPool")
  local pos = npc:position()
  local state = npc:get("ScriptState") or {}
  local faction = npc:get("FactionId").value

  -- Flee when low health
  if hp.current / hp.max < 0.2 then
    local stations = ctx.world:query_nearby(pos, 50000, {
      has = "StationTag",
      faction = faction,
    })
    if #stations > 0 then
      ctx:emit_intent("fly_towards", {
        entity_id = event.entity_id,
        target_position = stations[1]:position(),
      })
    end
    return
  end

  -- Engage nearby enemies
  local enemies = ctx.world:query_nearby(pos, 8000, {
    has = "ShipTag",
    not_faction = faction,
  })

  if #enemies > 0 then
    local target = enemies[1]
    ctx:emit_intent("fly_towards", {
      entity_id = event.entity_id,
      target_position = target:position(),
    })
    if ctx.world:distance(event.entity_id, target:guid()) < 500 then
      ctx:emit_intent("fire_weapons", {
        entity_id = event.entity_id,
        target = target:guid(),
      })
    end
  else
    -- Patrol
    local patrol_index = state.patrol_index or 1
    local patrol_points = state.patrol_points or {}
    if #patrol_points > 0 then
      ctx:emit_intent("fly_towards", {
        entity_id = event.entity_id,
        target_position = patrol_points[patrol_index],
      })
      local dist = ctx.world:distance_to_point(event.entity_id, patrol_points[patrol_index])
      if dist < 200 then
        ctx:emit_intent("set_script_state", {
          entity_id = event.entity_id,
          key = "patrol_index",
          value = (patrol_index % #patrol_points) + 1,
        })
      end
    end
  end
end
```

The handler reads world state to make decisions but every mutation goes through `emit_intent`. Rust validates each intent (ownership, range, fuel, ammo) before applying it.

#### 2.6.6 Example: Economy Station Tick

```lua
function on_economy_tick(ctx, event)
  local station = ctx.world:find_entity(event.station_id)
  local inventory = station:get("Inventory")
  local pos = station:position()

  -- Adjust prices based on supply
  if inventory.ore < 100 then
    local demand_factor = 1.0 + (0.05 * (100 - inventory.ore) / 100)
    ctx:emit_intent("adjust_price", {
      station_id = event.station_id,
      resource = "ore",
      factor = demand_factor,
    })

    -- Generate delivery missions when supply is low and traders are nearby
    local nearby_traders = ctx.world:query_nearby(pos, 20000, { has = "CargoHold" })
    if #nearby_traders >= 2 then
      ctx:emit_intent("create_mission", {
        type = "cargo_delivery",
        destination = event.station_id,
        cargo = "ore",
        quantity = 50,
        reward = math.floor(5000 * demand_factor),
      })
    end
  end
end
```

#### 2.6.7 Example: Mission Lifecycle

```lua
function on_mission_start(ctx, event)
  local player = ctx.world:find_entity(event.player_id)
  local mission_system = ctx.world:find_entity(event.solar_system_id)
  local system_pos = mission_system:position()

  -- Spawn convoy at edge of solar system
  local convoy_pos = {
    x = system_pos.x + 40000,
    y = system_pos.y + 10000,
  }
  ctx:emit_intent("spawn_entity", {
    archetype = "npc_convoy",
    position = convoy_pos,
    solar_system_id = event.solar_system_id,
    faction = "trade_guild",
    save_as = "convoy",
  })

  -- Schedule pirate ambush after 60 seconds
  ctx.events:schedule_after(60.0, function(ctx2)
    local convoy = ctx2.world:find_entity(ctx2.mission:get_ref("convoy"))
    if convoy and convoy:get("HealthPool").current > 0 then
      ctx2:emit_intent("spawn_entity", {
        archetype = "pirate_raider_squad",
        position = convoy:position(),
        target = convoy:guid(),
      })
    end
  end)
end

function on_mission_entity_destroyed(ctx, event)
  if event.role == "convoy" then
    ctx:emit_intent("fail_mission", {
      mission_id = event.mission_id,
      reason = "Convoy destroyed!",
    })
  end
end

function on_mission_entity_arrived(ctx, event)
  if event.role == "convoy" then
    ctx:emit_intent("complete_mission", {
      mission_id = event.mission_id,
      rewards = { credits = 5000, reputation = { trade_guild = 10 } },
    })
  end
end
```

## 3. Scriptable Domain Boundaries

### 3.1 Good Candidates (Script in Lua)

| Domain | Tick Rate | Why Scriptable |
|---|---|---|
| Missions / quests | ~1 Hz | High variation, rapid iteration, mod campaigns |
| NPC AI behaviors | 1-10 Hz | Faction-specific, state machines are concise in Lua |
| Economy / background sim | ~0.2 Hz | Complex event chains, balance tuning without recompile |
| Dialogue / narrative | Event-driven | Writers need iteration speed, branching narratives |
| Procedural generation | On-demand | Content variety, modder creativity |
| World bootstrap | Once at startup | Seed factions, regions, world layer entities |
| Galaxy layout / solar systems | Once at startup | Procedural or hand-crafted galaxy generation, per-system visuals (see `docs/features/galaxy_world_structure.md`) |
| Solar system events | Event-driven | System enter/exit triggers, proximity encounters, dynamic world events |

### 3.2 Bad Candidates (Stay in Rust/ECS)

| Domain | Why NOT Scriptable |
|---|---|
| Physics simulation | 30 Hz hot path, server-authoritative, performance-critical |
| Networking / replication | Ultra-low latency, security boundary, binary protocol |
| Client prediction / reconciliation | 60 Hz on client, deterministic requirement |
| Core component systems (thrust, fuel, mass) | High frequency, data-parallel, type safety critical |
| Rendering pipeline | Frame-rate critical, GPU-bound |

## 4. Security and Sandboxing

Sandboxing is enforced from the shared `sidereal-scripting` crate. All script execution goes through this path.

### 4.1 Implemented Baseline

1. **Restricted stdlib**: `StdLib::ALL_SAFE` excluding `IO`, `OS`, `PACKAGE`.
2. **Disabled globals**: `dofile`, `loadfile`, `require` set to `nil`.
3. **Memory limit**: configurable via `SIDEREAL_SCRIPT_MEMORY_LIMIT_BYTES` (default 8 MB).
4. **Instruction budget**: configurable via `SIDEREAL_SCRIPT_INSTRUCTION_LIMIT` (default 200,000 instructions per evaluation). Hook fires every N instructions (`SIDEREAL_SCRIPT_HOOK_INTERVAL`, default 1,000).
5. **Path security**: script paths must be relative, must end in `.lua`, and are canonicalized to prevent directory traversal outside the scripts root.
6. **Fail closed**: authoritative actions that fail validation are rejected with structured `ScriptError` variants (`Security`, `Io`, `Runtime`, `Contract`).

### 4.2 Future Sandboxing Work

- Add approved-path `require` replacement for cross-module imports within the scripts root.
- Add per-tick instruction budget (as opposed to per-evaluation) for runtime event handlers.
- Add network/filesystem audit logging for sandbox violation attempts.

## 5. Event Bridge

The event bridge is the Rust-to-Lua dispatch layer that triggers script handlers in response to authoritative gameplay events. Combined with the hybrid execution model (section 2.6), this is the primary runtime interface between ECS systems and script content.

### 5.1 Event Flow

```
Rust ECS System (authoritative)
  │
  ▼
ScriptEventQueue (Rust resource, buffered per tick)
  │
  ▼
Event Bridge Dispatcher (Rust, runs after physics/before replication)
  │  - resolves registered handlers for event type
  │  - creates handler execution context (ctx)
  │  - enforces per-handler instruction budget
  │  - isolates handler failures
  ▼
Lua Handler (sandboxed)
  │  - receives ctx + event payload
  │  - reads world state via ctx.world (read-only)
  │  - emits intent via ctx:emit_intent() (write)
  ▼
Intent Queue (Rust, validated and applied by authority systems)
```

### 5.2 Event Payload Contract

1. Event payloads use stable IDs and serializable values only: UUID strings, scalar fields, logical IDs.
2. No raw Bevy `Entity` handles cross the boundary.
3. Payloads are constructed by Rust event emitters and passed as Lua tables.
4. Each event type has a documented payload schema. Unknown fields are ignored by handlers for forward compatibility.

### 5.3 Design Decisions (Resolved)

**Subscription model: declarative.**

Scripts declare which events they handle via a manifest table returned from the script module, not via imperative `subscribe()` calls. This enables:

- Deterministic handler registration order (load order from bundle manifest).
- Static analysis of which scripts handle which events (tooling, conflict detection).
- No hidden subscription side effects during script evaluation.

```lua
local EconomyHandler = {}

EconomyHandler.events = {
  "economy_tick",
  "station_inventory_changed",
}

function EconomyHandler.on_economy_tick(ctx, event)
  -- handler body
end

function EconomyHandler.on_station_inventory_changed(ctx, event)
  -- handler body
end

return EconomyHandler
```

The Rust bridge reads the `events` table on module load and registers the corresponding `on_<event_name>` functions as handlers.

**Throttling: per-event-type rate limits with configurable policies.**

| Event Category | Default Rate | Throttle Policy |
|---|---|---|
| Timer intervals (`register_interval`) | Script-defined (e.g. 1 Hz, 5 Hz) | Scheduler-enforced; minimum interval floor of 0.1s |
| Gameplay events (mission, session, economy) | As emitted | No throttle; these are low-frequency by nature |
| Entity lifecycle (spawn, despawn) | As emitted | Aggregation window: batch into one handler call per tick |
| High-frequency (collision, damage) | Configurable | Default: max 10 per entity per second, aggregated into summary events |

**Ordering: bundle load order.**

When multiple scripts handle the same event, handlers execute in the order their modules appear in the bundle manifest (`load_order` field in `script_bundle_file`). For mods, mod priority determines inter-mod handler order. Intra-mod order follows file load order.

**Error isolation: per-handler.**

If a handler exceeds its instruction budget, panics, or returns an error:

1. The handler is aborted.
2. The error is logged with script path, function name, event type, and error details.
3. Remaining handlers for the same event still execute.
4. The failed handler is not disabled -- it will be called again on the next event. Persistent failures are tracked in observability metrics.
5. Critical/repeated failures may surface in dashboard alerts but do not stall the authoritative simulation.

### 5.4 Initial Event Allowlist

Phase C implementation should support at minimum:

| Event | Payload | Source |
|---|---|---|
| `world_boot` | `{ }` | Replication startup (one-time) |
| `player_session_start` | `{ player_id, account_id, solar_system_id }` | Session bind |
| `player_session_end` | `{ player_id, account_id, reason }` | Disconnect/logout |
| `entity_spawned` | `{ entity_id, archetype, solar_system_id }` | Entity lifecycle |
| `entity_despawned` | `{ entity_id, reason }` | Entity lifecycle |
| `entity_damaged` | `{ entity_id, damage, source_id, damage_type }` | Combat |
| `entity_destroyed` | `{ entity_id, killer_id }` | Combat |
| `system_enter` | `{ player_id, solar_system_id }` | 1 Hz context check |
| `system_exit` | `{ player_id, solar_system_id }` | 1 Hz context check |
| `economy_tick` | `{ station_id, solar_system_id }` | Interval timer |
| `mission_started` | `{ mission_id, player_id, mission_type }` | Mission system |
| `mission_objective_complete` | `{ mission_id, objective_id }` | Mission system |
| `mission_completed` | `{ mission_id, player_id }` | Mission system |
| `mission_failed` | `{ mission_id, player_id, reason }` | Mission system |

#### Spatial Partition Events (Future)

When the spatial partition tracks entity cell membership across ticks, derived events become available as script event sources. Raw cell events (`entered_cell`, `left_cell`) are not exposed to scripts — cell membership is an implementation detail. Instead, meaningful derived events are emitted:

| Event | Payload | Rate | Source |
|---|---|---|---|
| `entered_system` | `{ entity_id, solar_system_id }` | Max 1 per entity per second | 1 Hz solar system context check |
| `left_system` | `{ entity_id, solar_system_id }` | Max 1 per entity per second | 1 Hz solar system context check |
| `deep_space_enter` | `{ entity_id }` | Max 1 per entity per 5s | Entity exits all system radii |
| `approach_body` | `{ entity_id, body_id, body_kind, distance }` | Max 1 per entity per body per 5s | Entity enters proximity radius of celestial body |

Throttle policy is enforced in the Rust event emitter, not in the Lua handler. Oscillation across a boundary within the cooldown window emits at most one event pair.

These events are gated behind the spatial partition cell-tracking data structure (see `docs/features/spatial_partitioning_implementation_plan.md` section 10.6). They are not available until the partition implementation includes persistent cell assignment tracking.

Additional events are added by extending the Rust-side event emitter allowlist. Scripts cannot subscribe to events not in the allowlist.

## 6. Script Repository Design

### 6.1 Current Model (Filesystem-Direct)

Scripts are loaded directly from `data/scripts` at gateway registration time and replication startup. No versioning, no publish/activate workflow.

### 6.2 Target Model (Immutable Published Bundles)

Directly editing "live script rows" creates drift and rollback pain. The target model uses immutable published bundles:

- Draft content can change frequently during development.
- Published bundle is immutable and hash-addressed.
- Runtime activates one exact bundle ID/hash.
- Savegames/session metadata record active bundle ID/hash for deterministic reload/join checks.

### 6.3 Filesystem + DB Split

- Filesystem (`data/scripts`) is primary authoring workspace for development and version control.
- DB stores published runtime bundles for: dedicated server deployment, dashboard-driven editing/publish, authoritative multiplayer handshake.

### 6.4 Proposed DB Tables

```sql
create table script_file (
  script_file_id uuid primary key,
  path text not null,
  content text not null,
  sha256 text not null,
  updated_at timestamptz not null default now(),
  unique (path, sha256)
);

create table script_bundle (
  script_bundle_id uuid primary key,
  bundle_version text not null,
  entrypoint_path text not null,
  bundle_sha256 text not null,
  created_at timestamptz not null default now(),
  created_by text not null
);

create table script_bundle_file (
  script_bundle_id uuid not null references script_bundle(script_bundle_id) on delete cascade,
  script_file_id uuid not null references script_file(script_file_id),
  load_order int not null,
  primary key (script_bundle_id, script_file_id)
);

create table script_runtime_config (
  runtime_scope text primary key,
  active_bundle_id uuid not null references script_bundle(script_bundle_id),
  active_bundle_sha256 text not null,
  updated_at timestamptz not null default now()
);
```

AGE graph persistence remains canonical for entity/component state. Script tables are relational metadata/content repositories, not replacement persistence for gameplay ECS state.

## 7. world_init.lua Policy

### 7.1 Purpose

`world_init.lua` is bootstrap orchestration only:

- Seed factions/regions/system parameters.
- Spawn baseline world entities by archetype/variant IDs.
- Register recurring scripted events and mission hooks (future).

### 7.2 Generation Policy

- On first setup/bootstrap only: generate a minimal starter file if missing.
- Do not overwrite on each launch.
- Treat it as normal script content after creation (editable, versioned, publishable).

### 7.3 Current Implementation

`world_init.lua` now provides both:
- `world_defaults` (shader asset IDs), and
- `build_graph_records(ctx)` returning authoritative fullscreen-layer graph records (including `space_background_shader_settings` and `starfield_shader_settings` payloads).

The replication host reads and applies these records at boot with idempotent guard key `script_world_init_state`, and the gateway uses the same script payload for first-time persistence when records are missing.

### 7.4 Target Skeleton

```lua
local WorldInit = {}

function WorldInit.on_boot(ctx)
  if ctx.world:ensure_once("seed:factions:v1") then
    ctx.world:seed_faction("civilian_union")
    ctx.world:seed_faction("frontier_trade_guild")
  end

  if ctx.world:ensure_once("spawn:starter_zones:v1") then
    ctx.world:spawn_entity("station.hub", { variant_id = "station.hub.default" })
    ctx.world:spawn_entity("ship.npc_patrol", { variant_id = "ship.patrol.default" })
  end

  ctx.events:register_interval("economy_tick", 5.0, "economy/tick.lua")
end

return WorldInit
```

## 8. Runtime API Surface (v1 Target)

The runtime API is exposed to Lua handlers via the `ctx` object passed to every handler invocation. It follows the hybrid execution model (section 2.6): read-only world access plus intent-only writes.

### 8.1 Handler Context (`ctx`)

Every handler receives a `ctx` object as its first argument. This is the only entry point into the Rust runtime.

| Field / Method | Access | Description |
|---|---|---|
| `ctx.world` | Read-only | World query interface (section 8.2) |
| `ctx:emit_intent(action, payload)` | Write | Queue a validated action (section 8.3) |
| `ctx.events` | Write | Scheduling interface (section 8.4) |
| `ctx.mission` | Read/Write | Mission-scoped state (section 8.5), only available in mission handlers |

`ctx` is valid only for the duration of the handler call. It cannot be stored or used after the handler returns.

### 8.2 World Read/Query (`ctx.world`)

All read operations return point-in-time snapshots. Results cannot be held across ticks. Each query consumes instruction budget proportional to the work performed.

#### Entity Lookup

```lua
local entity = ctx.world:find_entity(uuid_string)
-- Returns: ScriptEntity or nil
```

#### ScriptEntity Interface

`ScriptEntity` is the read-only wrapper around an entity. No raw Bevy types are exposed.

| Method | Return | Description |
|---|---|---|
| `entity:guid()` | `string` | Entity UUID |
| `entity:position()` | `{ x, y }` | World position (f64 values) |
| `entity:velocity()` | `{ x, y }` | Linear velocity |
| `entity:rotation()` | `number` | Rotation in radians |
| `entity:get(component_kind)` | `table` or `nil` | Component data as a Lua table, or nil if absent |
| `entity:has(component_kind)` | `boolean` | Whether the entity has the component |
| `entity:labels()` | `{ string, ... }` | Entity label list (e.g. `{"Entity", "Ship"}`) |

Component data returned by `get()` is a deserialized snapshot of the component's serde representation. Field names match the Rust struct field names.

```lua
local fc = entity:get("FlightComputer")
-- fc = { throttle = 0.8, yaw_input = 0.0, turn_rate_deg_s = 90.0 }

local hp = entity:get("HealthPool")
-- hp = { current = 80.0, max = 100.0 }

local state = entity:get("ScriptState")
-- state = { patrol_index = 3, home_station = "abc-123-..." } or nil
```

#### Spatial Queries

```lua
local entities = ctx.world:query_nearby(position, radius, filter)
-- position: { x, y } (f64 galaxy coordinates)
-- radius: number (meters)
-- filter: table (optional)
-- Returns: array of ScriptEntity, sorted nearest-first
```

Filter options:

| Field | Type | Description |
|---|---|---|
| `has` | `string` or `{string, ...}` | Required component kind(s) |
| `has_any` | `{string, ...}` | At least one of these component kinds |
| `not_has` | `string` or `{string, ...}` | Must not have these component kind(s) |
| `faction` | `string` | Must have matching `FactionId` |
| `not_faction` | `string` | Must not have this `FactionId` |
| `labels` | `{string, ...}` | Must have all of these entity labels |
| `limit` | `number` | Max results (default: 100) |

```lua
-- Find all ships within 8km that aren't our faction
local enemies = ctx.world:query_nearby(npc:position(), 8000, {
  has = "ShipTag",
  not_faction = "pirate_clan",
  limit = 10,
})

-- Find stations with cargo holds nearby
local stations = ctx.world:query_nearby(pos, 50000, {
  has = { "StationTag", "Inventory" },
  faction = "trade_guild",
})
```

##### Spatial Query Implementation Contract

`query_nearby` uses the replication spatial partition grid (see `docs/features/spatial_partitioning_implementation_plan.md`). It does NOT iterate the full world entity set. The pipeline is:

```
1. Cell lookup: walk grid cells within ceil(radius / cell_size) of center
2. Distance filter: reject entities outside the requested radius
3. Component/faction/label filter: apply filter predicates
4. Sort by distance (nearest first)
5. Truncate at limit
```

##### Query Budget Guardrails

Spatial queries are bounded by hard limits enforced in Rust. Scripts cannot bypass them.

| Guardrail | Default | Env Override |
|---|---|---|
| Max query radius | 50,000 m | `SIDEREAL_SCRIPT_MAX_QUERY_RADIUS_M` |
| Max results per query | 100 | per-query `limit` parameter |
| Max spatial queries per handler invocation | 10 | `SIDEREAL_SCRIPT_MAX_QUERIES_PER_HANDLER` |
| Max total results per handler invocation | 500 | `SIDEREAL_SCRIPT_MAX_RESULTS_PER_HANDLER` |

When limits are exceeded:

- Radius silently clamped to max (no error).
- Results truncated at `limit` (nearest-first, so truncation drops the most distant).
- Per-handler query count exceeded: additional queries return empty arrays, warning logged.
- Per-handler total result count exceeded: additional queries return empty arrays.

These limits are enforced in the Rust closure implementing the query, not in Lua.

#### Distance Queries

```lua
local meters = ctx.world:distance(uuid_a, uuid_b)
-- Returns: number (meters between two entities) or nil if either not found

local meters = ctx.world:distance_to_point(uuid, { x = 1000, y = 2000 })
-- Returns: number (meters from entity to point) or nil
```

Distance queries use UUID-to-entity lookups (O(1) HashMap), not spatial queries. They do not count against the per-handler spatial query budget.

#### Solar System Queries

```lua
local system = ctx.world:find_system_at(position)
-- Returns: ScriptEntity for the solar system containing position, or nil (deep space)

local entities = ctx.world:query_in_system(system_uuid, filter)
-- Returns: array of ScriptEntity with matching SolarSystemId
-- filter: same options as query_nearby (minus position/radius)
```

`query_in_system` uses a `SolarSystemId` index in the script world snapshot (not spatial cells). It counts against the per-handler spatial query budget.

### 8.3 Intent Emission (`ctx:emit_intent`)

All mutations go through intent emission. Each intent is validated by Rust authority systems before being applied. Invalid intent is rejected and logged; the script continues executing.

```lua
ctx:emit_intent(action_name, payload_table)
```

#### Core Intent Actions

| Action | Payload | Description |
|---|---|---|
| `"fly_towards"` | `{ entity_id, target_position }` | Set flight computer target. Validated: entity ownership, fuel. |
| `"fly_away_from"` | `{ entity_id, away_from_position }` | Set flight computer to flee direction. |
| `"stop"` | `{ entity_id }` | Set flight computer to brake. |
| `"fire_weapons"` | `{ entity_id, target }` | Fire weapons at target. Validated: range, ammo, cooldown. |
| `"spawn_entity"` | `{ archetype, position, ... }` | Spawn entity from registered archetype. Validated: component-kind allowlist. |
| `"despawn_entity"` | `{ entity_id, reason }` | Remove entity. Policy-gated: scripts cannot despawn player entities. |
| `"adjust_price"` | `{ station_id, resource, factor }` | Adjust station price. Validated: station ownership. |
| `"create_mission"` | `{ type, destination, ... }` | Create a mission instance. |
| `"complete_mission"` | `{ mission_id, rewards }` | Mark mission complete and grant rewards. |
| `"fail_mission"` | `{ mission_id, reason }` | Mark mission failed. |
| `"set_script_state"` | `{ entity_id, key, value }` | Set a key/value on the entity's `ScriptState` component. |
| `"emit_event"` | `{ event_id, payload }` | Emit a script-defined event into the event bridge. |

The intent action set is extensible by adding Rust-side intent handlers. Scripts cannot invent new intent actions; unknown actions are rejected.

### 8.4 Scheduling (`ctx.events`)

```lua
-- Recurring timer (minimum interval: 0.1s)
ctx.events:register_interval(name, seconds, handler_function)

-- One-shot delayed callback
ctx.events:schedule_after(seconds, handler_function)

-- Cancel a registered interval
ctx.events:cancel_interval(name)
```

Scheduled callbacks receive a fresh `ctx` just like event handlers. They are subject to the same instruction budget and error isolation rules.

```lua
-- In world_init or a module's on_load:
ctx.events:register_interval("pirate_patrol_tick", 2.0, function(ctx)
  local pirates = ctx.world:query_nearby({ x = 0, y = 0 }, 500000, {
    has = { "NpcTag", "FlightComputer" },
    faction = "pirate_clan",
    limit = 50,
  })
  for _, pirate in ipairs(pirates) do
    handle_pirate_ai(ctx, pirate)
  end
end)
```

### 8.5 Mission State (`ctx.mission`)

Available only within mission event handlers. Provides scoped state that persists with the mission instance.

```lua
-- Store a reference to a mission-spawned entity
ctx.mission:set_ref("convoy", convoy_entity_id)

-- Retrieve it later
local convoy_id = ctx.mission:get_ref("convoy")

-- Store arbitrary mission state
ctx.mission:set("waves_spawned", 3)
local waves = ctx.mission:get("waves_spawned")
```

Mission state persists via graph records attached to the mission entity. It survives server restarts.

## 9. Offline Campaign and Host-Opened Sessions

### 9.1 Offline Single-Player

- Run authoritative host process locally with same script runtime.
- Store script bundle activation in local DB profile/save scope.
- Save metadata records active script bundle hash.

### 9.2 Player-Hosted Multiplayer (Listen-Host)

- Host chooses/activates bundle.
- On client join, server sends required script bundle hash/version metadata (not script execution authority).
- Join is rejected if session policy requires exact content hash and mismatch exists.
- Clients still do not execute authoritative world scripts.

### 9.3 Dedicated MMO Servers

- CI/CD publishes bundle and updates runtime activation pointer per environment/shard.
- Rollback = switch active bundle pointer to prior immutable bundle.

## 10. Integration With Existing Sidereal Systems

| System | Integration |
|---|---|
| Archetypes/components | Continue using `sidereal-game` components/macros as source of truth |
| Persistence | Scripts produce ECS changes; durable state persists via graph records (`GraphEntityRecord`/`GraphComponentRecord`) |
| Asset delivery | Scripts reference logical asset IDs/archetype/variant IDs only |
| Visibility/replication | Script-spawned entities pass through normal visibility and replication policy |

## 11. Implemented State (March 2026)

### 11.1 What Is Live Now

1. **Shared scripting crate** (`crates/sidereal-scripting`):
   - Sandboxed Lua VM factory with restricted stdlib, disabled `dofile`/`loadfile`/`require`, memory limit, and instruction budget hook.
   - Shared module loader with path security (canonicalization, traversal prevention, `.lua` extension enforcement).
   - Common table decode helpers (`table_get_required_string`, `table_get_optional_string`, `table_get_required_string_list`).
   - Component-kind validation against generated Rust registry.
   - Structured error types (`ScriptError` with `Security`/`Io`/`Runtime`/`Contract` variants).

2. **Script source root**: `data/scripts` (override via `SIDEREAL_SCRIPTS_ROOT`).

3. **Gateway script hooks** (`bins/sidereal-gateway/src/auth/starter_world_scripts.rs`):
   - `world/world_init.lua`: reads `world_defaults` and executes `build_graph_records(ctx)` for world layer records.
   - `accounts/on_new_account.lua`: calls `on_new_account(ctx)` for starter bundle selection.
   - `bundles/bundle_registry.lua`: loads bundle definitions and validates `required_component_kinds` against `generated_component_registry()`.
   - Starter/player records are now script-authored through bundle `graph_records_script` payloads (`starter_corvette` moved off Rust template path).
   - Script `context` includes `new_uuid()` for dynamic entity/module graph ID generation in Lua.
   - Lua-to-JSON recursive conversion is shared via `sidereal-scripting::lua_value_to_json`.

4. **Replication script hooks** (`bins/sidereal-replication/src/replication/scripting.rs`):
   - Loads `world/world_init.lua` at authoritative host boot.
   - Executes `build_graph_records(ctx)` and applies fullscreen backdrop layer graph records once, guarded by `script_world_init_state` DB marker.
   - World-init guard uses existing `GraphPersistence` connection (no extra DB clients).
   - Asset bootstrap reads `world_defaults` so script-selected backdrop shader asset IDs are included in always-required stream assets.

5. **Replication runtime scripting slice** (`bins/sidereal-replication/src/replication/runtime_scripting.rs`):
   - Persistent sandboxed Lua VM is initialized at host boot (non-send Bevy resource).
   - Interval scheduler runs fixed-tick Lua callbacks (current prototype: `ai/pirate_patrol.lua`).
   - Read-only `ctx.world:find_entity(uuid)` + `ScriptEntity` wrapper (`guid`, `position`, `has`, `get` for `script_state`).
   - Intent bridge prototype: `ctx:emit_intent("fly_towards" | "stop" | "set_script_state", payload)`.
   - Rust validates script control authority (requires `ScriptState` and non-player owner) and applies intents to `FlightComputer` / `ScriptState`.

6. **Registration flow**:
   - `register()` always invokes scripted starter-world persister after account creation.
   - Legacy atomic starter-world bypass removed; atomic account path no longer persists starter graph records directly.

7. **Script state component**:
   - `sidereal_game::ScriptState` added as persisted, non-replicated script-owned state (`HashMap<String, ScriptValue>`).

8. **Observability**: info logs for script root resolution, module load/eval, config selection, bundle selection, world init apply/skip.

### 11.2 Current Script Files

| File | Purpose |
|---|---|
| `data/scripts/world/world_init.lua` | World defaults + scripted world-init graph records for backdrop layers/settings |
| `data/scripts/accounts/on_new_account.lua` | Starter bundle selection for new accounts |
| `data/scripts/bundles/bundle_registry.lua` | Bundle definitions with component-kind allowlists |
| `data/scripts/bundles/starter_corvette.lua` | Script-authored player + starter corvette + module/hardpoint graph records |
| `data/scripts/bundles/debug_minimal_dynamic.lua` | Debug/test dynamic graph-records-script bundle |
| `data/scripts/ai/pirate_patrol.lua` | Runtime interval-driven patrol AI prototype |

### 11.3 Current Runtime Model

- Gateway scripting: account-registration-time persistent entity creation (starter/player graph records).
- Replication scripting: authoritative host boot world orchestration (`world_init`) with one-time guard.
- Replication runtime scripting: persistent Lua VM + interval callback execution + intent application during `FixedUpdate`.
- Persistence vs spawn: scripts currently drive graph record creation/persistence; runtime spawn into ECS world occurs by replication hydration/bootstrap from persisted graph state.

### 11.4 Current Compromises

1. `world_init` runs at replication startup, but gateway registration also reads script files for new-account bundle selection. Both hosts independently resolve the scripts root.
2. Script storage is filesystem-only; DB publish/version workflow is deferred.
3. World init now seeds one patrol NPC prototype for runtime scripting validation; broader world population orchestration remains pending.
4. Event bridge is still interval-first prototype (single script module); full declarative multi-module event routing from section 5 is pending.
5. `WorldInitScriptConfig` struct is defined in both gateway and replication (identical shape). Could be shared via the scripting crate if dependencies allow.
6. Lua table conversion is currently shape-inferred (array vs object). Empty table literals are ambiguous; script payloads should avoid relying on empty arrays until explicit array constructors are added.

## 12. Implementation Plan

### Phase A: Foundation (Complete)

- [x] Create `crates/sidereal-scripting` with sandbox policy, module loader, table helpers, path security.
- [x] Add Lua script files under `data/scripts`.
- [x] Wire gateway registration to use scripted bundle selection and world init config.
- [x] Wire replication startup to execute scripted world init with one-time DB guard.
- [x] Validate bundle component kinds against generated Rust component registry.
- [x] Support dynamic `graph_records_script` bundle mode.
- [x] Remove legacy atomic starter-world bypass.
- [x] Move world-init guard operations into `GraphPersistence` (reduce DB sprawl).

### Phase B: Phase 1 Completion

- [ ] Add end-to-end integration test: DB wipe -> first startup world init -> new account scripted bundle creation -> replication hydration on enter-world -> restart -> verify idempotent.
- [ ] Add restart test proving world-init marker skip behavior.
- [ ] Share `WorldInitScriptConfig` through the scripting crate or a shared types module to eliminate duplication between gateway and replication.
- [ ] Add approved-path `require` replacement so scripts can import shared modules within the scripts root.

### Phase C: Event Bridge + Handler Context

- [ ] Implement `ScriptEventQueue` Rust resource for buffering events per tick.
- [ ] Implement event bridge dispatcher: resolve registered handlers per event type, create `ctx` per invocation, dispatch to Lua.
- [ ] Implement declarative handler registration: read `events` table from script modules on load, register `on_<event_name>` functions.
- [ ] Implement per-handler instruction budget and error isolation (abort handler on budget exceeded, log error, continue to next handler).
- [ ] Add `ctx` handler context object exposing `ctx.world` (read-only) and `ctx:emit_intent()` (write).
- [ ] Add initial event allowlist from section 5.4 (world_boot, session, entity lifecycle, combat, system transitions, economy, mission).
- [ ] Add throttling/aggregation for high-frequency events (collision, damage): configurable per-event-type rate limits.
- [ ] Add event bridge observability: per-handler execution time, instruction count, error count, exposed via `bevy_remote`.
- [ ] Add integration test: Rust event emitted -> Lua handler fires -> intent queued -> state change validated.
- [ ] Reserve `SpatialEventBuffer` resource and persistent cell-tracking `HashMap<Entity, (i64, i64)>` in partition rebuild for future spatial events (entered_system, left_system). Implementation of event emission is deferred but the data structures should exist to avoid a later refactor.

### Phase D: Runtime Script API v1 + Mission Pilot

- [ ] Extend `ScriptEntitySnapshot` with `component_kinds: HashSet<String>`, `faction_id: Option<String>`, and `labels: Vec<String>` so filter predicates can be evaluated without ECS queries.
- [ ] Implement `ScriptEntity` read-only wrapper: `guid()`, `position()`, `velocity()`, `rotation()`, `get(kind)`, `has(kind)`, `labels()`.
- [ ] Implement `ctx.world:find_entity(uuid)` via entity GUID lookup.
- [ ] Add spatial index (`entities_by_cell`) to `ScriptWorldSnapshot` using the shared `cell_key` function from the visibility/partition module. See `docs/features/spatial_partitioning_implementation_plan.md` section 10.1.
- [ ] Implement `ctx.world:query_nearby(pos, radius, filter)` using the snapshot spatial index (cell walk, distance filter, component/faction/label filter, nearest-first sort, limit truncation).
- [ ] Implement query budget guardrails (max radius, max queries per handler, max results per handler) enforced in Rust. See section 8.2 of this document.
- [ ] Add `entities_by_system: HashMap<Uuid, Vec<String>>` index to `ScriptWorldSnapshot` for `query_in_system`.
- [ ] Implement `ctx.world:distance(a, b)` and `ctx.world:distance_to_point(uuid, pos)`.
- [ ] Implement `ctx.world:find_system_at(pos)` and `ctx.world:query_in_system(uuid, filter)` (solar system queries).
- [ ] Implement intent queue: `ctx:emit_intent(action, payload)` -> Rust-side validation -> authoritative state change.
- [ ] Add core intent actions: `fly_towards`, `stop`, `fire_weapons`, `spawn_entity`, `despawn_entity`, `set_script_state`, `emit_event`.
- [x] Add `ScriptState` generic persisted component (`HashMap<String, ScriptValue>`) with `#[sidereal_component(...)]` registration.
- [ ] Implement `ctx.events:register_interval()`, `schedule_after()`, `cancel_interval()` with minimum interval floor.
- [ ] Implement `ctx.mission` scoped state interface: `set_ref()`, `get_ref()`, `set()`, `get()`.
- [ ] Implement first scripted mission (escort convoy) using the event bridge and runtime API.
- [ ] Add persisted mission state model with graph persistence roundtrip (mission state survives restart).
- [ ] Add integration tests for mission start/update/complete/fail lifecycle across restart.
- [ ] Add script API version field to bundle manifest for forward compatibility checks.
- [ ] Verify hydration invariant: `EntityGuid` → entity → partition cell mapping is complete before first script interval tick fires. See `docs/features/spatial_partitioning_implementation_plan.md` section 10.7.

### Phase E: NPC AI Scripting

- [x] Implement AI tick prototype via fixed interval scheduler (`ai/pirate_patrol.lua`, 2s interval) on replication host.
- [ ] Consider providing optional Rust-side behavior tree primitives that scripts configure for deterministic execution, built-in profiling, and serializable state.
- [ ] Add movement intent actions: `fly_towards`, `fly_away_from`, `orbit`, `stop` (extend core intents from Phase D as needed).
- [ ] Add combat intent actions: `fire_weapons`, `set_target`, `disengage`.
- [ ] Create example AI scripts using the hybrid model (read-only queries + intent emission):
  - `ai/pirate_patrol.lua` -- patrol waypoints, engage enemies, flee when damaged.
  - `ai/trade_convoy.lua` -- follow trade route, dock at stations.
  - `ai/station_defense.lua` -- guard station, engage hostiles within radius.
  - `ai/faction_patrol.lua` -- faction-aligned patrol with faction-aware targeting.

### Phase F: Script Repository + Publish Pipeline

- [ ] Add migration-backed DB tables (`script_file`, `script_bundle`, `script_bundle_file`, `script_runtime_config`).
- [ ] Add publish command path: collect files -> hash -> persist immutable bundle -> activate runtime pointer.
- [ ] Add runtime loader path from active published bundle.
- [ ] Add rollback command path by bundle ID.
- [ ] Add session metadata carrying active bundle hash/version.
- [ ] Add join validation logic for content hash policy (multiplayer).
- [ ] Add offline save metadata checks (save records active bundle hash).

### Phase G: Hot-Reload and Developer Tooling

- [ ] Add file watcher on `data/scripts` for automatic re-evaluation during development.
- [ ] Add per-script execution time tracking exposed via `bevy_remote` inspection.
- [ ] Add script error log with file/line/function context.
- [ ] Add `/scripts/status` endpoint showing loaded scripts, last error, cumulative budget usage.
- [ ] Add script profiling: per-handler timing, instruction counts, memory usage.

### Phase H: Modding Support

- [ ] Define mod folder structure and manifest format.
- [ ] Implement mod discovery and loading with sandbox enforcement.
- [ ] Add mod load order with explicit priority.
- [ ] Add conflict detection: two mods hooking the same event with incompatible intent.
- [ ] Define override vs. chain semantics (does a mod replace a handler or wrap it?).
- [ ] Add mod dependency declaration and resolution.
- [ ] Document modding API with examples.
- [ ] Create example mods.

### Phase I: Dashboard Editor Integration

- [ ] Add draft edit endpoints.
- [ ] Add validate/lint endpoint.
- [ ] Add publish endpoint producing immutable bundle.
- [ ] Add activate/rollback controls.
- [ ] Add audit trail (who published/activated what and when).

### Phase J: Long-Term (Future)

- [ ] Player-facing scripting: heavily sandboxed subset for player automation (autopilot waypoints, trade route scripts, fleet command macros). Needs extreme sandboxing and rate limiting.
- [ ] Dialogue/quest system with branching narrative support.
- [ ] Economy event scripting with faction response chains.
- [ ] Procedural content generation hooks (asteroid fields, station placement, encounter generation).
- [ ] Visual scripting editor for non-programmers (optional tooling layer).
- [ ] Mod repository/marketplace hosting.
- [ ] Script bundle templates and dynamic graph-record payloads as primary content authoring path.

## 13. Performance Budget

Content scripts run at 1-10 Hz, not on the 30 Hz physics hot path.

```
30 Hz authority tick: 33 ms budget
  Physics:      ~10 ms
  Replication:  ~5 ms
  Rendering:    ~10 ms
  Scripts:      ~5 ms  (content scripts get 5 ms = plenty)

1 Hz mission update:
  10 missions @ 0.5 ms each = 5 ms total
  ~100,000 Lua instructions per mission per tick
```

Optimization strategies:

1. **Batch API calls**: provide bulk query APIs so scripts don't query ECS per-entity in loops.
2. **Cache lookups**: store entity handles in script state across ticks.
3. **Throttle script ticks**: missions at 1 Hz, AI at 5-10 Hz, economy at 0.2 Hz.
4. **Use Rust for heavy compute**: provide high-level APIs (e.g. `find_nearest`, `entities_in_radius`) implemented in Rust.
5. **Spatial index for queries**: `query_nearby` uses the partition grid, not full-world iteration. At 2km cell size with 50km max radius, a query walks at most 625 cells — bounded by the result limit and instruction budget.
6. **Query budget guardrails**: max 10 spatial queries per handler, max 500 total results per handler. Prevents a single runaway script from consuming the per-tick simulation budget.

## 14. Acceptance Criteria (Overall)

1. Server boots with a published bundle and executes `world_init.lua`.
2. Script-spawned entities persist/hydrate via graph records with deterministic outcomes after restart.
3. Offline save captures active bundle ID/hash and reload succeeds without drift.
4. Host-opened multiplayer rejects mismatched bundle hash according to policy.
5. Script timeout/memory limits prevent runaway callbacks from stalling authoritative sim.
6. Critical script failures surface in persistent client dialog UX where user acknowledgment is required.
7. Event bridge dispatches Rust events to Lua handlers with per-handler isolation.
8. Script API version is checked against bundle manifest on activation; incompatible bundles are rejected.

## 15. References

### Crate Documentation

- `mlua` docs: https://docs.rs/mlua/latest/mlua/

### Inspiration

- Factorio Lua API: https://lua-api.factorio.com/

### Related Sidereal Docs

- `docs/sidereal_design_document.md` -- architecture principles.
- `docs/component_authoring_guide.md` -- component registry and generation workflow.
- `docs/features/dr-0007_entity_variant_framework.md` -- variant/archetype framework.
- `docs/features/visibility_replication_contract.md` -- visibility and replication policy.
- `docs/features/asset_delivery_contract.md` -- asset streaming and catalog.
- `docs/features/galaxy_world_structure.md` -- galaxy/solar system world model and scripting integration.
- `docs/features/spatial_partitioning_implementation_plan.md` -- spatial partition grid, cell sizing, script query integration (section 10).

### Code Paths

- `crates/sidereal-scripting/src/lib.rs` -- shared Lua runtime, sandbox, loader.
- `bins/sidereal-gateway/src/auth/starter_world_scripts.rs` -- gateway script hooks.
- `bins/sidereal-gateway/src/auth/starter_world.rs` -- starter world persistence orchestrator.
- `bins/sidereal-replication/src/replication/scripting.rs` -- replication script hooks.
- `bins/sidereal-replication/src/replication/simulation_entities.rs` -- world init execution and hydration.
- `data/scripts/` -- script source root.
