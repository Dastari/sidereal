# Multiplayer Bootstrap Regression Investigation

Status: Active investigation handoff  
Date: 2026-03-15  
Scope: Native multiplayer join/rejoin correctness, prediction/control attach, observer interpolation/bootstrap, and suspicious local RTT reporting  
Audience: Fresh agent or engineer picking up the issue without prior thread context

Primary related docs:
- `docs/reports/lightyear_multiplayer_prediction_interpolation_audit_report_2026-03-15.md`
- `docs/plans/multiplayer_prediction_interpolation_reliability_plan_2026-03-15.md`
- `docs/features/lightyear_upstream_issue_snapshot.md`
- `docs/features/prediction_runtime_tuning_and_validation.md`
- `docs/features/visibility_replication_contract.md`
- `docs/plans/lightyear_runtime_alignment_implementation_plan.md`
- `docs/reports/native_runtime_system_ownership_audit_2026-03-09.md`

Historical code reference points:
- March 13 baseline commit: `2814bea`
- Current regression window committed head used during audit: `565a400`
- Current worktree contains additional uncommitted client/runtime changes beyond `565a400`

Update note (2026-03-15):
- This document is intentionally more operational than the audit report. It captures what the user observed, what was tried, which fixes were already attempted, what evidence is contradictory, and where the next agent should start.

Update note (2026-03-15, follow-up investigation):
- The local on-disk evidence is now stronger for the stationary confirmed/bootstrap failure and weaker for the observer/interpolated claim than this handoff originally implied.
- `data/debug/brp_dumps/client2_20260315_111824.json` is a zero-byte file, so the 2026-03-15 11:18:24 BRP set is not a complete synchronized three-process capture.
- In `data/debug/brp_dumps/client1_20260315_111824.json`, there are `predicted=2`, `interpolated=0`, and `replicated=27` entities. That specific snapshot does not contain any interpolated observer entities at all.
- That same client1 snapshot does contain at least five confirmed-only replicated asteroids stuck at `Position = [0,0]` while the replication dump shows those same GUIDs at non-zero authoritative positions. This strengthens the stationary confirmed/bootstrap diagnosis and indicates the current server-side visibility-gain resend is not yet sufficient.
- Sidereal is currently relying on Lightyear's default required `InputTimelineConfig` / `SyncConfig` on the client. There is no local Sidereal override for input delay or sync margins, which means the upstream localhost `no_input_delay` issue (`#1402`) is directly relevant to current native loopback behavior rather than just background reading.
- No repo-local Lightyear `LinkConditioner` or other artificial latency configuration was found in Sidereal's client/server transport setup. The suspicious `~303ms` RTT is therefore not explained by an intentional local latency simulation configured in this codebase.

Update note (2026-03-15, current symptom re-triage):
- The old asteroid-origin BRP evidence should now be treated as historical context only. The user has since reported that asteroids are fixed in the current runtime and the live regression has shifted.
- The strongest current live pattern is now:
  - fresh DB login gives correct local predicted control for both clients,
  - `CLIENT2` still cannot see `CLIENT1`'s ship in the fresh session,
  - `CLIENT1` sees `CLIENT2` only on a jittery orange/confirmed lane,
  - after both clients disconnect and reconnect, both clients collapse to seeing only `CLIENT1`'s ship at `0,0`.
- The strongest code-level suspect for the relog failure is now the reconnect/control rebinding path rather than stationary asteroid bootstrap:
  - `receive_client_auth_messages` re-registers the authenticated client and queues `PendingControlledByBindings` for the currently controlled runtime entity,
  - `apply_pending_controlled_by_bindings` reapplies `ControlledBy` and prediction/interpolation targets to that controlled runtime entity,
  - but the persisted player anchor is not symmetrically rebound through the same path when the player is controlling a ship,
  - while `sync_player_anchor_replication_mode` still depends on `ControlledBy` on the player anchor to keep owner-only player replication/prediction/interpolation mode aligned with the active client.
- Concretely: reconnect can leave the player anchor on stale or incomplete replication targeting even when the controlled ship itself is rebound. That is a plausible explanation for why fresh-session behavior is asymmetric and why relog collapses into a bad origin/visibility state.
- This is not yet a complete root cause for the fresh-session one-way visibility, but it is the strongest concrete defect found in the current code that directly matches the relog symptom.

## 1. Executive Summary

There are still at least two overlapping multiplayer problems:

1. Local control bootstrap was intermittently wrong on join.
2. Remote observer/bootstrap behavior is still wrong, especially on relog:
   - remote ships can fail to appear,
   - remote ships can appear on the confirmed/orange lane and jitter,
   - remote ships can appear at `0,0` until movement or after relog.

The local control/bootstrap problem now has a targeted client-side fix in the worktree and passes focused tests. The remote bootstrap problem is not yet closed and is not safely attributable to only the client or only the server.

The current evidence suggests:

- fresh-spawn local control is now better,
- remote observer state is still unstable,
- relog/persisted-entity bootstrap is worse than fresh DB bootstrap,
- reconnect/control rebinding around the player anchor is likely broken,
- the server/dashboard RTT shown for both local clients is unexpectedly high (`~303ms`) and may be contributing to observer jitter.

## 2. Current Symptom Matrix

### 2.1 User-observed symptom timeline

Earlier in the investigation, the user reported:

1. On login, the client sometimes did not think its own ship was predicted/controlled until the ship was reselected in the owned-ships menu.
2. Inputs still moved the ship, and the camera still followed it, even when HUD/debug did not agree.
3. Remote ships sometimes appeared at `0,0` and snapped to the correct place only after movement.
4. Sometimes remote ships instead appeared immediately but on the orange/confirmed lane and jittered heavily.
5. Asteroids and other discovered entities also sometimes appeared at `0,0`.

Later, after the local control fix and a database reset, the user observed:

1. `CLIENT1`: fresh player creation/login worked; own predicted ship controlled correctly; could move away from `0,0`; asteroids appeared in the correct place.
2. `CLIENT2`: fresh player creation/login worked; own predicted ship controlled correctly; could fly to `CLIENT1`, but could not see `CLIENT1`'s ship.
3. `CLIENT1` could see `CLIENT2`'s incoming ship, but it was jittery and had an orange AABB.
4. After quitting both clients and logging back in:
   - `CLIENT1` only saw `CLIENT1`'s ship at `0,0`
   - `CLIENT2` only saw `CLIENT1`'s ship at `0,0`
5. Dashboard/TUI showed both local clients at roughly `303ms` ping even though both clients and the server were on the same machine.

### 2.2 Current interpretation of symptoms

The issues are not limited to one simple "interpolation broken" failure:

1. Local predicted control attach was broken at least once in a client-only way.
2. Remote observer lane selection and bootstrap remain wrong.
3. Relog/bootstrap for persisted entities appears worse than fresh-DB fresh-spawn behavior.
4. Suspicious RTT values may be part of the runtime instability rather than just cosmetic dashboard noise.

## 3. User Actions Already Tried

The user explicitly tried:

1. Running both native clients on the same machine.
2. Using different BRP ports for each client.
3. Relying on ephemeral client transport ports.
4. Resetting the database and recreating both players from scratch.
5. Reproducing both fresh-login and relog behavior after restart.

The "same machine" setup is not expected to cause ships to disappear or spawn at `0,0` by itself. It can change timing and CPU scheduling pressure, but it is not a sufficient explanation for the observed remote/bootstrap failures.

## 4. Investigation Summary So Far

### 4.1 Documents and code already reviewed

Already reviewed during this investigation:

1. `docs/reports/lightyear_multiplayer_prediction_interpolation_audit_report_2026-03-15.md`
2. `docs/plans/multiplayer_prediction_interpolation_reliability_plan_2026-03-15.md`
3. `docs/features/lightyear_upstream_issue_snapshot.md`
4. `docs/plans/lightyear_runtime_alignment_implementation_plan.md`
5. `docs/plans/bevy_2d_rendering_optimization_completion_plan_2026-03-12.md`
6. `docs/features/prediction_runtime_tuning_and_validation.md`
7. `bins/sidereal-client/src/runtime/replication.rs`
8. `bins/sidereal-client/src/runtime/transforms.rs`
9. `bins/sidereal-client/src/runtime/visuals.rs`
10. `bins/sidereal-client/src/runtime/motion.rs`
11. `bins/sidereal-client/src/runtime/debug_overlay.rs`
12. `bins/sidereal-replication/src/replication/visibility.rs`
13. `bins/sidereal-replication/src/replication/simulation_entities.rs`
14. `bins/sidereal-replication/src/replication/control.rs`
15. `bins/sidereal-replication/src/replication/health.rs`
16. `crates/sidereal-net/src/lightyear_protocol/registration.rs`

### 4.2 Historical comparison already performed

The earlier audit compared:

1. `2814bea` (March 13 baseline),
2. `565a400` (committed head in the regression window),
3. the current local worktree.

Conclusion from that earlier comparison:

1. The remote/stationary bootstrap problem does not look like it came from one tiny March 14 diff alone.
2. Some of the worst local control/presentation problems were introduced by newer uncommitted worktree changes in the client runtime.

## 5. Relevant Code Paths

### 5.1 Client local-control resolution

Relevant file:
- `bins/sidereal-client/src/runtime/replication.rs`

Key functions:
- `resolve_authoritative_control_entity_id_from_registry` at `bins/sidereal-client/src/runtime/replication.rs:359`
- `resolve_control_target_entity_id` at `bins/sidereal-client/src/runtime/replication.rs:377`
- `sync_local_player_view_state_system` at `bins/sidereal-client/src/runtime/replication.rs:957`
- `sync_controlled_entity_tags_system` at `bins/sidereal-client/src/runtime/replication.rs:1058`

Why these matter:

1. The client persists/player-anchor authority path lives here.
2. The HUD/control/camera paths depend on `ControlledEntity` / `SimulationMotionWriter` being attached to the correct runtime entity.
3. Free-roam is valid in Sidereal and can legitimately point `ControlledEntityGuid` back at the player entity itself.

### 5.2 Client interpolation/bootstrap gating

Relevant file:
- `bins/sidereal-client/src/runtime/transforms.rs`

Key functions:
- `interpolated_presentation_ready` at `bins/sidereal-client/src/runtime/transforms.rs:53`
- `sync_interpolated_world_entity_transforms_without_history` at `bins/sidereal-client/src/runtime/transforms.rs:150`
- `reveal_world_entities_when_initial_transform_ready` at `bins/sidereal-client/src/runtime/transforms.rs:314`

Why these matter:

1. These are the client-side gates that decide whether an interpolated entity is ready to be shown.
2. They also attempt to seed a usable transform when interpolation history is not ready.
3. If an observer entity exists with `Interpolated` but no usable authoritative pose, this is one of the first places to inspect.

### 5.3 Client duplicate-lane / presentation ownership

Relevant file:
- `bins/sidereal-client/src/runtime/visuals.rs`

Important concepts:
- duplicate winner recomputation
- canonical presentation owner
- suppression of duplicate predicted visuals

Why this matters:

1. A remote entity rendered on the confirmed/orange lane instead of the interpolated/green lane can jitter heavily.
2. Earlier cleanup in this area caused at least one regression where a valid interpolated clone was not chosen promptly enough.

### 5.4 Client motion ownership and nearby proxies

Relevant file:
- `bins/sidereal-client/src/runtime/motion.rs`

Important section:
- observer/proxy handling around `bins/sidereal-client/src/runtime/motion.rs:370`

Why this matters:

1. The code already explicitly warns about local physics bootstrapping interpolated entities at the origin before their history exists.
2. That warning is consistent with the observed remote-entity-at-origin symptom.

### 5.5 Server visibility/bootstrap resend

Relevant file:
- `bins/sidereal-replication/src/replication/visibility.rs`

Key functions:
- `apply_visibility_membership_diff` at `bins/sidereal-replication/src/replication/visibility.rs:1340`
- `queue_visibility_gain_spatial_resend` at `bins/sidereal-replication/src/replication/visibility.rs:1374`
- `update_network_visibility` at `bins/sidereal-replication/src/replication/visibility.rs:1638`

Important detail:
- `queue_visibility_gain_spatial_resend` currently forces `Position`, `Rotation`, `LinearVelocity`, and `AngularVelocity` to `set_changed()` on visibility gain.

Why this matters:

1. This server-side fix was added specifically to address the "stationary entity remains at origin until movement" hypothesis.
2. It is not yet proven sufficient.

### 5.6 Server control and prediction/interpolation target assignment

Relevant files:
- `bins/sidereal-replication/src/replication/control.rs`
- `bins/sidereal-replication/src/replication/simulation_entities.rs`

Why these matter:

1. Sidereal dynamically hands control between the player anchor and owned ships.
2. Prediction/interpolation targets are intentionally not stock-Lightyear-simple in this project.
3. A fresh agent should verify target assignment on fresh spawn and on relog, not just assume the control model is consistent.

### 5.7 Dashboard/TUI RTT source

Relevant files:
- `bins/sidereal-replication/src/replication/health.rs`
- `bins/sidereal-replication/src/tui.rs`

Key lines:
- `bins/sidereal-replication/src/replication/health.rs:650`
- `bins/sidereal-replication/src/tui.rs:932`

Important fact:
- The displayed latency is not BRP latency or a dashboard-side estimate.
- It comes from `link.stats.rtt.as_millis()` on the replication server.

This means the `~303ms` number shown for both local clients is a real server-side transport statistic, not just a frontend placeholder.

## 6. Evidence Already Collected

### 6.1 Earlier BRP evidence

Earlier user-provided BRP JSON showed a remote ship entity on the client with:

1. `Interpolated`
2. `Position = [0, 0]`
3. `Rotation = identity`
4. `Transform.translation = [0, 0, 0]`
5. `ConfirmedTick` present

Interpretation:

- the remote entity already existed on the interpolation lane,
- but its initial spatial state was still default/origin.

That is important because it means the problem is not only "confirmed-only entities missing initial state." At least one observer/interpolated entity also bootstrapped at a default pose.

### 6.2 On-disk dump mismatch

The local on-disk dump files under:

- `data/debug/brp_dumps/client1_20260315_111824.json`
- `data/debug/brp_dumps/replication_20260315_111824.json`

did not exactly match one of the pasted JSON snippets from the thread.

What the on-disk files did show:

1. client-side `ce9e421c-8b62-458a-803e-51e9ad272908` at the correct non-zero position,
2. that same entity on the client marked `Predicted`,
3. the local player anchor also predicted,
4. neither the ship nor the player anchor had `ControlledEntity` or `SimulationMotionWriter` in that dump,
5. the replication server had the same ship at the same correct non-zero position.

Interpretation:

1. The on-disk dump strongly supported the local control/HUD-binding bug.
2. The pasted JSON strongly supported the remote interpolated-bootstrap-at-origin bug.
3. The next agent should not assume every pasted snippet has a matching on-disk dump file in `data/debug/brp_dumps/`.

Additional concrete status from the local files:

1. `data/debug/brp_dumps/client2_20260315_111824.json` is empty (`0` bytes).
2. `data/debug/brp_dumps/client1_20260315_111824.json` contains zero `Interpolated` entities, so it cannot be used to prove the observer/interpolated failure mode for that timestamp.
3. That same client1 dump does prove a broader confirmed/bootstrap failure than one single asteroid GUID:
   - `9d31dc36-97ee-4b06-99f5-c993cc51bf7b` (`Rocky Asteroid 005`)
   - `d9ac06bc-c047-408a-8572-c86b6af7953a` (`Rocky Asteroid 003`)
   - `1da853d9-8354-4388-a927-0bda47e2bb82` (`Gem-rich Asteroid 001`)
   - `8d7198f5-1e81-45db-902a-ff358070169a` (`Rocky Asteroid 002`)
   - `b508d2aa-879f-4c25-a8f3-e797108ab58b` (`Carbonaceous Asteroid 008`)
4. All five appear on client1 as confirmed-only replicated roots at `Position = [0,0]` with no `Predicted` or `Interpolated` marker, while the matching replication dump shows authoritative non-zero positions for every one of those GUIDs.

Interpretation:

1. The confirmed/bootstrap failure is reproducibly broader than one isolated entity.
2. The current 2026-03-15 11:18:24 disk artifacts are insufficient to close any interpolated-lane hypothesis.
3. A new synchronized relog capture is required before making stronger claims about the orange/observer lane.

### 6.3 Fresh DB / relog observations

Most useful user repro so far:

1. Fresh DB:
   - own ship control works on both clients
   - remote observer behavior still wrong
2. Relog:
   - remote/bootstrap behavior becomes much worse
   - ship-at-origin becomes reproducible again

Interpretation:

- relog/persistence/hydration is likely involved,
- not just first-spawn ephemeral control handoff.

### 6.4 Suspicious local RTT

User observed:

- both local clients show `~303ms` RTT in the dashboard/TUI while all processes run on the same machine.

Interpretation:

1. Same-machine testing alone should not produce a stable `~303ms` RTT.
2. That RTT is likely part of the runtime problem and could contribute to orange confirmed-lane observer ships and jitter.
3. A fresh agent should investigate the actual transport mode and Lightyear sync/RTT behavior, not dismiss this as "just local load."

## 7. Changes Already Tried

### 7.1 Server-side attempt

Already added in the current worktree:

- targeted spatial resend on visibility gain in `bins/sidereal-replication/src/replication/visibility.rs`

Mechanism:

1. detect gained visibility membership,
2. call `queue_visibility_gain_spatial_resend`,
3. force `Position`, `Rotation`, `LinearVelocity`, and `AngularVelocity` to look changed.

Reason:

- intended to close the "stationary entity only corrects after later movement delta" gap without introducing a broad client-side repair scan.

Current status:

- not yet proven sufficient against the user's latest relog repro.

### 7.2 Client-side local-control fix

Already added in the current worktree:

- provisional raw-GUID control resolution in `bins/sidereal-client/src/runtime/replication.rs`

What changed:

1. `resolve_authoritative_control_entity_id_from_registry` now falls back to the raw controlled GUID when no canonical runtime-registry mapping exists yet.
2. `resolve_control_target_entity_id` was added to let control binding keep using that provisional GUID.
3. `sync_controlled_entity_tags_system` now uses that fallback path.

Reason:

- the local ship already existed as predicted, but the client sometimes failed to attach `ControlledEntity`/`SimulationMotionWriter` because it over-relied on canonical runtime-registry lookup during bootstrap.

Tests added and passing:

1. `local_player_control_resolution_uses_raw_guid_when_registry_is_not_ready`
2. `controlled_tag_target_falls_back_to_raw_guid_when_registry_is_not_ready`

Current status:

- this fix addresses the local control/bootstrap symptom,
- but does not explain or close the remote observer/bootstrap failures.

### 7.3 Earlier client-side presentation/interpolation fixes attempted during the same investigation

Earlier in the thread, additional client-side work tried to address:

1. interpolated readiness being stricter than actual transform bootstrap,
2. canonical presentation owner choosing the wrong lane,
3. duplicate dirtying not reacting to pose changes quickly enough.

These changes improved some observed mismatches, but they did not close the full repro and should not be treated as the complete solution.

## 8. Strong Hypotheses

### 8.1 Strong hypothesis: relog/persisted-entity observer bootstrap is still wrong

Reason:

1. Fresh DB + first login is partially better now.
2. Relog is still worse and reintroduces ship-at-origin behavior.
3. That points toward a bootstrap/hydration/relevance issue, not only a purely local UI/control issue.

Most relevant code paths:

1. `bins/sidereal-replication/src/replication/simulation_entities.rs`
2. `bins/sidereal-replication/src/replication/visibility.rs`
3. `bins/sidereal-client/src/runtime/replication.rs`
4. `bins/sidereal-client/src/runtime/transforms.rs`

### 8.2 Strong hypothesis: observer lane is still falling back to confirmed/orange too often

Reason:

1. User still sees orange AABBs on remote ships.
2. Orange AABB is confirmed-lane in the current debug overlay, not "non-dynamic body."
3. Orange + jitter is consistent with "observer is visible from confirmed lane, not a healthy interpolated lane."

Most relevant code paths:

1. `bins/sidereal-client/src/runtime/visuals.rs`
2. `bins/sidereal-client/src/runtime/transforms.rs`
3. `bins/sidereal-client/src/runtime/debug_overlay.rs`

### 8.3 Strong hypothesis: the `~303ms` RTT is a real contributing signal

Reason:

1. It comes from `link.stats.rtt`, not a fake UI counter.
2. Two loopback clients should not normally sit at a stable `~303ms` RTT.
3. This could reflect a transport/sync/config issue or a severe local scheduling/backpressure issue.

New supporting local evidence:

1. Sidereal's native client transport is plain UDP (`UdpIo`) with loopback bind default `127.0.0.1:0 -> 127.0.0.1:7001`; no Sidereal-local link conditioner or artificial latency wrapper is present in the transport setup.
2. Sidereal does not currently insert a custom Lightyear `InputTimelineConfig` or `SyncConfig`; the Lightyear client plugin path therefore keeps the default `InputTimelineConfig::default()`, which uses `InputDelayConfig::no_input_delay()` and default sync margins.
3. Because the repo already records upstream issue `#1402` as "default `no_input_delay` config doesn't reliably deliver inputs on localhost due to sync error margin tolerance," this upstream path is now a concrete active suspect rather than a generic watchlist item.

Important boundary:

1. This does not prove that `~303ms` is caused by Lightyear sync defaults.
2. It does prove the current runtime is still exposed to that exact upstream default-config risk.
3. Any next transport/RTT investigation should validate behavior both before and after explicitly overriding the default input-delay/sync configuration.

Useful related upstream references:

1. `docs/features/lightyear_upstream_issue_snapshot.md`
2. upstream issue `#1402`
3. upstream issue `#1086`

## 9. Weak / Unproven Hypotheses

These are plausible but not yet proven:

1. same-machine multi-client execution alone is the root cause
2. BRP ports affect gameplay replication
3. the issue is only client-side
4. the issue is only server-side
5. the issue is only one bug

The current evidence supports a multi-layer problem more than a single root cause.

## 10. Recommended Next Investigation Order

For a fresh agent, the best next order is:

1. Reproduce the user's exact latest scenario:
   - fresh DB,
   - create two players,
   - verify fresh-login state,
   - quit both,
   - relog both,
   - compare fresh vs relog.
2. Fix the evidence capture gap before trusting new BRP artifacts:
   - verify both client BRP endpoints are reachable,
   - confirm dump files are non-empty immediately after capture,
   - record the exact file sizes / timestamps alongside the repro notes.
3. Capture synchronized BRP dumps after relog for:
   - replication server,
   - client 1,
   - client 2.
4. Compare, for the same ship GUIDs:
   - server `Position` / `Rotation`,
   - client lane markers (`Predicted`, `Interpolated`, `ConfirmedTick`),
   - client `Position` / `Rotation` / `Transform`,
   - whether `Confirmed<Position>` / `Confirmed<Rotation>` wrappers or history exist.
5. Verify actual transport path and latency:
   - what transport each client is using,
   - whether the `~303ms` RTT is stable and reproducible,
   - whether it changes after relog.
6. Add one deliberate Lightyear-defaults check during that repro:
   - confirm current run is using default `InputTimelineConfig`,
   - rerun with an explicit non-default input-delay configuration so localhost behavior can be compared instead of inferred.
7. Only after that, decide whether the next fix belongs primarily in:
   - server visibility/bootstrap delivery,
   - client adoption/bootstrap gating,
   - observer lane winner resolution,
   - or transport/sync configuration.

## 11. Suggested Concrete Starting Points For The Next Agent

### Option A: Relog/bootstrap first

Start here if prioritizing correctness:

1. instrument relog/rejoin path for one affected remote ship GUID,
2. inspect `simulation_entities.rs` and `visibility.rs`,
3. confirm whether the client is missing initial observer pose or discarding it.

### Option B: RTT/transport first

Start here if prioritizing suspicious latency:

1. instrument actual transport mode and Lightyear connection stats,
2. determine why loopback clients show `~303ms` RTT,
3. re-evaluate observer jitter after that.

### Option C: Observer lane first

Start here if prioritizing visible jitter:

1. verify why remote ships remain orange/confirmed instead of green/interpolated,
2. compare actual clone inventory and winner choice at runtime,
3. confirm whether the wrong lane is chosen because interpolation history is unavailable or because the winner logic is still wrong.

## 12. Useful Upstream / Local References

Most relevant local docs:

1. `docs/reports/lightyear_multiplayer_prediction_interpolation_audit_report_2026-03-15.md`
2. `docs/plans/multiplayer_prediction_interpolation_reliability_plan_2026-03-15.md`
3. `docs/features/lightyear_upstream_issue_snapshot.md`
4. `docs/features/prediction_runtime_tuning_and_validation.md`
5. `docs/features/visibility_replication_contract.md`

Most relevant upstream references already identified locally:

1. issue `#1034` `PredictionSwitching`
2. issue `#1380` required components / interpolation hydration
3. issue `#1402` localhost input/sync issues
4. issue `#1086` sync/timeline concerns
5. PR `#1421` interpolation history initialization on `Interpolated` add

## 13. Current Bottom Line

The current branch is not at a single-bug state.

Best current summary:

1. local control/bootstrap was at least partly fixed client-side and should be revalidated,
2. remote observer/bootstrap correctness is still broken,
3. relog/persisted-entity bootstrap is likely a distinct high-value clue,
4. `~303ms` RTT on same-machine clients is suspicious and should not be ignored,
5. the next agent should treat this as a server/client contract investigation, not just a UI bug or just a Lightyear blame exercise.
