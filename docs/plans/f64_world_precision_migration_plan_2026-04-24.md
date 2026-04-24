# f64 World Precision Migration Plan

Status: Active implementation plan
Last updated: 2026-04-24
Owners: gameplay simulation + replication + client runtime + dashboard
Scope: authoritative world-space precision, persistence, replication protocol, client render projection, scripting, and dashboard tooling

## 0. Implementation Status

2026-04-24 initial status note, superseded by the implementation update below:

1. This plan records the active implementation direction for moving Sidereal from f32 absolute world coordinates to f64 authoritative world coordinates.
2. Current runtime before this plan still uses `avian2d` with `f32` / `parry-f32`; `WorldPosition` is still `Vec2`; many protocol/dashboard read models still expose world-space coordinates as f32 arrays.
3. Implementation is intentionally breaking for local/dev graph data. Existing local/dev databases must be reset after the migration. Do not add compatibility aliases, f32 backfills, or dual coordinate payload paths.
4. Native impact: active motion, visibility, tactical contacts, VFX/audio origins, and render projection are affected. WASM impact: shared client/runtime code must compile with the same f64 authoritative coordinate model; platform differences remain at transport/render boundaries only.

2026-04-24 implementation update:

1. The first implementation pass is complete for the core f64 contract: Avian physics uses f64-backed vectors, `WorldPosition` uses f64-backed coordinates, `WorldRotation` is f64 radians, tactical/owner/notification protocol world positions use f64 arrays, scripting snapshots expose f64 world positions, client render/audio/UI systems project from f64 world data to f32 presentation data, and dashboard loaders preserve JSON numbers as TypeScript `number`.
2. The migration is not fully feature-complete until remaining f32 JSON helper surfaces are audited or narrowed to presentation-only use, explicit large-coordinate dashboard/persistence regression tests are added, and a reset-database runtime smoke verifies BRP, visibility, tactical contacts, owner manifests, discovery notifications, VFX/audio origins, and dashboard editing at a large coordinate.
3. `AGENTS.md` now treats f64 authoritative world coordinates as a contributor rule. Future work must not reintroduce authoritative f32 world positions or dual f32/f64 protocol payload paths.
4. Native/WASM impact: the shared code compiles with the f64 authoritative lane for both native and WASM target checks. Live browser validation remains separate follow-up while native stabilization remains the delivery priority.

2026-04-24 hardening update:

1. A dedicated Phase 8 has been added to turn the first implementation pass into a repo-wide f64 compliance hardening effort.
2. The hardening target is "64-bit authoritative world-position compliant across the board where practically possible": authoritative persistence, hydration, simulation, replication/read models, scripting, BRP, and dashboard data boundaries must preserve f64 precision; f32 remains allowed only for local gameplay magnitudes and final presentation projections.
3. The hardening work must classify every remaining `Vec2`/`Vec3`/`[f32; N]` world-position use as either migrated to f64 or documented as a presentation-only/local-magnitude exception with tests or clear ownership.

## 1. Goal

Make all authoritative world-space positions and velocities f64 while keeping Bevy render transforms, UI projection, shader uniforms, and small gameplay tunables in the narrower types that fit their domains.

Canonical lanes:

1. Authoritative motion lane:
   - Avian `Position` and `LinearVelocity` use f64-backed `DVec2`.
   - Avian scalar values use the scalar type produced by the `f64` feature.
2. Static non-physics spatial lane:
   - `WorldPosition` uses `DVec2`.
   - `WorldRotation` remains radians and may stay f32 unless the implementation requires f64 for type consistency.
3. Render/UI lane:
   - Bevy `Transform` and `GlobalTransform` remain f32.
   - Client world rendering derives camera-relative f32 transforms from f64 authoritative coordinates.
4. Dashboard lane:
   - TypeScript `number` and JSON numbers are the accepted f64 coordinate representation for dashboard editing/display.

## 2. Decisions

1. Replace workspace `avian2d` features `f32` and `parry-f32` with `f64` and `parry-f64`.
2. Bump `LIGHTYEAR_PROTOCOL_VERSION`.
3. Widen all protocol/read-model fields that carry absolute world positions or velocities to f64 arrays.
4. Keep small-magnitude gameplay values as f32 unless they are directly constrained by Avian f64 scalar APIs.
5. Do not introduce sector wrapping, origin rebasing, or dual coordinate topologies as part of this migration.
6. Use spatial grid/sector keys only as derived indexing and residency data, not as the canonical position model.

## 2.1 f64 Compliance Definition

An implementation is f64 world-position compliant when all authoritative world-space positions, velocities, origins, anchors, and absolute spatial query inputs satisfy these rules:

1. **Authoritative ECS:** simulated bodies use f64-backed Avian `Position` / `LinearVelocity`; static non-physics bodies use `WorldPosition(DVec2)` and `WorldRotation(f64)`.
2. **Persistence and hydration:** graph records, component reflection serialization, AGE property writes, AGE readback, and component hydration preserve f64 JSON numbers for authoritative world-space components and do not route them through `Vec3`, `[f32; 3]`, or other f32 helpers.
3. **Server read models:** visibility anchors, spatial-grid keys, tactical contacts, owner manifests, notification payloads, combat/VFX/audio origins, scripting snapshots, BRP/debug read models, and TUI/debug snapshots retain f64 until the final display boundary.
4. **Client runtime:** prediction/reconciliation and replicated world data retain f64. Bevy `Transform`, `GlobalTransform`, debug shapes, shader uniforms, audio backend positions, HUD/canvas coordinates, and map pixels are f32 only after subtracting the active camera/map origin or otherwise projecting to a small local coordinate space.
5. **Dashboard/frontend:** route loaders, BRP parsers, Zod schemas, explorer state, mutation payloads, and coordinate editors use JSON numbers / TypeScript `number` without rounding, integer coercion, stringification, or f32-style truncation. Canvas/WebGL drawing may project to display-local numbers.
6. **Protocol:** wire/read-model fields carrying absolute world positions or velocities use f64 arrays or f64-backed serialized component payloads. Do not add temporary dual f32/f64 payload paths.
7. **Practical f32 exceptions:** gameplay magnitudes such as sizes, ranges, local offsets, masses, thrust tunables, colors, shader parameters, UI dimensions, local hardpoint offsets, local collider extents, and presentation-only transforms may remain f32 when they do not encode absolute world coordinates.

## 3. Implementation Phases

### Phase 1: Dependency and Compile Surface

1. Update root `Cargo.toml`:

   ```toml
   avian2d = { version = "0.5.0", default-features = false, features = ["2d", "f64", "parry-f64", "parallel", "serialize", "xpbd_joints"] }
   ```

2. Run `cargo check --workspace` and use compiler errors to identify remaining f32 assumptions.
3. Convert Avian-adjacent `Vec2` construction and scalar math to `DVec2` / f64 where it feeds `Position`, `LinearVelocity`, `Forces`, rollback thresholds, interpolation, or physics math.

### Phase 2: Core Spatial Types

1. Change `WorldPosition(pub Vec2)` to `WorldPosition(pub DVec2)`.
2. Change `resolve_world_position` to return `Option<DVec2>`.
3. Add shared explicit conversion helpers for:
   - f64 authoritative world position to f32 render-local offset,
   - f64 authoritative world position to server/debug transform fallback,
   - f32 UI/render coordinates to f64 world coordinates where needed.
4. Update component registry/editor schema inference so `DVec2`, `DVec3`, and `DVec4` map to the existing vector editor kinds.

### Phase 3: Persistence and Content

1. Add f64 JSON parsing helpers in `sidereal-runtime-sync`.
2. Stop truncating authoritative persisted positions/velocities from f64 to f32.
3. Preserve Lua numeric array payloads; Lua numbers remain valid f64 source values.
4. Reset local/dev graph data after implementation.

### Phase 4: Server Runtime

1. Update hydration, admin spawn, and runtime scripting code to write/read f64 coordinates.
2. Update visibility caches and observer anchors to use `DVec2`.
3. Keep visibility cell keys as `i64` and compute them from f64 coordinates.
4. Update tactical contacts, fog read models, owner manifests, health/TUI/debug snapshots, notifications, VFX, and audio origin messages.
5. Ensure server gameplay prefers Avian `Position` / `WorldPosition` over f32 `GlobalTransform` whenever authoritative coordinates are available.

### Phase 5: Client Runtime

1. Store camera authoritative world position as `DVec2`.
2. Derive Bevy world-entity `Transform.translation.xy` from `entity_world_xy - camera_world_xy`, then cast the local offset to f32.
3. Update tactical UI, nameplates, VFX, audio, lighting, planet visuals, debug overlay, and post-process code to use f64 world data until final f32 render/UI projection.
4. Rebuild dashboard-embedded game-client WASM after the client compiles.

### Phase 6: Dashboard

1. Keep dashboard coordinate values as TypeScript `number`.
2. Keep Zod validation finite-number based.
3. Update graph/explorer/component editor utilities to accept and preserve f64 coordinate arrays.
4. Add tests for large coordinate values such as `[5_000_000_000_000, -5_000_000_000_000]`.
5. Follow `docs/frontend_ui_styling_guide.md`; do not introduce new visual patterns for coordinate editors.

### Phase 7: Documentation

1. [x] Update `docs/features/galaxy_world_structure.md`.
2. [x] Update `docs/features/visibility_replication_contract.md`.
3. [x] Update `docs/features/tactical_and_owner_lane_protocol_contract.md`.
4. [x] Update `docs/features/scripting_support.md`.
5. [x] Add `docs/decisions/dr-0035_f64_world_coordinates.md`.
6. [x] Add DR-0035 to `docs/decision_register.md`.
7. [x] Update `AGENTS.md` after implementation if the active contributor contract changes from "planned f64" to "required f64".

### Phase 8: f64 Compliance Hardening

1. Audit every remaining world-position-adjacent `Vec2`, `Vec3`, `[f32; 2]`, `[f32; 3]`, `as f32`, and TypeScript coordinate conversion in:
   - `crates/sidereal-runtime-sync`
   - `crates/sidereal-persistence`
   - `crates/sidereal-net`
   - `crates/sidereal-game`
   - `bins/sidereal-replication`
   - `bins/sidereal-client`
   - `bins/sidereal-gateway`
   - `dashboard/src`
2. Replace authoritative JSON parsing helpers with f64/DVec helpers:
   - add `parse_dvec2_value`, `parse_dvec3_value`, `value_as_dvec2_recursive`, and `value_as_f64_recursive` where needed;
   - deprecate or rename existing `parse_vec3_value`, `value_as_vec3_recursive`, and gateway `[f32; 3]` parsers so they are clearly presentation-only;
   - update call sites so authoritative hydration and read models never depend on f32 helper output.
3. Add persistence round-trip tests:
   - reflect-serialize and hydrate `WorldPosition(DVec2)` and `WorldRotation(f64)` with coordinates such as `[5_000_000_000_000.25, -5_000_000_000_000.5]`;
   - write/read graph AGE records containing f64 world positions through `GraphPersistence` when local AGE is available;
   - verify loaded JSON values preserve f64 values within an explicit f64 epsilon and are not truncated to f32-scale spacing.
4. Add server/runtime large-coordinate tests:
   - visibility cell-key generation at large positive and negative f64 coordinates;
   - observer anchor and visibility-source collection using Avian `Position` / `WorldPosition` without f32 fallback;
   - tactical contacts, owner manifests, notifications, combat events, VFX/audio origins, scripting snapshots, and BRP/debug read models.
5. Add client projection tests:
   - camera-relative transform projection from large f64 world coordinates to small f32 Bevy translations;
   - nameplate/HUD/map projection from f64 world data;
   - audio/VFX origins convert to f32 only after local projection or backend boundary conversion.
6. Add dashboard tests:
   - BRP parser preserves large coordinate JSON numbers;
   - explorer/component editor state preserves large coordinate arrays;
   - mutation payloads submit finite TypeScript `number` values without rounding/stringification;
   - canvas/grid rendering projects large world coordinates through a local origin before drawing.
7. Update docs after hardening:
   - mark this plan's status as `Implemented with ongoing compliance guardrails` when acceptance criteria pass;
   - update `docs/features/galaxy_world_structure.md` to remove the hardening caveat;
   - keep `AGENTS.md` rules aligned with any refined exception list.
8. Run final compliance gates:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo check --workspace`
   - `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
   - `cargo check -p sidereal-client --target x86_64-pc-windows-gnu`
   - targeted Rust tests for touched crates
   - dashboard lint/test/build for touched frontend paths
   - reset-database large-coordinate runtime smoke

## 4. Validation

Minimum Rust gates:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

Targeted tests:

```bash
cargo test -p sidereal-game
cargo test -p sidereal-runtime-sync
cargo test -p sidereal-net
cargo test -p sidereal-replication
cargo test -p sidereal-client
cargo test -p sidereal-gateway
```

Dashboard gates:

```bash
pnpm --dir dashboard test
pnpm --dir dashboard lint
pnpm --dir dashboard format
pnpm --dir dashboard build
```

Manual smoke:

1. Reset local/dev graph data.
2. Bootstrap starter world.
3. Spawn or teleport an entity near `x = 5_000_000_000_000`, `y = -5_000_000_000_000`.
4. Verify server BRP exposes f64 Avian/`WorldPosition` values.
5. Verify the client renders nearby entities camera-relative without visible jitter.
6. Verify visibility, tactical contacts, owner manifest, discovery notifications, VFX/audio origins, and dashboard graph editing work at the large coordinate.

Hardening acceptance criteria:

1. `rg "as f32|Vec2|Vec3|\\[f32" crates bins dashboard/src` has no unclassified authoritative world-coordinate use. Remaining matches are either local magnitudes, presentation projections, or explicitly documented helper boundaries.
2. Large-coordinate values round trip through graph persistence, registered component hydration, replication/read models, scripting snapshots, BRP, and dashboard parsing without f32-scale precision loss.
3. Client render/UI/audio/VFX code only downcasts after subtracting a camera/map/local origin or at a documented presentation/backend boundary.
4. No dual f32/f64 protocol payload paths or migration shims are introduced.
