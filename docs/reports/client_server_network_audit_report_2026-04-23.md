# Client/Server Network Audit Report - 2026-04-23

Status note 2026-04-23: Fresh audit run from current repository state. Existing audit reports were intentionally ignored as evidence. This pass used `docs/prompts/client_server_network_audit_prompt.md`, the source-of-truth networking/visibility/asset contracts, current Lightyear/Bevy references, and direct code inspection.

## Scope

Reviewed Lightyear dependency pinning, protocol messages/channels, authentication/session binding, control target changes, realtime input routing, server visibility membership, tactical/owner lanes, asset bootstrap/delivery, and client bootstrap/adoption behavior.

Current upstream context checked:

- Lightyear crate docs currently show `0.25.5` with `prediction`, `interpolation`, `replication`, `avian2d`, `udp`, `websocket`, and related features.
- Lightyear book still frames controlled prediction around server replication plus `PredictionTarget`, and non-controlled smoothing around interpolation.
- Project snapshot notes PR `cBournhonesque/lightyear#1421` was merged to upstream main via `af25682`, but this repository still pins the Dastari fork revision in `Cargo.toml`.

## Executive Summary

The network architecture is mostly aligned with Sidereal’s rules: session identity binds to authenticated player IDs, control requests reject spoofed player IDs, realtime input is server-routed by the authoritative player-to-controlled-entity map, visibility has a dedicated server-side membership pass, and asset payload bytes stay on HTTP rather than replication transport.

Important context: controlled-entity handoff was substantially refactored on March 22. The current code already has server-issued `control_generation` in control acks/rejects, per-player lease generations, explicit client `ControlBootstrapState` / `ControlBootstrapPhase`, pending-until-predicted client binding, narrower visibility rearm, and narrower transform repair. The audit finding is therefore not "this was not addressed." It is "the target architecture is partially implemented, but remaining containment paths and validation gaps should be closed before calling the handoff lifecycle done."

The biggest current failure area remains changing the selected/controlled entity, but future audits should judge it against the agreed target below rather than inventing a different architecture each time.

## Target Standard For Future Audits

Use this as the stable bar for controlled handoff and networking reviews:

1. Server authority remains one-way: client sends intent, shard validates and simulates, replication distributes, persistence records.
2. Session identity is bound once to authenticated `player_entity_id`; clients never successfully claim another player in control or input messages.
3. Control target changes are generation-scoped leases. A lease generation advances only when the authoritative target changes, and stale acks/inputs cannot activate the wrong target.
4. Client control activation is bootstrap-gated. A non-anchor controlled entity is not locally writable until the matching predicted root exists with required Avian motion components and role state.
5. The local simulation writer is single-owner. Confirmed/interpolated observer clones never receive local control writers for non-anchor entities.
6. Server role derivation has one authoritative place that maps authenticated player -> controlled entity into Lightyear-facing `ControlledBy`, `Replicate`, `PredictionTarget`, `InterpolationTarget`, and visibility refresh behavior.
7. Visibility rearm is bounded and explicit. If retained as a Lightyear workaround, it only affects the current generation and current visible authorized audience; it is measured and covered by tests.
8. Input carries enough control context to avoid stale intent crossing leases. If `ClientRealtimeInputMessage` does not carry `control_generation`, the code must prove stale controlled IDs cannot apply unsafe intent to a new target.
9. Selected-only UI state is separate from controlled state. Selection changes must not mutate prediction, input routing, or visibility authority unless a control request succeeds.
10. Repair systems are diagnostic or transitional. Duplicate visual suppression, marker sanitization, adoption deferral, and transform recovery should trend toward zero steady-state interventions in nominal two-client sessions.

This standard is intentionally conservative and matches the direction already documented in the March 22 plans. New recommendations should explain which target invariant is unmet, rather than proposing unrelated rewrites.

## Findings

### Critical - Controlled Entity Changes Are Partially Implemented But Not Yet Proven End-To-End

Evidence:

- Client requests include `player_entity_id`, `controlled_entity_id`, and `request_seq` (`crates/sidereal-net/src/lightyear_protocol/messages.rs:34`).
- Server responses include `control_generation` (`crates/sidereal-net/src/lightyear_protocol/messages.rs:42`).
- Server control receive binds the claimed player to `AuthenticatedClientBindings`, rejects mismatches, validates owned target, updates `ControlledEntityGuid`, advances lease generation only on target changes, and queues ack/reject messages (`bins/sidereal-replication/src/replication/control.rs:168`, `bins/sidereal-replication/src/replication/control.rs:324`, `bins/sidereal-replication/src/replication/control.rs:405`).
- Server role reconciliation derives player/entity `PredictionTarget` and `InterpolationTarget` from bindings and controlled maps (`bins/sidereal-replication/src/replication/control.rs:621`).
- Client bootstrap state keeps non-anchor control pending until a matching predicted root is available (`bins/sidereal-client/src/runtime/replication.rs:1580`, `bins/sidereal-client/src/runtime/replication.rs:1645`).
- Client adoption still defers controlled predicted adoption until Avian components arrive and warns on stalls (`bins/sidereal-client/src/runtime/bootstrap.rs:126`).

Impact:

The control request path is authenticated and generation-aware for control responses. The remaining transition is still distributed across control ack, Lightyear role mutation, visibility rearm, predicted clone spawn, local writer binding, and duplicate visual suppression. Rapid target switching can expose races among these pieces unless the generation/bootstrapping invariants are covered by tests and telemetry.

Recommendation:

Continue the current control-handoff state machine rather than replacing it. Extend `control_generation` into realtime input or prove stale-ID mismatch acceptance cannot cross leases. Require tests for self -> ship, ship -> self, ship A -> ship B, A -> B -> A, disconnect during handoff, visibility loss/regain during handoff, and stale ack/reject/input ordering.

### Critical - Visibility Rearm On Role Change Is A High-Risk Workaround

Evidence:

- Role changes call `ReplicationState::lose_visibility` and `gain_visibility` for every already-visible client (`bins/sidereal-replication/src/replication/control.rs:607`).
- This is invoked for both player anchors and controlled entities when topology changes (`bins/sidereal-replication/src/replication/control.rs:768`, `bins/sidereal-replication/src/replication/control.rs:828`).

Impact:

This likely forces Lightyear to resend role/topology state. March 22 work narrowed the rearm to actual replication-topology changes, which is the right direction. It can still cause despawn/respawn churn, duplicated presentation, lost interpolation history, and bandwidth spikes. Because visibility is also a security boundary, the ordering of lose/gain must not leak payloads to unauthorized clients or remove authorized clients longer than intended.

Recommendation:

Replace with the smallest Lightyear-supported role refresh once available. Until then, add integration tests that assert no unauthorized payload after rearm, no duplicate clones after repeated rearm, and bounded bandwidth/entity events during control changes.

### High - Repository Lightyear Pin Has Not Consumed The Merged Upstream Fix

Evidence:

- Actual dependency pin is `lightyear = { git = "https://github.com/Dastari/lightyear", rev = "87b7dfd95e415d2b7646effeb09a3af09272f86e", ... }` (`Cargo.toml:36`).
- The network prompt still references a different revision, `29867036`, so prompt metadata is stale.
- Project Lightyear snapshot says upstream PR `#1421` was merged into `cBournhonesque:main` via `af25682`, but no release/pin update is reflected in `Cargo.toml`.

Impact:

The codebase still carries local workarounds for upstream Lightyear confirmed-history/role behavior. A future upgrade may make some of these obsolete, but it may also change behavior around prediction/interpolation target updates.

Recommendation:

Do not remove local guards until Sidereal pins a version containing the upstream fix and passes targeted native reproduction tests. Any removal should be measured against the target standard above, not against a vague goal of "less custom code." Update the prompt’s pinned-revision text so future audits start from the actual dependency.

### High - Realtime Input Tolerates Controlled-ID Mismatch During Handoff

Evidence:

- Input receive authenticates the bound player and filters spoofed player IDs (`bins/sidereal-replication/src/replication/input.rs:305`).
- Validation enforces packet size, tick monotonicity, and per-second rate limit (`bins/sidereal-replication/src/replication/input.rs:178`).
- Drain applies actions to the authoritative target even when the client message’s controlled ID is stale/mismatched (`bins/sidereal-replication/src/replication/input.rs:507`).

Impact:

This avoids input loss during handoff, but it can apply intent meant for the previous target to the newly authoritative target. That is especially risky if entity capabilities diverge.

Recommendation:

Include `control_generation` in `ClientRealtimeInputMessage`. Accept mismatched controlled IDs only when the generation matches the server’s current generation or when the target is the player anchor self-control case.

### Medium - Message Directions Are Broader Than Their Semantics

Evidence:

- All protocol messages are registered bidirectional, including server-only snapshot/delta/control response messages and client-only request/input messages (`crates/sidereal-net/src/lightyear_protocol/registration.rs:72`).
- Channels are separated by reliability/priority: control reliable unordered, input sequenced unreliable, tactical snapshots ordered reliable, tactical deltas sequenced unreliable, manifest ordered reliable (`crates/sidereal-net/src/lightyear_protocol/registration.rs:112`).

Impact:

Channel modes are sensible, but bidirectional registration increases accidental misuse risk and makes protocol intent less obvious.

Recommendation:

If Lightyear supports narrower directions for these message types, register client-to-server and server-to-client directions explicitly. If not, document why bidirectional registration is required and enforce direction in receive systems/tests.

### Medium - Tactical And Owner Lanes Are Implemented, But Cadence/Scale Need Stress Tests

Evidence:

- Tactical snapshots/deltas use per-player sequence/base sequence and resnapshot requests (`bins/sidereal-replication/src/replication/tactical.rs:31`, `bins/sidereal-replication/src/replication/tactical.rs:670`).
- Tactical contacts are filtered by `ReplicationState::is_visible` (`bins/sidereal-replication/src/replication/tactical.rs:636`).
- Owner manifest snapshots/deltas use per-player sequences and periodic resnapshot (`bins/sidereal-replication/src/replication/owner_manifest.rs:20`, `bins/sidereal-replication/src/replication/owner_manifest.rs:224`).

Impact:

This aligns with the dual-lane contract, but the current audit did not find broad load tests for many entities/clients and rapid visibility churn.

Recommendation:

Add stress tests for tactical/owner sequence recovery, snapshot fallback after missed deltas, rapid visibility gain/loss, and many owned controllable entities.

### Medium - Asset Delivery Matches The Contract

Evidence:

- Client app states include `StartupLoading -> Auth -> CharacterSelect -> WorldLoading -> AssetLoading -> InWorld` (`bins/sidereal-client/src/runtime/app_state.rs:8`).
- Gateway startup manifest is public and bootstrap manifest/assets are authenticated (`bins/sidereal-gateway/src/api.rs:378`, `bins/sidereal-gateway/src/api.rs:426`, `bins/sidereal-gateway/src/api.rs:482`).
- Runtime asset manager tracks manifest/catalog readiness and fetch dependencies from HTTP/cache, not replication payload bytes (`bins/sidereal-client/src/runtime/assets.rs:50`, `bins/sidereal-client/src/runtime/assets.rs:969`).

Impact:

This area appears aligned with the asset delivery contract. The remaining network impact is bootstrap ordering with replication session readiness, not asset transport design.

Recommendation:

Keep asset bytes out of replication. Add end-to-end checks that world entry waits for session ready, asset bootstrap ready, and selected player presence before enabling in-world input.

## Strengths

- Control requests reject spoofed player IDs and not-owned targets.
- Input packets are rate-limited and size-limited.
- Visibility runs after physics writeback and before tactical/manifest streaming (`bins/sidereal-replication/src/plugins.rs:119`).
- Tactical contacts rely on server visibility state before delivery.
- Asset payload delivery stays HTTP-based and authenticated for runtime/bootstrap assets.

## Priority Actions

1. Extend the existing `control_generation` lease model into realtime input or prove the current mismatch tolerance is safe.
2. Build an end-to-end controlled-entity handoff test matrix around the target standard.
3. Replace or tightly test the narrowed visibility rearm behavior.
4. Upgrade/pin Lightyear only after validating upstream `#1421` against Sidereal handoff cases.
5. Narrow or document bidirectional protocol message registration.
6. Update the network audit prompt’s stale Lightyear revision reference.

## Validation

No cargo quality gates or live client/server runs were performed for this docs-only audit generation. The report is based on static inspection of current code, current source-of-truth docs, and current upstream reference checks.
