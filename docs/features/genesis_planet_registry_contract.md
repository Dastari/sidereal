# Genesis Planet Registry

Status: Active partial implementation spec
Last updated: 2026-04-28
Owners: dashboard + scripting + gameplay content + client rendering
Scope: Genesis dashboard planet authoring, Lua planet registry files, and typed runtime registry exposure
Primary references:
- `docs/features/procedural_planets.md`
- `docs/features/scripting_support.md`
- `docs/features/shader_editor_dashboard_implementation_spec.md`
- `docs/frontend_ui_styling_guide.md`
- `docs/core_systems_catalog_v1.md`

## 0. Implementation Status

- 2026-04-24: Initial implementation has started. Lua planet definitions now live under `data/scripts/planets/` with one named file per planet/celestial body and a `planets/registry.lua` index. `crates/sidereal-scripting` validates the registry and `PlanetBodyShaderSettings` payloads, and replication/gateway script contexts expose the validated definitions to `world_init.lua`. Native impact: starter planet/star content is moving from inline `world_init.lua` tables to registry-authored definitions while preserving the existing `planet.body` bundle and render path. WASM impact: no client authority split; browser impact is limited to dashboard tooling and shared shader preview paths.
- 2026-04-24: Stage 2 dashboard authoring has begun. `/genesis` now loads full editable planet definitions, exposes metadata/spawn/shader controls, supports deterministic randomization from the selected seed, and proxies save/publish/discard actions through script-catalog draft APIs for the planet file plus `planets/registry.lua`. Dashboard mutations are guarded by the existing dashboard admin session and Zod request validation. Remaining Stage 2 work: richer live shader preview integration, create/delete planet flows, and end-to-end gateway validation tests for saved Lua.
- 2026-04-26: `/genesis` now includes an initial live WebGPU visual preview panel for the selected planet definition. The preview reuses the dashboard shader preview renderer, loads the registry-declared `planet_visual_shader_asset_id` from the shader catalog, and maps `PlanetBodyShaderSettings` into the same `PlanetBodyUniforms` layout used by the native client main planet pass. Native impact: none; this is dashboard-only preview tooling. WASM impact: uses the existing browser WebGPU shader preview path, not the Bevy client runtime. Remaining preview work: exact multi-pass composition parity for clouds/rings/corona and tighter validation against generated shader editor ranges.
- 2026-04-26: Genesis catalog reads now fall back to repository disk files under `data/scripts/planets/` when the gateway script-catalog API is unavailable, so local dashboard-only development still shows Helion and Aurelia. Save, publish, and discard remain gateway script-catalog operations and do not write directly to disk from the dashboard.
- 2026-04-27: The `/genesis` preview now uses normal alpha blending in the dashboard preview pipeline and renders the planet visual shader in the same body/cloud/ring pass categories used by the native client, with pass flags rather than separate shader assets. The dashboard form exposes the currently authored cloud alpha/scale/speed, atmosphere alpha/falloff, corona size/intensity, and related lighting/weather controls. Remaining preview work: exact native transform scale/depth parity for all pass overlays and range metadata generated from the Lua shader editor schema.
- 2026-04-28: Genesis now supports the split star shader asset path. Star definitions should use `planet_visual_shader_asset_id = "star_visual_wgsl"` so the preview and runtime load `star_visual.wgsl`; changing Body Kind to Star in the dashboard switches the draft shader asset to the star asset, and changing away from Star restores the planet asset when the star default was active. Native impact: mirrors the client `world_polygon_star` material family. WASM impact: dashboard preview uses the existing browser shader-preview renderer against the new star source.

## 1. Purpose

Genesis is the dedicated planet/celestial authoring module for Sidereal. It exists because the generic shader workshop edits WGSL and shader metadata, but planet creation needs a higher-level content workflow:

1. named planet/celestial library entries,
2. deterministic randomization,
3. typed `PlanetBodyShaderSettings` controls,
4. preview through the existing planet visual shader family,
5. write-back to Lua content,
6. script-catalog draft/publish rather than direct runtime mutation.

Genesis does not replace the `planet.body` bundle. The bundle remains the graph-record factory for authoritative entities. Genesis authors reusable data definitions that the bundle consumes.

## 2. Core System Catalog Label

Genesis belongs to:

- Human title: `Genesis Planet Authoring System V1`
- Stable label: `system.genesis_planet_authoring.v1`
- Short slug: `genesis_planet_authoring_v1`

It is a content-authoring support system layered over `system.planet.v1`, `system.rendering.v1`, and `system.scripting_content_authoring.v1`.

## 3. Lua Authoring Contract

Canonical files:

```text
data/scripts/planets/registry.lua
data/scripts/planets/<planet_slug>.lua
```

`registry.lua` returns:

```lua
return {
  schema_version = 1,
  planets = {
    {
      planet_id = "planet.aurelia",
      script = "planets/aurelia.lua",
      spawn_enabled = true,
      tags = { "starter", "terran" },
    },
  },
}
```

Each planet file returns a data table:

```lua
return {
  planet_id = "planet.aurelia",
  display_name = "Aurelia",
  entity_labels = { "Planet", "CelestialBody" },
  tags = { "starter", "terran" },
  spawn = {
    entity_id = "0012ebad-0000-0000-0000-000000000010",
    owner_id = "world:system:starter",
    size_m = 760.0,
    spawn_position = { 8000.0, 0.0 },
    spawn_rotation_rad = 0.0,
    map_icon_asset_id = "map_icon_planet_svg",
    planet_visual_shader_asset_id = "planet_visual_wgsl",
  },
  shader_settings = {
    -- PlanetBodyShaderSettings-compatible fields.
  },
}
```

Rules:

1. `planet_id` is unique across `planets/registry.lua`.
2. `script` must be a relative `.lua` path under the active script catalog.
3. The planet file `planet_id` must match the registry entry.
4. `spawn_enabled=true` requires a `spawn` table and UUID `spawn.entity_id`.
5. `shader_settings` must decode to `PlanetBodyShaderSettings`.
6. `body_kind` remains `0 = planet`, `1 = star`, `2 = black_hole`.
7. `planet_type` remains `0..5` as documented in `docs/features/procedural_planets.md`.
8. Planet files are content data, not direct graph-record builders.

## 4. Runtime Contract

Rust owns validation and typed resources:

1. `sidereal_game::PlanetRegistry`
2. `sidereal_game::PlanetRegistryEntry`
3. `sidereal_game::PlanetDefinition`
4. `sidereal_game::PlanetSpawnDefinition`

`crates/sidereal-scripting` owns Lua decoding through:

1. `load_planet_registry_from_root`
2. `load_planet_registry_from_sources`

Replication and gateway contexts expose validated planet definitions to Lua world bootstrap through:

```lua
ctx.load_planet_definitions()
```

`world_init.lua` then maps enabled definitions into `ctx.spawn_bundle_graph_records("planet.body", ...)`.

## 5. Genesis Dashboard Direction

Route:

```text
/genesis
/genesis/$planetId
```

V1 panels:

1. Library: planet list, search, body-kind filters, tags, draft status.
2. Preview: planet/star visual preview using `planet_visual_wgsl` or `star_visual_wgsl` and `PlanetBodyShaderSettings`.
3. Inspector: identity, spawn metadata, shader settings, randomize controls, save/publish actions.

Write path:

1. Save draft through gateway `/admin/scripts/draft/{*script_path}`.
2. Publish through gateway `/admin/scripts/publish/{*script_path}`.
3. Update `planets/registry.lua` as a draft when spawn-enabled status or tags change, and when creating/removing entries.
4. Do not directly write dashboard edits to `data/scripts` in normal Genesis flows.

Current dashboard API surface:

```text
GET /api/genesis/planets
POST /api/genesis/planets/:planetId/draft
POST /api/genesis/planets/:planetId/publish
DELETE /api/genesis/planets/:planetId/draft
```

## 6. Randomization Contract

Genesis randomization is deterministic from a seed and selected body family. It must produce values that validate as `PlanetBodyShaderSettings` and stay within the shader editor schema ranges in `data/scripts/assets/registry.lua`.

Randomize affects only the local draft until the operator saves. Runtime services consume changes only after script draft publish/reload semantics already defined by `docs/features/scripting_support.md`.

## 7. Out of Scope for V1

1. Live mutation of already-persisted planet entities.
2. Galaxy-scale batch generation.
3. Orbital simulation.
4. New shader ABI fields.
5. New physics behavior for static planets.

## 8. Tests and Acceptance

Minimum coverage:

1. Scripting tests load the repository planet registry.
2. Scripting tests reject duplicate planet IDs and missing planet scripts.
3. World-init graph-record tests continue to spawn the starter planet and star through `planet.body`.
4. Dashboard Genesis APIs use Zod validation and `requireDashboardAdmin` for mutations.
5. Native/WASM client checks remain required when shared client/runtime planet behavior changes.
