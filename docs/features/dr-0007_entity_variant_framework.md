# Entity Variant System Plan

Status: Proposed feature plan  
Last updated: February 24, 2026  
Scope: generic runtime variant framework for any entity class (not ship-only)
Decision Register linkage: `DR-0007`

## 1. Goal

Add a robust, server-authoritative variant framework that can apply to:

- ships
- missiles/projectiles
- stations
- cargo containers
- modules
- environmental entities

The system must support:

1. deterministic hardcoded variant selection,
2. deterministic weighted/random selection,
3. visual overrides,
4. gameplay tuning overrides,
5. persistence + hydration roundtrip,
6. replication-safe behavior.

## 2. Design Principles

1. Generic, not ship-specific.
2. Server-authoritative variant selection only.
3. Variant identity is persisted on the entity.
4. Variant definitions are data-driven and versionable.
5. Base archetype remains minimal and reusable.
6. Variants are overlays, not duplicated bundles, unless structure differs fundamentally.

## 3. Core Model

## 3.1 Base + Overlay

- Base archetype defines default components and baseline behavior.
- Variant overlay defines optional component overrides and asset references.
- Overlay is applied at spawn (or controlled migration flow), then persisted.

## 3.2 Identity Components

```rust
#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug, Default)]
pub struct VariantId(pub String);

#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug, Default)]
pub struct VariantFamilyId(pub String); // e.g. "ship.corvette", "missile.light_kinetic"

#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug, Default)]
pub struct VariantSeed(pub u64); // optional deterministic weighted selection source
```

`VariantId` is the canonical persisted selected variant.

## 3.3 Definition Shape

Use definition records keyed by `variant_id`:

```rust
pub struct EntityVariantDef {
    pub variant_id: String,
    pub family_id: String,
    pub labels: Vec<String>,
    pub weight: u32, // for random pools
    pub overlays: VariantOverlay,
}

pub struct VariantOverlay {
    pub asset_overrides: Vec<AssetOverride>,
    pub component_overrides: Vec<ComponentOverride>,
}
```

`ComponentOverride` should map to reflect/serde-friendly component payloads, not ad-hoc runtime closures.

## 4. Selection Policies

## 4.1 Hardcoded selection

Spawn request or template specifies explicit `variant_id`.

## 4.2 Weighted deterministic selection

Server selects from a family pool using deterministic seed:

`seed = hash(account_id, entity_guid, family_id, season_or_release_id)`

This keeps behavior stable across reconnect/hydration.

## 4.3 Policy component

```rust
pub enum VariantSelectionPolicy {
    Explicit { variant_id: String },
    WeightedDeterministic { family_id: String, seed: u64 },
}
```

Selection policy should exist in spawn pipeline only; resolved `VariantId` is persisted on the entity.

## 5. Override Categories

## 5.1 Visual overrides

- visual/sprite asset ID
- material/skin asset ID
- shader asset IDs
- VFX/SFX profile IDs

## 5.2 Gameplay overrides

- engine/thrust profiles
- mass/tuning/health/capacity tweaks
- missile guidance profile
- station subsystem profile

## 5.3 Structural overrides (special case)

If a variant changes topology (hardpoints/module graph/children) significantly:

- use a distinct archetype/template and optionally still attach `VariantId`.
- do not force large structural differences through tiny overlay patches.

## 6. Persistence and Hydration

Required:

1. Persist `VariantId` (+ optional family/seed if needed for auditing).
2. Persist post-overlay component state as normal gameplay components.
3. During hydration, entity must rehydrate deterministically with same variant-visible behavior.
4. No random re-roll on hydration for existing entities.

## 7. Replication Contract

1. Variant selection happens server-side before/at authoritative spawn.
2. Clients receive replicated result (`VariantId` and resulting components/assets), not selection authority.
3. Clients never "choose" a stronger/faster variant.
4. Asset delivery system uses resulting visual/audio asset references from resolved variant state.

## 8. Integration with Asset Delivery

Variant overlays should reference logical asset IDs only.

Example:

```rust
pub struct AssetOverride {
    pub slot: String,       // e.g. "hull_model", "engine_sfx"
    pub asset_id: String,   // e.g. "ship.corvette.skin.blue"
}
```

When variant is resolved:

1. resolved asset IDs are present on entity components,
2. asset collector includes them in required-asset expansion,
3. placeholders remain active until streamed assets are ready.

## 9. Example Families

## 9.1 Corvette ship family

- `ship.corvette.default`
- `ship.corvette.blue_skin`
- `ship.corvette.tuned_engine_mk2`

## 9.2 Missile family

- `missile.light.default`
- `missile.light.fast_burn`
- `missile.light.long_range`

## 9.3 Cargo container family

- `cargo.container.standard`
- `cargo.container.hardened`
- `cargo.container.salvage_tagged`

## 10. Implementation Plan

## Phase A: shared generic variant core

1. Add generic variant components (`VariantId`, `VariantFamilyId`, `VariantSeed`) in `sidereal-game`.
2. Add shared definition/registry types in a shared crate.
3. Add selection utility (explicit + deterministic weighted).

## Phase B: spawn pipeline integration

1. Add variant resolution stage to server spawn/template pipeline.
2. Resolve overlays before final entity persistence/replication.
3. Persist selected `VariantId`.

## Phase C: asset and runtime integration

1. Ensure variant visual/audio IDs feed asset required-set collection.
2. Add placeholder + swap behavior coverage for variant-specific assets.

## Phase D: adoption by families

1. Migrate corvette to family-based variants.
2. Add at least one missile, one station, and one cargo family using same framework.

## 11. Test Plan

## 11.1 Unit

- deterministic weighted selection returns same result for same seed.
- variant overlay application merges expected components.
- invalid variant IDs are rejected cleanly.

## 11.2 Integration

- explicit variant spawn persists/hydrates with same variant.
- deterministic policy spawn remains stable across reconnect.
- variant-specific assets are requested and swapped without crash.
- unauthorized client cannot force variant change.

## 12. Risks and Mitigations

1. Risk: ad-hoc per-domain variant code forks.
   - Mitigation: central shared variant registry + overlay engine.
2. Risk: random re-roll after hydration.
   - Mitigation: persist `VariantId`; selection only at initial spawn.
3. Risk: asset misses spike with high variant cardinality.
   - Mitigation: dependency prefetch and cache-aware manifests.

## 13. Definition of Done

Feature is considered done when:

1. Variant system is generic and used by at least 3 non-identical families (e.g. ship + missile + cargo/station).
2. Variant selection is server-authoritative and deterministic.
3. Variant identity persists and hydrates roundtrip.
4. Asset delivery integrates variant asset references without crashes.
5. Tests cover deterministic selection, persistence, and stream fallback behavior.
