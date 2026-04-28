# Core Systems Catalog (V1)

Status: Active reference
Date: 2026-03-13
Owners: architecture + gameplay + client runtime + replication

Primary references:
- `docs/sidereal_design_document.md`
- `docs/sidereal_implementation_checklist.md`
- `docs/features/README.md`

2026-03-13 status note:
- This document establishes stable "Version 1" labels for major Sidereal systems that are already implemented, partially implemented, or clearly planned in the current repo.
- These labels are intended to give later design docs, plans, tickets, and dashboard/editor surfaces a consistent naming baseline.
- This catalog is a reference/index, not a replacement for the deeper contracts and plans it links to.

2026-04-26 status note:
- Asteroid fields now have a V2 system label for the field-root, fracture, resource-profile, and ambient-presentation replacement direction.
- `system.asteroid_field.v1` remains the label for the current deterministic eager field-member baseline and earlier root-expansion planning.
- New implementation work that introduces first-class field roots, deterministic member lineage, fracture/depletion state, Lua-authored resource profiles, or field ambient fullscreen effects should use `system.asteroid_field.v2`.

2026-04-27 status note:
- Visibility now has a V2 system label for the signal-detection, redacted unknown-contact, stable approximate-position, and zoom-safe culling direction; the first partial implementation is active.
- `system.visibility_replication.v1` remains the label for the current implemented authorization, delivery narrowing, payload redaction, and static-landmark discovery baseline.
- New implementation work that adds `SignalSignature`, unknown tactical contacts, signal-triggered landmark discovery, scanner contact-resolution accuracy, or zoom-out delivery/culling hysteresis should use `system.visibility.v2`.

2026-04-27 status note:
- `system.visibility.v2` now also covers the server-authoritative tactical scanner source rule: scanner-derived live fog cells, full contacts, signal-only unknown contacts, and gravity-well signal notifications are driven by the authenticated player's currently controlled non-player-anchor entity.
- Root and direct-mounted `ScannerComponent` sources are the current implemented source set. Scanner-tier payload redaction and the tactical contact spatial index remain active follow-up work under the same V2 label.

2026-04-28 status note:
- Lighting now has a V2 system label for the top-2 stellar, top-8 local emitter, authored-falloff, and deep-space ambient material contract.
- `system.lighting.v1` remains the current baseline for shared environment lighting and one dominant local contribution.
- New implementation work that changes world-facing material lighting, stellar falloff, dynamic light emitters, or lit generic sprite/ship rendering should use `system.lighting.v2`.

2026-04-28 update:
- `system.lighting.v2` implementation has started in client presentation: asteroids, runtime effects, generic world sprites, and planet materials now use or embed the shared V2 uniform contract. Remaining work is material response tuning, debug UI, and rollout cleanup.

2026-04-28 status note:
- `system.shipyard_ship_authoring.v1` is now the dashboard/content-authoring label for Shipyard ship authoring. V1 covers Lua ship/module registries, texture-overlay hardpoint editing, module-library mounting, component payload editing, and the script-catalog draft/publish workflow.

## 1. Purpose

Use this catalog when we need a stable label for a core system, for example:

1. feature docs,
2. plans,
3. task tracking,
4. dashboard/editor labeling,
5. future versioned system follow-ups (`V2`, `V3`, and so on).

## 2. Naming Rules

For each system, keep three stable forms:

1. Human title:
   - `Audio System V1`
2. Stable label:
   - `system.audio.v1`
3. Short slug:
   - `audio_v1`

If a system later gets a replacement or major architecture reset, keep the old label for historical docs and introduce a new one such as `system.audio.v2`.

## 3. Gameplay and World Systems

| Label | Human title | Current baseline | Primary references |
| --- | --- | --- | --- |
| `system.input_control.v1` | Input and Control System V1 | Active implementation. Client input is intent-only and routes through authoritative control/session binding. | `bins/sidereal-client/src/runtime/input.rs`, `bins/sidereal-client/src/runtime/control.rs`, `bins/sidereal-replication/src/replication/input.rs`, `bins/sidereal-replication/src/replication/control.rs` |
| `system.flight_gnc.v1` | Flight and GNC System V1 | Partial live baseline. Current flight stack is active; the fly-by-wire thrust-allocation replacement is the approved V1 direction. | `crates/sidereal-game/src/flight.rs`, `docs/features/fly_by_wire_thrust_allocation_contract.md`, `docs/decisions/dr-0034_fly_by_wire_thrust_allocation_and_gnc_stack.md` |
| `system.character_movement.v1` | Character Movement System V1 | Active implementation in shared gameplay code for non-ship/player movement flows. | `crates/sidereal-game/src/character_movement.rs`, `crates/sidereal-game/src/components/character_movement_controller.rs`, `crates/sidereal-game/tests/character_movement.rs` |
| `system.combat_projectile.v1` | Combat and Projectile System V1 | Active implementation. Authoritative weapon fire, projectile travel, impact resolution, and damage application already exist in shared/server runtime. | `crates/sidereal-game/src/combat.rs`, `docs/features/projectile_firing_game_loop.md`, `bins/sidereal-replication/src/replication/combat.rs` |
| `system.destruction_lifecycle.v1` | Destruction Lifecycle System V1 | Planned/proposed. Generic health-depleted to destroyed lifecycle flow is being formalized beyond raw health reduction. | `crates/sidereal-game/src/components/destructible.rs`, `docs/plans/destructible_lifecycle_component_plan_2026-03-13.md`, `docs/features/scripting_support.md` |
| `system.inventory_mass.v1` | Inventory and Mass System V1 | Active implementation. Inventory-bearing entities feed dynamic mass derivation and runtime physics mass updates. | `crates/sidereal-game/src/components/inventory.rs`, `crates/sidereal-game/src/mass.rs`, `crates/sidereal-game/tests/mass.rs` |
| `system.modular_hierarchy.v1` | Modular Hierarchy and Mounting System V1 | Active implementation. Parent-child and hardpoint mount relationships persist and hydrate as deterministic graph relationships. | `crates/sidereal-game/src/hierarchy.rs`, `crates/sidereal-game/src/components/hardpoint.rs`, `crates/sidereal-game/src/components/mounted_on.rs`, `docs/sidereal_design_document.md` |
| `system.background_world_simulation.v1` | Background World Simulation System V1 | Proposed contract. Offscreen economy, traffic, faction pressure, and actor residency are defined as a tiered authoritative simulation lane. | `docs/features/background_world_simulation_contract.md`, `docs/decisions/dr-0033_background_world_simulation_tiering.md`, `docs/features/galaxy_world_structure.md` |

## 4. Visibility, Tactical, and Navigation Systems

| Label | Human title | Current baseline | Primary references |
| --- | --- | --- | --- |
| `system.visibility_replication.v1` | Visibility and Replication System V1 | Active source-of-truth. Server-owned authorization, delivery narrowing, and payload redaction are the canonical visibility pipeline. | `docs/features/visibility_replication_contract.md`, `bins/sidereal-replication/src/replication/visibility.rs`, `bins/sidereal-client/src/runtime/replication.rs` |
| `system.visibility.v2` | Visibility System V2 | Active partial implementation. V2 layers server-authoritative controlled-entity scanner sources, generic signal detection, redacted unknown tactical contacts with stable approximate positions, signal-triggered static-landmark discovery, and zoom-safe delivery/client culling over the V1 authorization/redaction baseline. | `docs/features/visibility_system_v2_signal_detection_contract.md`, `docs/decisions/dr-0037_visibility_signal_detection_and_stable_unknown_contacts.md`, `docs/features/tactical_and_owner_lane_protocol_contract.md`, `docs/features/procedural_planets.md` |
| `system.fog_of_war_intel.v1` | Fog of War and Intel Memory System V1 | Active direction with live tactical lane support. Player exploration is permanent, live intel is temporary, and stale intel is server-stored memory. | `docs/decisions/dr-0018_fog_of_war_and_intel_memory_model.md`, `docs/features/tactical_and_owner_lane_protocol_contract.md`, `crates/sidereal-game/src/components/player_explored_cells.rs` |
| `system.tactical_map.v1` | Tactical Map System V1 | Active implementation. The client maintains tactical fog/contact caches and server-driven tactical snapshots/deltas. | `bins/sidereal-client/src/runtime/tactical.rs`, `docs/features/tactical_and_owner_lane_protocol_contract.md`, `crates/sidereal-game/src/components/tactical_map_ui_settings.rs` |
| `system.owner_asset_manifest.v1` | Owner Asset Manifest System V1 | Active implementation. Owned-asset UI data is delivered as a separate owner-only read model, not inferred from local-bubble world replication. | `bins/sidereal-client/src/runtime/owner_manifest.rs`, `bins/sidereal-replication/src/replication/owner_manifest.rs`, `docs/features/tactical_and_owner_lane_protocol_contract.md` |
| `system.static_landmark_discovery.v1` | Static Landmark Discovery System V1 | Active implementation. Static celestial landmarks use durable player discovery state and post-discovery authorization rules separate from live scanner overlap. | `crates/sidereal-game/src/components/discovered_static_landmarks.rs`, `crates/sidereal-game/src/components/static_landmark.rs`, `docs/decisions/dr-0032_discovered_static_landmark_visibility.md`, `docs/features/visibility_replication_contract.md` |

## 5. World Content and Presentation Systems

| Label | Human title | Current baseline | Primary references |
| --- | --- | --- | --- |
| `system.rendering.v1` | Rendering and Shader Composition System V1 | Active implementation. Client world rendering is converging on Lua-authored render-layer definitions, world-visual stacks, and shader-family material paths. | `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`, `docs/decisions/dr-0029_runtime_shader_family_taxonomy_and_lua_authoring_model.md`, `bins/sidereal-client/src/runtime/visuals.rs`, `bins/sidereal-client/src/runtime/render_layers.rs` |
| `system.camera.v1` | Camera System V1 | Active implementation. Gameplay camera follow, view-derived delivery scope, and rendering integration are core client runtime concerns. | `bins/sidereal-client/src/runtime/camera.rs`, `docs/sidereal_implementation_checklist.md`, `docs/ui_design_guide.md` |
| `system.lighting.v1` | Lighting System V1 | Active implementation. Shared environment-lighting state now drives client lighting for planets, asteroids, and related world visuals. | `bins/sidereal-client/src/runtime/lighting.rs`, `crates/sidereal-game/src/components/environment_lighting_state.rs`, `docs/features/procedural_planets.md`, `docs/features/scripting_support.md` |
| `system.lighting.v2` | Lighting System V2 | Active implementation direction. V2 replaces the single-primary/single-local material contract with top-2 stellar lights, authored stellar falloff, a deep-space ambient floor, top-8 dynamic local emitters, and lit world-sprite/ship material participation. | `docs/plans/lighting_v2_overhaul_plan.md`, `docs/plans/lighting_model_and_dynamic_space_events_plan.md`, `docs/decisions/dr-0038_lighting_v2_material_contract.md`, `bins/sidereal-client/src/runtime/lighting.rs`, `crates/sidereal-game/src/components/environment_lighting_state.rs` |
| `system.post_process.v1` | Post-Process Effects System V1 | Active implementation with planned growth. Camera-scoped authored post-process passes now exist as a distinct runtime lane. | `bins/sidereal-client/src/runtime/post_process.rs`, `crates/sidereal-game/src/components/runtime_post_process_stack.rs`, `docs/plans/destructible_lifecycle_component_plan_2026-03-13.md` |
| `system.audio.v1` | Audio System V1 | Proposed contract with minimal current baseline. The repo now has an explicit reusable audio runtime direction and audio registry crate/test surface. | `docs/features/audio_runtime_contract.md`, `docs/plans/audio_runtime_implementation_plan_2026-03-13.md`, `crates/sidereal-audio/`, `bins/sidereal-client/src/runtime/audio.rs` |
| `system.planet.v1` | Planet System V1 | Active implementation. Static celestial bodies are Lua-authored authoritative entities rendered through layered 2D shader passes. | `docs/features/procedural_planets.md`, `crates/sidereal-game/src/components/planet_body_shader_settings.rs`, `bins/sidereal-client/src/runtime/visuals.rs` |
| `system.genesis_planet_authoring.v1` | Genesis Planet Authoring System V1 | Active partial implementation. Dedicated planet/celestial authoring uses Lua planet registry files, typed validation, dashboard metadata/spawn/shader editing, deterministic randomization, and script-catalog draft/publish workflow. | `docs/features/genesis_planet_registry_contract.md`, `docs/features/procedural_planets.md`, `data/scripts/planets/registry.lua`, `crates/sidereal-scripting/src/lib.rs`, `dashboard/src/features/genesis/GenesisPage.tsx` |
| `system.shipyard_ship_authoring.v1` | Shipyard Ship Authoring System V1 | Active partial implementation. Dashboard ship authoring uses Lua ship/module registries, texture-overlay hardpoint editing with pan/zoom, optional grid snap and mirror mode, module-library mounting, component payload editing, and script-catalog draft/publish workflow. | `docs/features/shipyard_ship_authoring_contract.md`, `data/scripts/ships/registry.lua`, `data/scripts/ship_modules/registry.lua`, `data/scripts/bundles/ship/body.lua`, `crates/sidereal-scripting/src/lib.rs`, `dashboard/src/features/shipyard/ShipyardPage.tsx` |
| `system.asteroid_field.v1` | Asteroid Field System V1 | Active legacy baseline. Current live implementation uses deterministic Lua-authored eager asteroid member generation with procedural sprite/collision payloads. | `docs/features/procedural_asteroids.md`, `docs/features/asteroid_field_system.md`, `data/scripts/bundles/starter/asteroid_field.lua` |
| `system.asteroid_field.v2` | Asteroid Field System V2 | Active implementation direction. V2 replaces the eager-member baseline with first-class persisted field roots, deterministic member lineage, zero-health fracture, field-owned depletion/resource state, Lua-authored ore profiles, and optional field ambient fullscreen/post-process presentation. | `docs/features/asteroid_field_system_v2.md`, `docs/features/resources_and_crafting_contract.md`, `docs/features/scripting_support.md`, `data/scripts/bundles/starter/asteroid_field.lua` |

## 6. Content and Runtime Support Systems

| Label | Human title | Current baseline | Primary references |
| --- | --- | --- | --- |
| `system.scripting_content_authoring.v1` | Scripting and Content Authoring System V1 | Active contract. Lua owns authoritative content scripting, bundle authoring, asset registry definitions, and validated high-level world mutation intents. | `docs/features/scripting_support.md`, `crates/sidereal-scripting/src/lib.rs`, `bins/sidereal-replication/src/replication/scripting.rs`, `bins/sidereal-replication/src/replication/runtime_scripting.rs` |
| `system.asset_delivery_cache.v1` | Asset Delivery and Cache System V1 | Active contract. Asset payloads are delivered through authenticated gateway HTTP and stored in the MMO-style client cache. | `docs/features/asset_delivery_contract.md`, `crates/sidereal-asset-runtime/src/lib.rs`, `bins/sidereal-client/src/runtime/assets.rs`, `bins/sidereal-replication/src/replication/assets.rs` |
| `system.prediction_reconciliation.v1` | Prediction and Reconciliation System V1 | Active implementation with tuning still in progress. Lightyear-native prediction, rollback, interpolation, and correction behavior are part of the current client/runtime baseline. | `docs/features/prediction_runtime_tuning_and_validation.md`, `bins/sidereal-client/src/runtime/motion.rs`, `bins/sidereal-client/src/runtime/plugins/replication_plugins.rs`, `docs/sidereal_implementation_checklist.md` |

## 7. Notes on Scope

This catalog intentionally mixes:

1. gameplay systems,
2. tactical/intel systems,
3. presentation systems,
4. supporting runtime systems that materially shape player-visible behavior.

That is deliberate. In the current repo, systems such as rendering, visibility, audio, prediction, and asset delivery are not separable from the game feature surface.

## 8. Recommended Immediate Usage

When creating new docs, tickets, or editor labels, prefer the exact titles and labels in this catalog, for example:

1. `Audio System V1` / `system.audio.v1`
2. `Rendering and Shader Composition System V1` / `system.rendering.v1`
3. `Tactical Map System V1` / `system.tactical_map.v1`
4. `Planet System V1` / `system.planet.v1`
5. `Fog of War and Intel Memory System V1` / `system.fog_of_war_intel.v1`

If a new major system shows up repeatedly in code/docs and is not covered here, add it to this catalog in the same change that introduces or formalizes it.
