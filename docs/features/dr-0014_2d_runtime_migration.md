# DR-0014: Project-Wide Migration to Server-Authoritative 2D Runtime (Avian2D + Sprites)

Status: Proposed  
Date: 2026-02-25  
Owners: Gameplay runtime + replication + client runtime + asset pipeline

## 1. Context

Sidereal v3 currently runs on a 3D gameplay/runtime stack (Avian3D + 3D camera/render assumptions).  
We want the project to operate as a top-down 2D multiplayer architecture while preserving existing authority, identity, and persistence invariants.

Specific migration goals:

- replace Avian3D runtime usage with Avian2D,
- replace gameplay `Camera3d` path with top-down orthographic 2D camera behavior,
- remove GLTF runtime visuals and use sprites/atlases instead,
- keep native/WASM behavior parity through migration.

## 2. Decision

Adopt a phased whole-project migration to 2D:

1. Authoritative simulation and prediction move to Avian2D.
2. Gameplay camera moves to top-down orthographic 2D.
3. Runtime visuals move from GLTF model rendering to sprite rendering.
4. Persistence/hydration/protocol contracts are updated where dimensionality changes require it.
5. 3D runtime paths are removed after migration completeness gates pass.

The migration is governed by `docs/features/2d_migration_plan.md`.

## 3. Non-Goals

- No change to authority direction, identity/auth contracts, or session binding semantics.
- No reintroduction of client-authoritative transforms/state writes.
- No platform split between native and WASM gameplay logic.

## 4. Alternatives Considered

1. Keep 3D runtime and only render sprites in a pseudo-2D view.
- Rejected: leaves unnecessary 3D physics/runtime complexity and mixed contracts.

2. Big-bang rewrite in one change.
- Rejected: high risk for regressions across replication/persistence/prediction and poor rollback control.

3. Dual long-term 2D/3D runtime support.
- Rejected: sustained code duplication and contract drift risk.

## 5. Consequences

Positive:

- Runtime model aligns with desired top-down 2D gameplay.
- Reduced visual pipeline complexity by removing GLTF runtime dependency.
- Clearer long-term maintenance boundaries for camera/physics/content pipelines.

Negative:

- Multi-phase migration touches most runtime crates and contracts.
- Requires careful persistence and protocol compatibility handling.
- Requires broad test updates and migration-specific validation coverage.

## 6. Follow-Up Requirements

- Execute phased migration from `docs/features/2d_migration_plan.md`.
- Update source-of-truth contracts (`sidereal_design_document`, asset delivery contract, visibility contract when impacted) in same changes.
- Maintain required quality gates and native/WASM checks throughout migration.
- Remove obsolete Avian3D/GLTF runtime paths only after replacement coverage is in place.

## 7. References

- `docs/features/2d_migration_plan.md`
- `docs/sidereal_design_document.md`
- `docs/features/asset_delivery_contract.md`
- `docs/features/visibility_replication_contract.md`

