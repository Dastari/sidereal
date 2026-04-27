# Asteroid Field System V2

Status: Active partial implementation spec
Last updated: 2026-04-26
Owners: gameplay simulation + persistence + scripting + client rendering
Scope: first-class asteroid field roots, deterministic member lineage, fracture/depletion, ore/resource profiles, and field ambient presentation
Primary references: `docs/core_systems_catalog_v1.md`, `docs/features/procedural_asteroids.md`, `docs/features/resources_and_crafting_contract.md`, `docs/features/scripting_support.md`, `docs/features/visibility_replication_contract.md`, `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`

## 0. Implementation Status

- 2026-04-26: V2 supersedes the earlier V1 field-root proposal as the active implementation direction. Implemented in the first V2 slice: `asteroid.field` root authoring, V2 field/member/resource/fracture/ambient components, linked eager starter members, deterministic member/child key helpers, zero-health member fracture into linked child entities, field damage-state updates, procedural sprite style extensions, and asteroid shader style pass. Still pending: proximity activation/retirement, client-side ambient field blending, mining/extraction actions, and dashboard/editor V2 surfaces. Native impact: shared/server fracture is active; native client rendering consumes the updated procedural sprite/shader payloads. WASM impact: gameplay/procedural/fracture logic remains target-shared; platform differences stay at asset/render/transport boundaries.

## 1. Purpose

Asteroid-heavy regions should be authored and reasoned about as coherent world features, not only as a flat list of unrelated rocks. V2 makes an asteroid field a durable gameplay entity with an authored center, radius, shape, density, clusters, seed, deterministic member slots, server-authoritative fracture, field-owned depletion/resource state, and optional ambient presentation when the player enters the field.

## 2. Relationship To V1

`system.asteroid_field.v1` means the current eager member model:

1. `world_init.lua` calls `spawn_bundle_graph_records("asteroid.field_member", ...)`.
2. Each asteroid is persisted as an independent entity.
3. Procedural sprites and collision outlines are deterministic from the entity id and `procedural_sprite`.
4. No root field entity owns density, resource, fracture, or depletion state.

`system.asteroid_field.v2` keeps the successful V1 rendering/collision pieces, but changes the authoring and authority model:

1. world scripts place `asteroid.field` root entities;
2. members carry `AsteroidFieldMember` linkage;
3. fracture/depletion writes back to field-owned state keyed by deterministic member keys;
4. resource/ore composition is authored as field/member profile data, not hardcoded Rust tables.

`asteroid.field_member` stays as a migration helper and isolated-rock authoring path until field-root activation fully replaces eager spawning.

## 3. Non-Negotiable Rules

1. The server decides member spawning, health loss, fracture, depletion, and mining output.
2. Clients never authoritatively split asteroids or create resource outputs.
3. Field/member identity crossing scripts, persistence, replication, and UI uses UUIDs or deterministic string keys, never raw Bevy `Entity` IDs.
4. Persisted field/member state uses graph records/components and graph relationships where relationships are needed.
5. Active asteroid members reuse existing generic components where applicable: `HealthPool`, `Destructible`, `MassKg`, `SizeM`, collision components, `VisualAssetId`, `SpriteShaderAssetId`, `ProceduralSprite`, and Avian motion components.
6. Ore/resource composition is data-authored through Lua profiles and stable logical IDs such as `item_id` and `extraction_profile_id`.
7. Fracture uses fixed-step simulation flow. Frame-time deltas are presentation-only.
8. Field ambient dust/background/foreground effects are presentation support only. They must not mutate authoritative motion, visibility, mining, or depletion state.

## 4. Entity Model

### 4.1 Field Root

The primary authored primitive is `asteroid.field`. It emits one persisted root entity with:

1. `AsteroidField`;
2. `AsteroidFieldLayout`;
3. `AsteroidFieldPopulation`;
4. `AsteroidFieldDamageState`;
5. optional `AsteroidFieldAmbient`;
6. normal public metadata such as `DisplayName`, `EntityLabels`, `MapIcon`, `WorldPosition`, and `WorldRotation`.

The root is the durable owner of layout, density, resource profile selection, fracture policy selection, and depletion history.

### 4.2 Active Members

Active members are ordinary gameplay entities while they are interactable. They carry generic gameplay components for health, destruction, mass, collision, visuals, and motion plus `AsteroidFieldMember` for field linkage and deterministic lineage.

During the first V2 phase, the starter field may still eagerly spawn all initial members. The important change is that every member has root linkage and deterministic member keys, so later activation/retirement can be added without changing the member schema.

## 5. Component Contract

Initial V2 components:

1. `AsteroidField`
   - `field_profile_id`
   - `content_version`
   - `layout_seed`
   - `activation_radius_m`
   - `field_radius_m`
   - `max_active_members`
   - `max_active_fragments`
   - `max_fracture_depth`
   - `ambient_profile_id`
2. `AsteroidFieldLayout`
   - `shape`
   - `density`
   - `clusters`
   - `spawn_noise_amplitude_m`
   - `spawn_noise_frequency`
3. `AsteroidFieldPopulation`
   - target tier counts
   - size ranges by tier
   - `sprite_profile_id`
   - `resource_profile_id`
   - `fracture_profile_id`
4. `AsteroidFieldDamageState`
   - member entries keyed by `member_key`
   - state: `Intact`, `Activated`, `Fractured`, `Depleted`, `Harvested`
   - remaining health/mass where needed
   - spawned child keys
   - consumed resource units
5. `AsteroidFieldMember`
   - `field_entity_id`
   - `cluster_key`
   - `member_key`
   - `parent_member_key`
   - `size_tier`
   - `fracture_depth`
   - `resource_profile_id`
   - `fracture_profile_id`
6. `AsteroidFractureProfile`
   - child count ranges by tier
   - impulse range
   - mass retention ratio
   - terminal debris loss ratio
7. `AsteroidResourceProfile`
   - resource profile id
   - extraction profile id
   - yield table of logical `item_id` values and weights
   - depletion pool units
8. `AsteroidFieldAmbient`
   - trigger radius and fade band
   - optional background, foreground, and post-process shader asset ids
   - maximum intensity

## 6. Fracture Contract

Fracture is triggered when an active member with `AsteroidFieldMember`, `HealthPool`, and a fracture-capable profile reaches zero health.

Resolution order:

1. read parent `AsteroidFieldMember`, `HealthPool`, `MassKg`, `SizeM`, position, velocity, and fracture profile;
2. update the owning root `AsteroidFieldDamageState` by `member_key`;
3. derive child count and child keys from field id, parent key, content version, and child index;
4. write child state entries before spawning child entities;
5. spawn child entities one size tier smaller with deterministic offsets, reduced mass, derived health, related `ProceduralSprite`, and normal collision components;
6. mark the parent `Fractured` or `Depleted`;
7. despawn or tombstone the parent runtime entity.

Baseline tier behavior:

1. `Massive` -> `Large`;
2. `Large` -> `Medium`;
3. `Medium` -> `Small`;
4. `Small` -> terminal depletion, no more physics children.

Child total mass must be less than or equal to parent remaining mass multiplied by the profile's mass retention ratio. The remainder represents dust, lost fragments, or future resource output.

## 7. Visual Direction

V2 asteroids should read as a top-down space ARPG sprite style rather than soft spherical PBR rocks.

Shader and generator requirements:

1. chunkier silhouettes with chipped facets;
2. more visible shape families: rocky, carbonaceous, metallic, shard-like, and gem-rich;
3. posterized lighting bands;
4. darker readable rim/edge shading;
5. cracks and craters that survive sprite scaling;
6. controlled pixelation/quantization rather than blurry noise;
7. mineral accents driven by procedural sprite/resource profile data.

`data/shaders/asteroid.wgsl` remains a world-sprite shader. It should improve the style within the existing sprite material contract instead of adding a new ad-hoc material path.

## 8. Lua Authoring

V2 Lua authoring surfaces:

1. `data/scripts/asteroids/registry.lua`
   - field profiles;
   - sprite profiles;
   - fracture profiles;
   - resource profiles;
   - ambient profiles.
2. `asteroid.field`
   - emits root entity and V2 components;
   - may eagerly emit linked member entities during Phase 1.
3. `asteroid.field_member`
   - remains for isolated rocks and migration comparison;
   - should emit `AsteroidFieldMember` when called from a field root context.

Rust validates Lua-authored asteroid profiles. Lua owns tuning and content selection; Rust owns schema, deterministic generation, damage/fracture resolution, resource transactions, persistence, and replication.

## 9. Resource And Mining Readiness

V2 does not need to finish mining gameplay, but it must not block it.

Resource profile rules:

1. resources are stable logical `item_id` values;
2. yield weights and depletion pools live on field/member resource profiles;
3. clients may receive coarse public labels later, but exact yields are server-owned unless intentionally disclosed;
4. extraction actions in future mining gameplay consume member/field depletion state and trigger inventory/mass recomputation through the resources/crafting contract.

## 10. Ambient Field Presentation

Fields may define an ambient profile that the client can use when the camera or controlled entity enters the field volume.

Supported presentation hooks:

1. fullscreen background shader option;
2. fullscreen foreground shader option;
3. camera-scoped dust/post-process shader option;
4. fade-in and fade-out by distance from the field boundary.

The initial implementation may select one nearest/highest-intensity field. Later work can add blending between overlapping fields.

## 11. Phased Delivery

Phase 1: implemented foundation

1. add V2 docs and components;
2. extend procedural sprite schema and shader style;
3. add `asteroid.field` bundle and convert starter world bootstrap;
4. keep eager member spawning, but with root/member linkage.

Phase 2: partially implemented fracture

1. zero-health fracture resolution is implemented for active field members;
2. parent/child lineage and field damage-state updates are implemented in ECS state;
3. deterministic child generation tests are implemented;
4. persistence/hydration regression coverage for restarted worlds remains pending.

Phase 3:

1. add cluster activation/retirement;
2. keep only relevant members hydrated;
3. write retired member state back to the root.

Phase 4:

1. connect mining/extraction runtime;
2. expose tactical/editor surfaces;
3. add richer ambient/profile blending.

## 12. Tests

Required coverage:

1. component metadata for all new V2 components;
2. serde roundtrip of nested layout, population, damage, fracture, and resource payloads;
3. deterministic member and child key generation;
4. fracture mass/health/depth limits;
5. Lua registry validation and starter bundle smoke tests;
6. replication/server tests for parent destruction -> child spawn lifecycle;
7. native client visual validation and WASM compile check when client/shared runtime changes.
