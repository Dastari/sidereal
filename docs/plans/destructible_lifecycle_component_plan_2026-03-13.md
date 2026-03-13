# Destructible Lifecycle Component Plan

Status: Phase 1 infrastructure implemented; scripting override follow-up still open  
Date: 2026-03-13  
Owners: gameplay + replication combat + runtime scripting + client visuals

Update note (2026-03-13):
- Current combat only reduces `HealthPool.current` and does not yet have a generic authoritative destruction-resolution flow.
- The right shape is not "raw shader on a gameplay component plus direct Lua callback."
- The project docs already point toward a Rust-owned destruction profile/effect-preset contract with Lua as an exceptional override path via lifecycle events.
- Native impact: requires a new authoritative destruction/finalization path plus a replicated or client-triggered effect playback path.
- WASM impact: no intended architecture split; destruction resolution, profile schema, and effect-trigger contracts should remain shared, with browser/native differences only at the existing client render/asset boundary.

Update note (2026-03-13, implemented later):
- `Destructible` now exists as a persisted gameplay component with `destruction_profile_id` and `destroy_delay_s`.
- The authoritative fixed-step destruction path is now live: depleted `Destructible` entities enter `PendingDestruction`, broadcast a replicated destruction-effect message, emit `health_depleted` / `before_destroy` / `destroyed` runtime-script events, and despawn after the effect-delay plus one script-dispatch tick.
- The first visual slice reuses the existing client explosion billboard path rather than adding a new material family, but now drives it with ship-scale destruction timing/size instead of the tiny weapon-impact parameters.
- Native impact: destroyed ships now play a pre-despawn explosion effect and stay alive long enough for targeted runtime lifecycle events to execute.
- WASM impact: no intended behavior split; the authoritative destruction pipeline and replicated destruction-effect message are shared protocol/runtime behavior.

Update note (2026-03-13, post-process follow-up):
- The client explosion path now has extra shader-domain padding so the authored blast no longer grows into the edge of its billboard and clips visibly at the quad boundary.
- The gameplay camera now also runs a real Bevy 2D `ViewTarget` post-process pass that samples the composed scene color and applies localized shockwave distortion at the active explosion screen coordinates.
- The distortion pass is driven from live `WeaponImpactExplosion` entities, so the screen-space shockwave follows the same world-space explosion timing and growth as the billboard instead of guessing from a separate overlay timeline.
- Native impact: ship destruction and other runtime explosions now distort the underlying scene/background in-place without needing a second fake overlay material.
- WASM impact: no architecture split; the effect uses the same client render graph/plugin path and continues to compile on the WebGPU target.

Update note (2026-03-13, distortion performance follow-up):
- The fullscreen distortion pass now only activates for destruction-scale explosions, not ordinary weapon-impact bursts, to avoid keeping a fullscreen post-process active during normal gunfire.
- The gameplay camera only carries the distortion settings component while at least one active shockwave is present, so the custom post-process node drops out of the render path entirely when idle.
- Native impact: destruction blasts still distort the composed scene, but routine combat no longer pays the fullscreen shockwave cost.
- WASM impact: no architecture split; the same destruction-only gating and idle teardown behavior applies to browser clients.

## 1. Problem Statement

We need a generic way to say:

1. this entity should be finalized when its health reaches zero,
2. the authoritative runtime may run scripted override logic before finalization,
3. a destruction effect should play before the entity is finally removed from the world.

Today the codebase does not have that generic lifecycle:

1. `HealthPool` is only a damageable capability.
2. Projectile combat currently reduces health but does not enqueue generic `health_depleted` / `before_destroy` / `destroyed` lifecycle events.
3. The client already has explosion-capable runtime effect materials/pools, but those are wired for weapon-impact visuals, not authoritative entity-destruction resolution.

## 2. Existing Constraints

This plan must follow the already-documented project contract:

1. Rust owns authoritative destruction resolution and the validated destruction/effect profile schema.
2. Lua authors high-level content choices such as destruction profile IDs, effect preset IDs, and exceptional override behavior.
3. Scripts must not directly despawn entities, mutate raw health as a shortcut, or bind to low-level shader/material ABI details.
4. Default destruction behavior should come from authored components/profiles, not ad hoc Lua logic repeated per entity.

Relevant existing docs:

1. `docs/features/scripting_support.md`
2. `docs/features/asteroid_field_system.md`
3. `docs/decisions/dr-0029_runtime_shader_family_taxonomy_and_lua_authoring_model.md`
4. `docs/component_authoring_guide.md`

## 3. Recommended Shape

### 3.1 Gameplay Component

Add a new persistable gameplay component in `crates/sidereal-game/src/components/destructible.rs`.

Recommended initial shape:

```rust
pub struct Destructible {
    pub destruction_profile_id: String,
    pub destroy_delay_s: f32,
}
```

Rules:

1. `destruction_profile_id` points to a Rust-validated authored destruction profile or preset family, not a raw WGSL asset path.
2. `destroy_delay_s` is the authoritative delay between entering destruction resolution and final despawn. It allows the effect to play before final removal.
3. Do not put a raw Lua function name on this component as the primary callback path.

### 3.2 Script Hook Path

Use the existing `ScriptState.data.event_hooks` model for exceptional behavior instead of a dedicated callback field on `Destructible`.

Recommended events:

1. `health_depleted`
2. `before_destroy`
3. `destroyed`

Why:

1. It matches the existing runtime scripting direction.
2. It keeps callbacks consistent with the broader event bridge.
3. It avoids baking one special-case Lua callback field into gameplay schema when the same entity may later need other lifecycle hooks.

### 3.3 Runtime Resolution Path

Add a server-authoritative destruction pipeline:

1. detect `HealthPool.current <= 0` on entities with `Destructible`,
2. add a runtime-only pending-finalization component/resource state,
3. emit `health_depleted` / `before_destroy` script events,
4. resolve the final outcome:
   - normal destroy,
   - cancel/override destroy,
   - alternate authored outcome later (fracture, loot, etc.),
5. trigger destruction effect playback,
6. despawn only after the authoritative delay expires.

### 3.4 Effect Trigger Contract

Do not store a raw effect shader or low-level material ABI payload on `Destructible`.

Instead:

1. destruction profiles resolve to authored `effect_preset_id` values or equivalent validated preset references,
2. the client renders the effect from an authoritative outcome signal,
3. the actual render implementation may reuse the current `RuntimeEffectMaterial` explosion path or a later generic effect-event path.

## 4. Why Not A Direct Lua Callback Field?

A direct callback field such as `on_destroy_lua = "foo"` is not the best primary design because:

1. the runtime scripting system already uses `ScriptState.data.event_hooks`,
2. lifecycle events are explicitly planned there already,
3. a one-off callback field duplicates the handler-routing model,
4. it would encourage special-case schema that does not scale to fracture/loot/mission/event outcomes.

If a component-local override handle is still desired later, it should be a validated handler ID that integrates with the event system, not a raw arbitrary function pointer concept.

## 5. Minimal Implementation Slice

Phase 1 should stay narrow:

1. add `Destructible` gameplay component,
2. add a runtime-only pending-destruction state component on the server side,
3. add an authoritative fixed-step system:
   - when `HealthPool.current <= 0` and not already pending, enter destruction pending state,
   - trigger effect outcome,
   - countdown `destroy_delay_s`,
   - despawn when the timer expires,
4. add an initial destruction-effect trigger that reuses the existing explosion billboard path,
5. update docs to say lifecycle events are now partially implemented if that lands in the same change.

This first slice should not try to solve fracture, loot spawning, or generic alternate outcomes all at once.

## 6. Required Follow-Up Work

To fully match the intended contract, later work should add:

1. validated lifecycle-override intents in runtime scripting,
2. authored destruction profile definitions and validation,
3. richer final outcomes such as fracture, loot, or alternate approved destroy results,
4. broader tests proving script override safety and profile-driven effect selection,
5. optional editor/content tooling for destruction profile authoring.

## 7. Recommended Next Step

Recommended next implementation follow-up:

1. add validated script override intents for pre-despawn lifecycle hooks,
2. move from a hardcoded `"explosion_burst"` profile string to validated authored destruction profiles,
3. add content slices beyond ships so asteroids or other destructibles can opt into the same lifecycle path.
