# Procedural Planets

**Status:** Active implementation (phase 1 live)
**Last updated:** 2026-03-09

## 0. Status Notes

- 2026-03-09: Native client planet visuals no longer apply camera-driven x/y parallax offsets on top of the authoritative planet root transform. Planets still render on a lower z/depth layer, but the visible disc center now remains aligned with the replicated world position so AABB/debug overlays and selection stay correct away from camera center. WASM impact: shared client visual behavior change; no platform-specific divergence intended.
- 2026-03-09: Native client planet visuals now use a client-only projected render-center offset derived from the authoritative planet world position plus camera position. The authoritative planet root remains fixed at the real world center for visibility/exploration/culling decisions, while the visual child offset restores layer parallax for drawing only. WASM impact: shared client visual behavior change; no platform-specific divergence intended.
- 2026-03-09: Runtime world layers now also support an optional `screen_scale_factor` used by the native planet visual path. This changes apparent planet size for the layer without changing authoritative world position or adding extra parallax. It does not by itself fix projected render-frustum culling; projected visual bounds still need a dedicated follow-up.
- 2026-03-09: Native client planet passes now opt out of Bevy frustum culling and use client-side projected landmark bounds against the gameplay camera viewport plus buffer instead. That keeps parallaxed planets, rings, and clouds visible until their projected disc actually leaves the buffered view, while still allowing planet-local lighting and other `ViewVisibility` consumers to drop out once the projected landmark is offscreen. WASM impact: shared client visual behavior change; no platform-specific divergence intended.
- 2026-03-09: Replication delivery for discovered parallaxed planets now widens landmark delivery by the authored layer `parallax_factor` and bypasses the normal spatial candidate prefilter for already-discovered landmarks. This keeps the server from dropping a planet while its projected render center is still inside the buffered gameplay viewport. WASM impact: transport-agnostic authoritative visibility behavior change; no platform-specific divergence intended.

## 1. Runtime Model

Planets are authoritative world entities generated from Lua bundle data, but rendered on the client through a layered 2D shader path.

- Authoritative content authoring lives in Lua:
  - `data/scripts/bundles/starter/planet_body.lua`
  - `data/scripts/world/world_init.lua`
  - `data/scripts/assets/registry.lua`
- Shared schema lives in Rust:
  - `PlanetBodyShaderSettings`
- Client rendering lives in a unified 2D `PlanetVisualMaterial` shader family:
  - `data/shaders/planet_visual.wgsl`

This keeps art direction and per-body tuning in Lua while Rust owns replication schema, render plumbing, and shader execution. Shader delivery is still data-authored via `sprite_shader_asset_id`, while the planet material family keeps one stable bind/uniform contract across body, cloud, and ring passes.

## 2. Authoring Contract

Planets are authored through the `planet.body` Lua bundle.

The bundle emits:
- `display_name`
- `entity_labels`
- `owner_id`
- `size_m`
- `map_icon`
- `sprite_shader_asset_id`
- `world_position`
- `world_rotation`
- `planet_body_shader_settings`

Planets intentionally do **not** emit collision components and do not use Avian transform
components unless they become real physics entities.

The active settings contract now separates:
- `body_kind`
  - `0 = planet`
  - `1 = star`
  - `2 = black_hole`
- `planet_type`
  - `0 = terran/oceanic`
  - `1 = desert`
  - `2 = lava/volcanic`
  - `3 = ice/frozen`
  - `4 = gas_giant`
  - `5 = moon/rocky`

That split is important: surface families stay planet-specific, while stars and black holes no longer masquerade as a fake `planet_type = 6` exotic bucket.

The settings contract also now exposes runtime art-direction controls for debugging and tuning:
- `sun_intensity`
- `enable_surface_detail`
- `enable_craters`
- `enable_clouds`
- `enable_atmosphere`
- `enable_specular`
- `enable_night_lights`
- `enable_emissive`
- `enable_ocean_specular`

Planet seeds remain authored and persisted as ordinary integer values, but the client hashes and normalizes that seed before feeding it into shader uniforms. Raw large integer seeds must not be used directly in per-pixel trig/noise expressions.

## 3. Rendering Contract

Planets render as layered procedural billboards on quads:

- the parent entity holds world position/rotation
- the client spawns a body child on `PLANET_BODY_RENDER_LAYER`
- optional cloud and ring/accretion children are attached as separate renderables
- the planet family is offset to a lower z plane than normal world sprites
- planets render behind ships using z/depth layering and a client-only projected visual offset, while the authoritative root remains centered on the true world position
- optional layer `screen_scale_factor` can enlarge or reduce apparent planet screen size independently of parallax motion

The gameplay camera renders both:
- default world layer `0`
- planet layer `2`

This allows planets to sit visually behind ships without moving them into the fullscreen backdrop path.

The layered planet order is now:
- ring back
- cloud back
- body
- cloud front
- ring front

That order is deliberate. It gives 2.5D occlusion cues without real 3D geometry:
- backside clouds can peek around the limb before disappearing behind the planet body
- ring systems can be partially occluded by the body instead of reading as one flat decal

## 4. Shader Model

`planet_visual.wgsl` is a 2D side-view globe shader family, not a 3D PBR mesh shader.

Current body shader behavior:
- reconstructs the visible hemisphere from the quad silhouette
- rotates the sphere around a planetary axis for sideways globe spin
- samples deterministic procedural terrain and color from spherical coordinates
- derives a perturbed normal from the height field for bump-style lighting
- applies water/specular response from terran surface masks instead of a generic whole-body gloss
- handles only the body, atmosphere, and emissive response
- now uses a smoother noise path than the earlier grid-prone value-noise variant that was contributing visible lattice/banding artifacts

Current cloud shader behavior:
- renders clouds as dedicated back/front shell passes
- uses evolving weather-cell advection and domain warping instead of one static noise field
- uses softer layered billow noise instead of the previous scratchy line artifacts
- gates cloud coverage through density thresholds so cloud masses feel coherent instead of evenly noisy
- supports terran/oceanic and gas-giant cloud behavior separately

Current ring shader behavior:
- renders black-hole accretion disks as dedicated back/front passes
- renders optional gas-giant hero rings as separate back/front passes
- keeps ring/accretion visuals out of the planet body shader

This split removes the old artifact-prone “everything in one shader” path that was contaminating the terran surface with cloud/ring-looking line work.

## 5. Lighting / Bump Support

Planets support bump-style lighting in the shader today by deriving a perturbed normal from the procedural height field.

This is not yet a separate generated normal-map texture path like the asteroid CPU generator. For planets, the bump response currently comes from the shader’s own height sampling. If we later need cached normal maps for expensive hero planets, that can be added without changing the Lua authoring contract.

Current lighting state:
- planet body, cloud, and ring passes now consume shared world-light uniforms derived from replicated `EnvironmentLightingState`
- when a `PlanetBodyShaderSettings` entity with `body_kind = 1` exists, direct light direction is resolved per rendered entity from that star's world position instead of one shard-global direction
- `EnvironmentLightingState.primary_direction_xy` and `primary_elevation` are now fallback direct-light values for worlds that do not currently expose a star body
- `sun_intensity` is still a per-planet art-direction multiplier over the shared primary light, not a replacement for shard/system lighting
- with `sun_intensity = 0`, the body no longer keeps an artificial minimum daylight floor
- any remaining visibility at zero sun must come from explicit emissive or atmosphere response, not a hardcoded lit baseline
- planets now also receive one bounded dominant nearby local-light contribution, currently sourced from client-derived plume emitters
- full multi-emitter accumulation is still pending, so this is intentionally a cheap 2.5D approximation rather than arbitrary local-light stacking

## 6. Visual Tuning Notes

The current terran pass now follows the right structural direction:
- ocean base
- landmass mask
- shallow-water transition
- polar-ice transition
- separate cloud pass

Recent tuning also borrowed useful ideas from `docs/sample-planet-shadertoy`:
- stronger distinction between deep water and shallow water
- cleaner coastline transitions
- layered atmosphere behavior instead of hard edge glow
- water-weighted specular highlights
- thresholded cloud density shaping
- planet-as-globe shading instead of top-down projection logic

We are not copying the Shadertoy literally, but it is now being used as a reference for globe readability and terrain layering.

We also reviewed `docs/4d_smooth_noise_algoritm` as a useful reference for gas-giant tuning. The active gas-giant path now borrows that idea directly for time-evolving smooth-noise band and storm flow so gas giants animate without obvious looping or axis-locked drift, while still using the authored primary/secondary/tertiary palette controls.

## 7. World Bootstrap

Current starter content spawns one sample planet and one starter sun from `world_init.lua`.

The starter sun lives at `(0, 0)` with `body_kind = 1`, and the client uses that entity as the current direct-light source for planets, asteroids, and thruster plumes.

Like the asteroid field, this happens during one-time world bootstrap. To see layout or authored-setting changes in an existing local world, re-run world init by resetting the local DB or clearing the bootstrap marker and old generated entities.

## 8. Next Work

1. Fold backdrop radiance and bounded local-light accumulation into the shared lighting contract so planets react to more than the current global environment terms.
2. Add a dedicated atmosphere shell pass for hero planets if we need richer scattering than the current in-body atmosphere response.
3. Tighten gas-giant cloud and body coupling so the body bands, cloud shell, and any hero rings feel like one coherent visual family.
4. Add more authored body presets in Lua and dashboard tooling now that `body_kind` and the layered passes are stable.
5. Add tactical-map planet icon styling once planet entities are common in the world.
