# Bevy 2D Rendering Optimization Audit Report - 2026-04-23

Status note 2026-04-23: Fresh audit run from current repository state. Existing audit reports were intentionally ignored as evidence. This pass used `docs/prompts/bevy_2d_rendering_optimization_audit_prompt.md`, Bevy source-of-truth docs, and the local `bevy-game-engine` skill.

## Scope

Reviewed the native client 2D render/presentation path, streamed visuals, tactical map, nameplates, fullscreen backdrop/post-process systems, asset bootstrap/runtime fetches, camera/motion integration, and relevant Bevy 0.18 context.

Primary external references checked during this run:

- Bevy 0.18 release notes: fullscreen materials and cargo feature collections are first-class in 0.18.
- Bevy 0.17 to 0.18 migration guide: feature collections and render/asset API changes are current migration concerns.
- Lightyear documentation: prediction and interpolation require shared fixed-update simulation for predicted entities and interpolation for non-controlled entities.

## Executive Summary

The client already uses many appropriate Bevy patterns: state-scoped entities, pooled weapon effects, material handle reuse tests for post-process renderables, runtime asset prioritization, and GPU-backed fullscreen/tactical effects. The biggest visible performance and correctness risk is that rendering is downstream of replication/prediction role transitions. This area was substantially refactored on March 22 with explicit control bootstrap state, generation-aware control acks, narrower rearm, and narrower transform repair. The remaining Bevy concern is that duplicate visual suppression, delayed predicted adoption, and presentation readiness gates are still observable containment paths, so render symptoms can still come from networking lifecycle rather than GPU cost.

The second major risk is CPU-side UI/visual maintenance: tactical map marker upserts, nameplate sync/projection, fullscreen material uniform updates, and large world-visual systems scan or rebuild sizeable structures every frame/update.

## Findings

### Critical - Presentation Stability Still Exposes Remaining Handoff Repair Paths

Evidence:

- Duplicate predicted/interpolated visuals are detected and suppressed by GUID grouping in a world-level system (`bins/sidereal-client/src/runtime/visuals.rs:531`).
- The dirty tracking watches role markers, Avian motion, world-space components, and confirmed histories (`bins/sidereal-client/src/runtime/visuals.rs:635`).
- Client replication separately sanitizes entities that carry both `Predicted` and `Interpolated` (`bins/sidereal-client/src/runtime/replication.rs:1390`).
- Motion ownership refuses to bind a non-predicted non-player target as the local writer (`bins/sidereal-client/src/runtime/motion.rs:201`).

Impact:

Frame hitches, flicker, origin flashes, or jerky motion may be caused by remaining role-transition containment rather than GPU cost. The March 22 refactor reduced this by making control binding explicit and narrowing transform recovery; the remaining issue is proving that duplicate suppression and adoption deferral go quiet in normal play.

Recommendation:

Instrument handoff phases and render readiness separately from draw/update costs. Add telemetry counters for duplicate groups, suppressed entities, pending predicted adoption age, visual-ready deferrals, and role-marker conflicts. Treat sustained non-zero values during normal two-client sessions as lifecycle bugs, not rendering optimization opportunities.

### High - `visuals.rs` Is A Mixed Render Runtime

Evidence:

- The file is 3,349 lines and contains streamed sprite caches, duplicate resolution, planet visual stacks, weapon tracer pools, impact effects, projectile visuals, and tests.
- Planet visual attachment/cleanup spawns and removes child pass entities based on runtime visual stacks (`bins/sidereal-client/src/runtime/visuals.rs:1800`, `bins/sidereal-client/src/runtime/visuals.rs:1855`).
- Weapon tracer visuals perform ray casts and per-bolt material updates every frame (`bins/sidereal-client/src/runtime/visuals.rs:3142`).

Impact:

This makes profiling hard and increases the chance that a fix for one visual family affects another. Large systems also make Bevy schedule ordering and query conflicts harder to reason about.

Recommendation:

Split into modules/plugins for duplicate presentation, streamed sprites, planet/celestial visuals, projectile/tracer effects, and pooled impact effects. Keep each plugin’s systems narrow enough to profile independently.

### High - Tactical Map Overlay Does Too Much Per Update

Evidence:

- `update_tactical_map_overlay_system` handles camera activation, HUD visibility, cursor text, smoothing, marker cache rebuilds, marker spawn/update/despawn, SVG resolution, and tactical defaults in one system (`bins/sidereal-client/src/runtime/ui.rs:657`).
- The tactical fog overlay updates material uniforms and may rebuild the fog mask texture when camera/zoom/fog state changes (`bins/sidereal-client/src/runtime/ui.rs:1389`, `bins/sidereal-client/src/runtime/ui.rs:1477`).

Impact:

The tactical map is a likely CPU spike source when entering/exiting map mode or when many contacts change. The fog mask rebuild condition includes camera center and zoom, so panning/zooming can drive repeated CPU texture writes.

Recommendation:

Separate tactical overlay into input/camera, marker diffing, marker presentation, fog texture build, and material uniform systems. Throttle fog-mask rebuilds during continuous pan/zoom or move more work to shader-side math.

### High - Nameplates Scale With All Visible Health Entities

Evidence:

- Nameplate sync collects all `WorldEntity + CanonicalPresentationEntity + HealthPool` targets, sorts by `Entity::to_bits`, builds a target set, and allocates/spawns pooled entries as needed (`bins/sidereal-client/src/runtime/ui.rs:2019`).
- Position updates project every active nameplate each frame and query target transforms/health (`bins/sidereal-client/src/runtime/ui.rs:2105`).

Impact:

This is fine for small scenes but can become costly in MMO-scale visibility ranges. It also ties UI load directly to replicated entity count.

Recommendation:

Cull nameplate targets by distance/screen bounds before registry activation. Consider a cap, priority ordering, and lower cadence health fill updates separate from per-frame screen position.

### Medium - Fullscreen/Post-Process Systems Are Directionally Good But Still Broad

Evidence:

- Fullscreen renderables reuse a shared fullscreen mesh and stable material handles in tests (`bins/sidereal-client/src/runtime/backdrop.rs:351`, `bins/sidereal-client/src/runtime/backdrop.rs:2007`).
- Backdrop transform/camera sync runs over fullscreen/backdrop entities and resets camera state (`bins/sidereal-client/src/runtime/backdrop.rs:721`, `bins/sidereal-client/src/runtime/backdrop.rs:751`).
- Starfield/space background material systems update uniforms every frame (`bins/sidereal-client/src/runtime/backdrop.rs:1811`, `bins/sidereal-client/src/runtime/backdrop.rs:1863`).

Impact:

The design aligns with Bevy 0.18’s fullscreen material direction, but the systems still update all matching materials each frame. This is acceptable at low counts, but needs profiling once authored shader layers grow.

Recommendation:

Keep the stable-handle tests. Add counters for fullscreen renderable count, material update count, and material rebind count in normal runtime logs/debug overlay.

### Medium - Runtime Asset Prioritization Is Good But Dependency Scans Need Bounds

Evidence:

- Asset bootstrap state tracks startup/bootstrap phase readiness (`bins/sidereal-client/src/runtime/assets.rs:50`).
- Runtime asset fetch candidates prioritize critical shader assets, root visual assets, immediate dependencies, then lower-value assets (`bins/sidereal-client/src/runtime/assets.rs:969`).
- Gateway serves startup assets publicly and authenticated bootstrap/runtime assets via `/assets/<asset_guid>` (`bins/sidereal-gateway/src/api.rs:378`, `bins/sidereal-gateway/src/api.rs:482`).

Impact:

The asset lane matches the architecture, but large catalogs can make dependency expansion and candidate selection a CPU-side concern if marked dirty too often.

Recommendation:

Expose dependency scan/rebuild counters in the same HUD/perf surface as render counters. Add a stress test with many visual assets and dependencies.

## Strengths

- Bevy app states follow the documented loading flow (`bins/sidereal-client/src/runtime/app_state.rs:8`).
- Bootstrap watchdog errors use persistent dialogs for important failures (`bins/sidereal-client/src/runtime/bootstrap.rs:38`).
- Weapon effects use fixed-size pools (`bins/sidereal-client/src/runtime/visuals.rs:55`).
- Backdrop/post-process has tests proving stable entity/material reuse (`bins/sidereal-client/src/runtime/backdrop.rs:2007`).

## Priority Actions

1. Add runtime presentation telemetry for role conflicts, duplicate suppression, and predicted-adoption waits.
2. Split `visuals.rs` and `ui.rs` into domain plugins.
3. Add tactical map fog-mask rebuild counters and throttle rebuilds during continuous camera movement.
4. Add nameplate distance/screen priority culling before registry activation.
5. Profile fullscreen material update counts after authored shader layers increase.

## Validation

No cargo quality gates or Playwright/canvas checks were run for this docs-only audit generation. The report is based on static inspection plus current Bevy/Lightyear documentation review.
