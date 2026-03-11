# Asteroid Field System

**Status:** Proposed feature specification
**Last updated:** 2026-03-12

## 0. Status Notes

- 2026-03-12: This document defines the intended replacement for the current bootstrap-only `asteroid.field_member` scatter model. The target direction is a persisted asteroid-field entity that owns clustered asteroid content, supports large size tiers, and authoritatively fractures destructible asteroids into smaller child bodies. Native impact: server/runtime/client work required. WASM impact: shared gameplay, replication, and procedural-generation logic should remain target-shared; browser-specific work stays limited to asset/loading/render boundary behavior.

## 1. Purpose

Sidereal should not model asteroid-heavy regions as only a flat list of unrelated single asteroids scattered across a large radius.

The asteroid system should instead support:

1. authored asteroid field entities as first-class world content,
2. clustered asteroid populations around a field anchor,
3. larger asteroid size tiers than the current `4m..28m` default band,
4. authoritative destruction and fracture into smaller asteroid bodies,
5. persistent depletion state so mining/combat changes survive restart,
6. bounded runtime cost so dense fields do not require every rock to be hydrated everywhere at all times.

## 2. Current Baseline

Today the implementation is documented in `docs/features/procedural_asteroids.md` and works as follows:

1. `data/scripts/world/world_init.lua` runs one-time deterministic bootstrap generation.
2. It calls `spawn_bundle_graph_records("asteroid.field_member", overrides)` with `field_count`.
3. The bundle emits one fully persisted runtime entity per asteroid.
4. Each asteroid carries its own collision, health, mass, size, visual, and procedural sprite payload.
5. The current field is only a layout convention in Lua, not a real gameplay entity or authoritative container.

This baseline is a good proof that the procedural silhouette pipeline works, but it does not scale well to denser fields or destructible breakup chains.

## 3. Design Goals

### 3.1 Gameplay goals

1. Asteroid fields should feel spatially coherent: belts, clusters, pockets, lanes, and dense cores.
2. Large asteroids should read as meaningful obstacles/resources, not only visual variants of the same small rock.
3. Destruction should be progressive:
   - large asteroid -> medium fragments,
   - medium fragment -> small chunks,
   - small chunk -> destroyed/depleted.
4. Breakup should preserve local motion, mass reduction, and visible debris outcomes.

### 3.2 Architecture goals

1. Keep server authority over spawning, damage resolution, fracture, depletion, and persistence.
2. Keep identity crossing boundaries as UUID/entity IDs only.
3. Keep content authoring data-driven through Lua bundles/config, not Rust hardcoded content tables.
4. Reuse shared procedural generation for visuals and collision so child fragments remain deterministic.
5. Avoid requiring all field members to exist as always-hydrated runtime entities when no player is nearby.

### 3.3 Performance goals

1. One asteroid field should be a single persisted root entity plus bounded active member entities.
2. Dense fields should support runtime activation by proximity/cell/interest, not global always-on hydration.
3. Fracture should use spawn budgets and size thresholds to prevent runaway entity multiplication.

## 4. Proposed High-Level Model

Introduce a first-class persisted `AsteroidField` root entity.

That root entity is authoritative for:

1. field identity,
2. field layout seed,
3. cluster definitions,
4. size-tier distributions,
5. depletion state,
6. runtime activation policy,
7. fracture rules for members spawned from the field.

The field root is a gameplay/persistence entity, not just editor metadata.

At runtime:

1. the field root persists always,
2. the replication host activates member asteroids when players are near relevant clusters,
3. active members exist as ordinary authoritative runtime entities,
4. damaged members can fracture into child asteroid entities,
5. resulting depletion/fracture outcomes are written back to field-owned persistent state.

This preserves the rule that destructible gameplay participants are still normal runtime entities when active, while avoiding a world model where every asteroid in every field must always be hydrated.

## 5. Core Entity Model

### 5.1 Asteroid field root

Add a new persisted root entity kind for fields. It should carry components equivalent to:

1. `AsteroidField`
   - stable field UUID,
   - world seed/version key,
   - field archetype/profile id,
   - activation radius and cluster settings.
2. `WorldPosition` / `WorldRotation`
   - canonical anchor for a mostly static field.
3. `AsteroidFieldLayout`
   - cluster count,
   - cluster offsets/radii,
   - density curves,
   - ring/belt/noise parameters.
4. `AsteroidFieldPopulation`
   - target large/medium/small counts,
   - size-tier tuning,
   - material/prospect tables,
   - current depletion counters.
5. `AsteroidFieldRuntimeState`
   - currently activated cluster keys,
   - spawn budget metadata,
   - regeneration disabled/enabled policy if later needed.
6. `AsteroidFieldDamageState`
   - per-member depletion/fracture record keyed by deterministic member key, not Bevy entity id.

The exact component split can change, but the root must own both authored layout and persistent depletion data.

### 5.2 Active asteroid member entities

When a cluster is activated, the server spawns regular asteroid member entities with:

1. `EntityGuid`
2. `DisplayName`
3. `EntityLabels`
4. `HealthPool`
5. `MassKg` and mass-derivation companions as needed
6. `SizeM`
7. collision components (`CollisionProfile`, `CollisionAabbM`, optional `CollisionOutlineM`)
8. visual components (`VisualAssetId`, `SpriteShaderAssetId`, `ProceduralSprite`, map icon)
9. motion components
   - static for dormant attached rocks when appropriate,
   - dynamic for free-floating fragments after fracture.
10. field linkage components
   - `AsteroidFieldMember`
   - `field_entity_id`
   - deterministic `member_key`
   - `parent_member_key` for fracture lineage
   - `size_tier`
   - `fracture_depth`

Runtime member entities remain the authoritative damage/collision participants.

### 5.3 Fragment lineage

Fragments should not be anonymous.

Each spawned child keeps:

1. the owning field entity id,
2. a deterministic lineage path such as `cluster/member/child_index/...`,
3. a size tier,
4. a fracture depth,
5. a source seed key derived from parent lineage.

This allows deterministic regeneration of visual/collision payloads and stable persistence of depletion outcomes.

## 6. Field Layout and Clustering

### 6.1 Field shape

The system should support multiple field archetypes, for example:

1. ring belt,
2. elliptical belt,
3. clustered patch field,
4. dense core plus sparse halo,
5. lane-crossed debris field.

These are authoring profiles, not hardcoded game logic branches.

### 6.2 Cluster model

Each field contains one or more deterministic clusters. A cluster defines:

1. local offset from field anchor,
2. local radius/extents,
3. density weight,
4. preferred size-tier mix,
5. motion seed/bias,
6. optional material rarity weighting.

Members are generated relative to a cluster, not globally scattered across the whole field radius.

That is the main change needed to make the field read as one coherent space feature.

### 6.3 Deterministic member keys

Each authored/generated member slot should use a deterministic key:

`field_guid + cluster_index + member_index + content_version`

That key drives:

1. spawn position jitter,
2. size tier,
3. base resource/material profile,
4. procedural sprite parameters,
5. collision outline seed,
6. initial health and mass,
7. fracture-child seeds.

## 7. Size Tiers

The current asteroid range is too narrow for a field system. The new system should expose explicit tiers.

Recommended baseline:

1. `Small`
   - roughly `3m..12m`
   - final fracture stage
   - cheap collision and low health
2. `Medium`
   - roughly `12m..40m`
   - common field population
   - can break into several small fragments
3. `Large`
   - roughly `40m..120m`
   - sparse anchors/landmarks/resources
   - can break into medium fragments
4. `Massive` (optional authoring tier, low frequency)
   - `120m+`
   - special hazard/resource bodies
   - may fracture in staged events rather than immediately

These are authoring/runtime tiers, not just render scale buckets.

Tier affects:

1. health,
2. mass,
3. collision simplification policy,
4. fracture table,
5. sprite resolution budget,
6. activation distance,
7. tactical/minimap presentation,
8. loot/resource output.

## 8. Destruction and Fracture Model

### 8.1 Fracture rules

Asteroids should not simply disappear when health reaches zero.

Instead:

1. `Large` asteroids fracture into `2..5` `Medium` children.
2. `Medium` asteroids fracture into `2..6` `Small` children.
3. `Small` asteroids are removed and marked depleted.
4. Optional non-authoritative dust/spark debris can be purely visual and client-local if needed.

The fracture result must be authoritative and deterministic from:

1. parent lineage key,
2. parent size tier,
3. damage event/final fracture sequence number,
4. current field content version.

### 8.2 Child spawn behavior

Fracture children inherit:

1. parent world position as center,
2. slight deterministic radial offsets,
3. inherited local velocity plus breakup impulse,
4. reduced total mass and health,
5. a related but not identical procedural silhouette profile.

The sum of child mass should be less than or equal to parent mass, allowing some loss to dust/resource extraction.

### 8.3 Persistence rule

Fracture is not only an ephemeral runtime effect.

When fracture occurs, the field root’s persistent damage/depletion state must record:

1. parent member consumed,
2. child lineage keys created,
3. resulting child depletion states,
4. any harvested/resource-removed amounts.

On restart/hydration, the system must recreate the same surviving active member topology from field state.

### 8.4 Budget limits

To avoid runaway entity growth:

1. cap fracture depth per lineage,
2. cap active fragment count per field/cluster,
3. merge tiny terminal debris into depletion/resource yield rather than spawning physics bodies,
4. use per-tick spawn budgets when many asteroids fracture at once.

## 9. Runtime Activation Model

### 9.1 Why activation is needed

A single field may logically contain hundreds or thousands of potential members. Persisting each one as a continuously hydrated runtime entity is the wrong scaling model.

### 9.2 Activation contract

The persisted field root exists always. Member entities are activated when:

1. a player is within field activation range,
2. a cluster intersects a visibility/interest budget,
3. scripted/admin tools explicitly force activation.

When no longer relevant, intact inactive members should collapse back into field state instead of remaining hydrated forever.

### 9.3 What stays as entities

The following should remain separate runtime entities while active:

1. any asteroid currently collidable/interactable,
2. any fragment with non-trivial motion,
3. any asteroid with partially depleted health,
4. any asteroid under mining/combat interaction,
5. any large landmark asteroid the client must target individually.

### 9.4 What stays only in field state

The following may remain only in field-owned state until activation:

1. untouched member slots,
2. distant intact clusters,
3. depleted members already fully resolved,
4. aggregate field metadata.

## 10. Persistence and Hydration

### 10.1 Canonical persistence shape

Use graph records and relationships as the persistence source of truth.

The field root should be persisted as a normal graph entity with field components.

Active member entities may also persist as graph entities while live, but their canonical ownership must point back to the field root through explicit relationship/component linkage.

### 10.2 Field-owned member state

The canonical long-lived identity for untouched/depleted/fractured slots should be field-owned member keys, not permanent always-on entity rows for every asteroid ever possible.

This gives:

1. stable depletion history,
2. compact persistence,
3. deterministic reactivation,
4. no dependence on raw Bevy runtime ids.

### 10.3 Hydration flow

On shard startup:

1. hydrate asteroid field roots,
2. restore their persistent layout and depletion state,
3. do not immediately instantiate every member,
4. instantiate active clusters only when activation rules require it,
5. if saved active members exist, rebind them to the field root and validate lineage/state consistency.

## 11. Replication and Visibility

### 11.1 Root entity replication

The field root may replicate minimally or not at all to ordinary clients depending on presentation needs. If replicated, it should expose only safe/public metadata such as:

1. display name,
2. anchor position,
3. coarse field extents,
4. optional tactical marker metadata.

### 11.2 Member replication

Active member asteroids replicate like other world entities under normal visibility/range rules.

This keeps the existing visibility system generic:

1. no asteroid-specific backdoor delivery path,
2. no client-authored member activation,
3. normal public/faction visibility rules still apply.

### 11.3 Breakup replication

Fracture should appear to clients as ordinary server-authored entity lifecycle changes:

1. parent health reaches fracture threshold,
2. parent despawns or transitions,
3. child entities spawn with normal replicated state,
4. clients rebuild visuals from replicated procedural payloads.

## 12. Rendering and Collision

### 12.1 Shared procedural basis

The existing procedural asteroid pipeline should remain the base:

1. shared seed key,
2. shared `ProceduralSprite`,
3. shared collision half-extents derivation,
4. shared collision outline derivation,
5. shared normal-map generation.

That is already a strong foundation and should be extended, not replaced.

### 12.2 Larger-body rendering

Large and massive asteroids should support:

1. higher sprite resolution budgets,
2. more silhouette lobes/cracks/secondary detail,
3. optional tier-specific shader settings,
4. better lighting use of the normal map,
5. later promotion to richer render families if needed.

This should still stay under the generic world-sprite/render-layer architecture rather than reintroducing a dedicated ad-hoc render path.

### 12.3 Collision simplification

Collision complexity should scale by tier:

1. `Small`: AABB or low-point outline.
2. `Medium`: outline hull when active.
3. `Large`/`Massive`: outline hull or future multi-shape support if needed.

The collision representation should remain derivable from the procedural shape seed so visuals and collision stay aligned.

## 13. Authoring Workflow

### 13.1 Lua content ownership

Lua should author asteroid field content, not individual hardcoded Rust spawn tables.

Recommended authoring surfaces:

1. `asteroid.field` bundle
   - emits one field root entity,
   - defines layout/profile/seed/distribution settings.
2. `asteroid.fragment_profile` or equivalent authored tables
   - fracture counts,
   - tier transitions,
   - material weighting,
   - procedural sprite tuning ranges.
3. world-init or region-generation scripts
   - place field roots in systems/regions,
   - not every member asteroid directly.

### 13.2 Current bundle migration

`asteroid.field_member` should not disappear immediately, but it should become an internal helper or migration bridge rather than the main authored world primitive.

The new primary authored primitive should be the field root bundle.

## 14. Technical Overview

### 14.1 Recommended implementation flow

1. Add new persistable gameplay components for field roots and field-member linkage in `crates/sidereal-game`.
2. Register them through the normal shared component-generation path.
3. Add Lua bundle support for `asteroid.field`.
4. Keep procedural sprite generation in shared Rust and extend it with additional generator profiles/size-tier parameters.
5. Add replication-host systems that:
   - evaluate player proximity to fields,
   - activate/deactivate clusters,
   - spawn member entities from deterministic member keys,
   - write fracture/depletion changes back to field state.
6. Add fracture systems in shared gameplay/replication flow:
   - detect zero-health asteroid members,
   - consult fracture table,
   - spawn child members,
   - update persistent field state,
   - despawn or mark parent depleted.
7. Update client visuals only as needed to support:
   - larger resolution tiers,
   - better lit asteroid materials,
   - fragment spawn/despawn churn.

### 14.2 Recommended component families

Suggested new components:

1. `AsteroidField`
2. `AsteroidFieldLayout`
3. `AsteroidFieldPopulation`
4. `AsteroidFieldDamageState`
5. `AsteroidFieldRuntimeState`
6. `AsteroidFieldMember`
7. `AsteroidFractureProfile`
8. `AsteroidResourceProfile` if resource extraction becomes part of the same system

These should live under `crates/sidereal-game/src/components/` as normal shared gameplay components.

### 14.3 Runtime systems

Suggested system groups:

1. `asteroid_field_activation_system`
2. `asteroid_field_member_spawn_system`
3. `asteroid_field_member_retire_system`
4. `asteroid_fracture_resolution_system`
5. `asteroid_field_persistence_sync_system`
6. `asteroid_field_regrowth_system` only if regeneration becomes an actual design requirement later

### 14.4 Determinism contract

For a given field seed and content version:

1. intact untouched member slots generate identically every time,
2. child fragments generate identically from lineage seed,
3. collision and visuals match because both derive from the same procedural payload,
4. depletion state is the only persistent divergence from the pristine authored layout.

### 14.5 Native and WASM scope

No gameplay authority should be native-only.

Shared requirements:

1. components,
2. member activation logic,
3. fracture logic,
4. procedural sprite/collision derivation,
5. replication payloads.

Platform-specific scope should remain limited to client transport/bootstrap/render I/O differences.

## 15. Proposed Component Contract

This section defines the recommended first-pass component shapes closely enough to guide implementation.

Exact Rust field names may vary, but the persistence/authority intent should remain the same.

### 15.1 `AsteroidField`

Purpose: field identity and high-level authored behavior.

Recommended fields:

1. `field_profile_id: String`
2. `content_version: u32`
3. `layout_seed: u64`
4. `activation_radius_m: f32`
5. `cluster_activation_radius_m: f32`
6. `max_active_members: u32`
7. `max_active_fragments: u32`
8. `max_fracture_depth: u8`
9. `allow_regrowth: bool`

Recommended metadata:

1. `persist = true`
2. `replicate = true` only if clients need aggregate field presence/tactical display
3. visibility should default to `Public` if the root is replicated at all

### 15.2 `AsteroidFieldLayout`

Purpose: deterministic authored layout inputs.

Recommended fields:

1. `field_shape: AsteroidFieldShape`
2. `field_radius_m: f32`
3. `core_radius_m: f32`
4. `halo_radius_m: f32`
5. `clusters: Vec<AsteroidFieldCluster>`
6. `spawn_noise_amplitude_m: f32`
7. `spawn_noise_frequency: f32`
8. `rotation_bias_rad: f32`

Suggested nested types:

1. `AsteroidFieldShape`
   - `Ring`
   - `Ellipse`
   - `ClusterPatch`
   - `DenseCoreHalo`
   - `DebrisLane`
2. `AsteroidFieldCluster`
   - `cluster_key: String`
   - `offset_xy: Vec2`
   - `radius_m: f32`
   - `density_weight: f32`
   - `large_weight: f32`
   - `medium_weight: f32`
   - `small_weight: f32`
   - `rarity_weight: f32`

### 15.3 `AsteroidFieldPopulation`

Purpose: tier counts and authored resource/material distribution.

Recommended fields:

1. `target_large_count: u32`
2. `target_medium_count: u32`
3. `target_small_count: u32`
4. `large_size_range_m: Vec2`
5. `medium_size_range_m: Vec2`
6. `small_size_range_m: Vec2`
7. `material_table_id: String`
8. `fracture_profile_id: String`
9. `sprite_profile_id: String`

### 15.4 `AsteroidFieldDamageState`

Purpose: persistent depletion and fracture history.

This should be the canonical long-lived state for member slots and lineage.

Recommended fields:

1. `entries: Vec<AsteroidMemberStateEntry>`

Suggested nested payload:

1. `member_key: String`
2. `parent_member_key: Option<String>`
3. `state: AsteroidMemberStateKind`
4. `size_tier: AsteroidSizeTier`
5. `fracture_depth: u8`
6. `remaining_health: Option<f32>`
7. `remaining_mass_kg: Option<f32>`
8. `spawned_children: Vec<String>`
9. `resource_yield_consumed: f32`
10. `last_update_tick: Option<u64>`

Suggested state enum:

1. `Intact`
2. `Activated`
3. `Fractured`
4. `Depleted`
5. `Harvested`

Important rule:

The authoritative key is `member_key`, not a runtime entity id. Runtime entities are temporary realizations of these entries.

### 15.5 `AsteroidFieldRuntimeState`

Purpose: runtime-only or low-value persisted operational state.

Recommended fields:

1. `active_cluster_keys: Vec<String>`
2. `active_member_keys: Vec<String>`
3. `spawn_budget_available: u32`
4. `retire_budget_available: u32`
5. `last_activation_tick: Option<u64>`

If this component causes persistence churn, split out a non-persisted runtime-only variant and keep only durable state persisted.

### 15.6 `AsteroidFieldMember`

Purpose: link a spawned asteroid entity back to field-owned deterministic state.

Recommended fields:

1. `field_entity_id: String`
2. `cluster_key: String`
3. `member_key: String`
4. `parent_member_key: Option<String>`
5. `size_tier: AsteroidSizeTier`
6. `fracture_depth: u8`
7. `resource_profile_id: String`
8. `fracture_profile_id: String`

Recommended metadata:

1. `persist = true`
2. `replicate = true`
3. `visibility = [Public]`

### 15.7 `AsteroidFractureProfile`

Purpose: authored breakup behavior by size tier.

Recommended fields:

1. `break_large_into_medium_min: u8`
2. `break_large_into_medium_max: u8`
3. `break_medium_into_small_min: u8`
4. `break_medium_into_small_max: u8`
5. `child_impulse_min_mps: f32`
6. `child_impulse_max_mps: f32`
7. `mass_retention_ratio: f32`
8. `terminal_debris_loss_ratio: f32`

### 15.8 `AsteroidSizeTier`

Recommended enum:

1. `Small`
2. `Medium`
3. `Large`
4. `Massive`

## 16. Activation and Deactivation Contract

### 16.1 Activation triggers

An asteroid field cluster should activate when any of the following is true:

1. a player-controlled entity enters `cluster_activation_radius_m`,
2. an already active member in the cluster takes damage,
3. a scripted/admin event explicitly requests field activation,
4. a nearby active fragment crosses into the cluster volume.

### 16.2 Activation outputs

On activation:

1. resolve deterministic member keys for the cluster,
2. skip entries marked `Depleted` or fully consumed terminal states,
3. spawn runtime asteroid entities for intact or partially damaged entries,
4. restore health/mass overrides from `AsteroidFieldDamageState`,
5. rebuild procedural visual and collision payloads from deterministic seed plus persisted overrides.

### 16.3 Deactivation rules

A member may retire back into field-owned state only when all are true:

1. it is outside active observer interest,
2. it has no recent damage/mining interaction,
3. it has near-zero translational velocity or is explicitly static-safe,
4. it is not currently selected/targeted by a player,
5. retiring it will not destroy required combat or collision continuity.

### 16.4 Retirement outputs

On retirement:

1. snapshot remaining health/mass/state back into `AsteroidFieldDamageState`,
2. mark the member entry `Intact`, `Activated`, `Fractured`, or `Harvested` as appropriate,
3. despawn the runtime entity,
4. remove its `member_key` from `active_member_keys`.

### 16.5 Never-retire cases

The following should stay as live entities until resolved:

1. moving fracture children above a velocity threshold,
2. asteroids under active mining/fire,
3. large landmark asteroids flagged as persistent targets,
4. members participating in pending scripted events.

## 17. Fracture Resolution Contract

### 17.1 Trigger

Fracture runs when an `AsteroidFieldMember` entity reaches zero health and its tier/profile permits breakup.

### 17.2 Resolution order

Recommended authoritative order:

1. read `AsteroidFieldMember` and `AsteroidFractureProfile`,
2. read/update owning field `AsteroidFieldDamageState`,
3. determine child count and child seeds deterministically,
4. allocate child `member_key`s,
5. write child state entries before spawning runtime children,
6. spawn children with derived size/mass/health/motion,
7. mark parent entry `Fractured` or `Depleted`,
8. despawn or tombstone the parent runtime entity.

### 17.3 Child derivation rules

Children should be derived from:

1. `child_seed = hash(parent_member_key, child_index, content_version)`
2. child tier = one step smaller than parent tier
3. child mass total <= parent remaining mass * `mass_retention_ratio`
4. child health scales from child size/mass, not copied directly from parent
5. child position offset stays within parent bounds plus small breakup impulse offset

### 17.4 Terminal destruction

When `Small` asteroids are destroyed:

1. do not spawn more physics children,
2. convert remaining value into depletion/resource outputs,
3. mark the member entry terminal,
4. allow non-authoritative VFX on the client if desired.

## 18. Migration from Current `asteroid.field_member` Model

### 18.1 Current live model

Current world bootstrap directly spawns one entity per asteroid from `data/scripts/bundles/starter/asteroid_field.lua`.

That file should be treated as the migration origin, not the final system shape.

### 18.2 Migration path

Recommended migration:

1. keep `asteroid.field_member` working temporarily for isolated asteroid spawns and regression comparison,
2. add new `asteroid.field` bundle that emits one field root entity,
3. move world bootstrap to place field roots instead of all members,
4. make member runtime spawns occur from replication-host field activation systems,
5. gradually reduce direct world-init eager member spawning,
6. once field-root activation is stable, keep `asteroid.field_member` only as an internal helper or remove it.

### 18.3 Data compatibility

Per repo rules, do not add long-lived compatibility shims for old asteroid payload schemas.

When the canonical asteroid persistence shape changes:

1. update all producers/consumers in one change,
2. reset local/dev DBs as needed,
3. keep one canonical field/member schema.

## 19. Crate-Level Implementation Notes

### 19.1 `crates/sidereal-game`

Responsibilities:

1. shared asteroid field/member/fracture components,
2. deterministic field/member/fragment generation helpers,
3. fracture math,
4. mass/size derivation helpers,
5. tests for serde, generation, and fracture determinism.

### 19.2 `bins/sidereal-replication`

Responsibilities:

1. field activation/deactivation systems,
2. runtime member spawn/retire orchestration,
3. authoritative damage-to-fracture transition,
4. persistence sync for field damage state,
5. scripting bridge helpers for field-root bundles and authored defaults.

### 19.3 `bins/sidereal-client`

Responsibilities:

1. render active member asteroids only,
2. continue deriving visuals from replicated procedural payloads,
3. support larger resolution tiers/material tuning,
4. handle normal replicated spawn/despawn churn for fracture,
5. avoid client-authored fracture or depletion decisions.

### 19.4 `data/scripts`

Responsibilities:

1. author field-root bundles and defaults,
2. author field archetype/profile tables,
3. place fields during world bootstrap or region generation,
4. avoid directly hardcoding every asteroid member into world init.

## 20. Recommended Phased Delivery

### Phase 1: Field root introduction

1. Add field-root components and bundle.
2. Keep member spawning eager at first, but under a real field root.
3. Add size tiers beyond current defaults.

### Phase 2: Fracture support

1. Add `AsteroidFieldMember` linkage and lineage keys.
2. Add large -> medium -> small fracture resolution.
3. Persist depletion/fracture state on the field root.

### Phase 3: Activation/deactivation

1. Stop always hydrating all members.
2. Activate clusters by player proximity/interest.
3. Retire intact distant members back into field-owned state.

### Phase 4: Density and content richness

1. Add multiple field archetypes.
2. Add richer material/resource profiles.
3. Add better large-asteroid lighting/detail.

## 21. Testing Expectations

Implementation should include:

1. shared-component serde + hydration roundtrip coverage for new field/member components,
2. deterministic generation tests for field layout/member keys,
3. fracture tests validating parent -> child lineage and mass/health scaling,
4. persistence tests ensuring depletion survives restart,
5. replication tests ensuring fracture appears as normal server-authored entity lifecycle changes,
6. client validation for native, plus WASM compile verification when shared client/runtime code changes.

### 21.1 Suggested test ownership by crate

1. `crates/sidereal-game/tests`
   - component registry coverage for new field/member components,
   - serde roundtrip for nested field state payloads,
   - deterministic member-key generation,
   - deterministic fracture-child generation,
   - mass/health conservation assertions.
2. `bins/sidereal-replication` tests
   - activation spawns only nearby clusters,
   - retirement writes state back to field root,
   - zero-health large/medium members fracture correctly,
   - terminal small members deplete without further child spawn,
   - no mismatched field/member lineage restoration on hydration.
3. client/shared runtime checks
   - visual rebuild still works for late-arriving procedural payloads,
   - large asteroid tiers respect higher sprite settings,
   - fracture child spawns/despawns do not leave stale visual entities.

## 22. Open Questions

1. Should very large landmark asteroids always stay hydrated as standalone entities even when the rest of a field is abstracted?
2. Should mined-out fields regenerate over time, or remain permanently depleted unless a scripted event repopulates them?
3. Should fracture children always become free-moving dynamic bodies, or can some remain anchored/static in dense fields for performance?
4. Does tactical view need a field-level aggregate marker in addition to individual active members?
5. At what size threshold should asteroid rendering stay sprite-based versus moving to a richer mesh/polygon presentation, if ever?

## 23. Related Documents

1. `docs/features/procedural_asteroids.md`
2. `docs/features/scripting_support.md`
3. `docs/features/visibility_replication_contract.md`
4. `docs/sidereal_design_document.md`
5. `docs/decision_register.md`
