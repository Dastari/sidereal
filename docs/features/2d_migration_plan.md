# 2D Migration Plan (Avian2D + Top-Down Camera + Sprites)

Status: Draft migration plan  
Owners: gameplay runtime, replication, client runtime, asset pipeline  
Scope: Whole-project migration from 3D runtime/rendering to authoritative 2D runtime/rendering

## 1. Goal

Migrate Sidereal v3 from Avian3D + 3D camera + GLTF visuals to:

- Avian2D authoritative simulation/prediction runtime,
- top-down orthographic gameplay camera,
- sprite-based visual pipeline (GLTF removed from runtime paths),
- unchanged core authority/identity/persistence/replication invariants.

This is a runtime and content-pipeline migration, not a change to authority model or service boundaries.

## 2. Non-Negotiable Invariants (Carry Forward)

The following repository contracts remain unchanged during and after migration:

- one-way authority flow (`client input -> shard sim -> replication/distribution -> persistence`),
- authenticated input identity binding and reject-on-mismatch behavior,
- player runtime/progression state persisted on player ECS entity/components,
- UUID/entity-id boundary contract (no Bevy entity ids over service boundaries),
- graph-record persistence/hydration shape and deterministic relationship rebuild,
- shared gameplay logic across client/server/WASM,
- fixed-step simulation/prediction math (no frame-time authority math).

## 3. Phased Execution Plan

## Phase 0: Design Freeze and Contract Updates

Deliverables:

- Define 2D target architecture and migration constraints in docs.
- Add decision record (`dr-XXXX`) covering:
  - Avian2D adoption,
  - top-down camera contract,
  - GLTF removal and sprite replacement,
  - persistence/protocol compatibility strategy.
- Update:
  - `docs/sidereal_design_document.md`,
  - `docs/features/asset_delivery_contract.md`,
  - `docs/features/visibility_replication_contract.md` if affected.

Acceptance criteria:

- Design docs explicitly describe 2D runtime model and removed 3D assumptions.
- Migration sequencing and rollback strategy documented.

## Phase 1: Shared Runtime Abstractions and Dependency Wiring

Deliverables:

- Introduce Avian2D dependency wiring in touched crates.
- Add shared 2D gameplay motion interfaces/components in `crates/sidereal-game`.
- Keep existing runtime compiling while introducing 2D migration scaffolding.

Acceptance criteria:

- Workspace compiles with new 2D runtime abstractions in place.
- No duplication of gameplay logic across client/server for 2D behavior.

## Phase 2: Server Authoritative Simulation Migration (3D -> 2D)

Deliverables:

- Replace Avian3D authoritative components/systems with Avian2D equivalents in replication simulation paths.
- Convert movement/force/torque systems to 2D vector/scalar math.
- Ensure mass/inertia derivation remains synchronized with physics runtime.

Acceptance criteria:

- Server sim runs on Avian2D with equivalent gameplay intent behavior.
- Motion ownership and single-writer guarantees preserved.
- Unit/integration coverage exists for migrated motion and control flows.

## Phase 3: Client Prediction/Reconciliation Migration

Deliverables:

- Move client predicted/interpolated entity motion to Avian2D.
- Retain existing control ack/reject and pending-intent ownership rules.
- Preserve fixed-tick prediction and reconciliation contracts.

Acceptance criteria:

- Predicted local control works in 2D with reconciliation stability.
- Remote interpolation behavior remains smooth and deterministic.

## Phase 4: Camera Migration to Top-Down 2D

Deliverables:

- Replace gameplay `Camera3d` runtime path with orthographic top-down camera.
- Preserve camera follow/free-camera semantics and control-state coupling.
- Keep resize and viewport safety behavior robust under minimize/restore/rescale.

Acceptance criteria:

- In-world camera behavior is fully 2D top-down.
- HUD/overlay and gameplay layering remain correct.

## Phase 5: Visual Asset Pipeline Migration (GLTF -> Sprites)

Deliverables:

- Remove GLTF-backed runtime visuals from client runtime paths.
- Introduce sprite-based visual representation components and loaders.
- Update stream/caching contracts for sprite assets, atlases, and metadata.
- Replace model attach/sync systems with sprite attach/sync systems.

Acceptance criteria:

- Gameplay entities render via sprites only.
- Asset streaming/caching remains deterministic and versioned.
- No runtime GLTF dependency in client visual path.

## Phase 6: Persistence/Hydration and Data Migration

Deliverables:

- Update persistence/hydration mapping for 2D runtime state.
- Define migration behavior for existing 3D persisted transform data.
- Preserve relationship/hierarchy deterministic reconstruction.

Acceptance criteria:

- Hydration roundtrip tests pass for 2D motion/transform components.
- Existing persisted worlds can migrate without identity/relationship corruption.

## Phase 7: Protocol and Replication Contract Finalization

Deliverables:

- Remove obsolete 3D-specific protocol/schema fields where applicable.
- Version and document any wire-contract changes.
- Reconfirm visibility/redaction contracts remain entity-generic.

Acceptance criteria:

- Client/server protocol compatibility documented and tested.
- Replication delivery/redaction behavior unchanged except dimensionality changes.

## Phase 8: Cleanup and Deletion Pass

Deliverables:

- Delete remaining Avian3D and GLTF runtime code paths.
- Remove dead compatibility shims used only during migration.
- Ensure entrypoints remain wiring-focused and avoid new monolithic growth.

Acceptance criteria:

- No production runtime path depends on Avian3D or GLTF rendering.
- Codebase compiles clean with only 2D runtime/render assumptions.

## 4. Test and Validation Gates (Per Phase)

Required baseline checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
```

Client parity checks (required whenever client/runtime contracts are touched):

```bash
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

Additional expectations:

- Targeted unit tests for each touched crate.
- Integration tests when replication/persistence/protocol boundaries change.
- Hydration roundtrip coverage for newly persistable 2D components.

## 5. Risk Register and Mitigations

Risk: prediction/regression instability during 3D-to-2D motion conversion.  
Mitigation: migrate server sim first, then client prediction; add deterministic motion tests before cleanup.

Risk: protocol drift during incremental migration.  
Mitigation: version message/schema changes and document each in the same change.

Risk: persistence compatibility breaks for existing 3D worlds.  
Mitigation: explicit migration logic + hydration validation fixtures in CI.

Risk: WASM parity drift while native migration proceeds.  
Mitigation: run WASM checks every phase and keep shared gameplay logic ungated by platform features.

## 6. Completion Definition

Migration is complete when:

- authoritative runtime uses Avian2D end-to-end,
- gameplay camera is top-down orthographic 2D,
- runtime visuals are sprite-only (GLTF removed from runtime paths),
- docs/contracts/tests are updated and passing,
- native and WASM client targets continue to build successfully.

