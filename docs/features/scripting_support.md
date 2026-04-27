# Scripting Support

Status: Active implementation contract
Last updated: 2026-04-26
Owners: scripting + replication + gameplay
Scope: authoritative Lua scripting, script catalogs, validated mutation APIs, and content authoring boundaries

## 0. Implementation Status

2026-04-24 status note:

1. Implemented: `crates/sidereal-scripting` owns Lua loading/validation helpers and generated registry integration.
2. Implemented: replication runtime loads script/entity/asset/planet registries, hot-reloads catalogs, exposes BRP-inspectable resources, and uses script-authored world/bootstrap bundles.
3. Implemented: Lua-authored asset/audio/shader metadata now feeds gateway delivery, dashboard tooling, and runtime presentation paths.
4. Partial/open: long-term quest/dialogue/economy APIs, broader validated script mutation actions, and full mod security policy remain future work.
5. Native/WASM impact: authoritative script execution remains server-side; clients consume replicated state/catalog outputs through shared schemas.

Update note (2026-04-24):
- Genesis planet registry authoring has started. Planet/celestial definitions now use `data/scripts/planets/registry.lua` plus one Lua file per named body, decoded through `crates/sidereal-scripting` into a typed `PlanetRegistry` resource and consumed by `world_init.lua` through validated bundle spawn context. Genesis dashboard writes are expected to use script draft/publish APIs, not direct disk mutation. Native impact: server-side content registry and unchanged planet bundle/render path. WASM impact: no authoritative script execution moves client-side.

Update note (2026-04-26):
- Asteroid Field System V2 adds the target Lua authoring surface for field roots and asteroid resource/fracture/ambient profiles. `asteroid.field` is the primary world-content primitive for new fields, while `asteroid.field_member` remains a migration helper and isolated-rock path. The registry surface is `data/scripts/asteroids/registry.lua`, decoded through `crates/sidereal-scripting` by `load_asteroid_registry_from_root` / `load_asteroid_registry_from_source`, and validated for duplicate profile IDs plus field-profile references. Native impact: authoritative host and native client will consume the same validated field/member/profile outputs. WASM impact: no authoritative Lua execution moves client-side; browser clients consume replicated field/member metadata and shader assets.

Update note (2026-04-24):
- Runtime Lua contexts now expose `ctx:notify_player({...})` for validated server-authored non-blocking player notifications. The script API emits a Rust-owned notification intent; Lua does not receive UI, network, or database handles. Native impact: clients can render the resulting toast through the shared notification lane. WASM impact: protocol and payload model remain shared-client compatible.

Update note (2026-04-24):
- DR-0035 makes f64 authoritative world coordinates the target for script-authored and script-visible world positions. Lua numeric position arrays remain valid input, but Rust must parse validated world-space coordinates as f64 and write Avian `Position` / `WorldPosition` without f32 truncation. Native impact: scripted spawns, teleports, spatial queries, and snapshots work at galaxy-scale coordinates. WASM impact: no client-side authority split; browser/native clients consume shared f64 replicated outputs.

Update note (2026-03-12):
- Added the long-term ownership contract for destruction/lifecycle-driven VFX. Default explosion/fracture/loot behavior should live in Rust-defined authored profiles/components, while Lua remains responsible for high-level preset selection and exceptional event-driven overrides. Native impact: no immediate runtime change; this is a documentation/contract clarification. WASM impact: no direct impact because the authority split remains in shared gameplay/runtime code.

Update note (2026-03-13):
- Added the target fly-by-wire thrust-allocation contract. For the future flight-control stack, Lua should author validated actuator/profile/effect-reference data and emit high-level motion/navigation intents, but must not drive raw engine throttle, desired wrench, Avian force application, or plume shader ABI directly. Native impact: no immediate runtime change; this is a contract clarification for the planned flight-control replacement. WASM impact: no architecture split because the same shared gameplay/control code remains the target on both platforms.

Update note (2026-03-13):
- Added the target scriptable UI/dialogue presentation contract. Lua-authored quest/dialogue content should be able to trigger validated client presentation flows such as portraits, progressive text reveal, skip-to-end behavior, and branching player choices through replicated UI payloads rendered by the native `sidereal-ui` layer. Native impact: this defines the intended contract for future narrative/dialogue UI work. WASM impact: no authority split; server-authored scripted content remains authoritative and browser/native clients render the same replicated presentation data.

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

The following DR entries define scripting authority/runtime direction:

- **DR-0020**: Server-only authoritative quest/mission script execution -- accepted.
- **DR-0021**: Quest template/instance model with player-scoped persistence -- accepted.
- **DR-0022**: Quest progression and inventory mutation through script hooks + intent APIs -- accepted.
- **DR-0023**: Mod security and sandboxing policy -- tracked by this contract (section 4), add dedicated DR doc if policy scope expands.
- **DR-0024**: Privileged scripted world mutation via validated actions (no raw ECS writes) -- accepted.

## 2. Architecture Contract

### 2.1 Authority Boundary

Scripts execute on the authoritative host and mutate gameplay state through Rust-owned script APIs.
Lua does not receive raw ECS world references or direct component write handles.
Instead, scripts request mutation through validated intent/actions (including privileged mutation actions where allowed).

Script APIs must resolve to the same authority flow as all other gameplay:

```
client input -> shard sim -> replication/distribution -> persistence
```

- Script-side entity identity uses UUID/entity IDs only. No raw Bevy `Entity` handles cross the script boundary.
- Script runtime must never bypass replication visibility/redaction policy.
- Script-spawned entities pass through normal visibility and replication policy systems.
- High-power operations (teleport, transform override, batched repositioning, component field mutation) are allowed only via explicit server-side script API actions with validation, scheduling, and audit.

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
│  60 Hz authoritative simulation                         │
└─────────────────────────────────────────────────────────┘
```

Scripts execute only on the authoritative runtime host:

- Dedicated server for MMO deployments.
- Local host process for offline single-player.
- Local host process when a player opens their session to others.

Browser/native clients do not execute authoritative gameplay scripts; they receive replicated outcomes.

### 2.2.1 Script Logging API

All current authoritative Lua execution contexts expose a `ctx.log` table backed by Rust `tracing`.

Available methods:

- `ctx.log:debug(message)`
- `ctx.log:info(message)`
- `ctx.log:error(message)`

Behavior:

- The logger automatically tags output with the script path (gateway/replication bootstrap and bundle scripts) or handler name (replication runtime AI handlers).
- `message` may be a string or any Lua value convertible through the existing JSON bridge; non-string values are serialized before logging.
- This is intended for script diagnostics and operational tracing only. It must not be used as a substitute for gameplay-side state/telemetry persistence.

Example:

```lua
function WorldInit.build_graph_records(ctx)
  ctx.log:info("building starter world records")
  return {}
end
```

### 2.2.2 Script Notification API

Runtime scripts may request non-blocking player notifications through:

```lua
ctx:notify_player({
  player_entity_id = "11111111-1111-1111-1111-111111111111",
  title = "Objective Updated",
  body = "Return to station.",
  severity = "info",
  placement = "bottom_right",
  image_asset_id = nil,
  image_alt_text = nil,
  auto_dismiss_after_s = 5.0,
  event_type = "objective_update",
  data = { objective_id = "starter_return" },
})
```

Rules:

1. `player_entity_id`, `title`, and `body` are required.
2. `severity` accepts `info`, `success`, `warning`, or `error`.
3. `placement` accepts `top_left`, `top_center`, `top_right`, `bottom_left`, `bottom_center`, or `bottom_right`.
4. `auto_dismiss_after_s`, when present, must be finite and bounded.
5. The host validates and converts the request into a notification command; scripts cannot directly mutate UI state or notification history.
6. Critical errors still use Rust-owned dialog/error flows, not script-triggered toasts.

Integration points:

- `bins/sidereal-gateway`: account-registration-time script hooks (starter bundle selection, world init config).
- `bins/sidereal-replication`: authoritative host boot world orchestration, and (future) runtime event-driven script execution.
- Script-driven runtime shader binding and render-layer composition for 2D visuals now runs through Lua-authored `RuntimeRenderLayerDefinition`, `RuntimeRenderLayerRule`, `RuntimeRenderLayerOverride`, `RuntimePostProcessStack`, and `RuntimeWorldVisualStack` component data. Replication and gateway validate those authored records server-side, the client builds a runtime layer registry from replicated state, world entities resolve layer assignment as `override -> highest-priority rule -> default main_world`, fullscreen background/foreground plus camera-scoped post-process overlay passes execute from those authored definitions, and layered world visuals such as the current planet body/cloud/ring stack now consume an authored `RuntimeWorldVisualStack` instead of inferring pass composition client-side. The remaining migration gap is removal of the last content-specific shader adapters and continued reduction of the dedicated Rust `Material2d` families that still exist because Bevy material schemas are type-static (see `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`, `docs/plans/dynamic_runtime_shader_material_plan.md`, and `docs/features/asset_delivery_contract.md`).
- Procedural visual tuning payloads for authored world content are Lua-owned. Current examples include asteroid procedural sprite profiles and planet body shader settings emitted from Lua bundles, while Rust owns the validated component schema and render/runtime implementation.
- Genesis planet registry definitions are Lua-owned catalog data under `data/scripts/planets/`. Rust validates the registry and exposes typed `PlanetRegistry` data; Lua world bootstrap consumes those definitions through `ctx.load_planet_definitions()` and still spawns authoritative entities through the existing bundle path.
- Shared environment-lighting defaults are also Lua-authored now via the `environment.lighting` bundle and replicated `EnvironmentLightingState` component, while the client derives its render-time lighting resource from that ECS state.

### 2.3 Content Model

- Base archetypes/components remain in Rust (`sidereal-game`) and graph template paths (`sidereal-runtime-sync`).
- Scripts reference archetype/variant IDs and high-level commands; they do not define low-level physics internals.
- Variant/archetype resolution remains server-side and persistence-backed (aligned with `dr-0007_entity_variant_framework.md`).

### 2.3.1 Lifecycle And Visual-Effect Ownership Contract

For entity destruction, fracture, and other lifecycle-driven effects, the intended long-term split is:

1. Rust defines the authoritative lifecycle resolution path, validated component/profile schema, and the shader/material family ABI used to render effects.
2. Lua authors high-level content choices such as `effect_preset_id`, destruction-profile selection, render-layer/post-process composition, and exceptional scripted behavior.
3. Gameplay entities should reference effect presets or destruction/loot/fracture profiles, not raw shader asset paths, `effect_kind` integers, or packed uniform layouts.
4. Default behavior for common content such as asteroid explosions should be data-driven from authored components/profiles consumed by Rust destruction systems, not ad hoc Lua code that directly chooses a shader on every death.
5. Lua event hooks are the escalation path for exceptional behavior: for example canceling a normal death, restoring health, spawning escorts, emitting mission events, or selecting an alternate authored effect/loot outcome.
6. Scripts emit intents only. Scripts must not directly mutate authoritative ECS state, bypass destruction resolution, or bind directly to low-level material ABI details.

Recommended authoring shape:

1. Keep a Rust-defined destruction/lifecycle profile component or equivalent bundle-authored profile reference on the entity/archetype.
2. Put default VFX/fracture/loot selection in that profile.
3. Use `ScriptState.data.event_hooks` only when a specific entity/archetype needs custom lifecycle logic beyond the default profile.

### 2.4 Asset Registry Authority Contract

Lua scripting also owns authoritative content asset registry definitions:

1. Asset IDs, dependencies, bootstrap-required policies, and source references are authored in Lua registry scripts.
2. Rust runtime systems validate and consume generated catalog metadata; Rust must not define hardcoded per-asset runtime maps/lists.
3. Gameplay/script payloads reference logical `asset_id` only; gateway delivery resolves immutable `asset_guid` and serves payloads via `/assets/<asset_guid>`.
4. Script-authored registry changes must flow through catalog build/publish tooling before activation.

### 2.5 Component Extensibility Rules

To keep scripting powerful without breaking persistence/replication contracts:

1. Scripts may define and spawn content bundles/templates composed from existing registered component kinds.
2. Scripts may override allowed fields on allowed component kinds at spawn time.
3. Scripts may not introduce brand-new runtime component types in authoritative simulation.
4. Unknown/unregistered component kinds are rejected by spawn validation.
5. Restricted authority-sensitive fields (identity/ownership/session binding/core motion authority internals) are server-assigned or rejected when provided by script input.

Implication for mods:

- "New entity type" in mod terms means a new data/script archetype built from registered components and variant overlays.
- Truly new authoritative ECS component families require Rust component authoring, registry integration, and persistence/replication coverage through the normal workflow.

### 2.6 Generic Script-State Components (Planned)

Provide generic script-friendly persisted components (e.g. `ScriptState { data: HashMap<String, ScriptValue> }`) so mods can track custom per-entity logic state without requiring new Rust component types. This is the single biggest enabler for mod diversity and should be implemented before the modding phase.

Example policy-aligned pattern:

- A scripted archetype may include `Health` plus `Indestructible`.
- Damage systems in Rust check `Indestructible` and suppress health reduction/destruction.
- Behavior remains deterministic and authoritative because component semantics are implemented in Rust systems.

### 2.7 Script Execution Model: Event-Driven with Read-Only Queries

Scripts follow a hybrid execution model: event-driven execution triggers with read-only world access within handlers. This is the same model used by Factorio, WoW, and Roblox.

Three alternative models were evaluated:

| Model | Description | Verdict |
|---|---|---|
| System-style | Scripts define "systems" that iterate entities with component filters on a schedule | Rejected -- unbounded iteration, hard to budget, blurs authority boundary |
| Purely event-driven | Scripts only receive pre-packaged event payloads, no world access | Rejected -- too limiting; AI/economy logic needs to inspect world state to make decisions |
| **Hybrid (accepted)** | **Event-driven execution + read-only world queries within handlers + intent-only writes** | **Accepted** -- clean authority boundary, expressive enough for real content, budgetable |

#### 2.7.1 Execution: Event-Driven

Rust controls when script code runs. Scripts never poll or run in a loop. Two trigger mechanisms:

1. **Gameplay events**: Rust systems emit events that dispatch to registered Lua handlers (`on_entity_damaged`, `on_system_enter`, `on_mission_objective_complete`, etc.).
2. **Registered intervals**: scripts register recurring timer callbacks via `ctx.events:register_interval(name, seconds, handler)`. The Rust scheduler owns timing and respects instruction budgets.

Handlers are short-lived: a handler runs, reads world state, emits intent, and returns. No persistent coroutines or blocking waits.

#### 2.7.2 World Access: Read-Only Queries Within Handlers

When a handler fires, it can read any component on any entity on the authoritative host. This is safe because:

- Scripts only run on the authoritative server. There is no client-side data leak risk.
- Read-only access cannot break invariants. Only intent emission can change state.
- The instruction budget constrains how many reads a handler can do per invocation. Entity lookups and queries consume instructions like any other operation.
- Scripts receive `ScriptEntity` wrappers with `guid()`, `position()`, `get(component_kind)` methods. No raw Bevy `Entity` handles, `&World` references, or `Query<>` types cross the boundary.

Read API returns point-in-time snapshots. Scripts cannot hold references across ticks.

**Snapshot boundary**: The script world snapshot is built at the start of each FixedUpdate tick, after the previous tick's physics writeback has propagated. Scripts see world state as of the prior tick's physics resolution. Intent application occurs before the current tick's physics step, so script-driven motion is reflected in the same tick's physics. The replication visibility system evaluates positions after the current tick's physics writeback. Net effect: no one-tick visibility lag for script-driven motion. See `docs/plans/spatial_partitioning_implementation_plan.md` section 10.3 for the full schedule diagram.

#### 2.7.3 Mutations: Intent-Only (Including Privileged Script Actions)

All state changes go through `ctx:emit_intent(action, payload)` (or equivalent validated script-action entrypoints). This queues a validated action through the same authority pipeline as player input. The server processes it like any other gameplay action.

Scripts do not mutate ECS storage directly. However, privileged script actions may request authoritative mutations such as teleporting entities, setting transform state, or writing allowlisted component fields.

Invalid intent is rejected and logged. The script continues executing (intent rejection is not a script error).

#### 2.7.4 Script State Persistence

Scripts read and write per-entity custom state through the `ScriptState` component:

- **Read**: `entity:get("ScriptState")` returns the current state table (or nil if absent).
- **Write**: `ctx:emit_intent("set_script_state", { entity_id = id, key = "patrol_index", value = 3 })`.

`ScriptState` persists across ticks and across server restarts via graph persistence. This allows scripts to maintain stateful logic (patrol waypoint index, mission progress, economy accumulators) without requiring new Rust component types.

#### 2.7.5 Example: NPC AI Patrol/Engage/Flee

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

#### 2.7.6 Example: Economy Station Tick

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

#### 2.7.7 Example: Mission Lifecycle

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

### 3.1.1 Scripted UI Presentation Contract

Narrative and quest-facing UI should be scriptable through Lua-authored content, but the authority split remains strict:

1. Lua on the authoritative host decides **what** should be shown.
2. Rust validates and emits/replicates a client-facing presentation payload.
3. The native client `sidereal-ui` layer decides **how** to render that payload.
4. Player responses are sent back to the authoritative host as validated intent/choice messages.

This applies to future scripted experiences such as:

- contact/portrait dialogs,
- transmissions and hails,
- progressive text reveal with skip-to-end,
- branching dialogue choices,
- mission board / job offer panels,
- quest acceptance/completion prompts,
- scripted alerts or faction notifications.

Required rule:

- Lua must not directly mutate client UI trees or issue arbitrary client-side rendering commands.
- Lua authors declarative presentation payloads and dialogue state transitions; Rust/client UI renderers consume those payloads through allowlisted schemas.

Illustrative future payload fields:

- `presentation_kind`
- `dialog_id`
- `speaker_name`
- `portrait_asset_id`
- `body_text`
- `reveal_mode`
- `allow_skip_reveal`
- `choices = [{ id, label, hotkey }]`
- optional allowlisted theme/style hints

Branch handling contract:

1. The server/script sends a presentation payload with available choice IDs.
2. The client renders those choices and captures local input only as a selection request.
3. The authoritative host validates the selected choice for the current dialogue state.
4. Lua quest/dialogue logic advances to the next branch and emits the next presentation payload or resulting gameplay intent.

This preserves server authority while still allowing heavily script-authored dialogue and mission UX.

### 3.2 Bad Candidates (Stay in Rust/ECS)

| Domain | Why NOT Scriptable |
|---|---|
| Physics simulation | 60 Hz hot path, server-authoritative, performance-critical |
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

The event bridge is the Rust-to-Lua dispatch layer that triggers script handlers in response to authoritative gameplay events. Combined with the hybrid execution model (section 2.7), this is the primary runtime interface between ECS systems and script content.

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
| `inventory_changed` | `{ entity_id, owner_player_id?, delta, reason }` | Inventory/cargo systems |
| `cargo_collected` | `{ player_id, ship_entity_id, item_id, quantity, source_entity_id? }` | Loot/mining/salvage |
| `cargo_delivered` | `{ player_id, ship_entity_id, item_id, quantity, destination_entity_id }` | Trade/mission delivery |
| `docked` | `{ player_id, ship_entity_id, station_entity_id }` | Docking system |
| `undocked` | `{ player_id, ship_entity_id, station_entity_id }` | Docking system |
| `interaction_completed` | `{ player_id, entity_id, interaction_id }` | Generic interaction/action system |
| `gain_entity_visibility` | `{ observer_entity_id, target_entity_id, confidence? }` | Visibility/scanner systems |
| `lose_entity_visibility` | `{ observer_entity_id, target_entity_id, last_known_position }` | Visibility/scanner systems |

#### Spatial Partition Events (Future)

When the spatial partition tracks entity cell membership across ticks, derived events become available as script event sources. Raw cell events (`entered_cell`, `left_cell`) are not exposed to scripts — cell membership is an implementation detail. Instead, meaningful derived events are emitted:

| Event | Payload | Rate | Source |
|---|---|---|---|
| `entered_system` | `{ entity_id, solar_system_id }` | Max 1 per entity per second | 1 Hz solar system context check |
| `left_system` | `{ entity_id, solar_system_id }` | Max 1 per entity per second | 1 Hz solar system context check |
| `deep_space_enter` | `{ entity_id }` | Max 1 per entity per 5s | Entity exits all system radii |
| `approach_body` | `{ entity_id, body_id, body_kind, distance }` | Max 1 per entity per body per 5s | Entity enters proximity radius of celestial body |

Throttle policy is enforced in the Rust event emitter, not in the Lua handler. Oscillation across a boundary within the cooldown window emits at most one event pair.

These events are gated behind the spatial partition cell-tracking data structure (see `docs/plans/spatial_partitioning_implementation_plan.md` section 10.6). They are not available until the partition implementation includes persistent cell assignment tracking.

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
- `world_defaults` describing authored render layers/rules and starter bundle defaults, and
- `build_graph_records(ctx)` returning authoritative render-layer graph records (currently including fullscreen background layers, default `main_world`, `midground_planets`, the planet assignment rule, and shader-setting payloads for starfield/space background content adapters).
- deterministic world-content bundle spawning (currently pirate patrol + asteroid field members).
- `world_defaults` must reference logical `asset_id` values only; bootstrap-required behavior is declared in the Lua asset registry, not a Rust-maintained or ad-hoc list.

Migration note:
- `world_init.lua` no longer depends on fixed `space_background_shader_asset_id` / `starfield_shader_asset_id` fields. It authors the background, world, and rule definitions directly through `ctx.render:define_layer(...)` / `ctx.render:define_rule(...)`.
- Fullscreen background and fullscreen foreground layer execution now comes from those authored layer definitions. Camera-scoped post-process stacks are also authored data, but their currently supported shader adapters are limited to the existing fullscreen shader families until the fully generic runtime material path replaces the remaining content-specific adapters.
- The next render-scripting migration step is not "more fullscreen layers"; it is authored multi-pass visual stacks so layered content like planets/clouds/rings can be expressed as script-authored pass composition rather than bespoke Rust child-pass orchestration. That path is tracked in `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`.

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

The runtime API is exposed to Lua handlers via the `ctx` object passed to every handler invocation. It follows the hybrid execution model (section 2.7): read-only world access plus intent-only writes.

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

`query_nearby` uses the replication spatial partition grid (see `docs/plans/spatial_partitioning_implementation_plan.md`). It does NOT iterate the full world entity set. The pipeline is:

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

#### Quest/Inventory Query Helpers (Planned v1)

For scripted mission/quest progression, the runtime should expose dedicated helper queries in addition to generic entity/component lookups.

| Method | Return | Notes |
|---|---|---|
| `ctx.world:get_active_ship(player_id)` | `ScriptEntity or nil` | Resolves current controlled/active ship for that player. |
| `ctx.world:get_inventory(entity_id)` | `table or nil` | Snapshot of `Inventory` payload for entity. |
| `ctx.world:get_item_count(entity_id, item_id)` | `number` | Fast count helper (0 if absent). |
| `ctx.world:get_player_active_quests(player_id)` | `{QuestInstanceSummary, ...}` | Owner-scoped active instances only. |
| `ctx.world:get_quest_instance(quest_instance_id)` | `QuestInstanceSnapshot or nil` | Includes objective states/counters. |
| `ctx.world:quest_objective_progress(quest_instance_id, objective_id)` | `table or nil` | Counter/progress snapshot for one objective. |
| `ctx.world:get_entities_in_range(entity_id, radius, filter?)` | `{ScriptEntity, ...}` | Convenience wrapper around `query_nearby` using entity position as center. |
| `ctx.world:last_known_position(observer_entity_id, target_entity_id)` | `{x,y} or nil` | Visibility-memory helper for search behavior. |

Server-side scripts may read any authoritative entity/component state through existing query APIs; these helper calls provide canonical, validated shortcuts for common quest logic.

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
| `"accept_quest"` | `{ player_id, quest_template_id }` | Create player-scoped quest instance from template. |
| `"abandon_quest"` | `{ player_id, quest_instance_id }` | Abandon active quest instance. |
| `"advance_quest_objective"` | `{ quest_instance_id, objective_id, delta \| set_value }` | Update objective counters/progress. |
| `"complete_quest"` | `{ quest_instance_id, rewards? }` | Mark quest complete; rewards validated/applied in Rust. |
| `"fail_quest"` | `{ quest_instance_id, reason }` | Mark quest failed. |
| `"consume_inventory_item"` | `{ entity_id, item_id, quantity, reason }` | Remove items after validation (non-negative, available quantity). |
| `"grant_inventory_item"` | `{ entity_id, item_id, quantity, reason }` | Add items through canonical inventory system path. |
| `"transfer_inventory_item"` | `{ from_entity_id, to_entity_id, item_id, quantity, reason }` | Atomic server-validated transfer path. |

The intent action set is extensible by adding Rust-side intent handlers. Scripts cannot invent new intent actions; unknown actions are rejected.

#### Privileged Mutation Actions (Planned)

For scenario control, scripted events, and quest orchestration, the runtime should support privileged server-only mutation actions behind strict validation:

| Action | Payload | Typical use |
|---|---|---|
| `"teleport_entity"` | `{ entity_id, position, rotation?, zero_velocity? }` | Warp ship/player/NPC for mission transitions, cutscenes, admin recovery. |
| `"set_entity_transform"` | `{ entity_id, position?, rotation? }` | Scripted positioning for encounters/cinematics. |
| `"set_entity_velocity"` | `{ entity_id, linear_velocity?, angular_velocity? }` | Controlled impulse/launch effects. |
| `"batch_move_entities"` | `{ moves = [{ entity_id, position, rotation? }, ...] }` | Move fleets/waves atomically. |
| `"set_component_fields"` | `{ entity_id, component_kind, patch }` | Generic allowlisted field patch for non-authority-sensitive data. |
| `"despawn_entities_in_region"` | `{ bounds, filter, reason }` | Cleanup transient encounter entities. |
| `"spawn_from_template"` | `{ template_id, spawn_context }` | Deterministic event/encounter generation from script-owned templates. |
| `"set_visibility_override"` | `{ entity_id, mode, duration_s? }` | Scripted stealth/reveal event rules (policy-gated). |
| `"scanner_ping"` | `{ entity_id, radius_m?, duration_s? }` | Trigger active scan pulse/search sweep. |
| `"set_ai_mode"` | `{ entity_id, mode, reason? }` | Explicit AI-mode transitions for behavior trees/state machines. |

Safety rules for privileged actions:
1. Server-only execution path; never client-authoritative.
2. Explicit allowlist by action and component/field.
3. Schedule-aware writes (applied at deterministic stage before physics prepare).
4. Audit log entries for privileged mutation actions.
5. Denylist for identity/auth/session-binding and other restricted fields.
6. One-shot Avian override support is allowed (for teleport/snap), but continuous writer conflicts must be prevented.

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

### 8.6 Quest Hooks, Steps, and Counter Objectives (Planned v1)

This section defines the hook and data model needed for script-authored quests like:
"Fly to X, collect Y, return to Z and deliver."

#### 8.6.1 Objective Model

Each quest instance stores objective entries:

```lua
{
  objective_id = "collect_uranium",
  kind = "collect_item",
  item_id = "resource.uranium",
  required = 10,
  current = 5,
  completed = false,
}
```

Supported objective kinds (v1):
1. `visit_entity` / `visit_region`
2. `collect_item`
3. `deliver_item`
4. `interact_with_entity`
5. `kill_target` / `destroy_target`

Counter objectives (`collect_item`, `kill_target`, etc.) always track `current/required` and should be replicated for UI strings like `Collect 5/10 Uranium`.

#### 8.6.2 Quest Hook Lifecycle

Quest scripts should be able to implement:
1. `on_accept(ctx, quest)` - initialize instance state/objectives.
2. `on_event(ctx, quest, event)` - react to gameplay events (`inventory_changed`, `docked`, etc.).
3. `on_tick(ctx, quest)` - optional periodic consistency checks.
4. `can_complete(ctx, quest)` - hard validation gate before completion.
5. `on_complete(ctx, quest)` - apply rewards/cleanup.
6. `on_fail(ctx, quest, reason)` - failure handling/cleanup.

#### 8.6.3 Completion Gate Rules

`can_complete` must enforce all required conditions, for example:
1. all prior objectives marked complete,
2. active ship inventory contains required items,
3. player is at required destination/station.

Rust still performs final authoritative validation when completion intents are processed.

#### 8.6.4 Example: Collect 10 Uranium then Deliver

```lua
local Quest = {}

Quest.template_id = "starter_uranium_run_v1"

function Quest.on_accept(ctx, quest)
  ctx:emit_intent("advance_quest_objective", {
    quest_instance_id = quest.id,
    objective_id = "collect_uranium",
    set_value = { current = 0, required = 10, completed = false },
  })
end

function Quest.on_event(ctx, quest, event)
  if event.type == "cargo_collected" and event.item_id == "resource.uranium" then
    local ship = ctx.world:get_active_ship(event.player_id)
    if not ship then
      return
    end
    local count = ctx.world:get_item_count(ship:guid(), "resource.uranium")
    ctx:emit_intent("advance_quest_objective", {
      quest_instance_id = quest.id,
      objective_id = "collect_uranium",
      set_value = {
        current = count,
        required = 10,
        completed = count >= 10,
      },
    })
  end

  if event.type == "docked" and event.station_entity_id == quest.data.turn_in_station_id then
    if Quest.can_complete(ctx, quest, event.player_id) then
      local ship = ctx.world:get_active_ship(event.player_id)
      ctx:emit_intent("consume_inventory_item", {
        entity_id = ship:guid(),
        item_id = "resource.uranium",
        quantity = 10,
        reason = "quest_turn_in:" .. quest.id,
      })
      ctx:emit_intent("complete_quest", {
        quest_instance_id = quest.id,
        rewards = { credits = 2500, reputation = { miners_guild = 5 } },
      })
    end
  end
end

function Quest.can_complete(ctx, quest, player_id)
  local objectives = ctx.world:get_quest_instance(quest.id).objectives
  for _, objective in ipairs(objectives) do
    if not objective.completed then
      return false
    end
  end

  local ship = ctx.world:get_active_ship(player_id)
  if not ship then
    return false
  end
  local uranium = ctx.world:get_item_count(ship:guid(), "resource.uranium")
  return uranium >= 10
end

return Quest
```

#### 8.6.5 Validation and Safety Notes

1. Script logic may propose inventory/quest mutations, but Rust authority handlers must validate ownership, range/context, and quantity bounds.
2. `consume_inventory_item` must fail closed if available quantity is insufficient at commit time.
3. Quest objective counters are eventually consistent with gameplay events; scripts may recompute counters from authoritative inventory snapshots to self-heal missed events.
4. Multi-player isolation is mandatory: per-player quest instances must never share mutable objective state unless explicitly marked party-shared.

#### 8.6.6 Example AI Pattern: Threat Memory + Pursuit + Search + Give Up

The following pseudocode demonstrates the style needed for pirate/NPC combat logic:
1. maintain a war-list array (`at_war_with`) rather than one target,
2. decay hostility over time,
3. pursue last-seen position after visibility loss,
4. active scan attempt,
5. return to patrol if search fails.

```lua
local PirateCombat = {}

PirateCombat.handler_name = "pirate_combat"
PirateCombat.tick_interval_seconds = 0.5

local WAR_DECAY_S = 180.0
local ATTACK_RADIUS_M = 8000.0
local SEARCH_GIVE_UP_S = 20.0

local function now_s(ctx)
  return ctx.time:now_s()
end

local function read_state(entity)
  return entity:get("script_state") or { data = {} }
end

local function get_war_list(state)
  local list = state.data.at_war_with
  if list == nil then
    list = {}
  end
  return list
end

local function persist_war_list(ctx, entity_id, war_list)
  ctx:emit_intent("set_script_state", {
    entity_id = entity_id,
    key = "at_war_with",
    value = war_list,
  })
end

local function find_war_entry(war_list, target_id)
  for i, entry in ipairs(war_list) do
    if entry.target_id == target_id then
      return i, entry
    end
  end
  return nil, nil
end

function PirateCombat.on_damage_applied(ctx, event)
  local me = ctx.world:find_entity(event.entity_id)
  if me == nil then
    return
  end

  local source_id = event.shooter_entity_id
  if source_id == nil then
    return
  end

  local state = read_state(me)
  local war = get_war_list(state)
  local idx, entry = find_war_entry(war, source_id)
  if entry == nil then
    entry = {
      target_id = source_id,
      hostility = 0,
    }
    table.insert(war, entry)
    idx = #war
  end
  war[idx].hostility = math.min((war[idx].hostility or 0) + event.damage, 1000)
  war[idx].last_seen_s = now_s(ctx)
  war[idx].last_seen_pos = event.source_position -- if event provides it

  persist_war_list(ctx, event.entity_id, war)
  ctx:emit_intent("set_ai_mode", {
    entity_id = event.entity_id,
    mode = "attack",
    reason = "took_damage",
  })
end

function PirateCombat.on_gain_entity_visibility(ctx, event)
  local me = ctx.world:find_entity(event.observer_entity_id)
  if me == nil then
    return
  end
  local target = ctx.world:find_entity(event.target_entity_id)
  if target == nil then
    return
  end

  local state = read_state(me)
  local war = get_war_list(state)
  local _, entry = find_war_entry(war, event.target_entity_id)
  if entry ~= nil then
    entry.last_seen_s = now_s(ctx)
    entry.last_seen_pos = target:position()
    persist_war_list(ctx, event.observer_entity_id, war)
  end
end

function PirateCombat.on_lose_entity_visibility(ctx, event)
  local me = ctx.world:find_entity(event.observer_entity_id)
  if me == nil then
    return
  end

  local state = read_state(me)
  local war = get_war_list(state)
  local _, entry = find_war_entry(war, event.target_entity_id)
  if entry == nil then
    return
  end

  entry.last_seen_s = now_s(ctx)
  entry.last_seen_pos = event.last_known_position
  entry.search_started_s = now_s(ctx)
  persist_war_list(ctx, event.observer_entity_id, war)

  -- Move to last known position immediately.
  ctx:emit_intent("fly_towards", {
    entity_id = event.observer_entity_id,
    target_position = event.last_known_position,
  })

  -- Trigger scanner sweep while searching.
  ctx:emit_intent("scanner_ping", {
    entity_id = event.observer_entity_id,
    radius_m = 12000,
    duration_s = 3.0,
  })
end

function PirateCombat.on_tick(ctx, event)
  local me = ctx.world:find_entity(event.entity_id)
  if me == nil then
    return
  end
  local pos = me:position()
  local state = read_state(me)
  local war = get_war_list(state)
  local t = now_s(ctx)

  -- Decay and prune hostility entries (array form).
  local i = #war
  while i >= 1 do
    local entry = war[i]
    local age = t - (entry.last_seen_s or t)
    if age > WAR_DECAY_S then
      table.remove(war, i)
    else
      entry.hostility = math.max((entry.hostility or 0) - 0.5, 0)
      if entry.hostility <= 0 then
        table.remove(war, i)
      end
    end
    i = i - 1
  end

  -- Select highest-hostility visible target in range.
  local candidates = ctx.world:get_entities_in_range(event.entity_id, ATTACK_RADIUS_M, {
    has_any = { "ship_tag", "player_tag" },
    limit = 30,
  })
  local best_target = nil
  local best_score = -1
  for _, entity in ipairs(candidates) do
    local guid = entity:guid()
    local _, entry = find_war_entry(war, guid)
    if entry ~= nil then
      local score = entry.hostility or 0
      if score > best_score then
        best_score = score
        best_target = entity
      end
    end
  end

  if best_target ~= nil then
    ctx:emit_intent("set_ai_mode", { entity_id = event.entity_id, mode = "attack" })
    ctx:emit_intent("fly_towards", {
      entity_id = event.entity_id,
      target_position = best_target:position(),
    })
    persist_war_list(ctx, event.entity_id, war)
    return
  end

  -- No visible target: pursue/search last known position for remaining entries.
  local chase = nil
  for _, entry in ipairs(war) do
    if entry.last_seen_pos ~= nil then
      chase = entry
      break
    end
  end

  if chase ~= nil then
    ctx:emit_intent("set_ai_mode", { entity_id = event.entity_id, mode = "search" })
    ctx:emit_intent("fly_towards", {
      entity_id = event.entity_id,
      target_position = chase.last_seen_pos,
    })
    if chase.search_started_s ~= nil and (t - chase.search_started_s) > SEARCH_GIVE_UP_S then
      -- Give up on this target after timed search window.
      chase.hostility = 0
      chase.last_seen_pos = nil
    end
    persist_war_list(ctx, event.entity_id, war)
    return
  end

  -- No threats left: return to patrol behavior.
  ctx:emit_intent("set_ai_mode", { entity_id = event.entity_id, mode = "patrol" })
  persist_war_list(ctx, event.entity_id, war)
end

return PirateCombat
```

Implementation notes:
1. This example stores `at_war_with` as an array/list so it is straightforward to inspect in dashboard tooling.
2. If the list grows large, Rust-side helper APIs can provide optimized target-index lookup while preserving list semantics in script-visible state.

### 8.7 Context Exposure Expansion Matrix (Planned)

This matrix explicitly documents what must be added to `ctx` to support near-term questing and longer-term genre-agnostic scripting (dynamic events, procedural generation, etc.).

#### 8.7.1 Questing and Progression

Required new `ctx` surface:
1. `ctx.world:get_active_ship(player_id)`
2. `ctx.world:get_inventory(entity_id)`
3. `ctx.world:get_item_count(entity_id, item_id)`
4. `ctx.world:get_player_active_quests(player_id)`
5. `ctx.world:get_quest_instance(quest_instance_id)`
6. `ctx.world:quest_objective_progress(quest_instance_id, objective_id)`

Required new intents/events:
1. Intents: `accept_quest`, `abandon_quest`, `advance_quest_objective`, `complete_quest`, `fail_quest`, `consume_inventory_item`, `grant_inventory_item`, `transfer_inventory_item`
2. Events: `inventory_changed`, `cargo_collected`, `cargo_delivered`, `docked`, `interaction_completed`

Required Rust-side support:
1. Persisted quest instance components/resources.
2. Owner-scoped replication for quest journal/progress state.
3. Validation handlers for quest/inventory intents.

#### 8.7.2 Dynamic World Events

Required new `ctx` surface:
1. `ctx.events:emit_world_event(event_id, payload, scope?)`
2. `ctx.world:query_in_system(system_id, filter)` (planned in Phase D)
3. `ctx.world:find_system_at(position)` (planned in Phase D)

Required new intents/events:
1. Intents: `spawn_entity`, `despawn_entity`, `emit_event` (expanded payload/schema validation)
2. Events: `entered_system`, `left_system`, `deep_space_enter`, `approach_body` (already tracked as future partition-derived events)

Required Rust-side support:
1. Event rate limiting/cooldowns and dedupe.
2. Shard-safe event emission routing.
3. Observability for script-generated event chains.

#### 8.7.3 Procedural Generation (Asteroids, Encounters, Zones)

Current baseline (implemented):
1. Deterministic asteroid field generation at world bootstrap is live in `world_init.lua` by looping and calling `spawn_bundle_graph_records("asteroid.field_member", overrides)`.
2. This runs under the existing one-time `script_world_init_state` guard, so generated world content is idempotent across restarts.
3. Determinism currently uses Lua hash helpers (`hash01(index, salt)`); Rust-seeded RNG helper exposure is still future work.

Required new `ctx` surface:
1. `ctx.world:query_region(bounds, filter)` (or equivalent bounded spatial query helper)
2. `ctx.world:is_region_seeded(seed_key)` / `ctx.world:mark_region_seeded(seed_key)` (idempotent generation guards)
3. deterministic RNG helper exposed from Rust (`ctx.rand:next_*`) seeded by world/shard/region keys

Required new intents/events:
1. Intents: `spawn_entity` with archetype/variant + deterministic override payloads
2. Events: region/system bootstrap hooks (`region_unloaded`, `region_loaded`, `system_bootstrap`) as needed

Required Rust-side support:
1. Deterministic seed strategy contract (world seed + region coordinates + content version).
2. Idempotent persistence checks to avoid duplicate generation on restart/rejoin.
3. Budget controls for generation bursts (per tick/per region spawn caps).

#### 8.7.4 Cross-Genre Authoring Support

Required new `ctx` surface:
1. richer read-only component access in `ScriptEntity:get(kind)` beyond `script_state`
2. generic label/faction/component filter queries (already in target API model)
3. stable script API version negotiation per bundle

Required Rust-side support:
1. Script API compatibility checks on activation/join.
2. Schema validation and strict fail-closed behavior for unknown actions/events.
3. Native/WASM client parity for replicated script-driven outcomes (clients render state, not authority).

#### 8.7.5 High-Value Scripted Operations to Support

The following operations are likely needed across Sidereal and future non-space projects:

1. **Quest/campaign orchestration**
   - teleport player/party to mission phase area,
   - atomically advance multiple objectives,
   - consume/transfer required turn-in items,
   - spawn/despawn objective entities with stable references.
2. **Dynamic encounter control**
   - spawn enemy waves from templates with deterministic seeds,
   - retreat/warp waves on condition,
   - region cleanup after encounter completion/failure.
3. **Procedural world generation**
   - deterministic asteroid/debris field generation per region seed,
   - idempotent "generate once" guards,
   - regeneration policies for depleted/cleared regions.
4. **Live world events**
   - timed faction incursions or trade surges,
   - temporary hazard zones with scripted effects,
   - scripted server announcements and event-state replication.
5. **Cinematic and narrative moments**
   - batch move + orientation alignment for fleets/NPCs,
   - lockstep trigger chains (arrive -> dialogue -> spawn -> combat start),
   - deterministic rollback-safe event transitions.
6. **Admin/ops recovery tools**
   - recover stuck entities via server-side teleport,
   - scripted cleanup of invalid transient entities,
   - deterministic repair actions (rebuild missing encounter state from templates).

### 8.8 Trigger and Dialogue Orchestration Model (Planned)

This section defines how quests/dialogue can be triggered beyond simple "add quest" actions.

#### 8.8.1 Trigger Sources to Support

Scripts should be able to react to these trigger families (common across RPG/MMO designs):

1. **Spatial trigger volumes**
   - player enters/exits script-defined radius/shape around a trigger entity.
2. **Timer-based triggers**
   - player accumulated playtime thresholds,
   - quest elapsed time windows,
   - world/event schedule windows.
3. **Interaction triggers**
   - explicit interact with NPC/object/terminal.
4. **Progression triggers**
   - objective completion, quest chain prerequisite completion, reputation threshold.
5. **Inventory/economy triggers**
   - acquire/lose item, cargo threshold reached, delivery confirmation.
6. **Combat triggers**
   - damage received, target destroyed, survived ambush window.
7. **World-state triggers**
   - faction control changed, station state changed, dynamic event phase changes.
8. **Randomized deterministic triggers**
   - probability-based procs using deterministic seed inputs.

#### 8.8.2 Recommended Trigger Events

Add/standardize these events in the bridge:

1. `enter_script_trigger` `{ player_id, trigger_entity_id }`
2. `exit_script_trigger` `{ player_id, trigger_entity_id }`
3. `dialog_response_submitted` `{ player_id, dialog_id, choice_id }`

Note: `gain_entity_visibility` is useful for AI perception logic, but explicit trigger-volume events should be the primary mechanism for authored area triggers.
Timer threshold events are optional convenience events; creator-authored timer logic should prefer script-entity `on_tick` checks for maximum flexibility.

#### 8.8.2A Preferred Authoring Pattern: Script-Entity Tick Timers

Preferred design for quest creators:
1. Place a script entity with `on_tick_handler`.
2. Read authoritative time/player state each tick.
3. Run custom threshold/cooldown logic in Lua.
4. Emit quest/dialog intents when conditions pass.

This keeps timer logic fully data/script-authored instead of hardcoding many one-off Rust timer event types.

Required time helpers for this pattern:
1. `ctx.time:now_s()`
2. `ctx.time:world_accumulated_s()`
3. `ctx.time:player_accumulated_s(player_id)`
4. optional helper: `ctx.state:cooldown_ready(entity_id, key, duration_s)`

Example pseudocode:

```lua
local Trigger = {}
Trigger.handler_name = "trigger_quest_34234"
Trigger.tick_interval_seconds = 1.0

function Trigger.on_tick(ctx, event)
  local trigger = ctx.world:find_entity(event.entity_id)
  if trigger == nil then
    return
  end

  local players = ctx.world:get_entities_in_range(event.entity_id, 2500, {
    has = "player_tag",
    limit = 128,
  })

  local now = ctx.time:now_s()
  for _, player in ipairs(players) do
    local player_id = player:guid()
    local play_s = ctx.time:player_accumulated_s(player_id)

    -- Creator-authored timer rule: only after 20m playtime.
    if play_s >= 1200 and ctx.state:cooldown_ready(event.entity_id, "offer:" .. player_id, 600) then
      ctx:emit_intent("offer_quest", {
        player_id = player_id,
        quest_template_id = "quest_34234",
      })
    end
  end
end

return Trigger
```

#### 8.8.3 Blocking (Must-Respond) Dialogue Contract

For flows where player must respond before proceeding:

1. Lua emits `start_dialog` intent with blocking policy:
   - `{ player_id, dialog_id, nodes, blocking = true, timeout_s?, default_choice_id? }`
2. Rust creates authoritative active-dialog state for the player.
3. Client receives replicated dialog ticket and enters blocking UI mode.
4. While blocking dialog is active, server rejects blocked gameplay intents (movement/fire/interaction as configured).
5. Player must choose a valid response (or timeout policy resolves).
6. Rust emits `dialog_response_submitted`, Lua continues script branch.

This preserves server authority while enabling "you must respond" narrative gates.

Blocking dialog pseudocode (forced response gate):

```lua
function Trigger.start_forced_dialog(ctx, player_id)
  ctx:emit_intent("start_dialog", {
    player_id = player_id,
    dialog_id = "distress_call_intro_v1",
    blocking = true,
    timeout_s = 45.0,
    default_choice_id = "decline",
    nodes = {
      {
        id = "root",
        text = "Unidentified vessel, respond immediately.",
        choices = {
          { id = "accept", text = "We'll help." },
          { id = "decline", text = "Not interested." },
        },
      },
    },
  })
end

function Trigger.on_dialog_response_submitted(ctx, event)
  if event.dialog_id ~= "distress_call_intro_v1" then
    return
  end
  if event.choice_id == "accept" then
    ctx:emit_intent("accept_quest", {
      player_id = event.player_id,
      quest_template_id = "quest_34234",
    })
  else
    ctx:emit_intent("fail_quest", {
      quest_instance_id = "quest_34234:" .. event.player_id,
      reason = "Player declined distress call",
    })
  end
end
```

#### 8.8.4 Rust Systems Required for 8.8

1. Trigger-volume detection system (radius/shape enter/exit) with per-player dedupe.
2. Deterministic time exposure to scripts (`ctx.time` helpers) including player accumulated playtime persisted on player entity.
3. Dialog runtime components (`ActiveDialog`, dialog node state, blocking policy).
4. Input-gate validation in authoritative intent processing while blocking dialog is active.
5. Replication path for dialog tickets and quest/dialog notifications to client UI.
6. Idempotency/cooldown helpers so tick-driven scripts cannot repeatedly open the same flow.

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
| Asset delivery | Scripts own Lua asset registry definitions and reference logical `asset_id` values only; gateway serves payloads by immutable `asset_guid` |
| Visibility/replication | Script-spawned entities pass through normal visibility and replication policy |

### 10.1 Background World Simulation Integration

Update note (2026-03-11):
- Background/offscreen world simulation now has an explicit feature contract in `docs/features/background_world_simulation_contract.md`.
- This section defines the intended ABI-vs-Lua boundary for that system so it does not drift into either "everything hardcoded in Rust" or "scripts bypass the kernel".

The design goal is to maximize Lua authorship for economy behavior, faction behavior, mission generation, and encounter policy while keeping authority, determinism, persistence, and safety in the Rust ABI.

#### 10.1.1 High-Level Ownership Split

**Rust ABI owns the simulation kernel.**

Rust is the right owner for:

1. authoritative persistence schema and graph-record mapping for nodes, factions, abstract actors, mission state, and promoted runtime entities,
2. identity, ownership, and visibility invariants,
3. scheduler/timing infrastructure for low-cadence background steps and promotion/demotion boundaries,
4. spatial query/indexing infrastructure,
5. canonical transaction primitives for inventory, budget, cargo reservation, cargo transfer, delivery commit, and destruction/loss,
6. canonical promotion/demotion machinery,
7. weighted-resolution kernel for offscreen conflict once inputs are assembled,
8. validation and audit logging for all privileged mutation paths,
9. telemetry/metrics so tuning decisions are evidence-based rather than script-only guesswork.

**Lua owns policy, tuning, and authored world behavior.**

Lua is the right owner for:

1. faction goals and strategic priorities,
2. actor role selection (`trader`, `miner`, `pirate`, `escort`, `patrol`, `smuggler`, etc.),
3. route preference heuristics and destination choice,
4. price/demand response curves,
5. mission generation policy,
6. pirate/security escalation thresholds,
7. encounter template selection and narrative dressing,
8. hero actor behavior/state machines,
9. resource-regrowth policy formulas,
10. content-level cargo tables, encounter reward tables, and faction-flavor differences.

Rule of thumb:

1. If a behavior is about "what should this actor/faction/world pressure want to do?" it should bias toward Lua.
2. If a behavior is about "what authoritative state transitions are legal and how are they safely committed?" it should bias toward Rust.

#### 10.1.2 Recommended Boundary for Background Simulation

For background world simulation, the recommended split is:

**Rust ABI responsibilities**

1. `EconomicNode`, `FactionEconomyState`, `AbstractActor`, `TrafficLane`, and promotion-state schemas.
2. Route-progress bookkeeping (`origin`, `destination`, progress, ETA windows).
3. Finite inventory and finite budget accounting.
4. Reservation/commit semantics so two scripts cannot both "spend" the same cargo or budget.
5. Conflict-resolution execution from validated input summaries.
6. Promotion of abstract actors into full ECS entities and demotion back into abstract state.
7. Rebuild/recompute pipeline for derived lane heat and probability volumes.
8. Read/query budget enforcement for script access to background state.

**Lua responsibilities**

1. Decide when a faction prefers trade vs expansion vs defense vs piracy.
2. Decide which abstract actors take which jobs.
3. Choose which shortages become player-visible missions and how aggressively rewards escalate.
4. Decide how pirate factions react to high-value lanes.
5. Decide what convoy qualities/cargo profiles are typical for a faction or route.
6. Decide what kinds of security response a faction prefers.
7. Decide how hero actors differ from generic quanta in ambition, risk tolerance, and mission participation.

**Hybrid responsibilities**

Some systems should be kernel-driven but script-configured:

1. Offscreen conflict resolution:
   - Rust executes the weighted outcome kernel.
   - Lua authors weighting profiles, actor quality modifiers, retreat thresholds, loot preferences, and faction-specific risk appetite.
2. Promotion cargo/fleet realization:
   - Rust guarantees deterministic promotion from abstract state.
   - Lua provides archetype/template families and cargo profile tables.
3. Derived pressure:
   - Rust computes lane heat / piracy pressure / security pressure from authoritative world activity.
   - Lua decides what missions, events, and faction responses those pressures should trigger.

#### 10.1.3 What Should Not Live In Lua

The following should not be left to freeform Lua because they are kernel or security concerns:

1. direct graph persistence writes,
2. direct component storage mutation outside validated intents/actions,
3. promotion/demotion lifecycle ownership,
4. final inventory/budget/cargo commit semantics,
5. visibility authorization rules,
6. low-level replication payload control,
7. exact spatial-index ownership and query guardrails,
8. unrestricted actor creation that bypasses budget, ownership, or faction constraints.

#### 10.1.4 Recommended New Script Surfaces

The current `ctx.world` API is a strong base, but background world simulation likely needs domain-specific script surfaces so content authors are not forced to reconstruct economy state from generic entity reads alone.

Recommended additions:

**`ctx.economy`**

1. `ctx.economy:get_node(node_id)`
2. `ctx.economy:query_nodes(filter?)`
3. `ctx.economy:get_price(node_id, item_id)`
4. `ctx.economy:get_shortages(filter?)`
5. `ctx.economy:get_faction_budget(faction_id)`
6. `ctx.economy:get_lane(origin_id, destination_id)`
7. `ctx.economy:query_lanes(filter?)`
8. `ctx.economy:get_probability_volume(route_or_region_id)`

**`ctx.background`**

1. `ctx.background:get_actor(actor_id)`
2. `ctx.background:query_actors(filter?)`
3. `ctx.background:get_actor_route(actor_id)`
4. `ctx.background:get_actor_job(actor_id)`
5. `ctx.background:estimate_route_risk(origin_id, destination_id, cargo_profile?)`
6. `ctx.background:request_promotion(actor_id, reason)`

These should remain read-mostly helper surfaces. The mutation path should still be intents/actions, not direct write handles.

#### 10.1.5 Recommended Background-Sim Intents

The following intent family is the right shape for script customization without giving scripts raw authority bypasses:

1. `create_abstract_actor`
2. `assign_actor_job`
3. `assign_actor_route`
4. `reserve_node_inventory`
5. `commit_node_transfer`
6. `post_generated_mission`
7. `adjust_faction_budget`
8. `request_actor_promotion`
9. `retire_abstract_actor`
10. `apply_resource_regrowth`

Design rule:

1. An intent name should correspond to a meaningful domain operation with validation.
2. It should not be a disguised raw component patch unless the component/field is explicitly allowlisted and non-authority-sensitive.

#### 10.1.6 Data Authorship Model

To maximize customization while keeping ABI stable:

1. Rust should define the stable schema families.
2. Lua should author the policy and content data that fills those schemas.

Recommended authored data in Lua:

1. actor role templates,
2. faction doctrine profiles,
3. cargo profile tables,
4. mission template tables,
5. price elasticity / scarcity response tables,
6. pirate/security response weighting tables,
7. resource-field regeneration policy tables,
8. promotion encounter template families.

Recommended stable ABI in Rust:

1. actor state envelope,
2. node inventory/budget envelope,
3. lane-pressure envelope,
4. conflict input/output envelope,
5. promotion request/result envelope,
6. mission posting envelope.

This keeps scripting expressive while preserving binary/runtime compatibility and persistence sanity.

#### 10.1.7 Practical Design Rule For Sidereal

When deciding whether a new background-sim feature belongs in Rust ABI or Lua:

1. Put it in Rust if it defines invariants, persistence shape, authority validation, deterministic scheduling, or a reusable kernel primitive.
2. Put it in Lua if it is faction flavor, encounter flavor, economic tuning, mission logic, actor preference logic, or a content-specific response policy.
3. Split it if the engine needs a safe primitive but designers need to control its parameters or trigger conditions.

Examples:

1. "How does cargo theft cap at pirate hold size?" -> Rust ABI.
2. "How greedy is faction X, and when do they risk an ambush?" -> Lua.
3. "How is convoy quality translated into archetype selection on promotion?" -> Rust realization pipeline fed by Lua-authored template families.
4. "When does a shortage escalate into a player contract instead of being absorbed by quanta?" -> Lua over Rust-authored economy state.
5. "How are node budgets atomically debited?" -> Rust ABI.

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
   - `accounts/player_init.lua`: calls `player_init(ctx)` for ship bundle selection (`ship_bundle_id`).
   - `bundles/bundle_registry.lua`: loads bundle definitions and validates `required_component_kinds` against `generated_component_registry()`.
   - Account bootstrap now composes both the player entity record and starter ship graph records through Lua-authored bundle scripts selected by `accounts/player_init.lua`.
   - Script `context` includes `new_uuid()` for dynamic entity/module graph ID generation in Lua.
   - Lua-to-JSON recursive conversion is shared via `sidereal-scripting::lua_value_to_json`.
   - Gateway now resolves script execution through a cached in-memory `ScriptCatalogResource` instead of reading `.lua` files on every call.
   - Gateway authoritative load order is now:
     1. load the active script catalog from SQL tables if present,
     2. otherwise seed from disk and persist that seed set into SQL,
     3. fall back to disk only when the helper is used without a reachable database (for example isolated unit tests).
   - Gateway exposes `current_script_catalog(root)` and `reload_script_catalog_from_disk(root)` helpers; reload-from-disk now also replaces the active SQL-backed catalog.
   - Gateway starter-world bundle spawning and collision-outline helpers now resolve bundle/asset metadata from the cached catalog path rather than directly reloading registry scripts.

4. **Replication script hooks** (`bins/sidereal-replication/src/replication/scripting.rs`):
   - Loads `world/world_init.lua` at authoritative host boot.
   - Executes `build_graph_records(ctx)` and applies world bootstrap graph records once, guarded by `script_world_init_state` DB marker.
   - Bootstrap records now include deterministic Lua-generated asteroid field entities via `spawn_bundle_graph_records("asteroid.field_member", overrides)`.
   - World-init guard uses existing `GraphPersistence` connection (no extra DB clients).
   - Asset bootstrap metadata now derives from Lua asset registry policy (`bootstrap_required`) and script-selected `asset_id` references; payload fetches occur via gateway `/assets/<asset_guid>`.
   - Mirrors all discovered `.lua` source files into a runtime `ScriptCatalogResource` (BRP-visible), with per-entry `source`, `script_path`, `origin`, and `revision`.
   - Replication authoritative load order is now:
     1. load the active script catalog from SQL tables if present,
     2. otherwise seed from disk and persist that seed set into SQL,
     3. if authoritative SQL load is temporarily unavailable, boot from disk into the in-memory catalog and mark SQL persistence as pending,
     4. execute runtime scripts from the in-memory catalog built from that authoritative source.
   - Exposes `ScriptCatalogControlResource` (BRP-visible) with `reload_all_from_disk_requested` so tooling can request a full reload of seed scripts from disk.
   - `ScriptCatalogControlResource` now also reports the last persist result/time plus startup fallback state so BRP tooling can observe whether an in-memory edit has been durably flushed to SQL and whether startup is still running from a disk-seeded fallback catalog.
   - Mirrors Lua entity registry definitions into a runtime `EntityRegistryResource` (BRP-visible), derived from `ScriptCatalogResource`.
   - Mirrors Lua asset registry definitions into a runtime `AssetRegistryResource` (BRP-visible), derived from `ScriptCatalogResource`.
   - Runtime bundle spawning on the replication host resolves bundle source, bundle registry metadata, and asset registry metadata from those resources instead of reloading registry scripts from disk on every spawn request.
   - Nested Lua `spawn_bundle_graph_records(...)` calls reuse the same script-catalog/registry snapshot for that evaluation, so one script invocation is internally consistent.
   - Normalized catalog changes are now persisted back to SQL automatically, so BRP/script edits survive service restart.

5. **Replication runtime scripting slice** (`bins/sidereal-replication/src/replication/runtime_scripting.rs`):
   - Persistent sandboxed Lua VM is initialized at host boot (non-send Bevy resource).
   - AI/runtime handler modules are now compiled from `ScriptCatalogResource`, not read directly from disk.
   - When `ScriptCatalogResource.revision` changes, the runtime scripting host rebuilds its handler set from the updated in-memory source on the next execution pass.
   - Generic per-entity interval scheduler runs Lua `on_tick(ctx, event)` handlers selected via `ScriptState.data.on_tick_handler`.
   - Read-only `ctx.world:find_entity(uuid)` + `ScriptEntity` wrapper (`guid`, `position`, `has`, `get` for `script_state`).
   - Intent bridge prototype: `ctx:emit_intent("fly_towards" | "stop" | "set_script_state", payload)`.
   - Event queue path supports per-entity `on_<event>` handler dispatch via `ScriptState.data.event_hooks`.
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
| `data/scripts/accounts/player_init.lua` | New-account ship bundle selection for player spawn (`ship_bundle_id`) |
| `data/scripts/bundles/bundle_registry.lua` | Bundle definitions with component-kind allowlists |
| `data/scripts/bundles/entity_registry.lua` | Optional bundle lifecycle hooks (`on_spawned`) |
| `data/scripts/bundles/ship/*.lua` | Ship prefab bundle graph-record builders (for example `ship.corvette`, `ship.rocinante`) |
| `data/scripts/bundles/starter/asteroid_field.lua` | Asteroid field member bundle used by world bootstrap generation |
| `data/scripts/ai/pirate_patrol.lua` | Runtime interval-driven patrol AI prototype |

### 11.3 Current Runtime Model

- Gateway scripting: account-registration-time persistent entity creation (starter/player graph records).
- Replication scripting: authoritative host boot world orchestration (`world_init`) with one-time guard, executed from the in-memory script catalog.
- Replication runtime scripting: persistent Lua VM + interval callback execution + intent application during `FixedUpdate`, with handler source coming from the in-memory script catalog.
- Persistence vs spawn: scripts currently drive graph record creation/persistence; runtime spawn into ECS world occurs by replication hydration/bootstrap from persisted graph state.

### 11.4 Current Compromises

Status note 2026-03-11:
- `WorldInitScriptConfig`, world-init source decoding, graph-record decoding, and runtime render-graph validation now live in `crates/sidereal-scripting` and are consumed by both gateway and replication. The remaining compromises below reflect other current limitations, not that shared extraction work.

1. `world_init` is replication-startup-only (one-time marker guarded), while gateway registration only uses the gateway starter-world script surfaces (`player_init`, bundle registry, bundle graph records). Gateway is now catalog-backed too, but it still does not have replication's BRP-driven Bevy resource host model.
2. Durable script persistence now uses SQL tables, not graph records. Disk is seed/default content plus explicit reload source, not the long-term authority.
3. `reload_all_from_disk_requested` replaces the in-memory script catalog from the disk seed set and that replacement is then flushed into the active SQL-backed catalog. This intentionally overwrites any prior live-edited SQL state with disk content.
4. World init currently seeds legacy fullscreen layers, one patrol NPC prototype, and a deterministic asteroid field. Dynamic region-load/unload generation is still pending, and the fullscreen bootstrap contract should migrate to the DR-0027 authored render-layer model.
5. Event bridge is still interval-first prototype (single script module); full declarative multi-module event routing from section 5 is pending.
6. Lua table conversion is currently shape-inferred (array vs object). Empty table literals are ambiguous; script payloads should avoid relying on empty arrays until explicit array constructors are added.

### 11.4.1 Script Source Authority (Replication Host)

On the replication host, the authoritative runtime script model is now:

1. `.lua` files are loaded into `ScriptCatalogResource` at startup as seed content.
2. Runtime script execution resolves source from `ScriptCatalogResource`, not directly from disk.
3. If authoritative SQL load fails during startup, replication now still boots from disk into `ScriptCatalogResource`, derives runtime registries from that catalog, and retries durability on subsequent persist passes instead of entering an empty-script degraded mode.
4. BRP/dashboard edits to `ScriptCatalogResource.entries[*].source` are intended to take effect on the next relevant execution after the catalog revision advances.
5. `ScriptCatalogControlResource.reload_all_from_disk_requested = true` explicitly replaces the in-memory catalog from the filesystem seed set and bumps catalog revision.
6. Derived resources (`EntityRegistryResource`, `AssetRegistryResource`) follow `ScriptCatalogResource` revisions and are regenerated from current in-memory source, not independently polled from disk.
7. Replication automatically persists normalized catalog revisions to SQL tables after in-memory edits/reloads, so the catalog is durable across restart.

This means replication now has the correct direction for live editing:

1. edit in-memory source,
2. derived registries rebuild from that source,
3. future world-init/bundle/runtime executions observe the edited source.

It does **not** yet mean scripts are durable across restart; that still requires DB-backed published script persistence.

### 11.4.2 Script Source Authority (Gateway)

Gateway now follows the same source-of-truth direction, with a runtime shape appropriate to the HTTP/auth service:

1. Gateway loads the active script catalog from SQL on first use; when SQL has no script rows yet, disk seed content is loaded and written into SQL.
2. Gateway script execution resolves source from the cached in-memory `ScriptCatalogResource`, not directly from disk.
3. `reload_script_catalog_from_disk(root)` explicitly replaces both the cached gateway catalog and the active SQL-backed catalog from the filesystem seed set.
4. Gateway bundle spawning helper paths now resolve bundle and asset metadata through the cached catalog model.

Current limitation:

1. Gateway does not expose this as a Bevy `Resource`, because gateway is not a Bevy app runtime.
2. Gateway does not yet support BRP-driven live mutation of script source. It supports cached execution plus explicit reload-from-disk, with SQL as durable authority.
3. Dashboard publish/edit flows still need to write through a first-class API instead of relying on replication-local BRP edits only.

### 11.4.3 Durable Script Persistence (SQL Tables)

Durable script storage is now table-backed rather than graph-backed.

Authoritative layering is:

1. **Runtime authority**: in-memory `ScriptCatalogResource` / gateway cached catalog.
2. **Durable authority**: SQL tables.
3. **Seed/default source**: filesystem `.lua` under `data/scripts`.

Current SQL schema:

1. `script_catalog_documents`
   - `script_path` primary key
   - `script_family`
   - `active_revision`
   - `created_at_epoch_s`
   - `updated_at_epoch_s`
2. `script_catalog_versions`
   - `(script_path, revision)` primary key
   - `source`
   - `origin`
   - `created_at_epoch_s`

Current semantics:

1. Replication and gateway both load active script source from the SQL catalog when rows exist.
2. If the SQL catalog is empty, services seed it from disk.
3. If replication cannot reach SQL during startup, it still boots from disk and keeps a persist-pending in-memory catalog until SQL becomes reachable.
4. Replication persists normalized in-memory catalog changes back to SQL automatically.
5. Gateway reload-from-disk replaces the active SQL catalog with the disk seed set.
5. Scripts are **not** persisted as graph ECS entities/components.

This is intentional: scripts are content records, not world simulation entities.

### 11.4.4 Draft / Publish Framework (Gateway API)

The next workflow layer now exists on top of the active SQL catalog:

1. **Draft**
   - save/update unpublished source in `script_catalog_drafts`
2. **Publish**
   - create a new immutable entry in `script_catalog_versions`
   - update `script_catalog_documents.active_revision`
   - remove the corresponding draft row
3. **Discard draft**
   - removes the unpublished draft without changing the active published script
4. **Reload from disk**
   - replaces the active SQL catalog from filesystem seed content

Current authenticated gateway routes:

1. `GET /admin/scripts`
   - list script documents and whether a draft exists
2. `GET /admin/scripts/detail/{*script_path}`
   - load active + draft detail for one script
3. `POST /admin/scripts/draft/{*script_path}`
   - save/update a draft payload
4. `DELETE /admin/scripts/draft/{*script_path}`
   - discard a draft
5. `POST /admin/scripts/publish/{*script_path}`
   - publish the current draft as a new active immutable revision
6. `POST /admin/scripts/reload-from-disk`
   - replace the active catalog from disk seed files

Security contract:

1. All script-management routes require bearer auth.
2. All script-management routes require `admin` or `dev_tool` role.
3. These routes are gateway-owned operational/editor APIs, not public gameplay APIs.

Current limitation:

1. Publishing updates durable SQL authority immediately.
2. Running services do **not** yet automatically pull published changes into their live in-memory catalogs.
3. Replication live editing still exists through BRP resource mutation, but that is now a dev-time path rather than the intended long-term publish path.

### 11.5 Current Runtime Contract (As Implemented)

This subsection is the authoritative "what exists today" contract and should be kept in sync with runtime code.

#### 11.5.1 Script Execution Surfaces

1. **Gateway registration scripts**
   - `accounts/player_init.lua`:
     - Required function: `player_init(ctx) -> { starter_bundle_id = string }`
     - Injected context fields:
       - `account_id` (UUID string)
       - `player_entity_id` (UUID string)
       - `email` (string)
       - `new_uuid()` (function)
   - `bundles/bundle_registry.lua`:
     - Required table: `bundles`
     - Each bundle entry requires:
       - `graph_records_script`
       - `required_component_kinds`
2. **Replication startup scripts**
   - `world/world_init.lua`:
     - Required table: `world_defaults`
     - Required function: `build_graph_records(ctx)`
     - Injected context helpers:
       - `new_uuid()`
       - `spawn_bundle_graph_records(bundle_id, overrides?)`
3. **Replication runtime scripts**
   - Auto-discovered from `data/scripts/ai/*.lua`
   - Module fields consumed at load:
     - `handler_name` (optional, defaults to filename stem)
     - `tick_interval_seconds` (optional, default `2.0`)
     - `on_tick(ctx, event)` (optional)
     - any `on_<event_name>(ctx, event)` function (optional)

#### 11.5.2 Runtime Hook Binding Model (Current)

Runtime hook selection is **entity-driven** via `ScriptState.data`:

1. Tick hook binding:
   - `script_state.data.on_tick_handler = "<handler_name>"`
   - optional override: `script_state.data.tick_interval_s = <seconds>`
2. Event hook binding:
   - `script_state.data.event_hooks = { ["event_name"] = "<handler_name>" }`

Example payload currently used by world bootstrap:

```lua
script_state_data = {
  on_tick_handler = "pirate_patrol",
  tick_interval_s = 2.0,
  event_hooks = {},
}
```

#### 11.5.3 Runtime `ctx` Exposure (Current)

Current runtime-scripting `ctx` is intentionally minimal:

1. `ctx.world:find_entity(guid)`
2. `ctx:emit_intent(action, payload)`

Current `ScriptEntity` methods:
1. `entity:guid()`
2. `entity:position()`
3. `entity:has(component_kind)` where only `"script_state"` is supported today
4. `entity:get(component_kind)` where only `"script_state"` is supported today

Important current limitation:
- Runtime component access key is lowercase `"script_state"` in current implementation path (not canonical component-kind expansion yet).

#### 11.5.4 Runtime Events Currently Enqueued

Current event producers are combat-driven:

1. `shot_fired`
2. `shot_impact`
3. `damage_applied`
4. `health_depleted` for `Destructible` entities entering authoritative pending destruction
5. `before_destroy` for `Destructible` entities after destruction is committed but before despawn
6. `destroyed` for `Destructible` entities after delay expiry and one targeted script-dispatch tick before final despawn

Update note (2026-03-13):
- `health_depleted`, `before_destroy`, and `destroyed` are now emitted by the replication combat bridge for `Destructible` entities.
- Validated script override intents are still not implemented, so these hooks are currently notification-only.
- Native impact: targeted entity lifecycle callbacks can now run before final despawn if the entity has `ScriptState.data.event_hooks` configured.
- WASM impact: no protocol/runtime split; the same lifecycle event names and payload shape apply to browser and native clients.

Still not implemented:

1. richer lifecycle/destruction events such as `fractured` and `loot_spawned`,
2. validated override intents for changing authoritative destruction/finalization behavior in the live runtime.

Payloads are JSON/Lua tables generated by replication combat systems.

#### 11.5.5 Runtime Intents Currently Supported

Only these actions are accepted by current runtime scripting intent parser:

1. `fly_towards`
2. `stop`
3. `set_script_state`

All other actions are rejected as unsupported in current runtime implementation.

#### 11.5.6 Script Control Safety Guard (Current)

Current script-driven control intents are applied only if:
1. target entity has `ScriptState`,
2. target `OwnerId` exists and is **not** a player entity ID (`npc`/non-player ownership path),
3. payload passes intent parser validation.

This prevents runtime scripts from directly steering player-owned entities in the current slice.

#### 11.5.7 FixedUpdate Ordering (Current)

Current runtime scripting order in `FixedUpdate`:

1. `refresh_script_world_snapshot`
2. `run_script_intervals`
3. `run_script_events`
4. `apply_script_intents` (before `sidereal_game::process_flight_actions`)

This chain executes before physics prepare.

#### 11.5.8 Current/Planned Boundary Clarification

1. Section 11.5 describes **implemented behavior now**.
2. Sections 5, 8, and Phase D+ define the **target expanded API surface**.
3. If runtime behavior changes, section 11.5 must be updated in the same change.

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
- [x] Share `WorldInitScriptConfig` through the scripting crate or a shared types module to eliminate duplication between gateway and replication.
- [ ] Add approved-path `require` replacement so scripts can import shared modules within the scripts root.

### Phase B2: Lua Asset Registry Integration

- [ ] Add canonical script module for asset registry (for example `data/scripts/assets/registry.lua`) with schema versioning and validation.
- [x] Implement shared loader/decoder in `sidereal-scripting` so gateway/replication use one registry parsing path.
- [ ] Mark bootstrap-required assets in Lua registry (`bootstrap_required`) and remove ad-hoc always-required Rust lists.
- [ ] Build generated catalog metadata (`asset_id`, `asset_guid`, shader domain/schema metadata, optional compatibility aliases, checksum, dependencies, content type) from Lua registry input.
- [ ] Add startup manifest payload schema consumed by client `AssetLoading` flow (`required_assets` + optional full catalog).
- [ ] Add contract tests ensuring runtime Rust code does not rely on hardcoded concrete asset IDs/filenames.

### Phase C: Event Bridge + Handler Context

- [ ] Implement `ScriptEventQueue` Rust resource for buffering events per tick.
- [ ] Implement event bridge dispatcher: resolve registered handlers per event type, create `ctx` per invocation, dispatch to Lua.
- [ ] Implement declarative handler registration: read `events` table from script modules on load, register `on_<event_name>` functions.
- [ ] Implement per-handler instruction budget and error isolation (abort handler on budget exceeded, log error, continue to next handler).
- [ ] Add `ctx` handler context object exposing `ctx.world` (read-only) and `ctx:emit_intent()` (write).
- [ ] Add initial event allowlist from section 5.4 (world_boot, session, entity lifecycle, combat, system transitions, economy, mission).
- [ ] Make entity lifecycle support include a pre-resolution and post-resolution split for destructible entities (`health_depleted`/`before_destroy` before final resolution, `destroyed`/`fractured`/`loot_spawned` after final resolution) so scripts can request approved overrides without bypassing Rust authority.
- [ ] Extend allowlist with visibility and inventory-oriented events required for AI/quest logic (`gain_entity_visibility`, `lose_entity_visibility`, `inventory_changed`, `cargo_collected`, `cargo_delivered`).
- [ ] Add throttling/aggregation for high-frequency events (collision, damage): configurable per-event-type rate limits.
- [ ] Add event bridge observability: per-handler execution time, instruction count, error count, exposed via `bevy_remote`.
- [ ] Add integration test: Rust event emitted -> Lua handler fires -> intent queued -> state change validated.
- [ ] Reserve `SpatialEventBuffer` resource and persistent cell-tracking `HashMap<Entity, (i64, i64)>` in partition rebuild for future spatial events (entered_system, left_system). Implementation of event emission is deferred but the data structures should exist to avoid a later refactor.

### Phase D: Runtime Script API v1 + Mission Pilot

- [ ] Extend `ScriptEntitySnapshot` with `component_kinds: HashSet<String>`, `faction_id: Option<String>`, and `labels: Vec<String>` so filter predicates can be evaluated without ECS queries.
- [ ] Implement `ScriptEntity` read-only wrapper: `guid()`, `position()`, `velocity()`, `rotation()`, `get(kind)`, `has(kind)`, `labels()`.
- [ ] Implement `ctx.world:find_entity(uuid)` via entity GUID lookup.
- [ ] Add spatial index (`entities_by_cell`) to `ScriptWorldSnapshot` using the shared `cell_key` function from the visibility/partition module. See `docs/plans/spatial_partitioning_implementation_plan.md` section 10.1.
- [ ] Implement `ctx.world:query_nearby(pos, radius, filter)` using the snapshot spatial index (cell walk, distance filter, component/faction/label filter, nearest-first sort, limit truncation).
- [ ] Implement query budget guardrails (max radius, max queries per handler, max results per handler) enforced in Rust. See section 8.2 of this document.
- [ ] Add `entities_by_system: HashMap<Uuid, Vec<String>>` index to `ScriptWorldSnapshot` for `query_in_system`.
- [ ] Implement `ctx.world:distance(a, b)` and `ctx.world:distance_to_point(uuid, pos)`.
- [ ] Implement `ctx.world:find_system_at(pos)` and `ctx.world:query_in_system(uuid, filter)` (solar system queries).
- [ ] Implement intent queue: `ctx:emit_intent(action, payload)` -> Rust-side validation -> authoritative state change.
- [ ] Add core intent actions: `fly_towards`, `stop`, `fire_weapons`, `spawn_entity`, `despawn_entity`, `set_script_state`, `emit_event`.
- [ ] Add authoritative lifecycle-override intents for scripted exceptional cases (for example canceling destruction, restoring health through validated gameplay paths, selecting alternate authored effect/loot outcomes, or spawning reinforcement/content entities) without allowing raw ECS mutation from Lua.
- [ ] Add privileged runtime actions for scripted orchestration (`teleport_entity`, `set_entity_transform`, `set_entity_velocity`, `batch_move_entities`) with scheduling and validation guardrails.
- [ ] Add scanner/search support actions (`scanner_ping`, optional `set_ai_mode` state helper) for loss-of-visibility behavior loops.
- [x] Add `ScriptState` generic persisted component (`HashMap<String, ScriptValue>`) with `#[sidereal_component(...)]` registration.
- [ ] Implement `ctx.events:register_interval()`, `schedule_after()`, `cancel_interval()` with minimum interval floor.
- [ ] Implement `ctx.mission` scoped state interface: `set_ref()`, `get_ref()`, `set()`, `get()`.
- [ ] Expose `ctx.time:now_s()` (or equivalent deterministic runtime time helper) for decay/cooldown logic in scripts.
- [ ] Expose script cooldown/idempotency helpers (`ctx.state:ensure_once`, `ctx.state:cooldown_ready`) for tick-driven trigger entities.
- [ ] Add range/query convenience helpers used by behavior loops (`get_entities_in_range`, `last_known_position`, `get_active_ship`, inventory count helpers).
- [ ] Implement first scripted mission (escort convoy) using the event bridge and runtime API.
- [ ] Add persisted mission state model with graph persistence roundtrip (mission state survives restart).
- [ ] Add integration tests for mission start/update/complete/fail lifecycle across restart.
- [ ] Add script API version field to bundle manifest for forward compatibility checks.
- [ ] Verify hydration invariant: `EntityGuid` → entity → partition cell mapping is complete before first script interval tick fires. See `docs/plans/spatial_partitioning_implementation_plan.md` section 10.7.

### Phase D1: Genre-Agnostic Content Runtime + Quest System (Immediate Priority)

This subsection captures immediate roadmap decisions to keep Sidereal reusable across genres (space, platformer, tactics, etc.) while preserving deterministic authority.

#### D1.1 Accepted Direction: Genre in Scripts, Simulation in Rust

1. Rust remains the deterministic simulation kernel and authority boundary enforcer.
2. Lua is the primary genre/content layer (missions, dialogue, progression, encounter logic, spawn orchestration, scripted rules).
3. "Space game" behavior should continue moving from hardcoded Rust branching into script-authored data and handlers.
4. Genre-specific behavior should be expressed through:
   - script-authored events/handlers,
   - script-authored mission/objective definitions,
   - script-owned persistent state (`ScriptState` and/or dedicated generic progression components),
   - generic intent APIs that remain authority-safe.

#### D1.2 Accepted Direction: Server-Authoritative Quest Execution

Quest/mission logic runs on the authoritative host only (dedicated server or local host in offline/listen-host mode).

Clients do not execute authoritative quest scripts. Clients receive:
1. replicated quest state (owner-scoped where needed),
2. UI metadata (titles, objective text, rewards, waypoint hints),
3. server-authored progress/fail/complete outcomes,
4. targeted UI/notification/dialogue presentation payloads when needed.

Optional future client scripting is presentation-only and must not mutate authoritative gameplay state.

#### D1.3 Quest Data Model (Immediate Contract)

Use a template/instance model:

1. **QuestTemplate** (script-authored definition):
   - immutable ID (`quest_template_id`),
   - objective graph/stages,
   - reward rules,
   - optional branching conditions.
2. **QuestInstance** (per-player or per-party runtime state):
   - unique `quest_instance_id`,
   - owner scope (`player_entity_id` or party ID),
   - per-objective progress state,
   - status (`active`, `completed`, `failed`, `abandoned`),
   - references to spawned/target entities by UUID.
3. **Player quest journal/progression**:
   - persisted on the player ECS entity (owner-scoped),
   - script-readable and script-writable through validated intent APIs,
   - replicated owner-only by default.

The same mission accepted by multiple players creates separate `QuestInstance` records unless explicitly marked as shared-party content.

#### D1.4 Immediate API Additions Needed

Add generic server intent actions and query helpers to support fully scripted quests:

1. `accept_quest` `{ player_id, quest_template_id }`
2. `abandon_quest` `{ player_id, quest_instance_id }`
3. `advance_quest_objective` `{ quest_instance_id, objective_id, delta | set_value }`
4. `complete_quest` `{ quest_instance_id, rewards }`
5. `fail_quest` `{ quest_instance_id, reason }`
6. `set_player_script_state` `{ player_id, key, value }` (or equivalent generic player-state mutation intent)
7. query helpers:
   - `ctx.world:get_player_active_quests(player_id)`
   - `ctx.world:get_quest_instance(quest_instance_id)`
   - `ctx.world:find_entities_matching_objective(...)` (built on existing spatial/query contracts)

All mutations remain intent-only and are validated in Rust authority systems.

#### D1.5 Immediate Implementation Plan (Quest Vertical Slice)

1. **Quest runtime components/resources**
   - Add generic persisted quest components (template reference, progress, status, owner scope).
   - Keep player progression state on player entity components (aligns with project non-negotiables).
2. **Quest event bridge wiring**
   - Add canonical events:
     - `quest_accepted`, `quest_objective_progress`, `quest_completed`, `quest_failed`
     - gameplay-derived triggers (`cargo_delivered`, `entity_entered_region`, `inventory_changed`).
   - Implement quest hook lifecycle callbacks (`on_accept`, `on_event`, `on_tick`, `can_complete`, `on_complete`, `on_fail`).
3. **Lua quest module format**
   - Add `data/scripts/quests/*.lua` manifest style including template metadata, objective definitions, and lifecycle handlers.
4. **Replication/UI path**
   - Replicate owner-scoped quest journal state to client.
   - Client renders quest log/objective widgets from replicated state only.
5. **Vertical-slice mission**
   - Implement "fly to X -> collect Y -> deliver to Z" fully via Lua quest logic plus generic Rust intents/systems.
6. **Determinism tests**
   - restart/resume for in-flight quest instances,
   - same-template multi-player isolation (no cross-contamination),
   - invalid quest intent rejection coverage.

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

Content scripts run at 1-10 Hz, not on the 60 Hz physics hot path.

```
60 Hz authority tick: 16.7 ms budget
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
- `docs/decisions/dr-0007_entity_variant_framework.md` -- variant/archetype framework.
- `docs/features/visibility_replication_contract.md` -- visibility and replication policy.
- `docs/features/asset_delivery_contract.md` -- Lua registry-driven asset catalog and gateway asset delivery.
- `docs/features/galaxy_world_structure.md` -- galaxy/solar system world model and scripting integration.
- `docs/plans/spatial_partitioning_implementation_plan.md` -- spatial partition grid, cell sizing, script query integration (section 10).

### Code Paths

- `crates/sidereal-scripting/src/lib.rs` -- shared Lua runtime, sandbox, loader.
- `bins/sidereal-gateway/src/auth/starter_world_scripts.rs` -- gateway script hooks.
- `bins/sidereal-gateway/src/auth/starter_world.rs` -- starter world persistence orchestrator.
- `bins/sidereal-replication/src/replication/scripting.rs` -- replication script hooks.
- `bins/sidereal-replication/src/replication/simulation_entities.rs` -- world init execution and hydration.
- `data/scripts/` -- script source root.
