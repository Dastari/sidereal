# Background World Simulation Contract

Status: Proposed source-of-truth for background/offscreen world simulation direction  
Date: 2026-03-11

Primary references:
- `docs/sidereal_design_document.md`
- `docs/features/visibility_replication_contract.md`
- `docs/features/galaxy_world_structure.md`
- `docs/features/scripting_support.md`
- `AGENTS.md`

Update note (2026-03-11):
- Initial contract for economy-driven offscreen simulation, abstract actors, dynamic traffic lanes, and promotion from abstract simulation into full runtime ECS when player relevance requires it.

## 1. Goal

Keep the world alive without paying full runtime simulation cost everywhere:

1. Distant/offscreen actors continue to exist and progress through goals, travel, trade, piracy, faction response, and mission generation.
2. Nearby or otherwise player-relevant actors instantiate into full authoritative ECS/runtime simulation.
3. Economy, missions, and faction pressure come from the same authoritative world model for players and NPCs.
4. Abstract simulation must preserve encounterability: a player must be able to legitimately come across traffic that was previously offscreen.

This contract is the authoritative direction for "higher-level simulation" or "Quantum-style" background simulation in Sidereal.

## 2. Compatibility With Existing Architecture

This feature must preserve the existing architecture contract:

1. Authority remains one-way: `client input -> shard sim -> replication/distribution -> persistence`.
2. Clients never authoritatively create, move, or resolve background actors.
3. Durable world state remains graph-record oriented. Persistent background actors, factions, nodes, and mission state must use canonical graph entity/component persistence rather than ad-hoc delta stores.
4. Visibility still governs what connected clients receive. Background simulation may influence what is spawned/promoted, but it must not bypass the `Authorization -> Delivery -> Payload` contract.
5. Player, faction, node, route, and actor identity crossing boundaries uses UUID/entity IDs only.
6. Shared gameplay/economy logic that affects authoritative outcomes belongs in shared Rust crates or validated script APIs, not duplicated per platform.

## 3. World Model

Background world simulation is built from the following runtime concepts:

1. `EconomicNode`
   - A station, colony, refinery, shipyard, mining outpost, or other location with finite inventory, finite budget, ownership, and local demand.
2. `FactionEconomyState`
   - A faction-level budget, territorial/control footprint, hostility/alignment context, and replacement pressure for assets it owns or sponsors.
3. `TrafficLane`
   - A derived route pattern between origin/destination nodes or resource regions. Lanes are not fully hand-authored first; they emerge from repeated route usage.
4. `ProbabilityVolume`
   - A derived risk/opportunity volume built from current and recent traffic, security presence, faction tension, and resource value. These volumes influence encounter generation, piracy pressure, escort demand, and mission generation.
5. `AbstractActor`
   - An offscreen actor represented by high-level state instead of full physics/runtime ECS participation.
6. `HeroActor`
   - A named or otherwise important persistent actor that may remain meaningful across missions, faction leadership, story content, or player relationships.
7. `EncounterGroup`
   - A runtime-instantiated set of full ECS entities created from abstract actors, mission state, or probability-volume outcomes when full world interaction is required.

## 4. Actor Classes

Sidereal distinguishes between two broad actor classes:

### 4.1 Quanta / Abstract Workers

`Quanta` are lightweight background actors used to keep the economy and faction activity moving:

1. They are authoritative server-side actors with goals, traits, route progress, and role-specific state.
2. They are normally unnamed and non-player-facing while abstract.
3. They do not require full physics, AI steering, or replication while offscreen.
4. They may fulfill supply-demand pressure directly without always taking an explicit player-visible mission contract.
5. Public Star Citizen material describes Quantum as using lightweight individual quanta plus probability-volume-driven encounter realization; Sidereal follows that same high-level direction rather than treating all offscreen traffic as pure pooled counters.

### 4.2 Hero Actors

`HeroActor` is the lane for meaningful persistent NPCs:

1. They may accept/generated missions, own named ships, hold faction rank, accumulate history, and become recurring world actors.
2. They persist as explicit durable actors even when not instantiated into full ECS runtime.
3. They are eligible for promotion into full runtime simulation for player proximity, mission relevance, faction importance, story events, and similar reasons.

## 5. Canonical Abstract State

When an actor is not fully instantiated, its authoritative state is abstract rather than physics-authored.

Minimum abstract state for a mobile actor:

1. stable actor id,
2. actor class (`Quanta` vs `HeroActor`),
3. owner/faction/alignment,
4. current role (`trader`, `miner`, `pirate`, `escort`, `hauler`, `faction_patrol`, etc.),
5. route definition (`origin`, `destination`, optional waypoints),
6. route progress,
7. cargo summary,
8. ship/fleet quality summary,
9. attack score,
10. defense score,
11. wealth/budget linkage,
12. current mission or job,
13. next scheduled decision or arrival time.

Canonical travel state while abstract is:

1. `route A -> B`,
2. progress along that route,
3. optional route metadata such as departure time, estimated arrival window, and cargo obligation.

Sidereal does not require continuous offscreen physics integration for abstract actors. Route progress is the canonical state, not frame-by-frame hidden motion.

## 6. Simulation Tiers

Sidereal uses tiered simulation rather than one always-live runtime:

### 6.1 `FullRuntime`

Actor or entity exists as ordinary authoritative ECS/runtime state:

1. full component state,
2. full physics/motion ownership,
3. visibility/replication participation,
4. local player interaction,
5. ordinary combat/damage/cargo behavior.

### 6.2 `AbstractSimulation`

Actor exists as durable offscreen world state:

1. no continuous physics integration,
2. decisions resolved at lower cadence or scheduled times,
3. progress represented as route/job state,
4. outcomes applied through validated authoritative state transitions.

### 6.3 `DerivedPressure`

Some world pressure is not stored as per-actor physical simulation at all. It is derived from recent authoritative activity:

1. lane heat,
2. piracy pressure,
3. security response pressure,
4. escort demand,
5. market scarcity signals.

`DerivedPressure` is not a replacement for durable actors. It is a supporting runtime product used to influence mission generation, encounter probabilities, and faction response.

### 6.4 Runtime Residency Architecture

Background simulation residency is a distinct concern from client visibility.

Normative rules:

1. Visibility decides what a connected client receives.
2. Residency decides whether an actor exists as loaded authoritative ECS/runtime state or as abstract background state.
3. The spatial visibility grid is a visibility/delivery optimization. It must not become the only authority for load/unload decisions.

Current direction:

1. Near-term authority should remain inside the replication host.
2. Background simulation should initially run as a domain/plugin within `sidereal-replication`, not as an independent DB-writing binary.
3. Persistence remains durability, not the live arbitration bus between two competing authoritative writers.

Reasoning:

1. Separate binaries both writing canonical actor/node/faction state through the database would create a dual-writer architecture.
2. Promotion/demotion, cargo reservation, mission acceptance, and abstract combat resolution all become race-prone if replication and a background-sim service can both directly commit the same actor state.
3. The current project authority flow is still `client input -> shard sim -> replication/distribution -> persistence`; residency should fit inside that shape rather than bypassing it.

Future extraction direction:

1. If background-sim CPU/operational cost later justifies a separate process, split only after the boundary is explicit.
2. In that future model:
   - replication owns `LoadedRuntime`,
   - background service owns `AbstractOnly`,
   - promotion/demotion happens through an explicit service contract or control RPC,
   - persistence remains durability rather than the primary live coordination mechanism.

#### 6.4.1 Residency State Machine

Actors that can move between abstract and loaded runtime should follow an explicit residency state machine.

Canonical states:

1. `AbstractOnly`
   - authoritative state is high-level background state only,
   - no full runtime ECS entity is active.
2. `Promoting`
   - actor is being realized from abstract state into runtime ECS entities,
   - load/hydration/spawn must be atomic from the perspective of game logic.
3. `LoadedRuntime`
   - actor is fully instantiated in authoritative ECS/runtime simulation.
4. `Demoting`
   - runtime state is being folded back into canonical abstract state and then unloaded.

Optional future pinned substate:

1. `LoadedRuntimePinned`
   - actor remains loaded even if no player is immediately nearby because mission/story/faction rules require it.

#### 6.4.2 Promotion Triggers vs Visibility Triggers

Promotion is not the same thing as client visibility gain.

An actor may promote because of:

1. player bubble/local encounter relevance,
2. likely route interception based on player travel and actor route overlap,
3. mission relevance,
4. hero/faction importance,
5. scripted event demand,
6. explicit admin/debug controls.

An actor becoming visible to a player usually requires promotion, but promotion can happen slightly ahead of visibility gain so the world does not "pop" from abstract state directly into first-frame combat or docking.

#### 6.4.3 Unload / Demotion Rules

Demotion must be conservative and explicit.

An actor should only demote when:

1. it is no longer needed for nearby player interaction,
2. it is not pinned by mission/story/faction rules,
3. runtime outcomes that matter economically have already been committed,
4. cargo, damage, mission, and ownership state are representable in abstract form,
5. unload will not violate immediate encounter continuity.

Unload must not be:

1. a direct "not in spatial grid, therefore despawn" shortcut,
2. a visibility-only side effect,
3. a silent deletion of authoritative consequences.

#### 6.4.4 Load / Unload Pipeline

Recommended runtime pipeline:

1. Background simulation updates abstract actors/nodes/faction state.
2. Residency evaluator determines which abstract actors need promotion.
3. Promotion realizes abstract actor state into authoritative ECS/runtime entities.
4. Ordinary visibility/replication systems deliver those entities to clients once authorized and in scope.
5. When relevance ends, demotion folds ECS/runtime state back into canonical abstract state.
6. Runtime entities are then removed from loaded residency while durable consequences remain.

#### 6.4.5 Scripting Interaction Rules

Lua may influence residency policy, but Lua should not directly own residency transitions.

Rules:

1. Scripts may request promotion through validated domain intents or APIs.
2. Scripts may contribute reasons to keep an actor pinned/loaded.
3. Scripts may not directly despawn loaded authoritative actors as a substitute for demotion logic.
4. Residency state ownership remains a Rust/kernel concern because it controls authority, persistence boundaries, and race-sensitive promotion/demotion transitions.

## 7. Promotion and Demotion

### 7.1 Promotion Triggers

An abstract actor or encounter candidate may promote into `FullRuntime` for any of the following:

1. player relevance through visibility/local bubble proximity,
2. mission relevance,
3. story/event relevance,
4. faction importance,
5. high-value convoy or target status,
6. explicit scripted/admin-controlled event generation.

### 7.2 Promotion Rules

Promotion must preserve world continuity:

1. Spawn position/orientation derive from route progress and route geometry, not arbitrary nearby placement.
2. Cargo and fleet composition must be constrained by the actor's abstract role, quality, and current route obligation.
3. Hero actors retain their exact persistent identity across promotion.
4. Quanta promotion may materialize one or more full ECS entities representing the abstract actor or convoy.
5. Promotion must not invent value that the abstract actor did not economically justify.

### 7.3 Demotion Rules

When a runtime actor is no longer relevant for full simulation:

1. authoritative runtime state must be folded back into canonical abstract state,
2. cargo/value losses and mission outcomes must already be committed,
3. the resulting abstract state must remain encounter-safe and persistence-safe,
4. visibility loss alone must not silently delete durable world consequences.

## 8. Economic Nodes and Budgets

Each `EconomicNode` should eventually support:

1. finite inventory,
2. finite buy/sell budget,
3. ownership by faction, player, or no owner,
4. demand signals,
5. price adjustment,
6. job/mission generation,
7. optional production/refining/shipyard functions.

Ownership direction:

1. In the long term, durable world assets are expected to be owned by a faction, a player, or nobody.
2. Unowned assets/locations may be claimable by later gameplay systems.

Faction direction:

1. Factions have long-term goals, territory/control, stations or planets they control, finite budgets, and pressure to replace lost assets.
2. Factions may generate missions that aligned or otherwise eligible players can accept.
3. Faction destruction/loss should feed back into economy and security pressure rather than being purely cosmetic.

## 9. Dynamic Lanes and Probability Volumes

Traffic lanes are derived rather than fully authored:

1. repeated trade/mining/hauling routes raise lane heat between locations,
2. higher-value and higher-volume lanes attract pirate pressure,
3. higher pirate pressure raises security/escort demand,
4. successful security suppression lowers pirate success and future pressure,
5. lane popularity can be recomputed from recent rolling history; it does not need to be canonical persisted world state in v1.

Probability volumes are derived products of that lane/system pressure:

1. a pirate ambush is more likely where traffic value is high and security is weak,
2. a security patrol is more likely where faction assets are threatened,
3. hauling/escort/combat missions should emerge from those pressures,
4. probability volumes influence encounter generation but are not themselves a visibility authorization bypass.

## 10. Offscreen Conflict Resolution

Offscreen combat and interception are resolved abstractly in v1.

Normative rules:

1. Resolution is weighted, not binary deterministic.
2. Groups sum or otherwise aggregate attack and defense scores.
3. Cargo theft is capacity-constrained. A pirate group with small cargo capacity cannot fully loot a much larger convoy.
4. Valid outcomes include:
   - full destruction,
   - partial loot,
   - delay/disruption,
   - forced reroute,
   - defender victory,
   - pirate withdrawal.
5. Convoy quality should influence both survivability and likely cargo value.
6. Abstract outcomes must feed back into:
   - node inventories,
   - prices/demand,
   - repair/replacement demand,
   - faction/security pressure,
   - mission generation.

Non-goal for v1:

1. hidden always-live full ECS combat bubbles with no player relevance.

## 11. Mission Generation

Missions should emerge from the same world pressures that affect NPC actors.

Normative direction:

1. shortages or delivery gaps create hauling/logistics missions,
2. pirate pressure creates escort/combat missions,
3. faction threats create patrol/defense/blockade response missions,
4. player activity can satisfy, worsen, or redirect these pressures,
5. hero actors may accept or compete for generated missions,
6. quanta/workers may satisfy generic supply-demand pressure without always consuming a formal player-visible mission contract.

This preserves two lanes:

1. explicit contract/mission lane for players and hero actors,
2. background fill-in lane for quanta so the economy does not deadlock if players ignore a route.

## 12. Persistence Model

Durable background simulation must remain graph-record compatible.

Normative rules:

1. Durable nodes, factions, hero actors, persistent abstract actors, and mission state must be representable as graph records and component payloads.
2. Route heat, probability volumes, and other derived pressure products may be transient runtime products if they are safely recomputable.
3. Background simulation must not introduce a second canonical persistence authority that bypasses graph-record durability.
4. Promotion and demotion must preserve canonical identity across persistence boundaries.

## 13. V1 Scope

V1 for this feature is intentionally narrower than the full long-term vision.

V1 includes:

1. finite node inventories,
2. finite node budgets,
3. buy/sell demand signals,
4. abstract transport actors with route progress,
5. dynamic lane heat derived from recent traffic,
6. probability-volume-style piracy/security pressure,
7. weighted abstract conflict resolution,
8. generated hauling/escort/combat missions,
9. promotion into full runtime for player relevance and explicit scripted reasons.

V1 does not require:

1. complete station/factory production chains down to raw materials,
2. fully detailed manufacturing bill-of-material dependency graphs,
3. hidden offscreen full ECS combat bubbles,
4. final multi-shard handoff behavior.

## 14. Target Direction Beyond V1

The long-term target direction includes:

1. station/factory production chains down to raw resources,
2. resource depletion and regrowth across mining regions,
3. shipyard replacement constrained by faction budgets and industrial capacity,
4. more detailed faction strategic goals and territorial pressure,
5. cross-shard transit and shard-aware background simulation,
6. richer promotion rules for convoys, pirate gangs, faction leaders, and live world events.

## 15. Open Policy Questions

These questions remain intentionally open and should be resolved before implementation hardens around them:

1. Exact durable model for resource depletion and regrowth.
2. Exact formula family for abstract attack/defense/cargo-loss resolution.
3. Exact data schema for actor quality, fleet composition, and cargo synthesis on promotion.
4. Exact component split between node-owned inventory state, faction budget state, and future production-chain state.

## 16. Edit Checklist

For any implementation change touching background simulation:

1. Verify authority still remains server-side.
2. Verify abstract simulation does not bypass visibility or replication redaction rules.
3. Verify durable state remains graph-record compatible.
4. Verify promotion/demotion preserves identity and economically justified cargo/value.
5. Verify player and NPC pressures feed the same mission/economy model.
6. Update this contract and related docs when scope or policy changes.
