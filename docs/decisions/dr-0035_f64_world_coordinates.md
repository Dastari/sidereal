# DR-0035: f64 Authoritative World Coordinates

Status: Accepted
Date: 2026-04-24
Owners: gameplay simulation + replication + client runtime + dashboard

Primary references:
- `docs/features/galaxy_world_structure.md`
- `docs/plans/f64_world_precision_migration_plan_2026-04-24.md`
- `docs/features/visibility_replication_contract.md`
- `docs/features/tactical_and_owner_lane_protocol_contract.md`
- `docs/features/scripting_support.md`

## 1. Context

Sidereal is moving toward a continuous galaxy-scale 2D world. The current runtime still has f32 absolute coordinates in Avian `Position`, static `WorldPosition`, protocol read models, visibility caches, scripting snapshots, and dashboard editing paths.

At the intended galaxy scale, f32 absolute coordinates create visible quantization and physics instability. Bevy render transforms cannot be widened because `Transform` and `GlobalTransform` are f32, so authoritative coordinate precision and render projection must be separated.

## 2. Decision

Adopt f64 as the canonical authoritative world-coordinate precision:

1. Workspace `avian2d` uses `f64` and `parry-f64`, not `f32` / `parry-f32`.
2. Avian `Position` and `LinearVelocity` are the authoritative motion lane for simulated entities.
3. `WorldPosition` is the authoritative static/non-physics spatial lane and uses f64-backed coordinates.
4. Bevy `Transform` remains f32 and is a render/hierarchy/debug projection only.
5. Client rendering uses camera-relative f32 transforms derived from f64 authoritative coordinates.
6. Network protocol/read-model fields that carry absolute world coordinates or velocities use f64 arrays.
7. Dashboard coordinate editing uses TypeScript `number` / JSON numbers, which match Rust f64 precision for the planned Sidereal scale.
8. Existing local/dev graph data is reset after the migration; no legacy f32 compatibility aliases, backfills, or dual payload paths are added.

## 3. Alternatives Considered

1. Keep f32 absolute coordinates.
   - Rejected because f32 precision is already inadequate for large galaxy distances.
2. Use sector wrapping/rebasing as the canonical coordinate model.
   - Rejected because it complicates server authority, persistence, visibility, projectiles, prediction, and multi-player cross-sector interactions.
3. Use dual f32/f64 runtime payloads temporarily.
   - Rejected because early-development schema discipline requires one canonical schema and local/dev database resets when schemas change.
4. Use dashboard decimal strings for all f64 coordinates.
   - Rejected for this phase because TypeScript numbers are sufficient for Sidereal's planned world scale and keep tooling simpler.

## 4. Consequences

Positive:

1. Authoritative physics and persistence remain stable at and beyond the planned galaxy scale.
2. Rendering precision remains stable because Bevy transforms stay camera-relative near the local origin.
3. Visibility, tactical, scripting, and dashboard tooling share one coordinate contract.

Negative:

1. Network position payloads become larger.
2. Many systems that used `Vec2`/`Vec3` for world-space data must be audited and updated.
3. Local/dev persisted data must be reset.

## 5. Implementation Notes

The implementation plan lives in `docs/plans/f64_world_precision_migration_plan_2026-04-24.md`.

Do not treat f32 Bevy `GlobalTransform` as authoritative world state when Avian `Position` or `WorldPosition` exists. F32 transforms are allowed for render projection, hierarchy propagation, debug display, and fallback only.
