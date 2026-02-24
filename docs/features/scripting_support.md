# Scripting & Modding Language Feasibility Analysis

**Date:** 2026-02-18  
**Status:** 📋 Analysis / Planning

## Executive Summary

**Recommendation**: ✅ **Yes**, add scripting support - but **carefully scoped** to specific domains.

Your vision of a "single-player campaign that others can join with a large vibrant universe" is **perfectly suited** for scripting. However, the integration must respect your existing ECS architecture, not replace it.

**Best Approach**: Hybrid model where:
- **Core systems** (physics, networking, authority) remain pure Rust/ECS
- **Content systems** (missions, events, dialogue, quests, AI behaviors) use scripting
- Scripts **interact** with ECS through well-defined Rust APIs

## Understanding Your Vision

Based on your documentation, Sidereal is:

### Core Gameplay Loop
- **Server-authoritative physics** (30Hz Avian3D simulation)
- **MMO-scale architecture** (sharded, meshed, replicated)
- **Component-driven gameplay** (engines, fuel, flight computers, hardpoints)
- **Persistent universe** (economy, factions, ownership)

### Current State
- Early development (M1-M2 milestone range)
- Foundation: networking, physics, persistence ✅
- Content: missions, AI, economy 📋 planned

### Your Goal
> "Single-player campaign that others can join... large vibrant universe"

This means you need:
- **Dynamic mission generation**
- **NPC behaviors and dialogue**
- **Story progression and branching**
- **Economy events and faction responses**
- **Mod support for custom content**

**All of these are PERFECT use cases for scripting!**

## Does Scripting Defeat the Purpose of ECS?

### Short Answer: **No, if done correctly**

ECS is about **data-oriented architecture for hot-path systems**. Scripting is about **flexible content authoring**. They solve different problems.

### The Right Mental Model

```
┌─────────────────────────────────────────────────┐
│              RUST ECS CORE                      │
│  (Physics, Networking, Authority, Rendering)    │
│                                                  │
│  [30 Hz Authority Loop]                         │
│  [Client Prediction]                            │
│  [Replication]                                  │
└──────────────────┬──────────────────────────────┘
                   │
                   │ Well-Defined API
                   ▼
┌─────────────────────────────────────────────────┐
│           SCRIPTING LAYER                       │
│  (Missions, Quests, Dialogue, AI Behaviors)     │
│                                                  │
│  mission_system.lua                             │
│  faction_ai.lua                                 │
│  economy_events.lua                             │
└─────────────────────────────────────────────────┘
```

### What ECS is Good At ✅
- High-frequency systems (30-60Hz+)
- Data parallelism
- Cache-friendly iteration
- Type-safe composition
- Zero-cost abstractions

### What Scripting is Good At ✅
- Rapid content iteration
- Non-programmer authoring
- Hot-reloading during development
- Mod community contribution
- Complex branching logic
- State machines

### Real-World Examples

**Games that successfully use ECS + Scripting:**

1. **Overwatch** (Blizzard)
   - C++ ECS for core gameplay
   - Lua for hero abilities, events, game modes

2. **Factorio**
   - C++ ECS for simulation
   - Lua API for mods (massive mod ecosystem!)

3. **Bevy Ecosystem**
   - Multiple projects use `bevy_mod_scripting`
   - Scripts control high-level logic, ECS handles hot paths

**Key Pattern**: Scripts manipulate ECS entities/components through safe APIs, they don't replace the ECS.

## Scripting Options for Bevy

### Option 1: Lua via `bevy_mod_scripting` ⭐ RECOMMENDED

**Crate**: `bevy_mod_scripting` + `mlua`

**Pros:**
- ✅ Mature Bevy integration
- ✅ Lua is fast (~10-50x slower than Rust, but adequate for content)
- ✅ Huge modding precedent (WoW, Roblox, Factorio, Garry's Mod)
- ✅ Small memory footprint (~200KB per Lua state)
- ✅ Easy to sandbox (important for untrusted mods!)
- ✅ Simple syntax - designers/modders can learn quickly
- ✅ Hot-reloading built-in

**Cons:**
- ❌ Not as fast as Rust (but fast enough for content)
- ❌ Dynamically typed (but you can wrap with typed APIs)

**Integration Complexity:** ⭐⭐⭐☆☆ (Moderate)

### Option 2: Python via `pyo3`

**Crate**: `pyo3` (Python bindings)

**Pros:**
- ✅ Python is widely known
- ✅ Good for data processing / procedural generation
- ✅ Large ecosystem of libraries
- ✅ Could use for mission/dialogue authoring

**Cons:**
- ❌ Much slower than Lua (~100-200x slower than Rust)
- ❌ Larger runtime overhead (~30MB+ per interpreter)
- ❌ Harder to sandbox securely
- ❌ Less common in game modding
- ❌ GIL (Global Interpreter Lock) issues

**Integration Complexity:** ⭐⭐⭐⭐☆ (Complex)

### Option 3: Rhai (Rust-Native Scripting)

**Crate**: `rhai` + `bevy_mod_scripting`

**Pros:**
- ✅ Pure Rust (no FFI overhead)
- ✅ Rust-like syntax
- ✅ Easy integration
- ✅ Type-safe API bindings
- ✅ Good performance (~20-40x slower than Rust)

**Cons:**
- ❌ Less mature mod ecosystem
- ❌ Smaller community vs. Lua
- ❌ Unfamiliar to most modders

**Integration Complexity:** ⭐⭐☆☆☆ (Easy)

### Option 4: JavaScript/TypeScript via `deno_core`

**Crate**: `deno_core`

**Pros:**
- ✅ JavaScript is extremely popular
- ✅ TypeScript for type safety
- ✅ Modern async/await patterns
- ✅ Good tooling (VSCode, etc.)

**Cons:**
- ❌ Large runtime (~10MB+)
- ❌ More complex integration
- ❌ Less common for game modding

**Integration Complexity:** ⭐⭐⭐⭐☆ (Complex)

### Recommendation: **Lua via `bevy_mod_scripting`**

**Why Lua:**
1. **Proven** - Industry standard for game modding
2. **Fast enough** - Not hot-path, so 10-50x slower is fine
3. **Small** - Minimal memory/binary size overhead
4. **Sandboxable** - Critical for untrusted mods
5. **Bevy integration** - `bevy_mod_scripting` is mature
6. **Community** - Modders know Lua

## What Should Be Scriptable?

### ✅ GOOD Candidates (High-Level Content)

#### 1. Mission System ⭐⭐⭐⭐⭐
```lua
-- missions/escort_convoy.lua
function on_mission_start(mission)
    -- Spawn convoy
    convoy = world.spawn_npc_fleet("cargo_convoy", {
        position = {x=1000, y=2000},
        destination = {x=5000, y=8000},
        faction = "trade_guild"
    })
    
    -- Add mission objectives
    mission.add_objective("escort", {
        target = convoy,
        destination = convoy.destination,
        reward = {credits = 5000, reputation = 10}
    })
    
    -- Spawn threats
    world.schedule_event(60.0, function()
        pirates = world.spawn_npc_fleet("pirate_raiders", {
            position = {x=3000, y=5000},
            target = convoy
        })
    end)
end

function on_convoy_destroyed()
    mission.fail("Convoy destroyed!")
end

function on_convoy_arrived()
    mission.complete()
end
```

**Why scriptable:**
- Lots of variation (different missions need different logic)
- Non-critical path (mission logic runs at ~1Hz, not 30Hz)
- Content creators can add missions without touching Rust
- Mods can add entire campaigns

#### 2. NPC AI Behaviors ⭐⭐⭐⭐⭐
```lua
-- ai/pirate_patrol.lua
function npc_ai_tick(npc, delta_time)
    -- State machine
    if npc.state == "patrol" then
        patrol_route(npc)
        
        -- Detect player
        local enemies = npc.scan_for_enemies(1000.0)
        if #enemies > 0 then
            npc.state = "pursue"
            npc.target = enemies[1]
        end
        
    elseif npc.state == "pursue" then
        if npc.target.is_alive() then
            npc.fly_towards(npc.target.position)
            if npc.distance_to(npc.target) < 200.0 then
                npc.fire_weapons()
            end
        else
            npc.state = "patrol"
        end
        
    elseif npc.state == "flee" then
        if npc.health_percent() < 20 then
            npc.fly_away_from(npc.last_attacker)
        else
            npc.state = "pursue"
        end
    end
end
```

**Why scriptable:**
- Different factions need different behaviors
- Easy to tweak without recompiling
- Modders can create custom AI personalities
- State machines are verbose in Rust, concise in Lua

#### 3. Economy / Background Simulation ⭐⭐⭐⭐☆
```lua
-- economy/market_events.lua
function on_station_tick(station, delta_time)
    -- Adjust prices based on supply/demand
    if station.cargo["ore"].quantity < 100 then
        station.cargo["ore"].price = station.cargo["ore"].price * 1.02
    end
    
    -- Generate trade missions
    if station.faction == "mining_guild" and rng() < 0.01 then
        generate_cargo_mission({
            from = station,
            to = find_nearest_station("refinery"),
            cargo = "ore",
            quantity = 50
        })
    end
end

function on_faction_event(event)
    if event.type == "war_declared" then
        -- Pirate activity increases
        increase_spawn_rate("pirates", event.region, 2.0)
        
        -- Military patrols increase
        increase_spawn_rate("faction_military", event.region, 1.5)
    end
end
```

**Why scriptable:**
- Complex event chains and consequences
- Easy to balance without recompiling
- Modders can create custom economies

#### 4. Dialogue & Quest Systems ⭐⭐⭐⭐⭐
```lua
-- dialogue/station_master.lua
function talk_to_station_master(player)
    local dialogue = Dialogue.new()
    
    dialogue.add_line("Station Master", "Welcome to Haven Station, pilot.")
    
    if player.reputation["trade_guild"] > 50 then
        dialogue.add_line("Station Master", "Good to see a friend of the guild!")
        dialogue.add_choice("I need work", function()
            offer_high_reward_mission(player)
        end)
    else
        dialogue.add_choice("Looking for contracts", function()
            show_mission_board(player)
        end)
    end
    
    dialogue.add_choice("Repair my ship", function()
        if player.credits >= repair_cost(player.ship) then
            player.ship.repair()
            player.credits = player.credits - repair_cost(player.ship)
        else
            dialogue.add_line("Station Master", "Sorry, can't offer credit.")
        end
    end)
    
    dialogue.add_choice("Goodbye", function()
        dialogue.close()
    end)
    
    return dialogue
end
```

**Why scriptable:**
- Writers need iteration speed
- Branching narratives are complex
- Mod support for storylines

#### 5. Procedural Content Generation ⭐⭐⭐⭐☆
```lua
-- generation/asteroid_field.lua
function generate_asteroid_field(region)
    local count = rng_int(50, 200)
    local center = region.center
    local radius = region.radius
    
    for i = 1, count do
        local angle = rng_float(0, math.pi * 2)
        local dist = rng_float(0, radius)
        local pos = {
            x = center.x + math.cos(angle) * dist,
            y = center.y + math.sin(angle) * dist,
            z = 0
        }
        
        world.spawn_entity("asteroid", {
            position = pos,
            mass_kg = rng_float(1000, 50000),
            size_m = rng_float(5, 30),
            resource_type = weighted_choice({
                {"iron", 0.6},
                {"copper", 0.25},
                {"platinum", 0.1},
                {"uranium", 0.05}
            })
        })
    end
end
```

**Why scriptable:**
- Content variety
- Fast iteration on generation algorithms
- Modders can add custom content generators

### ❌ BAD Candidates (Keep in Rust/ECS)

#### 1. Physics Simulation ❌
```rust
// NO! This must stay in Rust
fn physics_step(world: &mut World) {
    // 30Hz hot path - every frame, every entity
    // Avian integration, collision, forces
}
```

**Why NOT scriptable:**
- 30Hz hot path (runs every frame)
- Performance critical
- Server-authoritative (security)
- Already well-abstracted by Avian

#### 2. Networking / Replication ❌
```rust
// NO! This must stay in Rust
fn send_state_delta(entities: Query<&Transform, &Velocity>) {
    // High-frequency, low-latency critical
}
```

**Why NOT scriptable:**
- Ultra-low latency requirements
- Security boundary (authority validation)
- Binary protocol encoding

#### 3. Client Prediction / Reconciliation ❌
```rust
// NO! This must stay in Rust
fn predict_controlled_entity() {
    // 60Hz on client, prediction replay
}
```

**Why NOT scriptable:**
- Frame-rate critical (60Hz)
- Already shared via `sidereal-sim-core`
- Deterministic requirement

#### 4. Core Component Systems ❌
```rust
// NO! This must stay in Rust
fn compute_engine_thrust() {
    // Called for every entity with Engine, every physics tick
}
```

**Why NOT scriptable:**
- High frequency
- Data-parallel iteration
- Type safety important

## Architecture: The Scripting Bridge

### Layered Approach

```
┌─────────────────────────────────────────────────────────┐
│                      MOD SCRIPTS                        │
│   missions/*.lua, ai/*.lua, economy/*.lua               │
│   (User/Modder Content - Hot Reloadable)                │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                  SCRIPT API LAYER                       │
│  (Rust) Safe wrappers around ECS operations             │
│                                                          │
│  ScriptWorld::spawn_entity()                            │
│  ScriptEntity::get_component<T>()                       │
│  ScriptQuery::find_entities_near()                      │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                  BEVY ECS CORE                          │
│  Physics, Networking, Persistence, Rendering            │
│  (30Hz authoritative simulation)                        │
└─────────────────────────────────────────────────────────┘
```

### Example: Mission System Integration

**Rust Side (`sidereal-scripting` crate):**

```rust
// bins/sidereal-replication/src/scripting.rs

use bevy::prelude::*;
use mlua::prelude::*;

#[derive(Component)]
pub struct ScriptedMission {
    pub script_handle: Handle<LuaScript>,
    pub state: MissionState,
}

pub fn setup_lua_api(lua: &Lua, world: &mut World) -> LuaResult<()> {
    let globals = lua.globals();
    
    // Expose world API
    let world_table = lua.create_table()?;
    
    world_table.set("spawn_npc_fleet", lua.create_function(|_lua, args: LuaValue| {
        // Safe wrapper that validates and spawns entities
        spawn_npc_fleet_internal(args)
    })?)?;
    
    world_table.set("find_entity_by_id", lua.create_function(|_lua, id: String| {
        // Lookup entity by UUID, return ScriptEntity wrapper
        find_entity_safe(id)
    })?)?;
    
    globals.set("world", world_table)?;
    
    Ok(())
}

fn spawn_npc_fleet_internal(args: LuaValue) -> LuaResult<ScriptEntity> {
    // Validate args, create ECS entities
    // Return safe handle that scripts can use
}
```

**Lua Side (`assets/missions/escort.lua`):**

```lua
function on_mission_start(mission)
    convoy = world.spawn_npc_fleet("cargo_convoy", {
        position = {x=1000, y=2000},
        faction = "trade_guild"
    })
    
    mission.convoy_id = convoy.id
end

function on_update(mission, dt)
    convoy = world.find_entity_by_id(mission.convoy_id)
    
    if not convoy.is_alive() then
        mission.fail()
    end
    
    if convoy.distance_to(mission.destination) < 100 then
        mission.complete()
    end
end
```

### Safety & Sandboxing

**Critical for Untrusted Mods:**

```rust
// Create sandboxed Lua environment
fn create_sandboxed_lua() -> Lua {
    let lua = Lua::new();
    
    // Remove dangerous functions
    lua.sandbox(true)?;
    lua.globals().set("os", Nil)?;        // No system access
    lua.globals().set("io", Nil)?;        // No file access
    lua.globals().set("require", Nil)?;   // No module loading
    lua.globals().set("dofile", Nil)?;    // No arbitrary code exec
    
    // Add custom safe require
    lua.globals().set("require", safe_mod_require)?;
    
    // CPU/memory limits
    lua.set_memory_limit(10 * 1024 * 1024)?;  // 10MB limit
    lua.set_hook(HookTriggers::all(), 100_000, |_lua, _debug| {
        // Abort if script runs too long
        Err(LuaError::RuntimeError("Script timeout".into()))
    })?;
    
    Ok(lua)
}
```

## Implementation Plan

### Phase 1: Proof of Concept (1-2 weeks)

**Goal**: Basic Lua integration working

1. Add `bevy_mod_scripting` + `mlua` dependencies
2. Create `crates/sidereal-scripting` library crate
3. Implement basic World API wrapper
4. Create simple test script (spawn entity, move it)
5. Verify hot-reloading works

**Deliverable**: Can load Lua script that spawns/moves an entity

### Phase 2: Mission System (2-3 weeks)

**Goal**: First real content use case

1. Design mission lifecycle (start, update, complete, fail)
2. Implement mission script loader
3. Add mission objectives API
4. Create 2-3 example missions
5. Add mission UI hooks

**Deliverable**: Playable scripted mission

### Phase 3: NPC AI (2-4 weeks)

**Goal**: Dynamic world with scripted behaviors

1. Design NPC behavior script interface
2. Implement AI tick system calling scripts
3. Add sensor/scan API for scripts
4. Add movement command API
5. Create example patrol/attack/flee behaviors

**Deliverable**: NPCs with scripted behaviors

### Phase 4: Modding Support (1-2 weeks)

**Goal**: External mods loadable

1. Define mod folder structure
2. Implement mod discovery/loading
3. Add mod manifest format
4. Create mod conflict resolution
5. Document modding API

**Deliverable**: Load external mods from `mods/` folder

### Phase 5: Polish & Tooling (Ongoing)

1. API documentation
2. Example mods
3. Error reporting / debugging
4. Hot-reload improvements
5. Performance profiling

## Performance Considerations

### Lua is Fast Enough (for content)

**Benchmarks** (rough guidelines):
- **Rust**: 1x (baseline)
- **Lua (LuaJIT)**: ~10-20x slower
- **Lua (mlua)**: ~20-50x slower
- **Python**: ~100-200x slower

**For content systems running at 1-10Hz**, even 50x slower is fine!

### Example Budget

```
30Hz physics loop: 33ms budget per frame
  - Physics: ~10ms
  - Replication: ~5ms
  - Rendering: ~10ms
  - Scripts: ~5ms  ← Scripts get 5ms = plenty!

1Hz mission update:
  - Can run 10 missions @ 0.5ms each = still only 5ms
  - That's 100,000 Lua instructions per mission!
```

### Optimization Strategies

1. **Batch API calls** - Don't query ECS per entity in loop
2. **Cache lookups** - Store entity handles in script state
3. **Throttle script ticks** - Missions at 1Hz, not 30Hz
4. **Use Rust for heavy compute** - Provide high-level APIs

## Bevy Support: Is This Well-Supported?

### Yes! ✅

**Bevy Scripting Ecosystem:**

1. **`bevy_mod_scripting`** - Official community plugin
   - Supports Lua (mlua), Rhai, JavaScript (Deno)
   - Event system integration
   - Hot-reloading built-in
   - Well-documented

2. **`bevy_rhai`** - Rhai integration
   - Pure Rust scripting
   - Good performance

3. **Active Community** - Many projects use scripting with Bevy

**Examples of Bevy + Scripting:**
- Tower defense games (Lua for enemy waves)
- RPGs (Lua for quests)
- Strategy games (Lua for AI)

**Documentation:**
- `bevy_mod_scripting` book: https://makspll.github.io/bevy_mod_scripting/
- Examples: https://github.com/makspll/bevy_mod_scripting/tree/main/examples

## Pros & Cons Summary

### Pros of Adding Scripting ✅

1. **Rapid Content Creation**
   - Add missions without recompiling
   - Iterate on AI behaviors quickly
   - Balance economy in real-time

2. **Mod Support**
   - Community-created campaigns
   - Custom factions and ships
   - Total conversions possible
   - Large, vibrant community potential

3. **Designer-Friendly**
   - Non-programmers can author content
   - Simpler syntax than Rust
   - Faster learning curve

4. **Hot-Reloading**
   - Tweak missions live in-game
   - No compile-test cycle
   - Speeds up development massively

5. **Separation of Concerns**
   - Core systems stable in Rust
   - Content flexible in scripts
   - Clear boundaries

6. **Proven Pattern**
   - Factorio, WoW, Overwatch, etc.
   - Industry standard for moddable games

### Cons of Adding Scripting ❌

1. **Performance Overhead**
   - 10-50x slower than Rust
   - Must carefully scope what's scriptable
   - Not suitable for hot paths

2. **Complexity**
   - New system to maintain
   - API surface to design/document
   - Security considerations (sandboxing)

3. **Debugging**
   - Lua errors less clear than Rust
   - Requires good error messages
   - Mod conflicts possible

4. **Type Safety**
   - Lua is dynamically typed
   - Can wrap with typed APIs
   - Runtime errors vs compile-time errors

5. **Learning Curve**
   - Team needs to learn Lua
   - API design is an art
   - Modders need documentation

6. **Binary Size**
   - +500KB-1MB for Lua runtime
   - Minor for desktop, matters for web

## Specific to Your Project

### Sidereal-Specific Considerations

#### ✅ Perfect Fit For Your Vision

Your stated goal:
> "Single-player campaign that others can join... large vibrant universe"

**This REQUIRES flexible content authoring:**
- Campaign missions: scripted
- NPC encounters: scripted
- Economy events: scripted
- Faction behaviors: scripted

Without scripting, you'll:
- Hard-code every mission in Rust (slow iteration)
- Recompile to tweak balance (painful)
- No mod support (limits community)

#### ✅ Fits Your Architecture

Your design already separates:
- Core simulation: `sidereal-replication` (30Hz Rust/ECS)
- Background sim: `sidereal-bg-sim` (eventual content)
- Content: missions, AI, economy

**Scripting fits perfectly in the "content" layer!**

#### ✅ Respects Your Principles

From `sidereal_design.md`:
> "Domain labels (`Ship`, `Missile`, `Asteroid`) describe archetypes, not special simulation code paths."

**Scripting aligns with this!** Scripts use capabilities (Engine, FuelTank, FlightComputer) not entity types.

#### ⚠️ Consideration: Server Authority

Your networking is server-authoritative. Scripts must:
- Run on **server side** for authority
- **Validate** all player actions
- **Never trust** client-side script results

This is achievable! Scripts run in `sidereal-replication`, clients receive results.

#### ⚠️ Consideration: Persistence

Mission state must persist. Design needed:
- Save mission progress to graph
- Restore mission state on load
- Handle mission cleanup on failure

This is solvable with good serialization.

## Recommended Path Forward

### Immediate Next Steps (Today)

1. **Read** `bevy_mod_scripting` documentation
2. **Prototype** basic Lua integration (1 day)
3. **Test** hot-reloading with simple script
4. **Evaluate** if it feels right for your workflow

### Short-Term (Next 2-4 Weeks)

1. **Scope** what systems should be scriptable
2. **Design** script API surface
3. **Implement** basic mission system
4. **Create** 2-3 example missions
5. **Document** API for future use

### Medium-Term (Next 2-3 Months)

1. **Expand** to NPC AI behaviors
2. **Add** dialogue/quest systems
3. **Implement** mod loading
4. **Create** modding documentation
5. **Release** mod tools

### Long-Term (6+ Months)

1. **Build** mod community
2. **Host** mod repository
3. **Curate** best mods
4. **Support** total conversions
5. **Expand** API based on feedback

## Alternative: Pure Rust Data-Driven

If you DON'T want scripting, consider:

**Data-Driven Mission System**

```yaml
# missions/escort.yaml
mission:
  id: escort_convoy_01
  name: "Escort Trade Convoy"
  
  on_start:
    - spawn_npc:
        archetype: cargo_convoy
        position: {x: 1000, y: 2000}
        faction: trade_guild
        save_as: convoy
    
    - add_objective:
        type: escort
        target: $convoy
        destination: {x: 5000, y: 8000}
        
    - schedule:
        delay: 60.0
        action:
          spawn_npc:
            archetype: pirate_raiders
            position: {x: 3000, y: 5000}
            target: $convoy
  
  on_convoy_destroyed:
    - fail_mission: "Convoy destroyed!"
    
  on_convoy_arrived:
    - complete_mission
    - give_reward:
        credits: 5000
        reputation: {trade_guild: 10}
```

**Pros:**
- Pure Rust (type-safe, fast)
- Declarative (easier to validate)
- No scripting overhead

**Cons:**
- Limited to predefined actions
- No custom logic (if/then/else very limited)
- Harder for complex missions
- Less flexible for modders

**Verdict:** Good for simple missions, insufficient for complex campaigns.

## Final Recommendation

### ✅ **YES - Add Lua Scripting**

**Why:**
1. Your vision requires flexible content creation
2. Perfectly complements ECS (doesn't replace it)
3. Industry-proven pattern for moddable games
4. Enables community contributions
5. Dramatically faster iteration on content
6. Bevy has good scripting support

**Scope:**
- ✅ Missions, quests, dialogue
- ✅ NPC AI behaviors
- ✅ Economy/background events
- ✅ Procedural generation algorithms
- ❌ Physics, networking, rendering (stay Rust)

**Recommended Implementation:**
- **Language**: Lua via `bevy_mod_scripting`
- **Integration**: Phase 1 (PoC) → Phase 2 (Missions) → Phase 3 (AI)
- **Timeline**: 6-10 weeks to production-ready mission system
- **Effort**: Moderate (well-supported in Bevy)

**Next Action:** Create small prototype to validate approach (1-2 days).

---

## References

### Documentation to Review
- `bevy_mod_scripting` book: https://makspll.github.io/bevy_mod_scripting/
- `mlua` docs: https://docs.rs/mlua/latest/mlua/
- Factorio Lua API (inspiration): https://lua-api.factorio.com/

### Your Related Docs
- `docs/sidereal_design.md` - Architecture principles
- `docs/ecs_components.md` - Component catalog
- `docs/sim_core_scope.md` - Shared simulation boundary
- `docs/implementation_plan.md` - Milestone M4 (background sim)

### Decision Register Impact
Consider adding:
- **DR-020**: Scripting Language Selection (Lua for content)
- **DR-021**: Script-ECS Boundary Definition
- **DR-022**: Mod Security and Sandboxing Policy
