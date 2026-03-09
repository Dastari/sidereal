# Action Acceptor Control Routing Model

Status: Proposed decision detail (`DR-0013`)  
Date: 2026-02-24  
Owners: gameplay runtime + replication + client input

Decision Register linkage: `DR-0013`

## 1. Intent

Adopt a generic, component-driven action routing model where:

1. Actions are high-level intents (`forward`, `backward`, `left`, `right`, `shoot`, `ping`, etc.).
2. Components on an entity explicitly accept and handle subsets of those actions.
3. Control context determines which entity receives intent (observer/player entity by default, controlled entity when mounted).
4. Keybinds map physical input to actions through a configurable binding layer.

This is designed to remain entity-generic (ships, asteroids, stations, characters, drones, future entity classes).

## 2. Current Baseline

Current behavior already has part of this model:

1. `EntityAction` intent enum and `ActionQueue` on entities.
2. Flight handling pipeline (`ActionQueue -> FlightComputer -> Engines/Fuel/Forces`).
3. Client input mapped to actions and sent through Lightyear.

Current gaps (remaining):

1. No explicit component-level action acceptor registration/dispatch contract beyond coarse `ActionCapabilities`.
2. No first-class configurable keybind system.
3. Action-family prediction policy (beyond current movement-centric behavior) is not fully specified/implemented.

## 3. Decision

### 3.1 Canonical Runtime Concepts

1. `Action`: high-level intent, transport/prediction-safe.
2. `ActionTarget`: entity currently selected by control context to receive actions.
3. `ActionAcceptor`: component/system pair that declares accepted actions and executes authoritative behavior.
4. `ControlContext`: per-session authoritative state describing active observer entity and selected controlled entity (if any).
5. `InputBinding`: client-side mapping from input source (keyboard/gamepad/etc.) to action(s).

### 3.1.1 Authoritative Follow Chain (Normative)

Runtime chain is explicitly:

1. `camera <- observer/player entity <- controlled entity (optional)`

Semantics:

1. `ControlledEntityGuid = Some(target)`:
- controlled entity simulation is the movement authority for that target,
- observer/player entity follows that controlled target authoritatively,
- camera follows observer/player entity.
2. `ControlledEntityGuid = Some(self player guid)`:
- observer/player entity movement acceptor is movement authority,
- camera still follows observer/player entity.
3. Detached free-cam is a separate explicit camera-mode policy:
- detached mode is entered/exited via explicit client camera mode switch,
- detached mode suppresses gameplay movement-intent emission to avoid dual-input conflicts,
- detached mode must not alter server-authoritative control routing semantics.

Invariants:

1. Exactly one gameplay movement writer is authoritative per layer.
2. Camera follow must not write simulation motion state.
3. Control state persists on the observer/player entity, never in ad-hoc session side tables.

### 3.2 Control Routing Rules

1. Movement actions always route to `ControlledEntityGuid` target.
2. Free-roam is represented by setting `ControlledEntityGuid` to the observer/player entity GUID.
3. If controlled entity exists, movement/combat/utility actions route to controlled entity unless explicitly marked observer-scoped.
4. Observer/player transform anchors camera by default.
5. When controlled entity is active, observer/player entity remains attached/anchored to controlled entity position (authoritative server rule).

### 3.3 Component Acceptance Rules

1. Multiple components may accept the same action on one entity.
2. Acceptance does not imply immediate effect; component-local cooldowns/constraints decide execution.
3. Action handling order must be deterministic and documented for conflicting handlers.
4. Server remains authoritative for all resulting state changes.

### 3.4 Action Dispatch Contract (Normative)

Dispatch ordering:

1. Input intent is decoded into canonical action verbs.
2. Server resolves authoritative `ActionTarget` from authenticated control context.
3. Action queue is appended on target entity.
4. Acceptor systems execute in deterministic fixed-order in fixed tick.
5. Effects are applied through authoritative gameplay/physics systems.

Determinism requirements:

1. Acceptor evaluation order must be explicit in schedule ordering.
2. Cooldowns/rate limits are acceptor-side policy (server authoritative).
3. Multiple acceptors handling the same action must produce deterministic outcomes under fixed-step replay/rollback.

## 4. Refactor Impact Map

### 4.1 Gameplay Core (`crates/sidereal-game`)

1. Evolve `EntityAction` taxonomy toward verb-oriented categories.
2. Introduce explicit action acceptor registration/dispatch contract.
3. Keep existing flight handler as one acceptor implementation.
4. Add character movement acceptor component/system for observer/player entity.
5. Add tests for multi-acceptor routing, deterministic ordering, and cooldown behavior.

### 4.2 Network Protocol (`crates/sidereal-net`)

1. Ensure input payload shape supports generic action sets (not flight-only assumptions).
2. Preserve backward compatibility or provide migration shim for existing message flow.
3. Keep client/server protocol parity for native and WASM targets.

### 4.3 Replication Runtime (`bins/sidereal-replication`)

1. Remove flight-only ingress filtering in input drain path.
2. Route incoming action intents to authoritative `ActionTarget` from authenticated control context.
3. Generalize control-target mapping naming where semantics are generic.
4. Preserve authenticated session binding and spoof rejection.
5. Add metrics for accepted/dropped actions by reason and by action family.

### 4.4 Client Runtime (`bins/sidereal-client`)

1. Insert input binding abstraction between physical input and actions.
2. Implement no-controlled-target routing semantics (observer/player entity movement).
3. Preserve current controlled-target path and camera behavior.
4. Add UI/settings path for keybinds (phased).
5. Keep native and WASM behavior lockstep in same changes.

### 4.5 Persistence/Hydration

1. Persist control context components on observer/player entity as canonical runtime state.
2. Ensure hydration restores control target and observer state deterministically.
3. Keep entity-generic persistence shape; no ship-only assumptions in persistence APIs.

### 4.6 Visibility/Authorization

1. Observer anchor remains player/character entity position.
2. Controlled entity attachment remains an input/control concern, not a visibility policy shortcut.
3. No visibility entitlement broadening from control routing changes.

### 4.7 Legacy Paths to Remove

1. Flight-only server action ingress filters.
2. Camera-to-controlled direct anchoring as default follow semantics.
3. Ship-only naming for generic control/visibility runtime resources where semantics are entity-generic.
4. Hardcoded input-to-action assumptions that bypass configurable binding layer.

## 5. Execution Plan

## Phase A: Vocabulary + Contracts

1. Finalize action taxonomy and acceptance contract in `sidereal-game`.
2. Document deterministic handler ordering.
3. Add/update decision/docs references.

Exit:
- Action and acceptor contracts are source-of-truth documented.

## Phase B: Server Input Routing Generalization

1. Remove flight-only action gate from replication input drain.
2. Route actions using authenticated control context to current action target.
3. Keep strict auth/session binding validation.

Exit:
- Server accepts generic actions and routes to authoritative target.

## Phase C: Character Movement Acceptor

1. Add character movement component/system that accepts movement actions.
2. Implement no-controlled-target movement on observer/player entity.
3. Ensure player-entity/camera anchor semantics are stable.

Exit:
- Movement works with and without controlled entity using same action pipeline.

## Phase D: Multi-Acceptor Dispatch

1. Add acceptor registry/dispatch layer.
2. Keep existing FlightComputer behavior as acceptor.
3. Add scanner/combat acceptor scaffolds (`ping`, `shoot`) with server-side cooldown enforcement points.

Exit:
- Multiple components can accept same action deterministically.

## Phase E: Client Keybind System (Foundational)

1. Add configurable input bindings resource.
2. Map keyboard to action IDs via binding layer.
3. Add persistence for bindings and reset-to-default path.

Exit:
- Key mappings are no longer hardcoded; bindings drive action emission.

## Phase F: Prediction/Reconciliation Policy

1. Classify actions:
   - predicted-continuous (movement),
   - predicted-discrete (optional),
   - server-authoritative-only (combat/scan, default).
2. Define swap-control behavior and pending intent ownership rules across target changes.
3. Add reconciliation tests for control handoff scenarios.

Exit:
- Explicit, tested policy for prediction vs authority by action family.

## Phase G: Migration + Cleanup

1. Remove deprecated naming and old routing shortcuts.
2. Keep compatibility adapters only until all call sites migrate.
3. Update integration/e2e coverage and performance baselines.

Exit:
- No duplicated legacy action pipeline remains.

## 6. Testing and Quality Gates

Required at each phase:

1. Unit tests for touched gameplay/input routing logic.
2. Integration tests across client -> replication action flow.
3. Control handoff tests (observer <-> controlled) under reconnect/swap.
4. Native + WASM + Windows client compile checks.
5. Replication performance sanity checks with action-heavy workloads.

## 7. Native/WASM Impact

1. Native and WASM must share the same action taxonomy, binding model, and control routing semantics.
2. Platform-specific networking remains only at transport boundary.
3. Any client behavior change in this plan must include explicit WASM parity validation in the same change.

## 8. Open Questions

1. Should `EntityAction` be renamed to `IntentAction` or retained for compatibility?
2. Do we encode action handler priority explicitly, or rely on deterministic system order per acceptor family?
3. Which non-movement actions (if any) are eligible for client prediction in initial rollout?

## 9. Implementation Progress

Initial slice implemented (2026-02-24):

1. Generic action ingress on replication no longer flight-only filters actions.
2. Character movement acceptor component/system added and wired into authoritative fixed simulation.
3. Control binding fallback now targets player/observer entity when no controlled ship is active.
4. Client controlled-tag fallback now routes input to local player entity when no controlled ship is selected.
5. Input axis mapping now emits canonical movement verbs (`Forward/Backward/Left/Right` + neutral actions), with legacy flight aliases still supported for compatibility.

Additional alignment implemented (2026-02-24):

1. Shared gameplay fixed-step system keeps observer/player entity synchronized to controlled target when `ControlledEntityGuid` is set.
2. Default camera anchor follows observer/player entity rather than directly following controlled target.
3. No-controlled mode preserves WASD free-roam on observer/player entity with camera follow by default.
4. Replication visibility observer anchor naming updated to player-observer terminology.

Phase status snapshot:

1. Phase A: in progress (taxonomy/contract established; dispatch registry still pending).
2. Phase B: completed.
3. Phase C: completed.
4. Phase D: pending.
5. Phase E: pending.
6. Phase F: pending.
7. Phase G: pending.
