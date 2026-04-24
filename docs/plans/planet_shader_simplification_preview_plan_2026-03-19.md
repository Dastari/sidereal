# Planet Shader Simplification Preview Plan

Status: Proposed and partially implemented preview study  
Date: 2026-03-19  
Owners: client rendering + dashboard shader tooling

## 0. Status Note

- 2026-03-19: Added a new dashboard-preview-only shader asset, `planet_preview_study_wgsl`, to the Lua asset registry plus mirrored source/cache workbench files. This shader is intentionally not wired into the live runtime planet path yet. It exists to evaluate a simpler art direction and parameter model before replacing or refactoring `planet_visual.wgsl`.

## 1. Problem

The current runtime planet shader family is doing too much work in one ABI:

1. rocky planets,
2. gas giants,
3. stars,
4. black holes,
5. atmosphere response,
6. cloud pass behavior,
7. ring behavior,
8. in-shader grading/tone mapping.

That creates three practical problems:

1. visual tuning is hard because too many controls interact,
2. performance risk increases as more branches/noise paths accumulate,
3. color response tends to flatten or wash out because the shader is also doing its own grading/tone-map shaping.

## 2. Simplification Direction

The target direction is not "one shader per body forever". It is:

1. one smaller body-surface family with a small number of explicit surface modes,
2. separate atmosphere treatment,
3. separate cloud treatment,
4. separate ring treatment,
5. palette-first art direction instead of heavy procedural color synthesis,
6. post/scene tone mapping outside the planet body shader wherever possible.

For the first preview study, the surface families are:

1. moon / rocky,
2. Mars / desert,
3. gas giant,
4. star.

These cover the user-facing body classes we need to compare visually while keeping the first prototype bounded.

## 3. Reference Direction

The preview study is based on the recurring pattern across practical planet-shader references:

1. compact surface shading with a small authored palette,
2. limited noise families per surface type,
3. separate atmosphere/corona treatment,
4. preset-driven tuning rather than one giant control surface.

Useful references reviewed:

1. `threejs-procedural-planets`
   - compact mesh-based planet shader with direct palette layers and simple lighting
   - <https://github.com/dgreenheck/threejs-procedural-planets>
2. GPU Gems 2, Chapter 16, "Accurate Atmospheric Scattering"
   - reference for a dedicated atmosphere path rather than overloading the body shader
   - <https://developer.nvidia.com/gpugems/gpugems2/part-ii-shading-lighting-and-shadows/chapter-16-accurate-atmospheric-scattering>
3. Eric Bruneton, "Precomputed Atmospheric Scattering"
   - reference for the high-quality end of the atmosphere problem, not for the first preview shader
   - <https://ebruneton.github.io/precomputed_atmospheric_scattering/>
4. Andrew Yi, "Procedural Planet Generation Project"
   - useful summary of rocky/gas-giant palette and fractal structure choices
   - <https://andrewyibc.github.io/planet_generation/>
5. `jsulpis/realtime-planet-shader`
   - another practical separation of terrain, atmosphere, and clouds
   - <https://github.com/jsulpis/realtime-planet-shader>

## 4. What The Preview Shader Is Testing

`planet_preview_study.wgsl` is intentionally simpler than `planet_visual.wgsl`:

1. fullscreen preview-only composition for dashboard inspection,
2. one compact centered globe renderer,
3. rocky branch for moon/Mars-like bodies,
4. gas giant branch,
5. star branch,
6. simple palette-band coloring,
7. cheap rim atmosphere/corona treatment,
8. no runtime cloud/ring/black-hole coupling,
9. no in-shader tone-map curve beyond a final lightweight gamma encode for preview readability.

## 5. Migration Use

If the preview shader direction proves visually stronger, the runtime follow-up should be:

1. split `planet_visual.wgsl` responsibilities more aggressively,
2. move body surface selection toward smaller explicit families,
3. keep atmosphere/cloud/ring as distinct passes,
4. preserve the existing authoritative data model (`body_kind`, `planet_type`) while mapping those values into the smaller surface-family implementation.

The intended runtime mapping is:

1. `body_kind = 0`, `planet_type = 5` -> moon / rocky branch,
2. `body_kind = 0`, `planet_type = 1` -> Mars / desert branch,
3. `body_kind = 0`, `planet_type = 4` -> gas giant branch,
4. `body_kind = 1` -> star branch.

## 6. Non-Goals Of This Preview

This preview asset does not yet attempt to solve:

1. the final runtime world-polygon ABI,
2. cloud pass replacement,
3. ring pass replacement,
4. black-hole rendering,
5. full physically-based atmospheric scattering,
6. production LOD strategy.

It is a look-development and simplification study first.
