# Projectile Firing and Damage Loop Plan

Status: Updated after ballistic projectile refactor  
Date: 2026-03-08  
Owners: gameplay runtime + replication + client

Implementation status note (2026-03-12):

1. The current corvette/rocinante `Ballistic Gatling` has been moved back to the authoritative hitscan-plus-tracer path by setting `projectile_speed_mps = 0.0`.
2. True projectile entities remain the supported path for weapon families that are intentionally modeled as inertial ballistic shots.
3. This restores the existing tracer presentation loop for the starter ship while projectile-backed combat work continues for distinct weapon classes.
4. Native client ballistic projectile visuals now render immediately from the locally pre-spawned projectile entity before replicated `WorldEntity` adoption, avoiding owner-view muzzle-origin drift caused by waiting for replication/adoption.
5. WASM impact: no current WASM-specific change in this slice.

Primary architecture references:
- `docs/sidereal_design_document.md`
- `docs/features/visibility_replication_contract.md`
- `docs/decisions/dr-0013_action_acceptor_control_routing.md`
- `docs/component_authoring_guide.md`

## 1. Goals

Implement a first playable combat slice with:

1. Two additional corvette hardpoints for weapons.
2. Two mounted gatling cannon module entities.
3. `FirePrimary` action emitted when pressing `Space`.
4. Server-authoritative hit resolution against valid colliders/targets.
5. `HealthPool` reduction on hit targets.
6. Native + WASM parity for input/protocol/gameplay behavior.

## 1.1 Implementation Status (2026-03-03)

Implemented in the current baseline:

1. Added a new corvette hardpoint `weapon_fore_center` (front-center, forward-oriented).
2. Added a mounted `Ballistic Gatling` module on that hardpoint.
3. Added modular weapon components/tags:
   - `WeaponTag`
   - `BallisticWeapon`
   - `AmmoCount`
   - `DamageType::Ballistic`
4. Added authoritative fixed-step weapon fire system.
5. Bound `Space` to `FirePrimary` and moved brake hold to `Ctrl` keys.
6. Preserved native/WASM parity by wiring fire intent through shared `PlayerInput` protocol (`sidereal-net`) used by both targets.

Current combat-slice weapon behavior:

1. The starter `Ballistic Gatling` currently uses the authoritative hitscan `ShotFiredEvent -> tracer message` path because its authored `projectile_speed_mps` is `0.0`.
2. Weapons with `projectile_speed_mps > 0` spawn true ballistic projectile entities.
3. Projectile initial velocity inherits shooter inertial velocity:
   - `projectile_velocity = shooter_linear_velocity + muzzle_forward * projectile_speed_mps`
4. The local shooter pre-spawns the projectile in fixed-step gameplay.
5. The replication server also spawns the same projectile in fixed-step gameplay and marks it `PreSpawned`, then:
   - predicts it for the owning shooter,
   - interpolates it for observers.
6. The hitscan `ShotFiredEvent -> ShotImpactResolvedEvent -> tracer message` path remains for weapons with `projectile_speed_mps <= 0`.
7. Runtime ballistic projectile entities carry a real `EntityGuid` for replication/prediction clone matching.
8. Replication persistence explicitly excludes ballistic projectile entities because they are ephemeral runtime combat state, not durable world state.

Current constraints in this baseline:

1. Initial combat damage is hull-only (`HealthPool`).
2. No replicated shot/hit UI events yet; authoritative state change is health reduction.
3. Fire direction uses mounted hardpoint forward orientation (no turret tracking yet).

## 2. Non-Goals (Initial Slice)

1. Shield/armor layers (hull-only `HealthPool` damage for now).
2. Full ballistic persistence for long-lived projectiles/missiles.
3. Advanced lag compensation or rewind hit validation.
4. Large VFX/SFX polish pass.

## 3. Research Summary and Direction

### 3.1 Updated direction

The original first slice used authoritative hitscan plus cosmetic tracers. That is no longer the right default for Sidereal's ballistic space weapons.

Current direction:

1. Weapons intended to behave as real inertial projectiles use authoritative projectile entities.
2. Hitscan remains valid only for weapon classes that are intentionally instantaneous.
3. Cosmetic tracer reconstruction should not stand in for real ballistic projectile gameplay when travel time, inertial drift, and dodgeability matter.

### 3.2 Rectangle cast idea

The “rectangle from muzzle to destination” idea maps directly to shape casting in Avian.  
For gatling bullets, model each shot as:

1. Ray cast for minimal hitbox, or
2. Capsule/shape cast for forgiving hit registration.

Do this per fixed tick with bounded range and collision filters.  
This is better than spawning full rigid-body bullets for high-RPM cannons in the first slice.

### 3.3 References

1. Avian spatial queries: <https://docs.rs/avian2d/latest/avian2d/spatial_query/index.html>
2. Avian `SpatialQuery` API: <https://docs.rs/avian2d/latest/avian2d/spatial_query/struct.SpatialQuery.html>
3. Bevy `spawn_batch` efficiency note: <https://docs.rs/bevy/latest/bevy/ecs/system/command/fn.spawn_batch.html>
4. Lightyear inputs module: <https://docs.rs/lightyear/latest/lightyear/inputs/index.html>

## 4. Authoritative Combat Loop

Fixed tick flow (`FixedUpdate`, server/replication):

1. Receive authenticated client input.
2. Route `FirePrimary` intent to authoritative controlled entity (existing control routing model).
3. Weapon acceptor checks:
   - module presence on mounted hardpoints,
   - per-weapon cooldown / rate-of-fire,
   - optional ammo/fuel/power checks (future extension hook).
4. For each accepted shot:
   - resolve muzzle world transform from hardpoint/module hierarchy,
   - compute shot direction from authoritative aiming source,
   - if the weapon is hitscan: run immediate spatial query and resolve impact,
   - if the weapon is ballistic: spawn a projectile entity with inherited shooter velocity.
5. Ballistic projectile loop:
   - advance projectile position in fixed-step,
   - raycast over the projectile step each tick,
   - apply damage on authoritative hit,
   - despawn on impact or lifetime expiry.
6. Replicate resulting state deltas and projectile entity state.

## 5. Data Model Additions (Proposed)

Add persistable gameplay components in `crates/sidereal-game/src/components/`:

1. `GatlingCannon`
   - fields: `rpm`, `muzzle_velocity_mps`, `projectile_mass_kg`, `max_range_m`, `spread_rad`, `cooldown_s`.
   - macro: `#[sidereal_component(kind = "gatling_cannon", persist = true, replicate = true, visibility = [OwnerOnly])]`
2. `WeaponHardpointTag` (optional explicit capability marker; can be skipped if `Hardpoint.hardpoint_id` naming is enough).
3. `CombatCooldownState` (if cooldown state should be explicit/persisted).

Damage model (initial):

1. Base kinetic energy: `0.5 * mass_kg * velocity^2`.
2. Map to gameplay damage with a tunable scalar + clamps.
3. Optionally include relative closing speed against target velocity (later refinement).

## 6. Ship Template and Hardpoint Changes

### 6.1 Corvette hardpoints

Extend hardpoint defaults from 5 to 7 by adding:

1. `weapon_left_fore`
2. `weapon_right_fore`

Files:

1. `crates/sidereal-game/src/entities/hardpoint.rs`
2. `crates/sidereal-game/src/entities/ship/corvette.rs`
3. `data/scripts/bundles/entity_registry.lua`

### 6.2 Corvette module specs

Extend `CorvetteModuleKind` and module defaults with:

1. `GatlingCannon` kind
2. `gatling_left` on `weapon_left_fore`
3. `gatling_right` on `weapon_right_fore`

Ensure mounts persist and hydrate as graph relationships (`ParentGuid`, `MountedOn`) like existing modules.

## 7. Input and Action Routing Changes

Current native input maps `Space` to `Brake`, so key mapping must change:

1. `Space` -> `EntityAction::FirePrimary`
2. Move brake to another key (recommended: `LeftShift`) or explicit alternate mapping.

Files:

1. `bins/sidereal-client/src/native/input.rs`
2. `crates/sidereal-net/src/lightyear_protocol/input.rs` (extend action construction path for fire input)

Maintain identical behavior for native and WASM builds (same gameplay/input path, platform-only transport differences).

## 8. Collision/Hit Implementation Notes

Use Avian spatial queries in gameplay systems, not ad-hoc geometry tests:

1. Query origin: world muzzle position from `GlobalTransform`.
2. Direction: authoritative aim vector.
3. Range: from `GatlingCannon.max_range_m`.
4. Filter:
   - exclude shooter root + mounted children,
   - include only damageable candidates (entities with `HealthPool` and valid collider presence),
   - collider presence must be explicit via `CollisionProfile` (`mode = Aabb`) on collidable entities.
5. Hit ordering:
   - pick closest valid hit.
6. Apply damage:
   - mutate `HealthPool.current = max(0, current - damage)`.

Collision authoring policy:

1. `Sprite`/visual presence never implies collision.
2. Collidable world entities must opt in with persisted+replicated `CollisionProfile`.
3. `CollisionProfile::None` means no runtime collider bootstrap even if the entity has `SizeM`.
4. For sprite-authored hulls, store `CollisionOutlineM` points generated from alpha-mask contour extraction and Ramer-Douglas-Peucker simplification; use this outline for runtime collider construction before any AABB fallback.

## 9. Optimization Plan

## 9.1 Immediate

1. Keep hitscan only for true hitscan weapons.
2. Use real projectile entities for ballistic space weapons.
3. Pre-spawn those ballistic projectiles on the local predicted path and mark matching server projectiles `PreSpawned`.

## 9.2 Client visual optimization

1. Implement local tracer VFX pool (ring buffer) for rendering only.
2. Reuse hidden tracer entities/instances instead of spawn/despawn every frame.
3. Keep pool size configurable (for example `SIDEREAL_CLIENT_TRACER_POOL_SIZE`).

## 9.3 Future

1. Introduce persistent projectile entities only for slow ordnance (missiles/plasma).
2. Use spatial partition/candidate narrowing if combat density grows (consistent with visibility candidate patterns).

## 10. Test Plan

Unit tests (`crates/sidereal-game`):

1. `FirePrimary` accepted only when weapon module exists and cooldown permits.
2. Cooldown/rate-of-fire determinism across fixed ticks.
3. Damage function produces stable bounded values.
4. Health reduction clamps at zero.

Integration tests (`bins/sidereal-replication`):

1. Authenticated client `Space` input routes to controlled entity and produces fire events.
2. Valid target with `HealthPool` takes damage.
3. Non-target/self entities are not incorrectly damaged.
4. Controlled-target spoof attempts remain rejected by existing input binding checks.

Cross-target compile checks:

1. `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
2. `cargo check -p sidereal-client --target x86_64-pc-windows-gnu`

## 11. Implementation Sequence

1. Add weapon components and combat systems in `sidereal-game` (no UI polish yet).
2. Add two hardpoints + two gatling modules in corvette defaults and graph templates.
3. Wire `Space -> FirePrimary` and remap brake key.
4. Add authoritative hit/damage pipeline in replication fixed step.
5. Add tests (unit + integration), then run quality gates.
6. Add optional client tracer pool as visual-only optimization.

## 12. Open Decisions

1. `HealthPool` visibility policy currently defaults to `OwnerOnly`; decide whether combat target health should expose a public/faction-safe projection component.
2. Decide if gatling fire should be strictly forward-facing in v1 or include mouse-aim/turret aiming.
3. Decide whether cooldown state must persist across logout/hydration or reset on session start.
