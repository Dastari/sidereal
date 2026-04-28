# Procedural Planet Shaders

## Current Runtime Direction

The live client path is a layered **2D planet billboard** pipeline built on Bevy `Material2d`, not the older 3D/PBR sketch path.

2026-04-24 update: the active runtime shader became the unified `planet_visual.wgsl` family. Lua still authors a layered visual stack, but planet, cloud, ring, star, and black-hole passes selected behavior through the stable planet material uniform/pass contract instead of separate body/cloud/ring shader files.

2026-04-28 update: stars are split out to `star_visual.wgsl` with shader family `world_polygon_star`. The split keeps the same `PlanetBodyShaderSettings` uniform ABI for now, but star corona/photosphere logic no longer lives in the planet shader path.

Active runtime shaders:
- `planet_visual.wgsl`
- `star_visual.wgsl`

Reference-only legacy sketches still in the repo:
- `planet_core.wgsl`
- `planet_normal.wgsl`
- `planet_atmosphere.wgsl`
- `stellar_corona.wgsl`
- `planetary_rings.wgsl`

Those older files are useful idea banks, but they are **not** the active runtime contract.

## Runtime Contract

- shader source is delivered as normal streamed assets via the Lua asset registry
- per-body settings are authored in Lua bundle data and replicated through `PlanetBodyShaderSettings`
- the client renders a world-space layered billboard:
  - ring back pass
  - cloud back pass
  - body pass
  - cloud front pass
  - ring front pass
- planets are world entities with position/rotation and no collision

## Body Kind vs Surface Family

The active schema now separates body class from surface family.

### `body_kind`
- `0` = `planet`
- `1` = `star`
- `2` = `black_hole`

### `planet_type`
- `0` = terran / oceanic
- `1` = desert
- `2` = lava / volcanic
- `3` = ice / frozen
- `4` = gas giant
- `5` = moon / rocky

This replaced the earlier overloaded “exotic = 6” path.

Additional runtime tuning controls now exist for live debugging/art direction:
- `sun_intensity`
- `enable_surface_detail`
- `enable_craters`
- `enable_clouds`
- `enable_atmosphere`
- `enable_specular`
- `enable_night_lights`
- `enable_emissive`
- `enable_ocean_specular`

## Active Pass Responsibilities

### Body pass
Responsible for:
- side-view globe reconstruction from a quad
- sideways planetary spin
- procedural terrain/surface coloring
- terran water-mask specular response
- derivative-based bump-style lighting from the height field
- atmosphere/rim/emissive response
- black-hole event-horizon body rendering

Not responsible for:
- cloud overlays
- ring/accretion disks

### Cloud back/front passes
Responsible for:
- separate back/front cloud-shell passes for planet bodies
- terran/oceanic cloud masses
- gas giant cloud/band behavior
- thresholded cloud density shaping
- soft cloud coverage without the old line/scratch artifacts
- evolving weather-cell motion from domain-warped 3D fBm

### Ring back/front passes
Responsible for:
- black-hole accretion disks split into back/front arcs
- optional gas-giant hero rings split into back/front arcs

## Important Result

The visible line artifacts on terran planets came from overloading the old body shader with cloud/ring-like procedural overlays. The current runtime keeps those responsibilities as separate Lua-authored visual passes while using one shader family and one Rust material ABI, which is the current long-term direction.

2026-04-24 rendering update: `planet_visual.wgsl` now avoids the older animated 4D value-noise path for clouds/stars and uses time-evolving domain-warped 3D fBm instead. The active body pass also uses a simple cellular crater field for rocky bodies, derives normal perturbation from screen-space height derivatives instead of extra height resamples, and applies a smoother terminator response for twilight atmosphere and direct-light falloff.

2026-04-24 cloud update: cloud density now branches by planet type instead of evaluating terran and gas-family cloud functions for every cloud/shadow sample. Terran clouds use a cheaper broad-cell plus feathered-wisp field, and Lua-authored cloud passes use a lower shell scale so Aurelia-style planets keep weather visually close to the surface.

2026-04-28 star/corona update: the active `planet_visual.wgsl` star path no longer samples corona or photosphere noise from unwrapped longitude coordinates. Corona fields now fade before the render quad edge, which avoids hard rectangular flare cutoffs, star bodies reserve more in-material space for radial streamer/tendril prominences, and the photosphere uses deeper orange mids. Planet color grading is slightly desaturated and darkened to reduce the washed-out look.

2026-04-28 Helion palette update: the starter star uses more separated hot-gold, orange, ember, and red-orange authored colors. The star shader also preserves dark convection lanes more aggressively and uses star saturation/contrast authoring so tone mapping does not flatten the surface into a uniform yellow disc.

2026-04-28 authored hue update: star hue now comes from material color fields only. The shader uses scalar weighting for surface/corona detail instead of hidden yellow/orange RGB multipliers, so all-white authored colors produce neutral white star shading.

2026-04-28 photosphere update: star surface detail now combines sphere-space flow with screen-disc convection cells to avoid center-pole convergence artifacts. Corona prominences are one-sided limb lobes that extend outward from the edge rather than symmetric quad-space arcs. Near-grayscale authored star palettes are forced back to neutral luminance after grading so Genesis all-white color controls stay visually white.

2026-04-28 prominence update: the star corona is now an edge-emitted polar plasma field. Alpha starts at the authored star radius, flare length varies by angle, radial filament bands taper outward from the limb, and color/alpha share the same density field so prominences read as attached fiery plasma rather than detached smoky blobs.

2026-04-28 star shader split: `star_visual.wgsl` is now the active star body shader. It fakes three separate phenomena instead of treating the sun as one noisy halo: a bright limb corona, high-frequency radial wisps/tendrils generated in polar space from the star edge, and sparse procedural parabolic prominence loops that grow/fade without CPU-side event state. The planet shader remains responsible for planets, clouds, rings, and black holes.

2026-04-28 Genesis star control update: the star shader maps existing `PlanetBodyShaderSettings` fields to star-specific controls so authoring does not require a schema migration yet. `cloud_speed` controls flare/arc event rate, `spot_density` and `surface_activity` control flare count/density, `bands_count` controls active arc events, `corona_intensity` controls flare reach, `atmosphere_thickness`/`atmosphere_alpha` control glow size/alpha, `color_atmosphere_rgb` controls glow color, and `color_night_lights_rgb` controls the star back/shadow color visible behind rim plasma.

## Tuning Notes

Useful references for the current look direction:
- `docs/sample-planet-shadertoy`
- `PixelPlanets` layering ideas already explored earlier in the project

What we are taking from those references is structural:
- water depth layering
- cleaner coast transitions
- side-view globe readability
- separate cloud treatment

We are **not** trying to reproduce a pixel-art pipeline.

## Lua / Schema Interface

Lua exposes the planet shader through the asset registry as `planet_visual_wgsl` with shader/editor metadata. Stars use `star_visual_wgsl` / `world_polygon_star`. Starter planet bundles attach that asset ID through `sprite_shader_asset_id`, replicate persisted tuning through `PlanetBodyShaderSettings`, and compose the pass stack through `RuntimeWorldVisualStack`.

The dashboard-facing schema remains data-driven from the Lua asset registry and generated metadata. Shader implementation changes must preserve:
- source and generated cache parity for `planet_visual_wgsl` and `star_visual_wgsl`
- the existing `PlanetBodyShaderSettings` field names unless Rust/Lua/dashboard schemas are updated together
- the existing planet material uniform/pass ABI unless the client runtime and editor metadata are updated in the same change

## Next Rendering Work

1. Integrate the shared 2.5D lighting contract so planets consume the same environmental and local-light model as asteroids, ships, plumes, and backdrop materials.
2. Add a richer dedicated atmosphere shell pass if hero planets need more scattering than the current body-pass atmosphere response.
3. Expand authored presets now that `body_kind` and layered passes are part of the runtime contract.
