# Client/Server Network Audit Report

Status: Completed  
Date: 2026-03-15  
Scope basis: `docs/prompts/client_server_network_audit_prompt.md`

## 1. Executive Summary

The current network architecture is largely coherent and already has several strong server-authoritative properties:

- gateway-owned auth and character ownership checks gate `/world/enter`,
- replication binds transport peers to canonical `player_entity_id`,
- authoritative gameplay input uses Sidereal's authenticated realtime input lane instead of trusting Lightyear native input on the server,
- dynamic control handoff is substantially more deliberate than a stock Lightyear sample,
- owner manifest and asset payload delivery are correctly split away from world-entity replication.

However, the implementation is not yet fully correct against the repository's current contracts. This audit found two high-severity contract/security gaps and two additional medium-severity correctness/operability gaps:

1. the replication control UDP plane is unauthenticated and is only safe when kept loopback/private,
2. the tactical lane does not implement stale intel memory at all and currently mirrors only live replicated visibility,
3. world-entry readiness is weaker than the control-handoff architecture actually requires,
4. several replication auth rejection paths fail by silent timeout instead of explicit deny.

Conclusion: the architecture is directionally sound for Sidereal's target model, but it should not be considered fully contract-aligned or production-hardened until the control plane and tactical memory model are corrected.

## 2. Audit Method and Source Hierarchy

This report treated the following as authoritative intent unless current code clearly superseded them:

- `AGENTS.md`
- `docs/sidereal_design_document.md`
- `docs/features/visibility_replication_contract.md`
- `docs/features/tactical_and_owner_lane_protocol_contract.md`
- `docs/features/asset_delivery_contract.md`
- `docs/decisions/dr-0002_explicit_world_entry_flow.md`
- `docs/decisions/dr-0013_action_acceptor_control_routing.md`
- `docs/decisions/dr-0017_dual_lane_replication_and_owner_asset_manifest.md`
- `docs/decisions/dr-0018_fog_of_war_and_intel_memory_model.md`
- `docs/features/lightyear_upstream_issue_snapshot.md`

Primary code paths audited:

- `bins/sidereal-client/`
- `bins/sidereal-replication/`
- `bins/sidereal-gateway/`
- `crates/sidereal-net/`
- `crates/sidereal-game/`
- `crates/sidereal-asset-runtime/`

Primary upstream references consulted:

- Lightyear book, visual interpolation: <https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/visual_interpolation.html>
- Lightyear issue `#1034` PredictionSwitching: <https://github.com/cBournhonesque/lightyear/issues/1034>
- Lightyear issue `#1380` required components on interpolated entities: <https://github.com/cBournhonesque/lightyear/issues/1380>
- Lightyear PR `#1421` confirmed-history bootstrap on `Interpolated` add: <https://github.com/cBournhonesque/lightyear/pull/1421>
- Bevy 0.18 release notes: <https://bevy.org/news/bevy-0-18/>

## 3. Current Architecture Snapshot

### 3.1 Transport and bootstrap topology

- Gateway auth + world entry:
  - `/world/enter` validates account ownership, then dispatches bootstrap and returns advertised replication endpoints plus optional WebTransport certificate digest (`bins/sidereal-gateway/src/auth/service.rs:163`, `bins/sidereal-gateway/src/api.rs:244`).
- Client transport:
  - native uses UDP (`bins/sidereal-client/src/runtime/transport.rs:162`),
  - WASM uses WebTransport and explicitly requires gateway-provided WebTransport address + cert digest (`bins/sidereal-client/src/runtime/auth_net.rs:36`, `bins/sidereal-client/src/runtime/transport.rs:110`).
- Replication sideband control plane:
  - gateway can hand off bootstrap/admin commands over UDP (`bins/sidereal-gateway/src/auth/bootstrap_dispatch.rs:32`),
  - replication listens on `REPLICATION_CONTROL_UDP_BIND` and forwards accepted messages into the Bevy world (`bins/sidereal-replication/src/bootstrap_runtime.rs:41`).

### 3.2 Client lifecycle state machine

The implemented state machine matches the current asset contract:

`StartupLoading -> Auth -> CharacterSelect -> WorldLoading -> AssetLoading -> InWorld`

References:

- state definitions: `bins/sidereal-client/src/runtime/app_state.rs:8`
- asset contract lifecycle: `docs/features/asset_delivery_contract.md:18`, `docs/features/asset_delivery_contract.md:198`

### 3.3 Protocol/channel inventory

Registered channels and modes:

| Channel | Mode | Priority | Runtime use |
| --- | --- | ---: | --- |
| `ControlChannel` | `UnorderedReliable` | 8 | auth/session-ready, control ack/reject, local view mode |
| `InputChannel` | `SequencedUnreliable` | 10 | realtime client input, combat event fanout |
| `TacticalSnapshotChannel` | `OrderedReliable` | 5 | fog/contact snapshots |
| `TacticalDeltaChannel` | `SequencedUnreliable` | 5 | fog/contact deltas |
| `ManifestChannel` | `OrderedReliable` | 6 | owner manifest + asset catalog version |

Reference: `crates/sidereal-net/src/lightyear_protocol/registration.rs:73`.

Notable protocol observation:

- all messages/channels are registered as `Bidirectional`, even where runtime use is effectively one-way (`crates/sidereal-net/src/lightyear_protocol/registration.rs:73`). This is not currently breaking behavior, but it weakens protocol readability and makes accidental future misuse easier.

## 4. Areas That Are Working Well

### 4.1 Authenticated authoritative input routing

This area is strong and aligned with `AGENTS.md` and `DR-0013`:

- replication stores canonical bindings by client entity and remote id (`bins/sidereal-replication/src/replication/auth.rs:29`),
- realtime input from unbound clients is dropped (`bins/sidereal-replication/src/replication/input.rs:252`),
- spoofed claimed `player_entity_id` values are rejected (`bins/sidereal-replication/src/replication/input.rs:278`),
- controlled target ids are canonicalized before application (`bins/sidereal-replication/src/replication/input.rs:357`).

The server still loads Lightyear's native input protocol plugin (`bins/sidereal-replication/src/main.rs:123`), but authoritative gameplay input is Sidereal's authenticated `ClientRealtimeInputMessage` lane. That split is reasonable and explicit rather than accidental.

### 4.2 Dynamic control routing is much better than a stock demo

The server-side handoff path validates ownership, enforces per-player request sequencing, and neutralizes the old controlled entity on rebind:

- stale request rejection: `bins/sidereal-replication/src/replication/control.rs:178`
- ownership validation for requested targets: `bins/sidereal-replication/src/replication/control.rs:286`
- previous controlled entity neutralization: `bins/sidereal-replication/src/replication/control.rs:360`
- player-anchor and controlled-ship replication role reconciliation: `bins/sidereal-replication/src/replication/control.rs:484`

This is a real Sidereal-specific architecture, not generic sample code.

### 4.3 Asset and owner-manifest lane separation is correct

The current implementation preserves the intended separation:

- asset payload bytes stay on gateway HTTP routes, not replication transport (`docs/features/asset_delivery_contract.md:39`, `bins/sidereal-gateway/src/api.rs:482`),
- owner manifest is a dedicated owner-only read model and does not depend on local-bubble world presence (`docs/features/visibility_replication_contract.md:70`, `bins/sidereal-replication/src/replication/owner_manifest.rs:88`).

## 5. Findings

### Finding 1: Replication control UDP plane is unauthenticated

Severity: High  
Category: Security / operational hardening

The replication control sideband accepts raw UDP payloads and only validates message structure plus limited bootstrap-store checks. It does not authenticate the sender or cryptographically protect the payload.

Evidence:

- gateway UDP dispatcher sends to `REPLICATION_CONTROL_UDP_ADDR` with no message authentication layer (`bins/sidereal-gateway/src/auth/bootstrap_dispatch.rs:32`),
- replication binds a plain UDP listener and trusts any datagram source (`bins/sidereal-replication/src/bootstrap_runtime.rs:41`, `bins/sidereal-replication/src/bootstrap_runtime.rs:76`),
- `BootstrapProcessor::handle_payload()` deserializes and validates wire shape, but admin-spawn messages do not receive replication-side role verification (`bins/sidereal-replication/src/bootstrap.rs:58`, `bins/sidereal-replication/src/bootstrap.rs:81`),
- gateway admin spawn requires admin claims, but that protection is only on the HTTP side; an exposed control port bypasses it (`bins/sidereal-gateway/src/auth/service.rs:193`).

Impact:

- if `REPLICATION_CONTROL_UDP_BIND` is misbound to a non-loopback/public interface, an attacker on the reachable network can inject bootstrap or admin-spawn commands,
- the default loopback bind reduces risk but does not enforce safety by design.

Assessment:

This is the single clearest hard security gap in the current network topology.

Recommendation:

- replace the UDP sideband with an authenticated local RPC path, or
- add authenticated message envelopes with replay protection and strict allowlisted sender binding, and
- hard-fail startup when control bind is non-loopback unless an explicit secure deployment mode is enabled.

### Finding 2: Tactical lane does not implement stale intel memory

Severity: High  
Category: Contract correctness / gameplay architecture

The current tactical contacts stream is derived only from currently visible replicated entities. It does not implement the stale-memory half of the documented fog/intel model.

Evidence:

- the tactical contact schema explicitly requires `is_live_now=false` for stale memory projections (`docs/features/tactical_and_owner_lane_protocol_contract.md:107`),
- `DR-0018` requires persisted `PlayerIntelMemory` and tactical output containing both live and stale contacts (`docs/decisions/dr-0018_fog_of_war_and_intel_memory_model.md:31`, `docs/decisions/dr-0018_fog_of_war_and_intel_memory_model.md:51`),
- current code iterates only replicated entities currently visible to the client and skips the rest (`bins/sidereal-replication/src/replication/tactical.rs:622`, `bins/sidereal-replication/src/replication/tactical.rs:636`),
- every emitted contact is marked `is_live_now: true` (`bins/sidereal-replication/src/replication/tactical.rs:650`).

Inference:

- I did not find a Rust implementation of `PlayerIntelMemory` in the inspected runtime/component code; the current tactical path appears to have no stale-memory backing store at all.

Impact:

- tactical contacts are presently a thinner projection of live replication visibility, not the intended independent intel-memory read model,
- the client cannot distinguish live scanner truth from stale last-known intel,
- the current implementation violates both the tactical protocol contract and `DR-0018`.

Recommendation:

- implement persisted player-scoped intel memory on the player entity/components,
- update tactical generation to union current live contacts with authorized stale memory,
- preserve `Authorization -> Delivery -> Payload` ordering from the visibility contract while building that lane (`docs/features/visibility_replication_contract.md:21`).

### Finding 3: World-entry gating is weaker than the control-handoff architecture requires

Severity: High  
Category: Correctness / user-visible runtime stability

The client does wait for session-ready and local player runtime presence before leaving `WorldLoading`, which matches the minimum in `DR-0002`. But it does not wait for controlled-entity predicted readiness when the authoritative controlled target is a ship rather than the player anchor.

Evidence:

- `DR-0002` requires the client to stay gated until session-ready and selected player runtime presence exist (`docs/decisions/dr-0002_explicit_world_entry_flow.md:28`),
- current `WorldLoading -> AssetLoading` transition checks only session-ready plus local player runtime presence (`bins/sidereal-client/src/runtime/replication.rs:461`),
- current `AssetLoading -> InWorld` transition checks only asset bootstrap completion (`bins/sidereal-client/src/runtime/replication.rs:492`, `docs/features/asset_delivery_contract.md:214`),
- controlled-entity adoption is explicitly deferred until required replicated motion components exist (`bins/sidereal-client/src/runtime/replication.rs:671`),
- the client refuses to bind local control to confirmed/interpolated ship fallback when a predicted clone is missing (`bins/sidereal-client/src/runtime/replication.rs:1121`, `bins/sidereal-client/src/runtime/motion.rs:184`),
- the client already has a dedicated watchdog warning for "Controlled Entity Adoption Delayed", which confirms this is a known post-entry failure mode (`bins/sidereal-client/src/runtime/bootstrap.rs:125`).

Impact:

- the client can enter `InWorld` while the authoritative controlled ship is still not usable as the local predicted motion writer,
- this creates a visible "entered world but control still stalled" gap,
- the gap is especially relevant because Sidereal supports dynamic control swap and persists the last authoritative control target.

Assessment:

This is not a full architecture failure, but it is still a correctness gap in the end-to-end world-entry experience.

Recommendation:

- keep the existing asset gate,
- add a second readiness condition for non-player-anchor controlled targets: predicted control clone present and motion-writer components ready,
- if design wants fast scene entry first, split "scene entered" from "control ready" explicitly rather than treating the current degraded state as fully in-world.

### Finding 4: Several replication auth failure paths still fail by silence instead of explicit deny

Severity: Medium  
Category: Operability / contract adherence

Replication explicitly sends `ServerSessionDeniedMessage` for missing/invalid JWT secret and for missing hydrated player entity, but several other invalid auth cases only log and continue.

Evidence:

- explicit deny on missing JWT config: `bins/sidereal-replication/src/replication/auth.rs:252`
- invalid token only logs and drops: `bins/sidereal-replication/src/replication/auth.rs:268`
- invalid token player id encoding only logs and drops: `bins/sidereal-replication/src/replication/auth.rs:278`
- token/message player mismatch only logs and drops: `bins/sidereal-replication/src/replication/auth.rs:286`
- account ownership mismatch only logs and drops: `bins/sidereal-replication/src/replication/auth.rs:330`
- the client then falls back to watchdog timeout/disconnect UX (`bins/sidereal-client/src/runtime/auth_net.rs:987`).

Why this matters:

- `DR-0002` says ownership mismatches must fail closed and that misconfiguration should not silently leave the client hanging (`docs/decisions/dr-0002_explicit_world_entry_flow.md:35`, `docs/decisions/dr-0002_explicit_world_entry_flow.md:49`),
- silent drop is technically fail-closed, but operationally poor: the user gets a timeout dialog instead of a reasoned session deny.

Recommendation:

- send `ServerSessionDeniedMessage` for all authenticated rejection causes that are deterministic and safe to disclose,
- reserve silent drop only for obviously malformed or abusive traffic if there is a deliberate anti-enumeration reason.

### Finding 5: Asset bootstrap begins before replication bind succeeds

Severity: Low  
Category: Efficiency / sequencing

After `/world/enter` is accepted, the client immediately starts authenticated asset bootstrap before replication session bind succeeds.

Evidence:

- world-entry acceptance stores transport config and immediately calls `submit_asset_bootstrap_request()` (`bins/sidereal-client/src/runtime/auth_net.rs:613`, `bins/sidereal-client/src/runtime/auth_net.rs:632`),
- auth bind messages are then retried during `WorldLoading`/`AssetLoading`/`InWorld` (`bins/sidereal-client/src/runtime/app_state.rs:86`, `bins/sidereal-client/src/runtime/auth_net.rs:845`).

Impact:

- a later session deny or timeout can still consume gateway asset bandwidth,
- this is mostly wasteful rather than incorrect because the asset contract still requires assets before `InWorld`.

Recommendation:

- either delay bootstrap manifest fetch until after `ServerSessionReadyMessage`,
- or keep the eager fetch but explicitly accept the bandwidth tradeoff in docs.

## 6. Lightyear / Bevy / Avian Assessment

### 6.1 Overall assessment

The codebase is using current Lightyear/Bevy capabilities reasonably well for its requirements. The project is not obviously "fighting" Lightyear across the board. Most of the custom client systems around dynamic handoff are narrow compensations for real edge cases rather than gratuitous reinvention.

### 6.2 Where the current design aligns well

- client uses `LightyearAvianPlugin` with `PositionButInterpolateTransform` and `FrameInterpolationPlugin<Transform>` in a modern, plausible configuration (`bins/sidereal-client/src/runtime/app_setup.rs:129`),
- native Lightyear input is present client-side for local prediction, while authoritative server input remains on the authenticated Sidereal lane (`bins/sidereal-client/src/runtime/app_setup.rs:136`, `bins/sidereal-replication/src/plugins.rs:64`),
- client-side fallback systems for missing interpolation history and stalled transforms are scoped narrowly and are consistent with the known upstream problem shape:
  - origin-flash prevention before interpolation history exists (`bins/sidereal-client/src/runtime/transforms.rs:150`),
  - explicit frame-interpolation marker synchronization (`bins/sidereal-client/src/runtime/transforms.rs:207`),
  - stalled interpolated transform recovery (`bins/sidereal-client/src/runtime/transforms.rs:248`),
  - predicted/interpolated marker conflict cleanup during dynamic handoff (`bins/sidereal-client/src/runtime/replication.rs:1092`).

### 6.3 Where the code still depends on local workaround territory

- controlled predicted adoption is deferred until required Avian state exists (`bins/sidereal-client/src/runtime/replication.rs:671`),
- the client refuses to treat a confirmed/interpolated ship as the local control lane (`bins/sidereal-client/src/runtime/replication.rs:1193`, `bins/sidereal-client/src/runtime/motion.rs:184`).

Assessment:

- these workarounds are justified by the current dynamic-handoff requirements and match the known upstream issue space better than they indicate local misuse,
- but they are still maintenance burden and should be revisited whenever the Lightyear fork is rebased or upstream handoff/history support materially improves.

## 7. Contract Alignment Summary

Aligned:

- authenticated session binding and input spoof rejection,
- owner-manifest lane separation,
- HTTP asset payload delivery,
- generic visibility authorization and delivery ordering in the main replication path,
- native/WASM transport split at the platform boundary.

Not aligned:

- stale tactical intel memory model,
- fully hardened bootstrap/control sideband security,
- explicit deny semantics for all deterministic auth failures,
- world-entry readiness strong enough for persisted controlled-ship handoff.

## 8. Priority Recommendations

1. Secure or replace the replication control UDP sideband before treating the current topology as deployable beyond trusted local/private environments.
2. Implement `PlayerIntelMemory` and rebuild the tactical contacts lane as an actual live-plus-stale read model.
3. Strengthen client world-entry readiness so `InWorld` means both scene-ready and control-ready for non-anchor controlled targets.
4. Convert silent auth drops into explicit `ServerSessionDeniedMessage` responses where safe.
5. Optionally narrow protocol registrations from fully bidirectional to their actual intended directions to reduce future drift.

## 9. Verification Limits

- This audit was based on direct code/document inspection and upstream documentation review.
- I did not execute runtime scenarios, integration tests, or packet capture during this report generation pass.
- Absence claims in this report, especially around `PlayerIntelMemory`, are informed inferences from inspected code paths and repository search, not from a formal whole-repo semantic proof.
