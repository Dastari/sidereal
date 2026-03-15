# Client/Server Network Audit Prompt

**Status:** Active Prompt  
**Date:** 2026-03-15  
**Scope Note:** Audit the actual current client/server network architecture in this repository, not an idealized design.

Perform a full end-to-end network architecture audit for this repository as a senior Bevy 0.18 / Avian2D / Lightyear / server-authoritative MMO networking engineer.

This is not a generic netcode review. Inspect this specific codebase, its current transport/protocol/runtime behavior, and its current documentation contracts. The deliverable is a formal written report saved under `docs/reports/`.

## Non-Negotiable Audit Stance

Start from a fresh, unbiased view of the codebase.

Do **not** trust prior plans, audits, implementation notes, or migration documents as truth. They may be used as leads or references only.

Only the following should be treated as authoritative project-intent contracts unless the code has clearly superseded them in the same area:

- `AGENTS.md`
- `docs/sidereal_design_document.md`
- `docs/decision_register.md`
- `docs/features/visibility_replication_contract.md`
- `docs/features/tactical_and_owner_lane_protocol_contract.md`
- `docs/features/asset_delivery_contract.md`
- `docs/decisions/dr-0002_explicit_world_entry_flow.md`
- `docs/decisions/dr-0013_action_acceptor_control_routing.md`
- `docs/decisions/dr-0017_dual_lane_replication_and_owner_asset_manifest.md`
- `docs/decisions/dr-0018_fog_of_war_and_intel_memory_model.md`
- `docs/features/lightyear_upstream_issue_snapshot.md`

All other plans, prompts, investigation docs, and implementation trackers are reference-only. Verify everything against current code before repeating it.

## Repository Context You Must Account For

- This is Sidereal, a server-authoritative multiplayer space MMO/ARPG rebuild.
- The workspace currently pins:
  - `bevy = 0.18.0`
  - a forked Lightyear dependency at `https://github.com/Dastari/lightyear`, rev `29867036`
- The current workspace enables Lightyear prediction, interpolation, frame interpolation, Avian2D integration, native UDP transport, and browser WebTransport support depending on target/runtime.
- The project allows dynamic control swap between owned entities.
- Free-roam is represented by controlling the player entity itself, not by null control.
- The player entity may move and act as the camera/control anchor, but it must **not** implicitly grant visibility just because it is the controlled entity. Only valid visibility/sensor-capable entities may expand visibility.
- The current project has a known Lightyear-related predicted/interpolated control-handoff edge and is temporarily carrying a forked dependency to support the required behavior. You must verify the exact local behavior and distinguish:
  - intentional local workaround,
  - upstream Lightyear limitation/bug,
  - local misuse or architectural drift.

## Read First

Before auditing code, read these documents carefully:

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

Then inspect the actual code paths under at least:

- `bins/sidereal-client/`
- `bins/sidereal-replication/`
- `bins/sidereal-gateway/`
- `crates/sidereal-net/`
- `crates/sidereal-game/`
- `crates/sidereal-asset-runtime/`

## Primary Audit Goals

Determine whether the current client/server network architecture is:

1. correct,
2. internally consistent,
3. aligned with the repository’s current contracts,
4. aligned with current Lightyear best practices and documentation,
5. making strong use of current Lightyear and Bevy 0.18 networking/runtime capabilities where appropriate,
6. suitable for a modern top-down space MMO/ARPG,
7. safe, scalable, and maintainable under the project’s control-swap and multi-lane visibility requirements.

## Mandatory Audit Scope

Audit all of the following. Do not skip any category.

### 1. Full network topology

Map the real end-to-end topology, including:

- gateway HTTP/auth/bootstrap routes,
- replication transport endpoints,
- native client transport path,
- WASM/browser transport path,
- WebTransport-first browser path and any fallback behavior,
- asset-delivery HTTP paths,
- internal control/admin paths that affect network authority,
- any debug/inspection network surfaces that matter to security or topology.

Explain what talks to what, over which protocol, with what trust boundary.

### 2. Complete channel and message inventory

Produce a complete matrix of all runtime channels and network-bearing paths, including:

- Lightyear channels,
- message types on each channel,
- reliability/ordering mode,
- priority,
- producer and consumer,
- intended purpose,
- whether current usage matches the documented contract.

At minimum verify the current usage of:

- `ControlChannel`
- `InputChannel`
- `TacticalSnapshotChannel`
- `TacticalDeltaChannel`
- `ManifestChannel`

Also inventory network paths that are **not** Lightyear channels, such as HTTP bootstrap/auth/asset flows.

### 3. World-entry, bootstrap, and initial-state delivery

Audit the complete world-entry sequence:

- auth,
- character selection,
- `/world/enter` or equivalent bootstrap request,
- replication bind/auth,
- `ServerSessionReady`,
- player-entity replication arrival,
- asset-loading gate,
- final transition to `InWorld`.

Pay special attention to:

- whether the selected player entity is guaranteed to replicate before usable world entry,
- whether the persisted last authoritative control target is preserved during bootstrap,
- whether the client speculatively reverts to self-control too early,
- whether the “last known controlled ship” is available to control before or as world entry completes,
- whether initial state delivery is ordered robustly enough for prediction/interpolation/control HUD correctness,
- whether spawned/interpolated entities avoid origin flashes or missing-history gaps during first relevance.

### 4. Player input, entity actions, and authority routing

Audit:

- authenticated input routing,
- session binding to `player_entity_id`,
- controlled-entity validation,
- action routing to `ControlledEntityGuid`,
- free-roam/self-control behavior,
- stale-input expiry,
- heartbeat/change-driven input behavior,
- rejection of mismatched claimed player/entity IDs,
- whether any path allows client-side authority leakage.

Be explicit about how Lightyear native input is used versus Sidereal’s authenticated realtime input lane.

### 5. Dynamic control swap and predicted/interpolated handoff

This is a mandatory deep-focus section.

Audit the complete control-handoff flow when switching between:

- player-anchor self-control,
- owned ship A,
- owned ship B,
- back to player-anchor self-control.

Determine whether the current architecture correctly handles:

- ownership validation,
- authoritative ack/reject flow,
- previous controlled-entity neutralization,
- player-anchor following behavior,
- predicted marker transfer,
- interpolated marker transfer,
- confirmed-history availability,
- interpolation-history bootstrap,
- frame interpolation continuity,
- rollback/prediction-history continuity,
- local-view/UI/control-state continuity.

Specifically check whether the current Lightyear forked behavior and local code are sufficient for the project’s dynamic predicted/interpolated switching requirements, or whether the architecture still has correctness, visual, or maintainability gaps.

### 6. Prediction, rollback, reconciliation, interpolation, and frame interpolation

Audit whether the project is making good use of current Lightyear functionality for:

- local prediction,
- rollback,
- reconciliation,
- correction policy,
- remote interpolation,
- frame interpolation,
- Avian2D replication integration,
- required-component availability on predicted/interpolated entities,
- confirmed-history and interpolation-history management.

Verify whether current custom systems are:

- necessary,
- temporary but justified,
- fighting Lightyear,
- duplicating upstream capabilities,
- or leaving modern Lightyear capabilities underused.

Call out where the codebase aligns well with Lightyear and where it diverges.

### 7. Avian2D physics state over the network

Audit whether networked entities receive sufficient authoritative Avian2D state and required components for stable behavior, especially during:

- spawn,
- hydration,
- visibility gain/loss,
- initial world entry,
- control swap,
- predicted/interpolated transitions.

Check in particular whether the effective runtime state includes what is needed for:

- `Position`
- `Rotation`
- `LinearVelocity`
- `AngularVelocity`
- mass/inertia parity where required,
- collision/physics-related required components,
- interpolation history bootstrap for newly relevant entities.

If the current design intentionally does **not** fully attach some physics state on some client clones, determine whether that is correct, risky, or now obsolete.

### 8. Visibility, authorization, delivery culling, and redaction

Read the visibility contract carefully and audit the implementation against it.

You must explicitly analyze the many layers of visibility, including:

- world truth,
- what the client is authorized to know,
- what the client is currently allowed to receive,
- payload redaction after authorization and delivery narrowing,
- owner/public/faction policy,
- scanner/visibility-range capability,
- tactical/fog/intel-memory products,
- discovered-landmark handling if relevant,
- current view-mode and delivery-range signaling,
- range culling for optimization.

You must verify the required ordering:

1. `Authorization`
2. `Delivery`
3. `Payload`

Be explicit about the distinction between:

- “what the client is allowed to see at all”
- “what the client currently can see right now”

Also verify that:

- spatial preselection is optimization-only and not treated as authorization,
- the player entity/free-roam anchor does not accidentally grant visibility unless it truly has valid visibility capability,
- only legitimate sensor/visibility sources extend visibility,
- tactical/fog/owner lanes do not bypass authorization,
- delivery culling is camera/view-mode driven only as a narrowing step.

### 9. Tactical map, fog-of-war, owner-manifest, and other auxiliary lanes

Audit the separate non-world-entity lanes and caches, including:

- tactical fog snapshot/delta flow,
- tactical contact snapshot/delta flow,
- owner asset manifest snapshot/delta flow,
- any local tactical smoothing or cache logic,
- any map-mode delivery-range expansion logic,
- resnapshot behavior and sequence safety.

Determine whether these lanes are:

- correctly separated from local-bubble replication,
- correctly authorized,
- robust under packet loss/order mismatch,
- and appropriate for a large-scale top-down MMO/ARPG.

### 10. Asset delivery as part of the network architecture

Audit asset delivery as a first-class part of the network stack.

Cover:

- startup asset manifest flow,
- startup asset payload flow,
- bootstrap-required asset flow,
- runtime lazy asset fetch flow,
- dependency expansion,
- cache validation/checksum/version behavior,
- asset-catalog version notification,
- separation between replication transport and HTTP asset payload delivery,
- native/WASM parity expectations.

Verify that replication never becomes the asset-payload transport lane and that world entry ordering around assets is correct.

### 11. Modern best-practice comparison

Compare the current implementation to current primary-source Lightyear and Bevy 0.18 references.

Use current primary sources at audit time, not blogspam or secondary summaries. Prefer:

- official Bevy 0.18 docs/release notes,
- official Lightyear docs/book/API docs,
- Lightyear GitHub repo/issues/PRs when directly relevant.

Record the exact upstream version/docs state you used.

You must distinguish:

- upstream-supported best practice,
- upstream-known limitation/bug,
- local intentional deviation,
- local accidental divergence.

### 12. Scale, performance, and maintainability

Audit whether the current architecture is likely to hold up for:

- many entities,
- multiple owned ships per player,
- frequent control swaps,
- multiple active visibility sources,
- tactical map use,
- fog/intel memory,
- asset streaming,
- native and WASM transport parity.

Call out:

- bandwidth risks,
- ordering risks,
- replay/resnapshot risks,
- authority-boundary risks,
- excessive complexity,
- duplicate systems fighting each other,
- places where the architecture is already strong and should be preserved.

## Required Special Checks

You must explicitly answer all of these:

1. Is the current initial world-state delivery sequence correct for a player who has a persisted non-self controlled ship?
2. Does the current implementation preserve the last authoritative controlled ship cleanly through world entry, prediction adoption, and HUD/control readiness?
3. Does the current implementation correctly separate player-anchor control/camera semantics from visibility/sensor semantics?
4. Are all network channels and side paths clearly justified, correctly configured, and currently used as intended?
5. Is the current tactical/fog/owner multi-lane model architecturally sound?
6. Is the current Lightyear fork usage justified, minimal, and clearly separated from project-local mistakes?
7. Is the project making strong enough use of current Lightyear interpolation/prediction/frame-interpolation capabilities, or is it carrying avoidable custom networking complexity?
8. Are predicted/interpolated Avian2D entities receiving the right physics state and history bootstrap for stable runtime behavior?
9. Does the current visibility implementation actually match the documented multi-tier authorization/delivery/redaction model?
10. If you were hardening this into a modern production-quality top-down space MMO/ARPG networking architecture, what must change first?

## Output Requirements

Produce a formal audit report with findings ordered by severity:

- `Critical`
- `High`
- `Medium`
- `Low`

For each finding include:

- title,
- severity,
- why it matters,
- exact file/path references,
- whether it is a correctness, security, architecture, performance, maintainability, or documentation issue,
- concrete recommendation,
- whether the recommendation is:
  - must fix,
  - should fix,
  - optional improvement.

Also:

- separate direct evidence from inference,
- call out where code and docs diverge,
- call out where the current design is correct and should be kept,
- do not give generic advice; tie every point to this repository,
- list recommendations in descending criticality order.

## Suggested Report Structure

1. Executive Summary
2. Audit Method and Source Hierarchy
3. Current Network Topology Map
4. Channel and Protocol Matrix
5. World Entry and Initial-State Findings
6. Input, Control, and Authority Findings
7. Prediction / Rollback / Interpolation / Control-Swap Findings
8. Visibility / Replication / Redaction Findings
9. Tactical / Fog / Owner-Lane Findings
10. Asset Delivery Findings
11. Lightyear and Bevy 0.18 Best-Practice Comparison
12. Prioritized Recommendations
13. Appendix A: Full Message Inventory
14. Appendix B: Full Channel Inventory
15. Appendix C: Relevant HTTP/Transport Routes and Endpoints

## Deliverable

Write the final report only to:

- `docs/reports/client_server_network_audit_report_YYYY-MM-DD.md`

Do not write the completed report anywhere else.
