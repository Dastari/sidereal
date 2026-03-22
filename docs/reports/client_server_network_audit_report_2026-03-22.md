# Client/Server Network Audit Report

Status: Completed audit  
Date: 2026-03-22  
Scope: Current repository implementation, not intended architecture

## 1. Executive Summary

The current Sidereal network architecture is directionally sound for a server-authoritative MMO/ARPG and already contains several strong decisions that should be kept:

- gateway HTTP/auth/bootstrap is clearly separated from replication transport;
- native runtime authority is server-owned, with authenticated player/session binding before input and control routing;
- player-anchor camera/control semantics are separated from visibility-source semantics;
- tactical fog/contact data and owner manifest data are correctly modeled as separate server-authored lanes instead of local-bubble world replication;
- the codebase is already compensating for real upstream Lightyear limits around native server input, dynamic predicted/interpolated control handoff, and interpolation bootstrap.

The main problems are not that the repo is “doing client authority”, but that a few protocol/runtime edges are still too loose:

1. replication silently drops several auth failures instead of explicitly denying them;
2. delivery-culling view-mode updates are transported on an unordered reliable control lane without sequencing;
3. the highest-priority input lane is overloaded with non-input combat FX traffic;
4. the client starts bootstrap-required asset downloads before replication bind success is known.

I did not find evidence that the player-anchor currently grants visibility by itself. The current code instead derives visibility range from `SimulatedControlledEntity` roots and moves the observer anchor to the authoritative controlled target when one exists.

## 2. Audit Method and Source Hierarchy

Authoritative source hierarchy used:

1. `AGENTS.md`
2. `docs/sidereal_design_document.md`
3. `docs/features/visibility_replication_contract.md`
4. `docs/features/tactical_and_owner_lane_protocol_contract.md`
5. `docs/features/asset_delivery_contract.md`
6. `docs/decision_register.md`
7. `docs/decisions/dr-0002_explicit_world_entry_flow.md`
8. `docs/decisions/dr-0013_action_acceptor_control_routing.md`
9. `docs/decisions/dr-0017_dual_lane_replication_and_owner_asset_manifest.md`
10. `docs/decisions/dr-0018_fog_of_war_and_intel_memory_model.md`
11. `docs/features/lightyear_upstream_issue_snapshot.md`

Primary code paths inspected:

- `bins/sidereal-gateway/src/api.rs`
- `bins/sidereal-gateway/src/auth/service.rs`
- `bins/sidereal-gateway/src/auth/bootstrap_dispatch.rs`
- `bins/sidereal-replication/src/main.rs`
- `bins/sidereal-replication/src/replication/auth.rs`
- `bins/sidereal-replication/src/replication/control.rs`
- `bins/sidereal-replication/src/replication/input.rs`
- `bins/sidereal-replication/src/replication/visibility.rs`
- `bins/sidereal-replication/src/replication/tactical.rs`
- `bins/sidereal-replication/src/replication/owner_manifest.rs`
- `bins/sidereal-replication/src/replication/assets.rs`
- `bins/sidereal-replication/src/replication/runtime_state.rs`
- `bins/sidereal-client/src/runtime/auth_net.rs`
- `bins/sidereal-client/src/runtime/transport.rs`
- `bins/sidereal-client/src/runtime/control.rs`
- `bins/sidereal-client/src/runtime/input.rs`
- `bins/sidereal-client/src/runtime/replication.rs`
- `bins/sidereal-client/src/runtime/tactical.rs`
- `bins/sidereal-client/src/runtime/owner_manifest.rs`
- `bins/sidereal-client/src/runtime/assets.rs`
- `crates/sidereal-net/src/lightyear_protocol/messages.rs`
- `crates/sidereal-net/src/lightyear_protocol/registration.rs`

Upstream primary sources checked on 2026-03-22:

- Bevy 0.18 release post: <https://bevy.org/news/bevy-0-18/>
- Lightyear book root: <https://cbournhonesque.github.io/lightyear/book/>
- Lightyear issues:
  - `#1034` Prediction switching
  - `#1200` native server input overflow panic
  - `#1380` required components / interpolation bootstrap

## 3. Current Network Topology Map

### 3.1 HTTP / gateway boundary

- Client -> gateway `POST /auth/register`, `POST /auth/login`, `POST /auth/refresh`, `GET /auth/me`, `GET /auth/characters`, `POST /world/enter`, `GET /assets/bootstrap-manifest`, `GET /assets/{asset_guid}`, `GET /startup-assets/manifest`, `GET /startup-assets/{asset_guid}`.
- Gateway validates account identity and ownership before `enter_world` in `bins/sidereal-gateway/src/auth/service.rs`.
- Gateway dispatches bootstrap/admin messages to replication over a separate UDP control path in `bins/sidereal-gateway/src/auth/bootstrap_dispatch.rs`.

### 3.2 Replication transport boundary

- Native client path: Lightyear raw UDP client/server.
- WASM path: Lightyear WebTransport client/server, WebTransport-first, no active WebSocket fallback path found.
- Replication starts both UDP and optional WebTransport servers in `bins/sidereal-replication/src/replication/lifecycle.rs`.

### 3.3 Authority boundary

- Authenticated identity flow is `gateway JWT -> ClientAuthMessage -> replication session bind`.
- Authoritative input flow is `ClientRealtimeInputMessage -> server-side bound player -> authoritative controlled entity map -> ActionQueue`.
- Control swaps are `ClientControlRequestMessage -> ownership validation -> server authoritative `ControlledEntityGuid` / `PlayerControlledEntityMap` update -> ack/reject`.
- Visibility flow is `Authorization -> Delivery -> Payload`, with authorization in `authorize_visibility(...)` and delivery narrowing in `finalize_visibility_evaluation(...)`.

### 3.4 Debug and admin surfaces

- Replication BRP can be enabled and is intended loopback-only.
- Gateway admin HTTP path `POST /admin/spawn-entity` forwards to replication control UDP.

## 4. Channel and Protocol Matrix

| Path | Transport | Ordering / reliability | Producer -> consumer | Current purpose | Notes |
|---|---|---|---|---|---|
| `ControlChannel` | Lightyear | `UnorderedReliable` | client <-> replication | auth bind, control swap req/ack/reject, view mode, disconnect notify | Semantically overloaded |
| `InputChannel` | Lightyear | `SequencedUnreliable` | client -> replication | realtime action snapshots | Correct for latest-wins input |
| `InputChannel` | Lightyear | `SequencedUnreliable` | replication -> client | weapon fired + destruction FX messages | Overloaded with non-input traffic |
| `TacticalSnapshotChannel` | Lightyear | `OrderedReliable` | replication -> client | fog/contact snapshots | Correct |
| `TacticalSnapshotChannel` | Lightyear | `OrderedReliable` | client -> replication | tactical resnapshot requests | Acceptable, though not separately isolated |
| `TacticalDeltaChannel` | Lightyear | `SequencedUnreliable` | replication -> client | fog/contact deltas | Matches contract |
| `ManifestChannel` | Lightyear | `OrderedReliable` | replication -> client | owner manifest snapshot/delta, asset catalog version invalidation | Correct |
| gateway auth/bootstrap | HTTP JSON | request/response | client <-> gateway | auth, character list, enter-world, manifests | Correct separation from replication |
| gateway asset payload | HTTP bytes | request/response | client <-> gateway | startup/bootstrap/runtime asset payloads | Correct separation from replication |
| gateway -> replication bootstrap | UDP JSON | best-effort command handoff | gateway -> replication | bootstrap player, admin spawn | Internal trusted control path |

## 5. World Entry and Initial-State Findings

Observed world-entry sequence:

1. client authenticates via gateway;
2. client calls `POST /world/enter`;
3. gateway validates ownership and dispatches bootstrap;
4. client immediately stores replication transport config and starts bootstrap-required asset request in `bins/sidereal-client/src/runtime/auth_net.rs:613-639`;
5. client Lightyear transport connects and sends `ClientAuthMessage`;
6. replication validates JWT/player binding and sends `ServerSessionReadyMessage` on success in `bins/sidereal-replication/src/replication/auth.rs:233-466`;
7. client waits for both `ServerSessionReadyMessage` and local player runtime presence before leaving `WorldLoading` in `bins/sidereal-client/src/runtime/replication.rs:455-476`;
8. client then gates `InWorld` on bootstrap-asset completion in `bins/sidereal-client/src/runtime/replication.rs:478-490`.

Assessment:

- The `session-ready + local player entity present` gate is correct and should be kept.
- The client-side bootstrap logic also correctly realigns desired control to the authoritative replicated control target during bootstrap, instead of immediately forcing self-control back onto the server (`bins/sidereal-client/src/runtime/replication.rs:1020-1050`).
- The main flaw is ordering efficiency: asset bootstrap starts before replication bind success is known.

## 6. Input, Control, and Authority Findings

Current authority routing is strong:

- replication binds transport/session identity to `player_entity_id` before accepting realtime input or control requests;
- control requests validate ownership against hydrated runtime entities in `bins/sidereal-replication/src/replication/control.rs`;
- realtime input spoofing is rejected on player mismatch in `bins/sidereal-replication/src/replication/input.rs`;
- authoritative server control map remains the final routing source, and the server intentionally tolerates transient client-controlled-id mismatch during handoff to avoid dropping local intent during rebind (`bins/sidereal-replication/src/replication/input.rs:447-470`).

This is a good local design choice for dynamic handoff and is aligned with Sidereal’s explicit server-authoritative control model.

## 7. Prediction / Rollback / Interpolation / Control-Swap Findings

The repo is carrying significant custom logic around dynamic predicted/interpolated switching:

- server-side role reassignment in `bins/sidereal-replication/src/replication/control.rs`;
- observer-anchor movement/follow state in `bins/sidereal-replication/src/replication/runtime_state.rs:141-180`;
- client-side adoption, marker sanitization, and predicted-target selection in `bins/sidereal-client/src/runtime/replication.rs`.

This complexity is currently justified, not gratuitous:

- upstream Lightyear still tracks prediction switching separately (`#1034`);
- upstream issue `#1380` shows interpolation/required-component bootstrap remains a live problem;
- Sidereal also correctly avoids Lightyear native server input as the authoritative path, consistent with upstream issue `#1200` and local DR-0031.

Inference:

- The custom control-handoff layer is still necessary today.
- It is also a maintenance hotspot and should remain tightly bounded; it should not spread into unrelated runtime systems.

## 8. Visibility / Replication / Redaction Findings

The current implementation matches the core visibility contract better than earlier repo documents imply:

- authorization is evaluated before delivery narrowing in `authorize_visibility(...)` and `finalize_visibility_evaluation(...)`;
- delivery scope is observer-anchor driven and does not widen authorization;
- map mode expands candidate generation and delivery scope inputs, but authorization still runs separately afterward;
- player-anchor visibility leakage is not present in the inspected code path.

Direct evidence:

- observer anchor follows the authoritative controlled entity when one exists, but visibility range is computed only for `SimulatedControlledEntity` roots in `bins/sidereal-replication/src/replication/runtime_state.rs:141-180` and `:193-235`;
- map-mode candidate expansion happens in `bins/sidereal-replication/src/replication/visibility.rs:1143-1218`;
- authorization and delivery are split in `bins/sidereal-replication/src/replication/visibility.rs:2784-2910`.

## 9. Tactical / Fog / Owner-Lane Findings

The tactical and owner-manifest lanes are architecturally sound:

- fog/contact data has snapshot + delta + resnapshot handling with explicit sequence checks;
- owner manifest is independent from local-bubble relevance and is fed from authoritative server state;
- client caches fail closed on sequence mismatch and request resnapshot for tactical data.

The current lane split is appropriate for the project and should be preserved.

## 10. Asset Delivery Findings

The macro-level architecture is correct:

- replication does not stream asset payload bytes;
- payloads come from gateway HTTP routes only;
- runtime invalidation uses manifest-channel version messages and client-side manifest refresh.

The problem is timing, not lane choice:

- bootstrap-required asset fetch begins as soon as `/world/enter` is accepted, before replication session bind success is known.

## 11. Findings Ordered by Severity

### High

#### 11.1 Silent replication auth rejection paths violate the explicit deny contract

- Severity: High
- Type: correctness, maintainability, documentation
- Why it matters:
  - Several auth failures are fail-closed, but not fail-explicit.
  - The client then waits for `ServerSessionReady`, hits the generic watchdog, and surfaces a misleading “protocol/build mismatch” error instead of a concrete auth rejection.
  - This diverges from the repo’s world-entry/session-denial contract.
- Direct evidence:
  - `bins/sidereal-replication/src/replication/auth.rs:243-291` logs and `continue`s on invalid player id, invalid token, and token/message player mismatch without sending `ServerSessionDeniedMessage`.
  - `bins/sidereal-client/src/runtime/auth_net.rs:986-1045` turns missing `ServerSessionReady` into a generic timeout dialog.
- Inference:
  - Ownership/account mismatches that fail before the later explicit-denial branches will currently look like transport/protocol failures from the client’s perspective.
- Recommendation:
  - Send explicit `ServerSessionDeniedMessage` for every terminal auth failure after the server can identify the requesting peer, not only for config failure and missing hydrated player cases.
  - Reserve the watchdog path for true transport silence.
- Priority: must fix

### Medium

#### 11.2 Delivery-culling view-mode updates are not protected against reordering

- Severity: Medium
- Type: correctness, performance, architecture
- Why it matters:
  - `ClientLocalViewModeMessage` controls server-side delivery narrowing.
  - It currently shares `ControlChannel`, which is `UnorderedReliable`.
  - The message itself has no sequence/timestamp, and the server simply overwrites the last value on arrival.
  - An older reliable packet can therefore regress `view_mode` or `delivery_range_m`.
- Direct evidence:
  - `crates/sidereal-net/src/lightyear_protocol/registration.rs:113-118` configures `ControlChannel` as `UnorderedReliable`.
  - `bins/sidereal-client/src/runtime/control.rs:124-210` sends view-mode heartbeats and changed ranges without a monotonic sequence.
  - `bins/sidereal-replication/src/replication/visibility.rs:870-906` blindly overwrites `ClientLocalViewSettings`.
- Recommendation:
  - Either move view-mode updates to an ordered/sequenced lane, or add a monotonic sequence field and ignore stale arrivals server-side.
- Priority: should fix

#### 11.3 `InputChannel` is overloaded with server combat FX traffic at the same highest priority as authoritative input

- Severity: Medium
- Type: architecture, performance, maintainability
- Why it matters:
  - The highest-priority latest-wins input lane should be reserved for actual client intent unless there is a strong reason otherwise.
  - Sidereal currently also sends `ServerWeaponFiredMessage` and `ServerEntityDestructionMessage` on that same channel.
  - This couples visual-effects traffic to input-lane behavior and makes channel semantics harder to reason about.
- Direct evidence:
  - `crates/sidereal-net/src/lightyear_protocol/registration.rs:120-124` gives `InputChannel` the highest priority and `SequencedUnreliable`.
  - `bins/sidereal-replication/src/replication/combat.rs:173-176` sends `ServerWeaponFiredMessage` on `InputChannel`.
  - `bins/sidereal-replication/src/replication/combat.rs:248-250` sends `ServerEntityDestructionMessage` on `InputChannel`.
- Recommendation:
  - Split combat event/VFX notifications onto their own event/effects channel with an explicitly chosen priority and delivery mode.
  - Keep `InputChannel` semantically “authoritative realtime intent lane”.
- Priority: should fix

#### 11.4 Client starts bootstrap-required asset downloads before replication bind success is known

- Severity: Medium
- Type: performance, maintainability, flow-ordering
- Why it matters:
  - On rejected or stalled replication binds, the client can still start fetching bootstrap-required world assets.
  - This is wasteful and complicates failure semantics.
  - It also blurs the intended sequence between runtime bind readiness and pre-world asset gating.
- Direct evidence:
  - `bins/sidereal-client/src/runtime/auth_net.rs:623-639` submits asset bootstrap immediately after gateway `EnterWorld` acceptance.
  - Replication bind success is only known later via `ServerSessionReadyMessage`.
- Recommendation:
  - Delay bootstrap-required asset fetch until after successful replication bind, or explicitly document and justify the current parallelization as intentional overlap.
  - If overlap is kept, cancel/abort asset bootstrap immediately on session denial.
- Priority: should fix

## 12. Modern Best-Practice Comparison

### Alignment

- Strong use of Lightyear for replication, prediction, interpolation, and transport.
- Correct local decision to keep authoritative server input on Sidereal’s authenticated realtime lane instead of Lightyear’s native server input path, given upstream issue `#1200`.
- Correct local recognition that dynamic predicted/interpolated control transfer is not a solved upstream problem yet (`#1034`).
- Correct defensive handling around interpolation/bootstrap edge cases, consistent with upstream issue `#1380`.

### Divergence

- View-mode/delivery messages currently do not use ordering semantics that match their effect on server delivery state.
- Combat FX traffic shares the input lane, which is a local protocol-design choice rather than an upstream requirement.

## 13. Prioritized Recommendations

1. Make all terminal replication auth failures send explicit `ServerSessionDeniedMessage`.
2. Give `ClientLocalViewModeMessage` ordering protection: either a dedicated ordered/sequenced lane or message sequencing.
3. Split combat FX notifications off `InputChannel`.
4. Move asset bootstrap to post-bind, or document/cancel the current overlap explicitly.
5. Keep the current player-anchor/visibility separation and tactical/owner-lane split exactly as the current code implements them.
6. Keep the custom dynamic control-handoff layer isolated and audited; it is still justified by upstream Lightyear limitations.

## 14. Appendix A: Full Message Inventory

### Control lane messages

- `ClientAuthMessage`
- `ServerSessionReadyMessage`
- `ServerSessionDeniedMessage`
- `ClientDisconnectNotifyMessage`
- `ClientControlRequestMessage`
- `ServerControlAckMessage`
- `ServerControlRejectMessage`
- `ClientLocalViewModeMessage`

### Input lane messages

- `ClientRealtimeInputMessage`
- `ServerWeaponFiredMessage`
- `ServerEntityDestructionMessage`

### Tactical snapshot lane messages

- `ServerTacticalFogSnapshotMessage`
- `ServerTacticalContactsSnapshotMessage`
- `ClientTacticalResnapshotRequestMessage`

### Tactical delta lane messages

- `ServerTacticalFogDeltaMessage`
- `ServerTacticalContactsDeltaMessage`

### Manifest lane messages

- `ServerOwnerAssetManifestSnapshotMessage`
- `ServerOwnerAssetManifestDeltaMessage`
- `ServerAssetCatalogVersionMessage`

## 15. Appendix B: Full Channel Inventory

- `ControlChannel`
  - Mode: `UnorderedReliable`
  - Priority: `8.0`
- `InputChannel`
  - Mode: `SequencedUnreliable`
  - Priority: `10.0`
- `TacticalSnapshotChannel`
  - Mode: `OrderedReliable`
  - Priority: `5.0`
- `TacticalDeltaChannel`
  - Mode: `SequencedUnreliable`
  - Priority: `5.0`
- `ManifestChannel`
  - Mode: `OrderedReliable`
  - Priority: `6.0`

Source: `crates/sidereal-net/src/lightyear_protocol/registration.rs:113-146`

## 16. Appendix C: Relevant HTTP / Transport Routes and Endpoints

### Gateway HTTP

- `GET /health`
- `POST /auth/register`
- `POST /auth/login`
- `POST /auth/refresh`
- `POST /auth/password-reset/request`
- `POST /auth/password-reset/confirm`
- `GET /auth/me`
- `GET /auth/characters`
- `POST /world/enter`
- `POST /admin/spawn-entity`
- `GET /startup-assets/manifest`
- `GET /startup-assets/{asset_guid}`
- `GET /assets/bootstrap-manifest`
- `GET /assets/{asset_guid}`

### Replication transport

- UDP bind: `REPLICATION_UDP_BIND` default `0.0.0.0:7001`
- WebTransport bind: `REPLICATION_WEBTRANSPORT_BIND` default `0.0.0.0:7003`
- Control UDP bind: `REPLICATION_CONTROL_UDP_BIND` default `127.0.0.1:9004`
- Health bind: `REPLICATION_HEALTH_BIND` default `127.0.0.1:15716`

### BRP / inspection

- Replication BRP is optional and intended loopback-only.

