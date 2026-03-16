# Resources and Crafting Contract

**Status:** Proposed feature specification
**Last updated:** 2026-03-16
**Primary architecture reference:** `docs/sidereal_design_document.md`
**Related contracts:** `docs/features/background_world_simulation_contract.md`, `docs/features/scripting_support.md`, `docs/features/asteroid_field_system.md`, `docs/component_authoring_guide.md`

## 0. Status Notes

- 2026-03-16: Initial contract for Sidereal's material, refining, and crafting foundation. Native impact: future shared gameplay/runtime work is required for extraction nodes, facility queues, inventory transactions, and module/ship manufacturing flows. WASM impact: no browser-only authority split is introduced; recipe validation, queue progression, and item/facility definitions should remain in shared gameplay/runtime code, with browser differences limited to UI/input and asset-loading boundaries.

## 1. Purpose

Sidereal needs a resource and crafting model that supports the whole industrial ladder of a top-down space ARPG:

1. extraction from asteroids, ice fields, wrecks, stations, and anomaly regions,
2. refining and chemical processing,
3. alloy/composite/synthetic production,
4. module-grade and hull-grade subassemblies,
5. downstream manufacture of ships, weapons, upgrades, ammunition, drones, and faction-specific gear.

This document defines the target direction for:

1. the content taxonomy,
2. the data-authoring model,
3. the Bevy/runtime integration contract,
4. the starter material tree and progression bands.

## 2. Design Goals

### 2.1 Gameplay goals

1. Common items should craft through short readable chains, while advanced faction/endgame outputs can use deeper industrial chains.
2. Real-ish materials should make the economy legible at a glance.
3. Fictional materials should provide Sidereal-specific faction identity and endgame flavor.
4. Crafting should be a foundation for combat, traversal, logistics, and progression rather than a disconnected side system.

### 2.2 Architecture goals

1. Keep authority server-side for extraction, refining, queue progression, recipe validation, and output creation.
2. Keep content authoring data-driven and mod-friendly through Lua-authored registries rather than Rust hardcoded content tables.
3. Keep runtime state on authoritative ECS entities and persist it through graph records.
4. Keep shared item/recipe/facility definitions available to both native and WASM clients without platform-specific gameplay forks.

### 2.3 Production-graph goals

1. Tier 0-1 should cover mining, salvage, pumping, and basic refining.
2. Tier 2-3 should define the industrial backbone used by most ship, weapon, and module recipes.
3. Tier 4-5 should provide faction-specialized and endgame material identities.
4. Recommended chain depth:
   - common outputs: 2 steps,
   - mid-game outputs: 3 steps,
   - rare or faction outputs: 4-5 steps.

## 3. Non-Negotiable Invariants

1. Client input may request extraction/crafting actions, but clients never authoritatively create materials, advance jobs, or spawn finished equipment.
2. Item, recipe, facility, and extraction definitions are content data, not ad hoc Rust constant maps.
3. Sidereal should follow the existing Lua-authored catalog model used elsewhere in the project: authoritative definitions live in script catalogs, Rust validates them, and runtime systems consume generated registries/resources.
4. Durable runtime state such as facility inventories, queue entries, unlock state, and extraction depletion belongs on graph-persisted ECS entities/components, not side SQL tables that bypass the ECS authority model.
5. Logical crafting identities crossing script/UI/network boundaries use stable IDs such as `item_id`, `recipe_id`, `facility_id`, and `blueprint_id`; raw Bevy `Entity` IDs must not cross those boundaries.
6. Fixed-step simulation resources drive extraction rates, queue progress, fuel/power consumption, and manufacturing timers. Frame deltas are UI-only.
7. Cargo mass/value must remain compatible with the existing mass-derivation direction: item definitions must expose canonical unit mass, and authoritative inventory/crafting mutations must trigger derived mass recomputation on the owning entity tree.
8. Modules, weapons, ships, and mounted equipment remain real ECS entity archetypes with graph-record bootstrap/hydration. Crafting should produce stackable materials, subassemblies, deployables, blueprint unlocks, or validated spawn jobs; it must not bypass the existing entity/bundle model.
9. Extraction and salvage logic must stay generic over entity families. Do not hardcode ship-only or asteroid-only terminology into shared resource/crafting runtime structures.

## 4. High-Level Model

### 4.1 Two-layer model: content catalogs and runtime state

The system should be split into two clear layers:

1. **Content catalogs**
   - item definitions,
   - recipe definitions,
   - facility definitions,
   - extraction/salvage yield profile definitions,
   - faction recipe-set and unlock definitions.
2. **Runtime state**
   - inventories on entities,
   - facility queues on stations/ships/outposts,
   - extraction depletion on resource entities/fields,
   - player/faction/station unlock state,
   - spawned finished items or graph-template manufacturing jobs.

Catalog data is shared read-only reference data.
Runtime state is authoritative ECS state.

### 4.2 Item classes

Sidereal should keep a readable item taxonomy:

1. `raw_mineral`
2. `raw_volatile`
3. `raw_biological`
4. `refined_material`
5. `chemical_feedstock`
6. `alloy`
7. `composite`
8. `synthetic`
9. `exotic_material`
10. `subassembly`
11. `ammo`
12. `equipment_kit`
13. `blueprint`
14. `end_product`

This keeps early mining/cargo readable while still allowing module-grade and shipyard-grade outputs later.

### 4.3 Content-authoring ownership

Canonical direction:

1. `data/scripts/economy/item_registry.lua`
2. `data/scripts/economy/recipe_registry.lua`
3. `data/scripts/economy/facility_registry.lua`
4. `data/scripts/economy/extraction_profiles.lua`

Rust should load, validate, and expose these through runtime registries/resources derived from the active script catalog, following the same ownership split already documented for assets and bundles.

Rust owns:

1. schema validation,
2. dependency graph validation,
3. fixed-step queue execution,
4. inventory transaction rules,
5. entity spawning/bootstrap for finished ships/modules/equipment,
6. persistence and replication rules.

Lua owns:

1. item lists,
2. recipe inputs/outputs,
3. facility capabilities,
4. faction flavor and unlock tables,
5. authored extraction yield tables,
6. progression tuning and content balance values.

## 5. Canonical Authoring Shape

The user's JSON-like starter schema is directionally correct, but Sidereal's canonical authoring surface should be Lua catalogs rather than JSON blobs embedded in Rust.

### 5.1 Item registry shape

Illustrative shape:

```lua
return {
  schema_version = 1,
  items = {
    {
      item_id = "resource.iron_ore",
      display_name = "Iron Ore",
      category = "raw_mineral",
      rarity = "common",
      tier = 0,
      fictional = false,
      stack_size = 100,
      unit_mass_kg = 2.0,
      base_value = 4,
      tags = { "ore", "metal" },
      sources = { "rocky_asteroid", "planetary_crust" },
      used_in = { "material.iron_ingots" },
      description = "Common ferrous ore used across hull and weapon production.",
    },
  },
}
```

### 5.2 Recipe registry shape

Illustrative shape:

```lua
return {
  schema_version = 1,
  recipes = {
    {
      recipe_id = "recipe.alloy.aetherium_alloy",
      facility_id = "advanced_alloy_forge",
      output = { item_id = "material.aetherium_alloy", amount = 1 },
      inputs = {
        { item_id = "material.titanium_alloy", amount = 2 },
        { item_id = "material.aetherite_shards", amount = 1 },
      },
      craft_time_s = 20,
      power_cost = 8,
      unlocks = { "metallurgy_3" },
      tags = { "alloy", "advanced_structure", "energy_conductive" },
    },
  },
}
```

### 5.3 Facility registry shape

Illustrative shape:

```lua
return {
  schema_version = 1,
  facilities = {
    {
      facility_id = "advanced_alloy_forge",
      display_name = "Advanced Alloy Forge",
      categories = { "alloy", "exotic_material" },
      queue_slots = 2,
      allowed_recipe_tags = { "alloy", "advanced_structure" },
      power_buffer_mw = 10,
    },
  },
}
```

### 5.4 Rust mirror types

Runtime loaders/resources should use strongly typed shared structs along the lines of:

```rust
pub struct ItemDef {
    pub item_id: String,
    pub display_name: String,
    pub category: ItemCategory,
    pub rarity: Rarity,
    pub tier: u8,
    pub fictional: bool,
    pub stack_size: u32,
    pub unit_mass_kg: f32,
    pub base_value: u32,
    pub tags: Vec<String>,
    pub sources: Vec<String>,
    pub used_in: Vec<String>,
    pub description: Option<String>,
}

pub struct RecipeDef {
    pub recipe_id: String,
    pub facility_id: String,
    pub inputs: Vec<RecipeInput>,
    pub output: RecipeOutput,
    pub craft_time_s: u32,
    pub power_cost: u32,
    pub unlocks: Vec<String>,
    pub tags: Vec<String>,
}
```

The exact type names can change, but the shared-definition pattern should not.

## 6. Bevy and ECS Integration Contract

### 6.1 Runtime resources

The Bevy app should eventually expose read-only definition resources such as:

1. `ItemCatalogResource`
2. `RecipeCatalogResource`
3. `FacilityCatalogResource`
4. `ExtractionProfileCatalogResource`

These are authoritative runtime mirrors of validated Lua content, not hand-maintained gameplay tables.

### 6.2 Persisted gameplay components

Expected component families include:

1. `Inventory`
   - entity-local item stacks and quantities,
   - persisted and owner-replicated by existing inventory policy.
2. `CraftingFacility`
   - facility type/capabilities on a station, ship module, or colony structure.
3. `CraftingQueue`
   - active recipe jobs, progress, reserved inputs, output staging, power/fuel requirements.
4. `RecipeUnlocks`
   - player/faction/station-scoped unlock state.
5. `ExtractionNode` or `ResourceYieldProfile`
   - links world entities to yield tables and depletion behavior.
6. `ManufacturingJob`
   - validated job describing a future entity spawn or kit output for ships/modules/weapons.

These should follow the normal component workflow in `docs/component_authoring_guide.md`: one file per primary component, `#[sidereal_component(...)]`, reflection + serde, graph persistence, and tests.

### 6.3 Inventory identity contract

Current docs already expose inventory operations in terms of `item_id`, while the current `InventoryEntry` Rust shape still carries an item UUID field.

Target direction:

1. recipe evaluation, script APIs, UI, pricing, and economy logic should resolve stackable goods by stable `item_id`,
2. if inventory retains UUID-backed entries for unique items later, those entries must still resolve to a canonical catalog `item_id` for crafting/economy operations,
3. crafting must never depend on raw Bevy entity handles inside recipe definitions.

### 6.4 Fixed-step systems

Expected authoritative systems:

1. extraction tick system,
2. salvage/loot conversion system,
3. crafting queue progression system,
4. inventory reservation/commit system,
5. mass dirtying + recomputation trigger path,
6. manufacturing completion system,
7. station/faction demand and price update hooks.

These run in fixed schedules and mutate authoritative ECS state only on the server/host authority side.

### 6.5 Downstream output model

Crafting outputs should support three downstream classes:

1. **Stack outputs**
   - ore, ingots, acids, polymers, ammo, circuit packs, armor plates.
2. **Kit outputs**
   - module kits, hull kits, weapon kits, drone kits.
3. **Manufacturing jobs**
   - validated outputs that spawn graph-record-backed entities or multi-entity archetypes through the existing bundle/template path.

This preserves Sidereal's current entity model for ships and mounted modules while still allowing industrial crafting to feed those outputs.

## 7. Facility Ladder

Recommended facility roster:

1. `extractor`
   - mining drills, gas pumps, salvage rigs.
2. `smelter`
   - metal and mineral refining.
3. `chemical_refinery`
   - volatile separation, acids, coolants, fuels.
4. `polymer_plant`
   - plastics and resin feedstocks.
5. `composite_fabricator`
   - carbon fiber, ceramic matrix, stealth fabrics, gels.
6. `advanced_alloy_forge`
   - exotic alloys and high-grade structural materials.
7. `biotech_vat`
   - living polymers, fungal gels, symbiotic tissue.
8. `capital_forge`
   - endgame hull plating, engine casings, citadel-grade armor.
9. `drydock`
   - ship hull and large module manufacturing jobs.

Facilities are capabilities on entities, not free-floating global menus.

## 8. Starter Material Taxonomy

### 8.1 Tier 0 raw extraction

**Metals and minerals**

- Iron Ore
- Nickel Ore
- Copper Ore
- Titanium Ore
- Aluminum Ore (Bauxite)
- Tungsten Ore
- Chromite Ore
- Cobalt Ore
- Silica
- Carbonaceous Rock
- Sulfur
- Lithium Brine
- Uraninite
- Rare Earth Concentrate

**Volatiles and chemicals**

- Water Ice
- Methane Ice
- Ammonia Ice
- Hydrogen Gas
- Helium-3
- Nitrogen Gas
- Chlorine Salts
- Crude Hydrocarbons
- Acidic Brine

**Exotic or faction-signature raws**

- Aetherite Crystal
- Void Salt
- Graviton Shale
- Sunspine Coral
- Phase Quartz
- Neutron Dust
- Mycelium Bloom
- Cryoflora Resin

### 8.2 Tier 1 refined outputs

**Standard refined materials**

- Iron Ingots
- Nickel Ingots
- Copper Cathodes
- Titanium Sponge
- Aluminum Ingots
- Tungsten Bars
- Chromium Metal
- Cobalt Metal
- Silicon Wafers
- Carbon Powder
- Graphite
- Refined Sulfur
- Lithium Salts
- Enriched Uranium
- Rare Earth Oxides

**Chemical feedstocks**

- Distilled Water
- Liquid Hydrogen
- Liquid Oxygen
- Ammonia
- Methanol
- Hydrocarbon Plastics Feedstock
- Industrial Acids
- Ceramic Slurry

**Refined exotic materials**

- Aetherite Shards
- Void Salt Crystals
- Graviton Flakes
- Sunspine Fibers
- Phase Quartz Lattice
- Neutron Paste
- Mycelial Substrate
- Cryoresin

### 8.3 Tier 2-3 industrial backbone

**Real or real-adjacent alloys**

- Steel
- Stainless Steel
- Titanium Alloy
- Alnico
- Nichrome
- Tungsten Steel
- Duralumin
- Copper-Nickel Alloy
- Cobalt Superalloy

**Core composites and synthetics**

- Polymer Resin
- Reinforced Polymer
- Carbon Fiber
- Fiberglass
- Ceramic Matrix Composite
- Titanium-Ceramic Plate
- Thermal Gel

### 8.4 Tier 3-4 Sidereal signature materials

**Fictional alloys**

- Voidsteel
- Aetherium Alloy
- Gravimetal
- Phase Nickel
- Sunforged Alloy
- Neutronium Laminate

**Fictional composites and biotech materials**

- Aetherglass
- Void Weave
- Phase Mesh
- Graviflex
- Cryoceramic
- Neurogel
- Sunspine Bioplastic
- Lattice Foam

### 8.5 Tier 5 faction or endgame assemblies

- Shadow Hull Plating
- Arc-Circuit Assembly
- Inertial Core Housing
- Stellar Engine Casing
- Citadel Armor
- Symbiotic Ship Tissue

These should be the bridge between materials and actual equipment/ship recipes.

## 9. Starter Production Tree

### 9.1 Baseline chain direction

Tier ladder:

1. Tier 0: raw extraction
2. Tier 1: refining
3. Tier 2: basic industrial products
4. Tier 3: advanced industrial products
5. Tier 4: exotic alloys
6. Tier 5: faction and endgame assemblies

### 9.2 Condensed tree

```text
RAW
|- Iron Ore -> Iron Ingots
|- Nickel Ore -> Nickel Ingots
|- Copper Ore -> Copper Cathodes
|- Titanium Ore -> Titanium Sponge
|- Aluminum Ore -> Aluminum Ingots
|- Tungsten Ore -> Tungsten Bars
|- Chromite Ore -> Chromium Metal
|- Cobalt Ore -> Cobalt Metal
|- Silica -> Glass / Ceramic Slurry / Silicon Wafers
|- Carbonaceous Rock -> Carbon Powder -> Graphite
|- Water Ice -> Distilled Water -> Hydrogen + Oxygen
|- Methane Ice / Crude Hydrocarbons -> Plastics Feedstock
|- Sulfur -> Refined Sulfur -> Industrial Acids
|- Aetherite Crystal -> Aetherite Shards
|- Void Salt -> Void Salt Crystals
|- Graviton Shale -> Graviton Flakes
|- Sunspine Coral -> Sunspine Fibers
|- Phase Quartz -> Phase Quartz Lattice
|- Neutron Dust -> Neutron Paste
|- Mycelium Bloom -> Mycelial Substrate
`- Cryoflora Resin -> Cryoresin

ALLOYS
|- Iron + Carbon -> Steel
|- Iron + Chromium + Nickel -> Stainless Steel
|- Titanium + Aluminum -> Titanium Alloy
|- Nickel + Chromium -> Nichrome
|- Aluminum + Copper -> Duralumin
|- Steel + Void Salt + Nickel -> Voidsteel
|- Titanium Alloy + Aetherite -> Aetherium Alloy
|- Tungsten + Graviton + Cobalt -> Gravimetal
|- Nickel + Phase Quartz -> Phase Nickel
|- Aluminum + Titanium + Sunspine Fibers -> Sunforged Alloy
`- Titanium Alloy + Neutron Paste -> Neutronium Laminate

COMPOSITES
|- Plastics Feedstock -> Polymer Resin
|- Polymer Resin + Carbon -> Reinforced Polymer
|- Carbon + Polymer Resin -> Carbon Fiber
|- Silica + Polymer Resin -> Fiberglass
|- Carbon Fiber + Ceramic Slurry -> Ceramic Matrix Composite
|- Titanium Alloy + Ceramic Matrix -> Titanium-Ceramic Plate
|- Silica + Aetherite + Rare Earths -> Aetherglass
|- Sunspine Fibers + Void Salt + Polymer Resin -> Void Weave
|- Carbon Fiber + Phase Quartz -> Phase Mesh
|- Mycelial Substrate + Graviton + Cryoresin -> Graviflex
|- Ceramic Slurry + Cryoresin + Tungsten -> Cryoceramic
|- Mycelial Substrate + Rare Earths + Acids -> Neurogel
`- Sunspine Fibers + Polymer Resin + Mycelial Substrate -> Sunspine Bioplastic

ENDGAME
|- Voidsteel + Void Weave -> Shadow Hull Plating
|- Aetherium Alloy + Aetherglass + Neurogel -> Arc-Circuit Assembly
|- Gravimetal + Phase Mesh -> Inertial Core Housing
|- Sunforged Alloy + Cryoceramic -> Stellar Engine Casing
|- Neutronium Laminate + Phase Nickel -> Citadel Armor
`- Sunspine Bioplastic + Graviflex -> Symbiotic Ship Tissue
```

## 10. Example Dependency Chains

1. **Basic hull**
   - Iron Ore -> Iron Ingots -> Steel -> Hull Plates
2. **Advanced stealth hull**
   - Nickel Ore -> Nickel Ingots
   - Phase Quartz -> Phase Quartz Lattice
   - Carbonaceous Rock -> Carbon Powder
   - Hydrocarbons -> Polymer Resin
   - Carbon Powder + Polymer Resin -> Carbon Fiber
   - Carbon Fiber + Phase Quartz Lattice -> Phase Mesh
   - Nickel Ingots + Phase Quartz Lattice -> Phase Nickel
   - Phase Mesh + Phase Nickel -> Stealth Skin
3. **Heavy battleship armor**
   - Titanium Ore -> Titanium Sponge
   - Aluminum Ore -> Aluminum Ingots
   - Titanium Sponge + Aluminum Ingots -> Titanium Alloy
   - Neutron Dust -> Neutron Paste
   - Titanium Alloy + Neutron Paste -> Neutronium Laminate
   - Neutronium Laminate -> Citadel Armor Segments
4. **Biotech faction hull**
   - Mycelium Bloom -> Mycelial Substrate
   - Sunspine Coral -> Sunspine Fibers
   - Hydrocarbons -> Polymer Resin
   - Sunspine Fibers + Polymer Resin + Mycelial Substrate -> Sunspine Bioplastic
   - Cryoflora Resin -> Cryoresin
   - Graviton Shale -> Graviton Flakes
   - Mycelial Substrate + Graviton Flakes + Cryoresin -> Graviflex
   - Sunspine Bioplastic + Graviflex -> Symbiotic Ship Tissue

## 11. Progression and Faction Identity

### 11.1 Suggested progression bands

**Early game**

- Iron
- Copper
- Aluminum
- Silica
- Carbon
- Water
- Hydrocarbons
- Steel
- Polymer Resin
- Fiberglass
- Carbon Fiber

**Mid game**

- Nickel
- Chromium
- Titanium
- Tungsten
- Cobalt
- Stainless Steel
- Titanium Alloy
- Nichrome
- Ceramic Matrix Composite
- Titanium-Ceramic Plate

**Late game**

- Aetherite
- Void Salt
- Phase Quartz
- Graviton Shale
- Sunspine Coral
- Aetherium Alloy
- Voidsteel
- Phase Mesh
- Aetherglass
- Gravimetal
- Cryoceramic

**Endgame**

- Neutron Dust
- Helium-3
- Rare Earth Oxides
- Neurogel
- Neutronium Laminate
- Citadel Armor
- Arc-Circuit Assembly
- Symbiotic Ship Tissue

### 11.2 Faction material identities

Recommended direction:

1. **Industrial / human**
   - Steel, Titanium Alloy, Ceramic Matrix Composite, Duralumin.
2. **Stealth / pirate / black-ops**
   - Voidsteel, Void Weave, Phase Mesh, Phase Nickel.
3. **High-tech / energy**
   - Aetherium Alloy, Aetherglass, Neurogel.
4. **Heavy military**
   - Gravimetal, Cryoceramic, Neutronium Laminate, Citadel Armor.
5. **Biotech / alien**
   - Sunspine Bioplastic, Graviflex, Symbiotic Ship Tissue.

## 12. Downstream Equipment Foundation

The material tree should become the common input language for all major crafted outputs:

1. **Ships**
   - hull plates,
   - engine casings,
   - armor segments,
   - cockpit/canopy optics,
   - drydock kit outputs.
2. **Weapons**
   - barrel alloys,
   - capacitor housings,
   - recoil frames,
   - specialty emitters and sensor meshes.
3. **Upgrades and modules**
   - flight computers,
   - sensor suites,
   - stealth linings,
   - shield/energy assemblies,
   - biotech control substrates.
4. **Ammunition and consumables**
   - ballistic rounds,
   - missiles,
   - volatile warheads,
   - coolant packs,
   - fuel cells.

Guideline:

1. ammunition should usually use shallow chains,
2. common ship modules should use the Tier 2-3 backbone,
3. faction-signature modules should consume Tier 4-5 materials,
4. capital-ship recipes should be drydock/capital-forge scale rather than ordinary inventory crafting.

## 13. Open Policy Questions

1. Final inventory payload shape for stackables: direct `item_id` stacks, UUID-backed entries with `item_id` resolution, or a hybrid model.
2. Exact split between portable crafted kits and direct facility manufacturing jobs for ships and mounted modules.
3. Exact station/faction unlock ownership model: player-only, faction-shared, station-local, or mixed.
4. Exact resource depletion and regrowth rules for mining regions, wreck salvage, and anomaly harvesting.
5. Exact price model and whether `base_value` is only authoring guidance or an authoritative economy seed input.
6. Whether recipes can support byproducts, catalysts, and probabilistic yields in V1 or should stay deterministic first.

## 14. Edit Checklist

For future implementation work touching resources/crafting:

1. Verify item/recipe/facility definitions remain Lua-authored and Rust-validated.
2. Verify queue/extraction state remains on persisted ECS entities or graph-related records, not side tables.
3. Verify inventory and crafting outputs trigger cargo/mass recomputation on affected entities.
4. Verify client/UI paths consume logical IDs and authoritative replicated state rather than local-only crafting simulation.
5. Verify native and WASM clients still compile against the same shared gameplay definition/runtime code.
6. Update this contract, related scripting docs, and background economy docs when the runtime model hardens further.
