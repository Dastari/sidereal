# Lightyear Audit Report

Date: 2026-03-08  
Scope: Sidereal Lightyear integration audit against current repo state and the Lightyear book

## 1. Executive Summary

Sidereal is already using a meaningful part of Lightyear correctly:

- component replication,
- per-client `PredictionTarget` / `InterpolationTarget`,
- rollback-capable predicted components,
- per-entity replication groups,
- server-driven visibility and delivery,
- Lightyear Avian integration,
- client-local native input for prediction.

The biggest missing smoothness feature is not prespawning. It is render-time frame interpolation between fixed ticks.

The workspace enables Lightyear's `frame_interpolation` feature, but the client does not currently add `FrameInterpolationPlugin` or `FrameInterpolate<...>` markers. On a high-refresh display, that alone can make a localhost game still feel jittery even when simulation and replication are otherwise functioning.

The second major issue is architectural drift on the client. The client now layers several compensating systems on top of Lightyear:

- replication adoption deferral,
- transform bootstrap fallbacks,
- duplicate predicted/interpolated visual suppression,
- player-anchor render syncing,
- camera smoothing,
- motion ownership enforcement.

That level of compensation strongly suggests the predicted/interpolated entity lifecycle is still not clean enough, especially around control handoff.

## 1.1 Sidereal-Specific Constraints That Differ From Typical Lightyear Examples

This project is not a straight adaptation of the stock Lightyear character/vehicle examples. The audit should be read in the context of three important Sidereal-specific constraints.

### A. Dynamic swapping of the predicted entity

Sidereal allows the locally predicted entity to change at runtime.

The controlled target is not fixed for the whole session. A player can:

- control one ship,
- release control,
- control a different ship,
- or fall back to controlling the player entity itself.

That means Sidereal must handle repeated transitions between:

- predicted local control,
- interpolated remote observation,
- and confirmed-only/private player-anchor states.

This is a much harder lifecycle than the simpler "one client permanently predicts one pawn" model used by many Lightyear examples. It is also the main reason control handoff and predicted/interpolated switching are central concerns in this audit.

### B. Persisted player entity with free-roam and camera-follow semantics

Sidereal keeps a real player entity in the world as the persistent runtime anchor for:

- control target,
- camera-follow chain,
- selection/focus state,
- and free-roam behavior.

The project supports a "Free Roam" mode where the player entity itself can move around the map and the camera can follow that entity, even when the player is not controlling a ship.

This differs from many Lightyear examples where:

- the controlled pawn is the only important local gameplay entity,
- the camera simply follows that pawn,
- and there is no separate persisted player-anchor entity with its own movement/follow semantics.

Because Sidereal has this extra player-anchor lane, the client has additional complexity around:

- player-anchor replication,
- controlled-entity handoff,
- anchor-to-controlled render syncing,
- and camera target resolution.

### C. Strict server-side visibility, redaction, and delivery rules

Sidereal does not use a simple "nearby entities only" model.

The project has strict visibility and disclosure rules involving:

- server-side visibility/range checks,
- owner-only data,
- public and faction visibility,
- tactical/fog/intel products,
- and explicit redaction before delivery.

This is richer than the simpler room/relevance examples in the Lightyear book. As a result:

- Sidereal correctly keeps its own visibility contract authoritative,
- Lightyear's built-in interest-management patterns are informative but not a drop-in replacement,
- and any Lightyear recommendation must be filtered through Sidereal's stronger visibility/redaction model.

## 1.2 Audit Guardrails For Future Refactors

Future audits should not treat every deviation from stock Lightyear samples as accidental.

Two especially important Sidereal-specific rules are:

- Do not bind local motion ownership to an `Interpolated` fallback just because a `Predicted` clone is missing.
  Dynamic control handoff can temporarily leave the desired controlled GUID without a spawned predicted clone. In Sidereal, promoting an interpolated ship into the local motion-writer lane breaks the single-writer simulation rule and produces misleading "jerky prediction" symptoms.
- Free-roam through the persisted player anchor is a valid non-stock control mode.
  The player anchor exists for camera, selection, and detached/free-roam semantics. It should not be evaluated using the simpler "exactly one permanently predicted pawn" assumptions from Lightyear examples.

If an audit finds code that looks more defensive than the book examples, it must first check whether that code exists to preserve one of these invariants.

## 2. Current Sidereal Usage

### 2.1 Actively used and materially important

- Lightyear protocol/channel registration:
  - `crates/sidereal-net/src/lightyear_protocol/registration.rs`
- client plugins, prediction stack, Avian integration:
  - `bins/sidereal-client/src/native/mod.rs`
- server plugins and replication sender:
  - `bins/sidereal-replication/src/main.rs`
  - `bins/sidereal-replication/src/replication/lifecycle.rs`
- per-entity prediction/interpolation targeting:
  - `bins/sidereal-replication/src/replication/simulation_entities.rs`
  - `bins/sidereal-replication/src/replication/control.rs`
- server-side visibility and delivery:
  - `bins/sidereal-replication/src/replication/visibility.rs`
- client rollback policy/correction tuning:
  - `bins/sidereal-client/src/native/replication.rs`

### 2.2 Concrete observations

- Avian motion components are registered with prediction and interpolation:
  - `Position`
  - `Rotation`
  - `LinearVelocity`
  - `AngularVelocity`
- control/session/input/manifest/tactical traffic already uses explicit channels with priorities.
- replication sender runs at 60 Hz and uses `SendUpdatesMode::SinceLastAck`.
- untouched default replication groups are normalized to per-entity groups.
- controlled entities are predicted for the owning client and interpolated for others.
- Sidereal intentionally keeps authoritative server input on its own authenticated realtime input lane instead of trusting Lightyear server-native input runtime.

## 3. What Sidereal Is Not Using

### 3.1 Important missing Lightyear usage

- `FrameInterpolationPlugin`
- `FrameInterpolate<C>`
- `SkipFrameInterpolation`
- `PreSpawned` / `PreSpawnedPlayerObject` style projectile prespawning in game code
- `PrePredicted`
- `DeterministicPredicted`
- explicit transport bandwidth caps via `PriorityConfig`
- built-in Lightyear room-based interest management
- authority transfer features
- explicit gameplay use of `InterpolationDelay`

### 3.2 Why the biggest missing item is frame interpolation

The Lightyear book's visual interpolation guidance exists specifically to smooth render output between fixed ticks. Sidereal compiles with the feature enabled but does not appear to actually instantiate the plugin or attach frame-interpolation state to visual components.

That means:

- fixed-step motion is correct,
- interpolation between replicated snapshots may exist,
- but render-time motion still advances in visible fixed-tick chunks.

This is the first thing to fix for "localhost still doesn't feel smooth".

## 4. What Sidereal Is Using Wrong Or Fighting

### 4.1 Duplicate visual arbitration is still too heavy

The client suppresses duplicate predicted/interpolated visuals by GUID and scores winners manually:

- `bins/sidereal-client/src/native/visuals.rs`

That is a strong sign the client does not fully trust the displayed entity lifecycle. If render correctness depends on winner-picking between multiple Lightyear copies, the stack is still too fragile.

### 4.2 Transform bootstrap and history-gap patching remain active

The client still carries fallback systems to prevent origin flashes and deal with interpolated entities before history is ready:

- `bins/sidereal-client/src/native/transforms.rs`

This is pragmatic, but it means the interpolation pipeline is still not "clean native Lightyear". It is patched.

### 4.3 Anchor and camera smoothing stack can amplify perceived jitter

The client:

- copies the local player anchor render transform from the controlled entity,
- then separately resolves a preferred follow target,
- then separately applies camera smoothing.

Relevant files:

- `bins/sidereal-client/src/native/camera.rs`

This can make netcode feel worse than it is because multiple visual layers are compensating independently.

### 4.4 Client-side adoption path is still complex

The replication plugin ordering shows a large amount of client-side adoption and readiness logic before visuals stabilize:

- `bins/sidereal-client/src/native/plugins.rs`
- `bins/sidereal-client/src/native/replication.rs`

That complexity is likely contributing to the "not smooth" feel, especially during control changes and relevance churn.

At the same time, future cleanup work should preserve the hard invariant that only predicted local roots may enter the client-side motion-writer lane. Simplifying this area by allowing confirmed/interpolated fallbacks would make the code look more like stock samples while regressing actual runtime behavior for Sidereal.

### 4.5 Internal docs are partially stale

There is doc drift between older Lightyear analysis notes and the current runtime. Some docs still describe the system as bypassing interpolation broadly, while the current runtime clearly does use Lightyear predicted/interpolated markers in parts of the pipeline.

That drift should be cleaned up before further refactors.

## 5. Prespawning Assessment

## 5.1 What the Lightyear book recommends

Prespawning is intended for entities that:

- are created on both client and server,
- are spawned in fixed-tick systems,
- should appear instantly for the local player,
- and later be matched against the authoritative replicated entity.

Classic example: bullets or short-lived projectiles.

## 5.2 Why prespawning is not the first fix for Sidereal

Sidereal's current combat slice is not authoritative projectile-entity combat. It is:

- server-authoritative shot resolution,
- hitscan/query-based impact resolution,
- compact tracer messages for visuals.

Relevant files:

- `docs/features/projectile_firing_game_loop.md`
- `bins/sidereal-replication/src/replication/combat.rs`
- `bins/sidereal-client/src/native/visuals.rs`

That means `PreSpawned` bullets are not the primary missing optimization for the current gameplay loop.

For the present design:

- gatling/hitscan fire should remain event/tracer based,
- prespawning is better reserved for future true projectile entities such as missiles, plasma, or slow ballistic rounds.

## 5.3 Recommendation on prespawning

- Do not prioritize prespawn for the current hitscan tracer loop.
- Do evaluate `PreSpawned` when Sidereal introduces real networked projectile entities.
- If that happens, follow the Lightyear projectile example pattern closely and keep spawning in fixed-tick systems only.

## 6. Smoothness Diagnosis

The most likely reasons the game still feels bad on localhost are:

1. No real frame interpolation between fixed ticks.
2. Too many compensating visual layers on top of Lightyear.
3. Duplicate predicted/interpolated entity arbitration.
4. Control-handoff lifecycle still not clean.
5. Camera/anchor smoothing sometimes amplifying visual disagreement.

This does not look primarily like:

- bandwidth pressure,
- raw server CPU shortage,
- GPU shortage,
- or memory pressure.

## 7. What More of Lightyear Sidereal Should Use

### Priority 1: Frame interpolation

Add real Lightyear frame interpolation for the visual components actually rendered.

Expected result:

- smoother motion at render framerate,
- less visible fixed-tick stepping,
- immediate improvement even on localhost.

### Priority 2: Cleaner predicted/interpolated lifecycle

Reduce or eliminate the need for:

- duplicate visual suppression,
- manual winner selection by GUID,
- extra transform bootstrap logic where possible.

Expected result:

- less archetype churn,
- clearer ownership of displayed state,
- fewer handoff artifacts.

### Priority 3: `DeterministicPredicted` evaluation

This is worth evaluating for special cases where entities should participate in rollback semantics without being full rollback triggers.

This is lower priority than frame interpolation and lifecycle cleanup.

### Priority 4: Prespawning for future projectile entities

Use only when projectile entities become real authoritative game objects.

## 7.1 New explicit fix: real ballistic projectile entities

Sidereal should add a proper projectile path for weapons whose design goal is:

- ballistic drift,
- lead,
- travel time,
- dodgeability,
- and proper local prediction.

For that class of weapon, the current hitscan-plus-tracer model is the wrong foundation.

The correct direction is:

- real projectile entities,
- client-local predicted spawn,
- server-authoritative validation and replication,
- Lightyear predicted/interpolated handling for projectile ownership and observation,
- and `PreSpawned` flow where appropriate for short-lived locally fired projectiles.

This is now part of the recommended fix list for the networking/combat feel problems.

Important scope note:

- not every weapon must become a true projectile,
- but any weapon that is meant to behave like a real inertial space projectile should use this path instead of tracer reconstruction from hitscan events.

### Priority 5: Bandwidth caps and priority tuning

Lightyear transport priority and bandwidth quota support exists, but localhost feel is not currently blocked on bandwidth throttling. This is a later optimization.

## 8. What Sidereal Should Keep Doing Its Own Way

### 8.1 Authoritative input validation

Sidereal's custom authenticated input path is the right choice for now.

Given upstream input and host-mode issues, this should remain explicit and server-validated.

### 8.2 Visibility and redaction policy

Sidereal's visibility/fog/owner-lane model is richer than Lightyear's generic interest-management abstractions.

Do not replace the current visibility contract wholesale with Lightyear rooms/relevance.

### 8.3 Hierarchy and relationship authority

Sidereal is correct to keep hierarchy semantics authoritative on its side rather than trusting upstream relationship replication to fully preserve the project's mount/hardpoint model.

## 9. Prioritized Recommendations

1. Implement real Lightyear frame interpolation in the client runtime.
2. Instrument and trace one controlled ship GUID across server/client1/client2 to verify:
   - which entity is predicted,
   - which entity is interpolated,
   - which entity is rendered,
   - which systems write motion and transform.
3. Remove or reduce duplicate visual winner-selection logic once the lifecycle is stable.
4. Simplify player-anchor render syncing and camera follow smoothing so only one layer owns final visual smoothing.
5. Re-test control handoff after the above.
6. Implement a proper ballistic projectile path for weapons that are supposed to behave as true space projectiles.
7. Use `PreSpawned` / predicted projectile flow for those projectile entities instead of reconstructed hitscan tracers.
8. Keep the current event/tracer path only for weapon classes that are intentionally hitscan.

## 9.1 System-order alignment with Lightyear

The Lightyear system-order guidance and flow diagrams were reviewed directly, including:

- packet receive and replication in `PreUpdate`,
- buffered input writing before fixed-step gameplay,
- gameplay/prediction work in fixed schedules,
- prediction history updates after fixed-step simulation,
- packet send in `PostUpdate`.

For Sidereal, the correct rule is:

- align with Lightyear's schedule model by default,
- deviate only where Sidereal's architecture explicitly requires it,
- and avoid adding custom networking/prediction logic in ways that fight the expected Lightyear schedule flow.

This is especially important for any future projectile implementation:

- local predicted/prespawn projectile creation must happen in the fixed-step gameplay path,
- not in ad-hoc `Update` systems,
- and any render-only smoothing must stay separate from authoritative simulation state.

Sidereal-specific exceptions still apply:

- custom authenticated server input lane,
- strict server visibility/redaction contract,
- dynamic predicted-entity swapping,
- and the persisted player-anchor/free-roam camera model.

Those are valid reasons for local divergence, but they should remain explicit and minimal rather than accidental schedule drift.

## 10. File References

- [`crates/sidereal-net/src/lightyear_protocol/registration.rs`](/home/toby/dev/sidereal_v3/crates/sidereal-net/src/lightyear_protocol/registration.rs)
- [`bins/sidereal-client/src/native/mod.rs`](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/mod.rs)
- [`bins/sidereal-client/src/native/plugins.rs`](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/plugins.rs)
- [`bins/sidereal-client/src/native/replication.rs`](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/replication.rs)
- [`bins/sidereal-client/src/native/transforms.rs`](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/transforms.rs)
- [`bins/sidereal-client/src/native/visuals.rs`](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/visuals.rs)
- [`bins/sidereal-client/src/native/camera.rs`](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/camera.rs)
- [`bins/sidereal-client/src/native/input.rs`](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/input.rs)
- [`bins/sidereal-replication/src/replication/lifecycle.rs`](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/lifecycle.rs)
- [`bins/sidereal-replication/src/replication/simulation_entities.rs`](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/simulation_entities.rs)
- [`bins/sidereal-replication/src/replication/control.rs`](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/control.rs)
- [`bins/sidereal-replication/src/replication/visibility.rs`](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/visibility.rs)
- [`bins/sidereal-replication/src/replication/combat.rs`](/home/toby/dev/sidereal_v3/bins/sidereal-replication/src/replication/combat.rs)
- [`docs/features/projectile_firing_game_loop.md`](/home/toby/dev/sidereal_v3/docs/features/projectile_firing_game_loop.md)
- [`docs/features/lightyear_upstream_issue_snapshot.md`](/home/toby/dev/sidereal_v3/docs/features/lightyear_upstream_issue_snapshot.md)
- [`docs/lightyear_handoff_debug_summary.md`](/home/toby/dev/sidereal_v3/docs/lightyear_handoff_debug_summary.md)

## 11. External Sources

Lightyear book:

- <https://cbournhonesque.github.io/lightyear/book/>
- <https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/prediction.html>
- <https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/interpolation.html>
- <https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/visual_interpolation.html>
- <https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/prespawning.html>
- <https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/bandwidth_management.html>
- <https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/interest_management.html>
- <https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/client_replication.html>

Upstream Lightyear repo and examples reviewed locally:

- `/home/toby/dev/lightyear/examples/projectiles`
- `/home/toby/dev/lightyear/lightyear_avian`
- `/home/toby/dev/lightyear/lightyear_frame_interpolation`
- `/home/toby/dev/lightyear/lightyear_interpolation`
