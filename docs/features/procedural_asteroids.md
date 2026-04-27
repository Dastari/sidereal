# Procedural Asteroids

Status: Active feature reference
Last updated: 2026-04-26
Owners: gameplay content + scripting + client rendering
Scope: current Lua-authored asteroid field-member bootstrap and procedural asteroid visual/collision baseline

## 0. Status Notes

- 2026-04-24: Current implementation remains phase-1 live: `asteroid.field_member` graph records are generated from Lua, persisted as individual world entities, replicated through normal world visibility, and rendered with procedural sprite/streamed shader support. Not yet implemented: the field-root activation/depletion/fracture model described in `docs/features/asteroid_field_system.md`. Native impact: current path is live. WASM impact: shared content/runtime path should remain target-shared; live browser validation remains deferred behind native stabilization.
- 2026-04-26: Asteroid Field System V2 is now the active replacement direction for field roots, deterministic member lineage, zero-health fracture, resource profiles, and field ambient effects; see `docs/features/asteroid_field_system_v2.md`. This document remains the live reference for the current procedural sprite/member baseline that V2 builds on. Native impact: current asteroid visuals will evolve in the native client first. WASM impact: procedural generation and schemas remain shared-client compatible.
- 2026-04-26: Procedural asteroid payloads now include surface style, pixel-step, crack intensity, mineral vein intensity, mineral accent color, and optional family seed key. The generator uses these to produce chunkier top-down ARPG-style silhouettes, quantized color bands, cracks, and ore accents. Native impact: native streamed asteroid visuals rebuild from the expanded payload. WASM impact: schema is shared and must be consumed by browser builds before live parity resumes.
- 2026-03-12: The current implementation remains the live baseline, but it is no longer the intended end-state architecture. Planned direction is a persisted field-root model with clustered activation, larger size tiers, and authoritative fracture/depletion state; see `docs/features/asteroid_field_system.md`. Native impact: future runtime/client work required. WASM impact: shared gameplay/procedural-generation logic should stay target-shared.
- 2026-03-09: Native client streamed asteroid visuals now rebuild when replicated `sprite_shader_asset_id` or `procedural_sprite` data arrives after the initial `visual_asset_id`. This prevents some asteroids from remaining on the `asteroid_texture_red_png` fallback after late component adoption. WASM impact: no architecture change; shared streamed-visual retry behavior should match once parity work resumes.

## 1. What Is Implemented

### 1.1 Authoritative generation is Lua-driven

Asteroids are now generated from `data/scripts/world/world_init.lua` using bundle spawns, not hardcoded Rust seeding.

- `world_init.lua` calls `spawn_bundle_graph_records("asteroid.field_member", overrides)` in a deterministic loop.
- Default field profile:
  - `asteroid_field_count = 120`
  - center at `(0, 0)`
  - radial distribution with deterministic jitter
  - diameter range `4m..28m`
- World init remains one-time/idempotent via `script_world_init_state` guard.

### 1.2 Asteroid bundle

New bundle: `data/scripts/bundles/starter/asteroid_field.lua`

`asteroid.field_member` emits graph records with:
- `display_name`, `entity_labels`
- `health_pool`
- `mass_kg`, `size_m`
- `collision_profile` + `collision_aabb_m`
- optional `collision_outline_m` generated from the same procedural silhouette
- `visual_asset_id`
- optional `sprite_shader_asset_id`
- `procedural_sprite`
- `map_icon`
- Avian components (`avian_position`, `avian_rotation`, `avian_linear_velocity`, `avian_angular_velocity`, `avian_rigid_body`, damping)

The bundle is registered in `data/scripts/bundles/bundle_registry.lua` as class `world`.

### 1.3 Asset registry wiring

`data/scripts/assets/registry.lua` now contains:
- `asteroid_texture_red_png` -> `data/textures/red.png` (fallback source only; live asteroid silhouette is generated client-side)
- `asteroid_wgsl` -> `data/shaders/asteroid.wgsl` (live 2D asteroid shader)

### 1.4 Lua-authored procedural sprite profile

Asteroid bundles now author a replicated `procedural_sprite` payload in Lua.

That payload defines:
- generator/profile ID
- sprite resolution
- edge noise
- lobe amplitude
- crater count
- dark/light palette colors

The client generates the actual asteroid sprite from that payload plus entity GUID, then applies `asteroid_wgsl` on top.
The authoritative host uses that same payload plus entity ID to derive:
- collision half extents
- RDP collision outline
- the same procedural silhouette deterministically

The shared generator also produces a normal-map texture from the generated height field so asteroid lighting can be added later without changing the authored content model.

## 2. Determinism Model

Current field generation is deterministic from index-driven hash functions in Lua (`hash01(index, salt)`), which drives:
- angle offset
- radial jitter
- diameter
- mass
- health
- spin
- initial rotation
- asteroid type label roll

Given the same script defaults, first-world bootstrap produces stable layout and property distributions.

## 3. Current Rendering Model

Asteroids are now rendered as 2D procedurally generated sprites:
- Lua bundle scripts author the replicated procedural sprite profile.
- Client runtime generates an irregular alpha-masked sprite from the entity GUID.
- Client runtime also generates a matching normal map from the same height field and keeps it available for later lighting work.
- `data/shaders/asteroid.wgsl` runs as the asteroid-specific 2D sprite shader.

## 4. Collision Model

Asteroid collision is now derived from the procedural silhouette, not just a fixed square box:
- script hosts expose `compute_collision_half_extents_from_procedural(entity_id, procedural_sprite, length_m)`
- script hosts expose `generate_collision_outline_rdp_from_procedural(entity_id, procedural_sprite, half_extents)`
- `data/scripts/bundles/starter/asteroid_field.lua` uses those helpers when building graph records

This keeps visual sprite generation and collision generation aligned because both come from the same shared deterministic Rust generator.

## 5. Next Steps

1. Move more surface-generation parameters out of Rust and into Lua-authored replicated payloads.
2. Add multiple asteroid generator profiles beyond `asteroid_rocky_v1`.
3. Bake the shared 2.5D lighting contract into the sprite/material stack so asteroid albedo + normal generation feeds the same world-lighting model as ships and planets.
4. Add LOD policy for sprite resolution and shader detail.
5. Consume the generated normal map in asteroid lighting/material work.
6. Replace the fallback `red.png` placeholder source with a neutral white mask asset or remove that dependency entirely.

## 6. Files

- `data/scripts/world/world_init.lua`
- `data/scripts/bundles/starter/asteroid_field.lua`
- `data/scripts/bundles/bundle_registry.lua`
- `data/scripts/assets/registry.lua`
- `data/shaders/asteroid.wgsl`
