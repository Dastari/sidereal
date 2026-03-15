# Lightyear / Multiplayer / Prediction / Interpolation Audit Report

Status: Audit report  
Date: 2026-03-15  
Scope: Native client multiplayer correctness and smoothness regression audit  
Primary inputs:
- current client/runtime code under `bins/sidereal-client/src/runtime/`
- protocol registration under `crates/sidereal-net/src/lightyear_protocol/registration.rs`
- current local BRP dumps:
  - `data/debug/brp_dumps/replication_20260315_111824.json`
  - `data/debug/brp_dumps/client1_20260315_111824.json`
- historical plans:
  - `docs/plans/lightyear_runtime_alignment_implementation_plan.md`
  - `docs/plans/bevy_2d_rendering_optimization_implementation_plan_2026-03-10.md`
  - `docs/plans/bevy_2d_rendering_optimization_completion_plan_2026-03-12.md`
  - `docs/features/prediction_runtime_tuning_and_validation.md`
- historical baseline commit: `2814bea`
- committed optimization head in the current regression window: `565a400`

Update note (2026-03-15):
- This audit compares three states, because they are materially different:
  1. the March 13 baseline (`2814bea`),
  2. the committed March 14 branch head (`565a400`),
  3. the current local worktree, which contains additional uncommitted client runtime changes.
- The current multiplayer failures are not explained by one single change. There is one underlying stationary-entity bootstrap problem, and there are additional newer client-side control/presentation regressions layered on top.

## 1. Executive Summary

The real multiplayer problem is not primarily Bevy 2D rendering. It is a runtime contract problem between:

1. Lightyear lane ownership (`Predicted` / `Interpolated` / confirmed-only),
2. Sidereal's custom control-transfer model,
3. entity bootstrap/adoption timing for stationary replicated entities,
4. client presentation winner selection and HUD/control binding.

The strongest unresolved issue in the supplied evidence is this:

- stationary confirmed entities can exist on the replication server with a correct authoritative `Position`, while the client receives or keeps them at `Position = [0, 0]` until a later movement delta arrives.

That is visible in the supplied dumps for asteroids and matches the earlier remote-ship symptom where a ship appears at origin and only snaps into place when it moves.

Separately, the current local worktree introduced additional client regressions:

- the local ship can already be predicted and receiving input while the HUD/control-tag path still thinks nothing is controlled,
- duplicate-visual / canonical-presentation cleanup can keep the wrong lane visible long enough to create choppy observer motion or false debug anomalies.

So the diagnosis is:

1. there is one older multiplayer bootstrap problem for stationary replicated entities,
2. the newer optimization work added extra client-side regressions around control binding and presentation ownership,
3. the planning documents were mostly directionally correct, but the execution skipped the validation gate that those plans required.

## 2. What The Plans Actually Said

The key planning direction was mostly sound.

From `docs/plans/lightyear_runtime_alignment_implementation_plan.md`:

- adopt Lightyear frame interpolation,
- reduce duplicate winner-selection ambiguity,
- preserve the current dynamic control-swap model instead of pretending the project is a stock Lightyear example.

From `docs/plans/bevy_2d_rendering_optimization_implementation_plan_2026-03-10.md` and `docs/plans/bevy_2d_rendering_optimization_completion_plan_2026-03-12.md`:

- measure first,
- preserve Lightyear interpolation and the current camera-follow ordering,
- finish visibility / asset / polling cleanup before deeper tactical/nameplate/presentation work,
- do not treat rendering-only work as the top root cause when cadence and CPU-side churn are still active.

The plan mistake was not the direction. The execution mistake was that Phase 4 / Phase 5 style work:

- nameplate redesign,
- duplicate presentation arbitration cleanup,
- debug/HUD expansion,

was allowed to proceed without a fresh multiplayer validation loop for:

- local control attach on join,
- stationary remote ship visibility,
- stationary world-object bootstrap,
- observer interpolation lane selection.

## 3. Baseline Comparison

### 3.1 March 13 baseline: `2814bea`

This baseline still used the older duplicate-runtime handling:

- runtime entity registration accepted the replicated lane broadly,
- later clone adoption for a live runtime ID was skipped,
- there was no `CanonicalPresentationEntity` marker,
- duplicate visual suppression was simpler and did not attempt to formalize one global presentation owner.

That design was less clean architecturally, but it also meant the system did not depend on one strict winner contract across:

- control HUD binding,
- debug overlay lane classification,
- nameplates,
- streamed-visual attachment selection.

### 3.2 Committed optimization head: `565a400`

The committed diff from `2814bea` to `565a400` is surprisingly small in the core networking path.

The visible committed changes in the audited files are mainly:

- debug/HUD instrumentation growth in `debug_overlay.rs`,
- large Bevy UI reshaping in `ui.rs`,
- a projectile `PreSpawned` hash change in `replication.rs`.

I did not find evidence that `565a400` alone explains the full current multiplayer failure set.

### 3.3 Current local worktree

The current worktree contains additional uncommitted client changes in:

- `bins/sidereal-client/src/runtime/replication.rs`
- `bins/sidereal-client/src/runtime/transforms.rs`
- `bins/sidereal-client/src/runtime/visuals.rs`

These newer changes do explain several of the observed regressions:

1. canonical-runtime-ID ownership narrowed to the confirmed lane,
2. canonical presentation winner logic became stricter and more centralized,
3. local player view-state sync was still assuming the local player anchor could be found through the canonical runtime registry,
4. debug overlay anomaly reporting started surfacing those mismatches.

In other words: some current breakage is not in the committed March 14 history. It lives in the current local client worktree.

## 4. Findings

### Finding 1: Stationary confirmed entities are not bootstrapping with authoritative position on the client

Severity: Critical

Evidence:

- In `data/debug/brp_dumps/replication_20260315_111824.json`, asteroid `d9ac06bc-c047-408a-8572-c86b6af7953a` has:
  - `Position = [494.0451965332031, 619.6416625976562]`
- In `data/debug/brp_dumps/client1_20260315_111824.json`, the same GUID has:
  - `Position = [0, 0]`
  - `Transform.translation = [0, 0, 0]`
  - `Replicated = true`
  - no `Predicted`
  - no `Interpolated`

This exactly matches the user-visible symptom:

- stationary remote objects appear at origin,
- later movement causes a snap to the correct place,
- observer interpolation only looks acceptable after real motion begins.

Implication:

- the client is not reliably getting or applying the initial authoritative position for some confirmed-only late-relevance entities.
- This is not a nameplate bug.
- This is not a tactical overlay bug.
- This is not primarily a Bevy UI bug.

Most likely layer:

- replication bootstrap / late-relevance full-state delivery,
- or Lightyear/Avian initial state hydration for stationary confirmed entities.

Why this matters:

- It breaks ships and world objects the same way.
- It explains why movement “fixes” the problem: later deltas arrive, but the initial stationary state is wrong.

### Finding 2: Local control/HUD binding was broken by a client-side lookup contract mismatch

Severity: High

Evidence from the supplied client dump:

- the controlled ship GUID `ce9e421c-8b62-458a-803e-51e9ad272908` already has `lightyear_core::prediction::Predicted`,
- input routing still works,
- camera follow still behaves as if the ship is controlled,
- but the HUD/debug path reports no proper controlled predicted root.

Root cause:

- `sync_local_player_view_state_system` in `bins/sidereal-client/src/runtime/replication.rs` was relying on the canonical runtime entity registry to find the local player anchor.
- After the newer runtime-ID narrowing, the local player anchor could exist only as a predicted clone during join bootstrap, so that lookup failed.

This is a real client regression introduced by the newer presentation/runtime cleanup work, not by the earlier March 13 baseline.

Status:

- There is now a local uncommitted fix in `bins/sidereal-client/src/runtime/replication.rs` that resolves the local player's authoritative control target directly from player-anchor clones instead of depending on the canonical runtime registry entry.
- That fix passes targeted client tests, but it is not yet committed.

### Finding 3: Presentation-ownership cleanup is architecturally correct, but it was implemented without a strong enough lifecycle contract

Severity: High

Relevant files:

- `bins/sidereal-client/src/runtime/visuals.rs`
- `bins/sidereal-client/src/runtime/transforms.rs`
- `bins/sidereal-client/src/runtime/debug_overlay.rs`

What happened:

- The audit/plan direction to simplify duplicate predicted/interpolated/confirmed presentation was correct.
- The implementation then made downstream client systems depend on a stronger “canonical visible entity” contract than the Lightyear clone lifecycle could consistently satisfy during bootstrap and control transfer.

Observed consequences from this investigation:

1. a remote GUID can be presented from the wrong lane long enough to produce choppy motion,
2. a predicted local ship can exist without the HUD/control path binding to it,
3. the debug overlay can correctly expose a mismatch that the rest of the client still does not resolve,
4. current fixes in the worktree are compensating for those mismatches after the fact.

This is not an argument to abandon presentation cleanup. It is an argument that the cleanup needs an explicit contract first:

- how a GUID resolves during join bootstrap,
- how a GUID resolves during predicted -> observer transfer,
- how a GUID resolves when interpolated history is not ready,
- which lane owns HUD/control state,
- which lane owns visuals,
- which lane owns debug classification.

### Finding 4: The current architecture still lives in the Lightyear control-switching exception path, and the plans understated how fragile that remains

Severity: Medium

This is documented in:

- `docs/features/prediction_runtime_tuning_and_validation.md`
- `docs/features/lightyear_integration_analysis.md`
- `docs/features/lightyear_upstream_issue_snapshot.md`

Important upstream context:

- upstream issue `#1034` (`PredictionSwitching`) is still directly relevant,
- upstream PR `#1421` is promising but not a complete Sidereal solution,
- Sidereal still owns the predicted/interpolated control-transfer contract locally.

Conclusion:

- the project is still in a hybrid model,
- Lightyear is not yet providing the control-swap lifecycle the project needs,
- so any cleanup that assumes “there is one stable lane owner now” must be treated as high-risk until that hybrid contract is explicitly encoded.

## 5. What Actually Regressed, And When

### 5.1 Regression likely older than the newest local worktree

The stationary `0,0 until movement` behavior is the strongest candidate for the older underlying problem.

Reason:

- it affects confirmed-only stationary asteroids,
- it affects stationary remote ships,
- it is visible directly in the supplied server/client BRP dumps,
- and it is not explained by the newer local HUD/control-tag regression alone.

This likely predates the newest uncommitted presentation fixes.

### 5.2 Regression clearly introduced by newer local client changes

The following are clearly newer client regressions layered on top:

1. local ship predicted lane exists but HUD/control binding fails,
2. debug overlay reports `remote guid ... resolved as Predicted` for the local ship case,
3. lane selection / canonical presentation logic can disagree with transform readiness,
4. current nameplate and duplicate-resolution work now depends on those lane decisions.

These are current worktree problems, not just committed March 14 history.

## 6. Comparison Against The Bevy 2D Plans

The Bevy 2D optimization plans were mostly right about priority:

1. visibility cadence and replication cost first,
2. asset delivery and polling cleanup next,
3. only then tactical/nameplate/HUD and duplicate presentation follow-up.

The actual implementation drift was:

- the branch moved into nameplate and presentation-owner cleanup while the multiplayer bootstrap/correctness baseline was still not locked,
- there was no explicit regression gate for “join a second client with stationary ships and stationary landmarks already visible,”
- no plan status note appears to have captured a before/after multiplayer correctness checklist for those cases.

So the issue is not “the Bevy 2D plan was wrong.”

The issue is:

- the plan assumed the Lightyear/runtime contract was stable enough for later optimization layers,
- but that contract was still in a transitional state.

## 7. Recommended Next Order Of Work

### 7.1 First: fix the stationary initial-state bootstrap problem

Do not keep layering client presentation fixes over it.

Required investigation target:

- why confirmed-only late-relevance entities can arrive with correct server position but client `Position = [0,0]`.

Primary files to inspect next:

- `bins/sidereal-replication/src/replication/visibility.rs`
- entity replication/visibility handoff around gain-visibility and state re-send
- Lightyear/Avian initial component application path for confirmed-only entities

Acceptance gate:

- a stationary remote ship and stationary asteroids must appear in the correct place immediately on late join, before any movement occurs.

### 7.2 Second: land the local player control-resolution fix cleanly

The local uncommitted fix in `bins/sidereal-client/src/runtime/replication.rs` is directionally correct and should be committed after full validation.

Acceptance gate:

- on first join, the controlled ship is predicted,
- the HUD/control state binds immediately,
- the debug overlay no longer reports the local ship as a remote predicted anomaly.

### 7.3 Third: write the presentation-ownership contract down before more cleanup

Create or update a doc that explicitly states:

1. which lane owns control/HUD,
2. which lane owns visuals,
3. which lane owns debug classification,
4. what bootstrap fallback is allowed before interpolation history is ready,
5. how predicted -> observer and observer -> predicted transitions are expected to behave.

Until this exists, more “cleanup” in `visuals.rs`, `transforms.rs`, `ui.rs`, and `debug_overlay.rs` will keep being risky.

### 7.4 Fourth: add one mandatory multiplayer regression checklist

For every change touching:

- prediction,
- interpolation,
- duplicate presentation,
- control transfer,
- adoption/bootstrap,

validate at least:

1. client joins with its own controlled ship already assigned,
2. another client already in world is stationary,
3. stationary asteroids/landmarks are already visible,
4. both remote and local ships show the expected lane,
5. observer motion is smooth after movement begins,
6. stationary remote objects never start at origin incorrectly.

## 8. Bottom Line

The current branch does not have one single regression.

It has:

1. an older unresolved stationary-entity bootstrap bug in the multiplayer replication path,
2. newer client-side control/presentation regressions introduced while cleaning up duplicate runtime ownership,
3. a process problem where the optimization phases moved forward without a hard multiplayer correctness gate.

The Bevy 2D plans were not fundamentally wrong. The mistake was treating the Lightyear/prediction/interpolation contract as stable enough for downstream optimization layers before the stationary bootstrap and control-transfer edge cases were actually locked down.
