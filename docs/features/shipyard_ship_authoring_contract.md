# Shipyard Ship Authoring Contract

Status: Active partial implementation spec
Last updated: 2026-04-28
Owners: dashboard + scripting + gameplay content + client presentation
Scope: dashboard Shipyard route, Lua ship/module registries, ship bundle generation path, hardpoint authoring
Primary references: `docs/core_systems_catalog_v1.md`, `docs/features/scripting_support.md`, `docs/features/asset_delivery_contract.md`, `docs/frontend_ui_styling_guide.md`, `crates/sidereal-game/src/components/hardpoint.rs`, `crates/sidereal-game/src/components/mounted_on.rs`

## 0. Implementation Status

2026-04-28 status note:
- Implemented in this slice: canonical Lua ship and module registries under `data/scripts/ships/` and `data/scripts/ship_modules/`, typed Rust registry definitions/loaders/validation in `sidereal-game` and `sidereal-scripting`, gateway/replication script-context accessors, generic `data/scripts/bundles/ship/body.lua`, and the dashboard `/shipyard` route with hardpoint overlay editing.
- Implemented dashboard V1 behavior: ship library search, draft badges, identity/visual/dimension/root payload editing, hardpoint table, mounted-module assignment, module library default editing, JSON component payload editing, validation panel, and draft/publish/discard APIs backed by the gateway script catalog.
- Hardpoint authoring uses the canonical local X/Y plane: `+X` is starboard/right, `+Y` is forward/up on the overlay, and V1-authored `offset_m[3]` must be `0`.
- Texture hardpoint authoring supports mouse wheel zoom, empty-space mouse panning, marker dragging, reset view, optional grid overlay, snap spacing from `0.1m` to `10m`, and mirror mode across local X with mirrored `x = -x`, same `y`, `z = 0`.
- Native impact: authoritative ship spawning now resolves `ship.corvette` and `ship.rocinante` through registry-authored definitions while preserving the existing bundle IDs and starter `controlled_bundle_id = "ship.corvette"`.
- WASM impact: no client authority or transport change. Browser/native clients continue consuming replicated entity/component results and asset IDs.

## 1. System Label

Shipyard V1 is cataloged as:

`system.shipyard_ship_authoring.v1` | `Shipyard Ship Authoring System V1`

The system covers dashboard ship authoring using Lua ship registries, texture-overlay hardpoint editing, module-library mounting, component payload editing, and script-catalog draft/publish workflow.

## 2. Registry Layout

Canonical ship registry files:

```text
data/scripts/ships/registry.lua
data/scripts/ships/<ship_slug>.lua
```

Canonical module registry files:

```text
data/scripts/ship_modules/registry.lua
data/scripts/ship_modules/<module_slug>.lua
```

Ship registry entries keep stable runtime bundle IDs:

```lua
{
  ship_id = "ship.corvette",
  bundle_id = "ship.corvette",
  script = "ships/corvette.lua",
  spawn_enabled = true,
  tags = { "starter", "combat", "small" },
}
```

Module definitions are global library defaults. Ship-mounted modules may override component payload fields through `mounted_modules[].component_overrides` without changing the module default.

## 3. Validation Contract

Rust loaders must reject:

1. duplicate `ship_id`, `bundle_id`, `module_id`, registry script paths, or per-ship `hardpoint_id`;
2. indexed scripts that are missing or whose returned IDs do not match the registry;
3. ship `visual_asset_id` values that do not exist in the asset registry or do not resolve to image content;
4. non-finite hardpoint coordinates or V1 hardpoints with non-zero `z`;
5. mounted modules that reference missing hardpoints or missing modules;
6. module mounts where `hardpoint.slot_kind` is not listed in `module.compatible_slot_kinds`;
7. module component kinds outside the generated component registry;
8. module-authored generated hierarchy/identity fields such as `parent_guid`, `mounted_on`, `owner_id`, and `entity_guid`.

## 4. Bundle Generation

`data/scripts/bundles/ship/body.lua` is the generic ship bundle builder for registry-authored ships.

It must:

1. load the ship definition by `ctx.bundle_id`;
2. synthesize exactly one root ship graph record;
3. synthesize deterministic hardpoint child records with `ParentGuid`;
4. synthesize mounted module records with `ParentGuid` and `MountedOn`;
5. preserve UUID/entity-ID-only hierarchy and mount references;
6. generate collision AABB/outline from the selected texture when `collision_from_texture = true`;
7. keep Avian runtime mass components and gameplay mass components synchronized from authored values.

## 5. Dashboard Contract

The `/shipyard` dashboard route follows the Genesis layout pattern:

1. left library for ships, search, tags, bundle IDs, draft badges, and texture summary;
2. main inspector for identity, visuals, dimensions, root components, hardpoints, module slots, and advanced payload editing;
3. texture workbench for visual hardpoint placement;
4. right action panel for validation, dirty state, save draft, publish, discard, and module-library edit state.

All reads require `requireDashboardAdmin(request, "scripts:read")`.
All mutations require `requireDashboardAdmin(request, "scripts:write")` and write through gateway script-catalog draft/publish APIs. Disk fallback is read-only and local-development only.

`GET /api/shipyard/assets/:assetId` may serve existing registry image bytes for preview, but it must resolve asset IDs through `assets/registry.lua` and must not expose arbitrary filesystem paths.

## 6. Migration Note

Current ship data moved from `data/scripts/bundles/ship/corvette.lua` and `data/scripts/bundles/ship/rocinante.lua` into registry definitions and module-library defaults.

Legacy hardpoint offsets were normalized into the Shipyard X/Y plane:

1. old `{ x, y, z }` values where `z` represented longitudinal placement become `{ x, z, 0 }`;
2. old values with `z = 0` and meaningful `y` keep `{ x, y, 0 }`;
3. all V1 hardpoints must serialize `z = 0`.

## 7. Out Of Scope For V1

V1 does not include live mutation of already persisted/spawned ships, texture uploads, full asset-registry editing, procedural ship generation, arbitrary nested module graphs beyond one module per hardpoint, or separate player-owned loadout persistence.
