# Component Authoring Guide

This guide defines the required workflow for creating new gameplay components.

## 1. File Location and Layout

- Define custom gameplay components in `crates/sidereal-game/src/components/`.
- Use one primary component per file.
- Keep tightly-coupled helper structs/enums in the same file when needed.
- Re-export through `crates/sidereal-game/src/components/mod.rs`.

## 2. Required Derives for Persistable Components

Persistable gameplay components must derive and register for reflection + serde:

```rust
#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
```

## 3. `#[sidereal_component(...)]` Macro

Use the macro on new persistable/replicated gameplay components:

```rust
#[sidereal_component(
    kind = "inventory",
    persist = true,
    replicate = true,
    visibility = [OwnerOnly]
)]
```

Arguments:

- `kind`: Stable persisted/network component kind string.
- `persist`: Include in graph persistence/hydration.
- `replicate`: Include in network replication registration/policy.
- `visibility`: Allowed delivery scopes array.

The macro auto-registers the component for reflection and metadata discovery; do not manually edit
`register_generated_components` when adding a new custom component.

Visibility scopes:

- `OwnerOnly`
- `Faction`
- `Public`

If `visibility` is omitted, owner-only is the default policy.

## 4. Examples

Simple component:

```rust
#[sidereal_component(kind = "cost", persist = true, replicate = true, visibility = [OwnerOnly, Public])]
#[derive(Component, Reflect, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct Cost {
    pub credits: u64,
}
```

Nested/complex component:

```rust
#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[reflect(Serialize, Deserialize)]
pub struct InventoryEntry {
    pub item_entity_id: uuid::Uuid,
    pub quantity: u32,
    pub unit_mass_kg: f32,
}

#[sidereal_component(kind = "inventory", persist = true, replicate = true, visibility = [OwnerOnly])]
#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct Inventory {
    pub entries: Vec<InventoryEntry>,
}
```

## 5. External/Runtime Components (Bevy/Avian)

Do not treat all Bevy/Avian components as durable gameplay schema by default.

- Runtime-only/transient physics internals remain non-persisted.
- Durable gameplay state should live in Sidereal gameplay components.
- For boundary translation cases (for example hierarchy rebuild or UUID lookup), keep explicit hydration logic at the runtime boundary.

## 6. Entity Archetype Layout (Bundles + Spawn Helpers)

For gameplay entities, keep archetype defaults and spawn helpers in `crates/sidereal-game/src/entities/`.

Current direction:

- one module per entity family (`ship`, `missiles`, `debris`, etc.),
- one module per archetype (`corvette`, `light_missile`, etc.),
- bundle + spawn helper in the same archetype file.

Example layout:

```text
crates/sidereal-game/src/entities/
  mod.rs
  ship/
    mod.rs
    corvette.rs
  missiles/
    mod.rs
    light_missile.rs
  debris/
    mod.rs
    small_debris.rs
```

Use this pattern for scalability. Do not spread one archetype's defaults across unrelated crates/services.

## 7. Bundle vs Spawn Function Rules

1. Use a `Bundle` for the base component set of a single entity.
2. Use `spawn_*` helpers when an archetype is a multi-entity graph (for example hull + hardpoints + modules).
3. Keep spawn-time overrides minimal and explicit (owner, shard, position, display name, etc.).
4. Prefer `*Overrides` structs over large ad-hoc config objects unless serialization/config IO requires otherwise.

Illustrative pattern:

```rust
#[derive(Bundle, Clone, Debug)]
pub struct MyEntityBundle {
    // base components
}

#[derive(Clone, Debug, Default)]
pub struct MyEntityOverrides {
    pub position: Option<Vec3>,
    pub display_name: Option<String>,
}

pub fn spawn_my_entity(commands: &mut Commands, overrides: impl Into<MyEntityOverrides>) {
    let overrides = overrides.into();
    // spawn root bundle
    // spawn children/modules if needed
}
```

## 8. Bootstrap and Persistence Shape (Graph Records)

Bootstrap/persistence must use graph records as canonical shape (`GraphEntityRecord` + `GraphComponentRecord`).

Rules:

1. If an archetype needs bootstrap templates, keep a shared graph-template builder in shared crates (currently `sidereal-runtime-sync` templates are used by gateway/replication flows).
2. Gateway/direct bootstrap and replication bootstrap paths must produce equivalent full component-bearing graph records.
3. Avoid duplicate hand-maintained default loadouts in multiple services.

This keeps hydration deterministic and prevents drift between gateway and replication startup behavior.

## 9. Authoring Checklist for New Archetypes

When adding a new archetype (ship/missile/station/container/etc.):

1. Add bundle + spawn helper under `crates/sidereal-game/src/entities/`.
2. Add/verify persistable components in `crates/sidereal-game/src/components/` with `#[sidereal_component(...)]`.
3. Add or update graph-template bootstrap builder (if archetype participates in starter/bootstrap flows).
4. Ensure runtime asset references are logical `asset_id` values (not hardcoded disk paths).
5. Add tests for:
   - bundle/spawn default correctness,
   - persistence/hydration roundtrip,
   - replication visibility/policy constraints as applicable.
