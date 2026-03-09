# Debug Gizmo / Overlay Reimplementation Plan

Status: Proposed  
Date: 2026-03-09

Update note (2026-03-09):
- This plan replaces the current native client debug overlay/gizmo stack with a simpler, snapshot-driven design.
- It is motivated by live issues that the current overlay has made harder to diagnose:
  - collision AABB / velocity-arrow flicker,
  - ambiguity between predicted / interpolated / confirmed duplicate roots,
  - unstable control-target diagnostics during dynamic handoff,
  - text overlays whose layout shifts too much under changing numeric values.
- This plan is intentionally broader than "fix F3 flicker". It is meant to produce a stable long-term debugging surface for prediction, interpolation, camera, visibility, and render/runtime ownership issues.

## 1. Purpose

The current debug overlay has accumulated too many responsibilities in one place:

- input toggle state,
- root-entity winner selection,
- control-target resolution,
- predicted/confirmed comparison drawing,
- free-form gizmo drawing,
- runtime summary logging,
- HUD-style text overlays,
- special-case fallbacks for handoff and clone churn.

That has made the overlay itself part of the debugging problem. We need a replacement that is:

- deterministic,
- snapshot-based,
- schedule-safe,
- explicit about logical entity selection,
- visually stable under changing numeric data,
- useful for future audits and runtime triage.

## 2. Why A Full Reimplementation Is Justified

The current overlay in [debug_overlay.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/debug_overlay.rs) has already required multiple corrective passes:

- dedupe stable duplicate roots by GUID,
- stop using `ViewVisibility` for winner selection,
- resolve the local controlled target from `ControlledEntity` instead of confirmed registry state,
- preserve a confirmed ghost even when prediction is missing,
- add more logs to explain missing predicted clones and visual correction state.

Those were necessary fixes, but they also show the current shape is too coupled to the live runtime.

Related audit references:

- [native_runtime_system_ownership_audit_2026-03-09.md](/home/toby/dev/sidereal_v3/docs/reports/native_runtime_system_ownership_audit_2026-03-09.md)
- [bevy_2d_rendering_optimization_audit_report_2026-03-09.md](/home/toby/dev/sidereal_v3/docs/reports/bevy_2d_rendering_optimization_audit_report_2026-03-09.md)

The reimplementation should treat the debug overlay as a diagnostic projection of already-resolved runtime state, not as the place where runtime arbitration happens.

## 3. Goals

### 3.1 Functional goals

The new overlay must:

- show collision AABBs / outlines without flicker,
- show local predicted ship and authoritative confirmed ghost clearly,
- show remote interpolated entities clearly,
- show velocity / angular state / input / correction state in a stable text panel,
- keep useful FPS/runtime diagnostics,
- support future debugging of:
  - prediction,
  - interpolation,
  - handoff,
  - visibility,
  - render-layer issues,
  - camera/runtime mismatch.

### 3.2 Non-functional goals

The new overlay must:

- avoid whole-world re-arbitration during the draw pass,
- avoid layout jitter in text,
- avoid depending on render culling state,
- avoid hidden dependence on camera follow heuristics,
- be cheap enough to leave available in normal native debugging sessions,
- be simple enough that future agents can inspect it without re-deriving the whole runtime.

## 4. Non-Goals

This reimplementation is not meant to:

- replace BRP,
- replace timestamped runtime logs,
- replace remote inspection,
- become a production HUD,
- solve prediction/interpolation by itself.

It is a debugging instrument. It must help diagnose runtime truth, not create new truth.

## 5. Core Design Decision

### 5.1 Replace "draw directly from live ECS queries" with "build a stable debug snapshot, then draw from it"

Instead of the draw system deciding which entity to render as it iterates all `WorldEntity` copies, the new design will:

1. collect logical debug candidates,
2. resolve one stable winner per logical GUID and lane,
3. write a `DebugOverlaySnapshot` resource,
4. render gizmos and text only from that snapshot.

This is the most important architectural change.

### 5.2 Why this is better

It separates:

- runtime state resolution,
- data formatting,
- rendering.

That gives us:

- easier reasoning,
- easier testing,
- less frame-to-frame arbitration churn,
- clearer ownership of control-target / predicted / confirmed / interpolated lanes.

## 6. Proposed Architecture

## 6.1 Resources

Introduce the following resources in `bins/sidereal-client/src/native/resources.rs` or a new dedicated debug module:

```rust
#[derive(Resource, Default)]
pub(crate) struct DebugOverlayState {
    pub enabled: bool,
    pub mode: DebugOverlayMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DebugOverlayMode {
    #[default]
    Minimal,
    Full,
}

#[derive(Resource, Default)]
pub(crate) struct DebugOverlaySnapshot {
    pub frame_index: u64,
    pub entities: Vec<DebugOverlayEntity>,
    pub controlled_lane: Option<DebugControlledLane>,
    pub stats: DebugOverlayStats,
    pub text_rows: Vec<DebugTextRow>,
}
```

### 6.1.1 Entity snapshot types

```rust
#[derive(Debug, Clone)]
pub(crate) struct DebugOverlayEntity {
    pub guid: uuid::Uuid,
    pub entity: Entity,
    pub lane: DebugEntityLane,
    pub position_xy: Vec2,
    pub rotation_rad: f32,
    pub velocity_xy: Vec2,
    pub angular_velocity_rps: f32,
    pub collision: DebugCollisionShape,
    pub visible: bool,
    pub is_controlled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DebugEntityLane {
    Predicted,
    Confirmed,
    Interpolated,
    ConfirmedGhost,
    Auxiliary,
}
```

### 6.1.2 Text model

```rust
#[derive(Debug, Clone)]
pub(crate) struct DebugTextRow {
    pub label: String,
    pub value: String,
    pub severity: DebugSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DebugSeverity {
    Normal,
    Warn,
    Error,
}
```

This split is important because text formatting must be stable and independent from runtime queries.

## 6.2 Systems

The new overlay should be split into these systems:

### 6.2.1 Input/toggle systems

- `toggle_debug_overlay_system`
- `cycle_debug_overlay_mode_system`

These only mutate `DebugOverlayState`.

### 6.2.2 Snapshot collection systems

- `collect_debug_overlay_candidates_system`
- `resolve_debug_overlay_winners_system`
- `build_debug_overlay_text_snapshot_system`

These should run in `PostUpdate`, after:

- `FrameInterpolationSystems::Interpolate`
- `RollbackSystems::VisualCorrection`
- any Sidereal transform recovery systems we intentionally keep

and before:

- `TransformSystems::Propagate` if using local `Transform`

or after propagation if using `GlobalTransform` only.

Recommended schedule:

```rust
PostUpdate:
  debug::collect_debug_overlay_snapshot_system
    .after(FrameInterpolationSystems::Interpolate)
    .after(RollbackSystems::VisualCorrection)
    .after(transforms::recover_stalled_interpolated_world_entity_transforms)
```

### 6.2.3 Rendering systems

- `draw_debug_overlay_gizmos_system`
- `update_debug_overlay_text_ui_system`

These should read only from `DebugOverlaySnapshot`.

The draw systems should not:

- resolve control targets,
- inspect `RuntimeEntityHierarchy`,
- choose winners,
- infer predicted-vs-confirmed policy.

That logic belongs in the snapshot stage only.

## 7. Logical Winner Selection Rules

This is the most important correctness section.

### 7.1 One logical winner per GUID per lane

For each gameplay GUID, the snapshot builder should resolve explicit lanes:

- `Predicted` winner
- `Confirmed` winner
- `Interpolated` winner
- optional `ConfirmedGhost` view for the locally controlled predicted entity

The overlay must not draw arbitrary live duplicates.

### 7.2 Explicit selection policy

For the local controlled GUID:

- prefer `Predicted` as the primary displayed lane,
- use `ConfirmedGhost` only from `Confirmed<T>` wrappers or the confirmed root if wrappers are absent,
- never draw the confirmed root as a second arbitrary "main" body.

For remote observer GUIDs:

- prefer `Interpolated`,
- fall back to confirmed only if interpolation is absent,
- never draw both unless the user explicitly enables a "show duplicates" diagnostic mode.

### 7.3 Sanity checks

The snapshot builder should record anomalies if:

- the same entity has both `Predicted` and `Interpolated`,
- a controlled entity is not `Predicted`,
- a remote observer entity is `Predicted`,
- a logical GUID resolves to multiple primary winners.

These anomalies should appear in the text panel and optionally logs.

## 8. Gizmo Rendering Plan

## 8.1 Separate gizmo groups by purpose

Define separate draw passes conceptually, even if they share Bevy gizmos:

- collision geometry
- confirmed ghost lane
- prediction error vector
- velocity arrows
- hardpoint markers
- optional visibility range circles

Each should have a consistent color contract.

Suggested color contract:

- predicted controlled body: cyan
- confirmed ghost: magenta
- prediction error vector: red
- interpolated observer: green
- confirmed-only fallback: amber
- hardpoints: yellow
- velocity arrow: blue

## 8.2 Draw only from immutable snapshot data

Example:

```rust
for entity in &snapshot.entities {
    match entity.lane {
        DebugEntityLane::Predicted => draw_predicted_body(...),
        DebugEntityLane::Interpolated => draw_interpolated_body(...),
        DebugEntityLane::Confirmed => draw_confirmed_body(...),
        DebugEntityLane::ConfirmedGhost => draw_confirmed_ghost(...),
        DebugEntityLane::Auxiliary => {}
    }
}
```

This keeps the draw pass predictable.

## 8.3 Avoid using runtime visibility as a winner filter

Do not use:

- `ViewVisibility`
- `InheritedVisibility`
- camera culling data

to choose overlay winners.

At most, `Visibility` may be represented as a displayed status field in the debug text.

The overlay must represent gameplay/runtime truth, not render culling truth.

## 9. Text Overlay Reimplementation

## 9.1 Replace ad hoc dynamic text with a fixed-grid panel

The current text overlay should be reintroduced as a structured panel with:

- fixed-width font,
- fixed label width,
- fixed value width where practical,
- preformatted numeric fields.

This is required because rapidly changing float strings cause visible column jitter and make runtime changes harder to read.

## 9.2 Layout rules

Use a monospaced face and fixed columns:

- left column: label
- right column: value

Recommended rendering rules:

- right-align numeric values,
- clamp precision,
- avoid full `Debug` dumps,
- use consistent units suffixes,
- use short stable labels.

Example rows:

```text
FPS            143
Frame ms        6.98
Mode      predicted
Ctrl GUID 57c26097…
Pred Pos   399.78,1066.90
Conf Pos   399.74,1066.88
Err m         0.045
Rot rad      -2.619
Conf rad     -2.621
Err rad       0.002
Vel m/s     220.00,  0.00
Ang r/s       0.000
Rollback         no
VisCorr         yes
Interp cnt        2
Pred cnt          2
Dup GUIDs         0
```

## 9.3 Numeric formatting policy

To reduce jitter:

- positions: `{:>8.2}`
- velocities: `{:>7.2}`
- small error magnitudes: `{:>7.3}`
- booleans: `yes` / `no`
- counts: `{:>4}`

Do not emit raw long `Vec2(...)` / `Quat(...)` debug strings into the panel.

## 9.4 Useful future-proof text fields

Reintroduce FPS, but also include higher-value runtime fields:

- FPS
- frame time ms
- predicted / interpolated / replicated counts
- controlled GUID
- controlled runtime entity id
- confirmed tick
- rollback active
- visual correction active
- correction magnitude
- input send tick
- latest input ack age
- nearby collision proxy count
- duplicate GUID group count
- active camera count on default layer
- current control request pending state

## 10. Data Collection Policy

The debug overlay should only use stable data sources.

### 10.1 Preferred sources

For local controlled entity:

- `Position`
- `Rotation`
- `LinearVelocity`
- `AngularVelocity`
- `Confirmed<T>`
- `ConfirmedTick`
- `PredictionHistory<T>` presence
- `VisualCorrection`

For remote observer entities:

- `Position`
- `Rotation`
- `LinearVelocity`
- `ConfirmedHistory<T>` presence
- `Interpolated`

### 10.2 Avoid for primary truth

Do not use these as the main source of debug truth:

- `RuntimeEntityHierarchy` alone
- `ViewVisibility`
- child visual transforms
- derived VFX transforms
- UI camera state

These may be useful as additional diagnostics, but not as the primary overlay source.

## 11. Camera / Layer / Rendering Integration

## 11.1 Debug camera policy

Per [bevy_2d_rendering_optimization_audit_report_2026-03-09.md](/home/toby/dev/sidereal_v3/docs/reports/bevy_2d_rendering_optimization_audit_report_2026-03-09.md), the debug overlay should not force extra render cost when inactive.

Recommended policy:

- if overlay is disabled:
  - disable debug overlay camera entirely
  - disable debug text UI entities
- if overlay is enabled:
  - mirror gameplay camera transform/projection once per frame
  - keep debug gizmos on a dedicated render layer

## 11.2 Single draw point

The gizmo rendering system should only be scheduled once. Do not have multiple systems drawing overlapping collision overlays on different schedules or cameras.

## 11.3 Explicit audit for active cameras

Keep a diagnostic row:

- `World cams`
- `Dbg cam`

and warn if there are multiple active default-layer cameras unexpectedly.

## 12. Proposed Module Split

Instead of keeping everything in one file, split into:

- `debug/mod.rs`
- `debug/state.rs`
- `debug/snapshot.rs`
- `debug/gizmos.rs`
- `debug/text_panel.rs`
- `debug/metrics.rs`

Suggested ownership:

- `state.rs`: resources and toggle state
- `snapshot.rs`: ECS queries and stable snapshot build
- `gizmos.rs`: gizmo draw only
- `text_panel.rs`: UI nodes + fixed-width text formatting
- `metrics.rs`: runtime counters and formatting helpers

This avoids continuing monolithic growth in [debug_overlay.rs](/home/toby/dev/sidereal_v3/bins/sidereal-client/src/native/debug_overlay.rs).

## 13. Rollout Plan

### Phase 1: Build snapshot-only minimal overlay

Implement:

- toggle state
- snapshot resource
- collision AABB/outline draw from snapshot only
- local predicted + confirmed ghost lanes
- no text panel yet

Success criteria:

- no visible flicker from duplicate/winner churn
- no dependency on `ViewVisibility`

### Phase 2: Add stable text panel

Implement:

- fixed-width font/text panel
- FPS + frame time
- controlled-entity state rows
- prediction/interpolation counts
- correction/rollback rows

Success criteria:

- no visible column jitter during rapid numeric change
- no text-induced alignment churn

### Phase 3: Add anomaly reporting and future-proof diagnostics

Implement rows and/or warnings for:

- duplicate GUID groups
- `Predicted + Interpolated` conflicts
- controlled entity missing `Predicted`
- active default-layer camera anomalies
- nearby collision proxy counts
- control-request pending state

### Phase 4: Remove legacy overlay

Delete or archive old systems:

- current draw arbitration path
- old HUD FPS/debug text entities if superseded
- any duplicate debug-only ad hoc state no longer needed

## 14. Testing Strategy

### 14.1 Manual tests

1. Single client, free-roam only:
   - F3 toggles once per press
   - overlay stays stable on static scene

2. Single client, controlled ship:
   - predicted AABB stable
   - confirmed ghost stable
   - velocity arrow stable
   - rotation display stable

3. Dual client:
   - client 1 sees client 2 as interpolated only
   - client 2 sees client 1 immediately on spawn
   - no observer origin flash

4. Dynamic handoff:
   - local target lane switches cleanly
   - no duplicate overlay for same GUID

5. Flat background:
   - ship/gizmo motion can be judged without parallax references

### 14.2 BRP cross-checks

Use [brp_debugging_workflow.md](/home/toby/dev/sidereal_v3/docs/features/brp_debugging_workflow.md) to verify:

- the overlay’s chosen controlled/predicted/interpolated winners match actual runtime components
- no entity drawn as local predicted still has stale `Interpolated`

### 14.3 Code-level tests

Add unit tests for:

- winner selection logic
- numeric formatting helpers
- lane conflict resolution
- text-row formatting width stability

Potential examples:

```rust
#[test]
fn local_control_winner_prefers_predicted_over_confirmed() { ... }

#[test]
fn observer_winner_prefers_interpolated_over_confirmed() { ... }

#[test]
fn formatted_numeric_rows_keep_fixed_width() { ... }
```

## 15. Risks

### 15.1 Risk: overlay masks runtime bug instead of exposing it

Mitigation:

- preserve anomaly rows instead of silently suppressing bad states
- log explicit warnings for impossible runtime combinations

### 15.2 Risk: new overlay becomes another monolith

Mitigation:

- split into submodules from the start
- separate snapshot build from rendering

### 15.3 Risk: text panel becomes noisy

Mitigation:

- define `Minimal` and `Full` modes
- keep most rows opt-in under `Full`

## 16. Success Criteria

The reimplementation is successful when:

- collision AABBs and velocity arrows no longer visibly flicker in stable scenarios,
- F3 overlay can stay enabled during normal debugging without hurting comprehension,
- predicted vs confirmed vs interpolated lanes are obvious,
- dual-client interpolation and handoff debugging become easier, not harder,
- text remains readable under rapid float changes,
- future agents can inspect one doc and one module split instead of reverse-engineering many overlay hacks.

## 17. Recommended First Implementation Cut

If implementation starts immediately, the recommended first cut is:

1. create `DebugOverlaySnapshot`,
2. port collision AABB/outline drawing onto snapshot data only,
3. add winner selection unit tests,
4. add fixed-width `FPS / frame time / controlled state / prediction counts` text panel,
5. remove old dynamic draw arbitration from `draw_debug_overlay_system`.

That is the minimum slice that gives high value quickly while reducing the current debugging surface area.
