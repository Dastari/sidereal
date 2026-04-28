# DR-0038: Lighting V2 Material Contract

Status: Accepted
Date: 2026-04-28
Owners: client rendering + gameplay runtime + asset/shader authoring

## Context

Lighting V1 established a shared client-side `WorldLightingState` and moved planets, asteroids, and runtime effects away from purely shader-private light constants. It still resolves one primary stellar direction and one dominant nearby local light per rendered object. That is not enough for Sidereal's continuous galaxy model, where stellar influence must fade by distance and transient combat effects need to light nearby surfaces.

## Decision

Sidereal Lighting V2 uses a material-uniform-driven 2.5D lighting contract with:

1. top 2 stellar lights per rendered object;
2. authored stellar falloff using inner radius, outer radius, intensity, color, elevation, and priority;
3. deep-space ambient lighting from `EnvironmentLightingState` that remains active even when direct stellar light is zero;
4. top 8 dynamic local lights per rendered object;
5. client-derived local emitters from bullets/tracers, ballistic projectile visuals, thrusters, impacts, explosions, and future transient effects;
6. shared material participation for asteroids, planets, generic world sprites/ships, and runtime effects.

Lighting remains presentation-only. It must not write authoritative gameplay state.

2026-04-28 update:
- Lit world sprite shaders must evaluate light directions in sprite-local, Y-up object space. Runtime material code passes rotation from Avian `Rotation` first, then static `WorldRotation`, into shader-local rotation uniforms.
- Generic sprite shader materials expose `@group(2) @binding(2)` as `SharedWorldLightingUniforms` and `@group(2) @binding(3)` as `local_rotation: vec4<f32>`.
- Asteroid sprite shader materials expose `@group(2) @binding(2)` as `SharedWorldLightingUniforms`, `@group(2) @binding(3..4)` as the generated normal texture/sampler, and `@group(2) @binding(5)` as `local_rotation: vec4<f32>`.

## Alternatives Considered

1. Keep one dominant light: rejected because bullets/impacts and overlapping star regions need multiple contributors.
2. Use physical inverse-square falloff: rejected because gameplay-scale readability and authoring control matter more than physical correctness.
3. Use Bevy 3D PBR lights: rejected because Sidereal's world render stack is primarily 2D `Material2d` shaders and needs deterministic native/WASM behavior.

## Consequences

Positive:

1. Star systems can fade naturally into deep space.
2. Objects remain readable outside direct stellar range.
3. Combat visuals can illuminate nearby surfaces without adding an authoritative light replication lane.
4. Ships/generic sprites no longer remain the unlit exception.

Negative:

1. The shared material ABI grows and all shader cache paths must be kept in parity.
2. Shader loops become more expensive, especially with 8 local lights.
3. Existing generic sprite shaders need a binding contract migration.

## Follow-Up

Implement the phased plan in `docs/plans/lighting_v2_overhaul_plan.md` and keep `docs/plans/lighting_model_and_dynamic_space_events_plan.md` updated as the V1-to-V2 migration progresses.
