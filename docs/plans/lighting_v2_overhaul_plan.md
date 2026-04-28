# Lighting V2 Overhaul Plan

Status: Active implementation plan
Last updated: 2026-04-28
Owners: client rendering + gameplay runtime + asset/shader authoring
Scope: Client presentation lighting for world-facing 2D `Material2d` shaders, dynamic local emitters, stellar light falloff, and deep-space ambient readability.
Primary references: `docs/plans/lighting_model_and_dynamic_space_events_plan.md`, `docs/core_systems_catalog_v1.md`, `docs/decisions/dr-0038_lighting_v2_material_contract.md`, `docs/features/procedural_asteroids.md`, `docs/features/procedural_planets.md`, `docs/features/asset_delivery_contract.md`

## 0. Implementation Status

2026-04-28:
- Lighting V2 is the active replacement direction for `system.lighting.v2`.
- The existing V1 baseline already has `EnvironmentLightingState`, `WorldLightingState`, shared world-light uniforms, star-position-derived primary lighting, asteroid normal-map lighting, planet lighting, and one dominant local light contribution.
- The V2 build must replace the single-primary/single-local material contract with top-2 stellar lights, top-8 local dynamic lights, authored light falloff, and a persistent deep-space ambient floor.
- Native impact: all world-facing render materials are expected to consume the V2 material ABI unless explicitly emissive/UI/exempt.
- WASM impact: shader source, streamed cache, published cache, and material bindings must remain in parity with native. The V2 uniform layout must be browser/WebGPU-safe.

2026-04-28 update:
- First implementation slices are in progress: `StellarLightSource`, the client V2 resolver, asteroid/runtime-effect V2 uniforms, generic sprite V2 uniforms, and planet V2 embedded lighting uniforms have been wired.
- Generic world sprites now require per-entity shader materials because their lighting uniforms are resolved from each rendered entity's world position.
- Asteroid and generic sprite lighting now use sprite-local, Y-up object space with runtime rotation uniforms sourced from Avian `Rotation` first and static `WorldRotation` second.
- Remaining work is polish and rollout: tune art response per material, add richer authored local emitter profiles, expose debug overlays, and remove any remaining explicitly documented exemptions.

## 1. Goals

1. Stellar lighting fades across the continuous galaxy instead of illuminating every object globally.
2. Objects outside direct stellar falloff remain readable through low-level deep-space ambient lighting.
3. Bullets, projectile visuals, thrusters, impact sparks, explosions, and destruction bursts can illuminate nearby surfaces.
4. Asteroids, planets, ships/generic sprites, and runtime effects consume one shared 2.5D lighting contract.
5. Lighting remains presentation-only and never writes authoritative simulation state.

## 2. Decisions

1. Use top 2 stellar lights per rendered object.
2. Use top 8 dynamic local lights per rendered object.
3. Use authored gameplay falloff with `inner_radius_m`, `outer_radius_m`, smooth fade, intensity, color, elevation, and priority.
4. Keep deep-space ambient in `EnvironmentLightingState`; it is always available even when no direct stellar source reaches an object.
5. Keep Bevy 3D lights out of the authoritative world-lighting contract. Sidereal world lighting remains material-uniform driven.

## 3. Data Model

Add a persisted, replicated public component:

```rust
#[sidereal_component(kind = "stellar_light_source", persist = true, replicate = true, visibility = [Public])]
pub struct StellarLightSource {
    pub enabled: bool,
    pub color_rgb: Vec3,
    pub intensity: f32,
    pub inner_radius_m: f32,
    pub outer_radius_m: f32,
    pub elevation: f32,
    pub priority: f32,
}
```

Default values:

| Field | Default |
| --- | ---: |
| `enabled` | `true` |
| `color_rgb` | `[1.0, 0.86, 0.48]` |
| `intensity` | `1.25` |
| `inner_radius_m` | `3500.0` |
| `outer_radius_m` | `18000.0` |
| `elevation` | `0.36` |
| `priority` | `1.0` |

For `planet.body` Lua bundles with `body_kind == 1`, attach `stellar_light_source` unless the bundle context explicitly disables it.

## 4. Runtime Resources

`WorldLightingState` becomes the global presentation-lighting resource containing:

1. fallback primary direction/color/intensity/elevation from `EnvironmentLightingState`;
2. ambient/backlight/flash color and intensity;
3. exposure/debug metadata;
4. bounded replicated stellar-light candidates.

`CameraLocalLightSet` contains bounded camera-local dynamic emitters derived from visible presentation effects.

Budgets:

```rust
MAX_STELLAR_LIGHTS = 2
MAX_LOCAL_LIGHTS = 8
MAX_STELLAR_LIGHT_CANDIDATES = 32
MAX_CAMERA_LOCAL_LIGHT_EMITTERS = 64
```

## 5. Uniform ABI

Use one shared uniform shape for lit world materials:

```rust
pub struct SharedWorldLightingUniforms {
    pub metadata: Vec4,
    pub ambient: Vec4,
    pub backlight: Vec4,
    pub flash: Vec4,
    pub stellar_dir_intensity: [Vec4; 2],
    pub stellar_color_params: [Vec4; 2],
    pub local_dir_intensity: [Vec4; 8],
    pub local_color_radius: [Vec4; 8],
}
```

Meanings:

1. `metadata.x`: active stellar light count.
2. `metadata.y`: active local light count.
3. `metadata.z`: exposure scale.
4. `metadata.w`: debug mode.
5. `ambient.rgb/a`: deep-space ambient color/intensity.
6. `backlight.rgb/a`: scene rim/backlight.
7. `flash.rgb/a`: global transient flash.
8. `stellar_dir_intensity[i].xyz/w`: normalized object-to-star direction and falloff-adjusted intensity.
9. `stellar_color_params[i].rgb/w`: stellar color and reserved scalar.
10. `local_dir_intensity[i].xyz/w`: normalized object-to-emitter direction and falloff-adjusted intensity.
11. `local_color_radius[i].rgb/w`: local light color and outer radius.

Unused slots are zero-filled.

## 6. Falloff

For a light source:

```text
surface_distance = max(0, distance(object, source) - source_visual_radius)
t = clamp((surface_distance - inner_radius) / (outer_radius - inner_radius), 0, 1)
falloff = 1 - smoothstep(t)
effective_intensity = intensity * falloff * priority
```

If `outer_radius <= inner_radius`, runtime clamps the effective outer radius to `inner_radius + 1`.

When explicit stellar sources exist but no source reaches an object, direct stellar slots are empty and only ambient/backlight/flash remain. When no stellar sources exist at all, the fallback primary directional light from `EnvironmentLightingState` fills one stellar slot.

## 7. Local Emitter Profiles

| Source | Color | Intensity | Inner radius | Outer radius | Elevation |
| --- | --- | ---: | ---: | ---: | ---: |
| Thruster plume | material plume color | existing plume formula | `radius * 0.35` | `radius` | `0.35` |
| Weapon tracer | `[1.0, 0.86, 0.42]` | `1.3 * alpha` | `20m` | `140m` | `0.25` |
| Ballistic projectile sprite | `[1.0, 0.78, 0.32]` | `0.9` | `8m` | `80m` | `0.25` |
| Impact spark | `[1.0, 0.82, 0.46]` | `2.4 * ttl_norm` | `10m` | `160m` | `0.45` |
| Impact explosion | `[1.0, 0.56, 0.18]` | `3.2 * ttl_norm` | `25m` | `260m` | `0.55` |
| Destruction explosion | `[1.0, 0.50, 0.16]` | `4.0 * ttl_norm` | `50m` | `420m` | `0.60` |

## 8. Material Migration

1. Asteroids: iterate V2 stellar/local arrays and rotate light directions into asteroid-local normal-map space.
2. Planets: embed the V2 uniform data in `PlanetBodyUniforms` and accumulate stellar/local direct terms.
3. Generic sprites/ships: upgrade `StreamedSpriteShaderMaterial` from image-only to lit sprite material with albedo, flat/default normal map, V2 lighting, and object rotation.
4. Runtime effects: remain primarily emissive, sample global scene tint only, and emit local lights through the CPU collector rather than feeding back their own local slots.

## 9. Testing

Rust unit tests:

1. stellar falloff is full inside inner radius;
2. stellar falloff is zero after outer radius;
3. top-2 stellar selection is deterministic by effective intensity;
4. fallback primary light is used only when no stellar sources exist;
5. deep-space ambient remains when stars are out of range;
6. top-8 local light selection zero-fills unused slots;
7. projectile/tracer/impact/explosion entities contribute local emitters.

Shader validation:

1. `asteroid.wgsl`;
2. `planet_visual.wgsl`;
3. `runtime_effect.wgsl`;
4. `sprite_pixel_effect.wgsl`.

Manual acceptance:

1. asteroids near the same star share a coherent star-facing lit side;
2. objects outside a star radius are dim but not black;
3. firing near an asteroid creates moving bullet light and short impact flashes;
4. ships/generic sprites no longer render as the unlit exception;
5. two-star overlap blends the two strongest direct lights.

## 10. Rollout

1. Documentation and decision record.
2. `StellarLightSource` component and Lua bundle wiring.
3. CPU light resolution and V2 uniform ABI.
4. Asteroid material/shader migration.
5. Planet material/shader migration.
6. Generic sprite/ship material migration.
7. Runtime effect/local emitter migration.
8. Shader cache/published cache parity.
9. Tests and quality gates.
