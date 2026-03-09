# DR-0028: Generic Visibility Range Components

Status: Accepted  
Date: 2026-03-07  
Owners: replication + gameplay visibility + scripting

Primary references:
- `docs/features/visibility_replication_contract.md`
- `docs/sidereal_design_document.md`
- `AGENTS.md`

## 1. Context

Current visibility runtime needed to remove two legacy behaviors:

1. old scanner-oriented component names,
2. an implicit `ShipTag` baseline range rule.

This conflicts with the project rule that visibility/range logic must stay generic over entities and not embed ship-only assumptions.

## 2. Decision

Adopt the following generic component model:

1. `VisibilityRangeM`
   - effective resolved visibility/disclosure range used by the replication visibility hot path
   - authoritative final number read by range/candidate checks
2. `VisibilityRangeBuffM`
   - generic contributing modifier to visibility range
   - may exist on roots, children, mounted modules, or temporary effect entities
3. Root entities may carry both:
   - `VisibilityRangeM`
   - `VisibilityRangeBuffM`

Normative semantics:

1. The visibility system reads only the effective root `VisibilityRangeM`.
2. `VisibilityRangeBuffM` is not read directly by the visibility hot path.
3. A separate aggregation system computes final root visibility range from generic contributors.
4. No implicit baseline range may come from `ShipTag` or any other genre-specific tag.
5. If a ship or any other archetype should have starter visibility capability, that must be authored explicitly in data/bundles/modules.

## 3. Rationale

This keeps the engine/runtime semantics generic while still allowing genre-specific Lua authoring.

Examples:

1. space-game Lua can describe a `scanner` module that adds `visibility_range_buff_m`,
2. fantasy-game Lua could describe a `watchtower` or `scrying crystal` that adds the same generic component,
3. temporary actions such as `scanner_ping` can grant a short-lived `VisibilityRangeBuffM` on the root or on a helper child/effect entity.

This preserves:

1. generic engine vocabulary in Rust,
2. genre-specific naming in Lua/content,
3. fast hot-path visibility evaluation without Lua callbacks or deep hierarchy scans every check.

## 4. Aggregation Model

The intended runtime model is:

1. gather generic visibility-range contributions from:
   - root-local `VisibilityRangeBuffM`,
   - child/module/helper entities carrying `VisibilityRangeBuffM`,
   - temporary effects/buffs,
2. compute one final effective `VisibilityRangeM` on the root,
3. run visibility candidate/range checks from that final root value only.

This means root entities may validly have both:

1. an effective `VisibilityRangeM`, and
2. one or more `VisibilityRangeBuffM` contributors.

## 5. Explicitly Rejected

1. `ShipTag`-based default visibility/scanner range
   - rejected because it is genre-specific hidden behavior
2. Lua callbacks during hot visibility checks
   - rejected because visibility must remain bounded, deterministic, and cheap
3. scanning children/modules deeply inside every visibility evaluation
   - rejected because aggregation should happen once on relevant structural/component changes, not per candidate test
4. `SensorRange*` as engine-owned built-in names
   - rejected because it is too domain-flavored for the generic runtime layer

## 6. Implementation Status

This decision is now implemented in active runtime paths:

1. runtime aggregation and visibility code use `VisibilityRangeM` / `VisibilityRangeBuffM`,
2. `ShipTag` no longer grants implicit baseline visibility range,
3. Lua ship bundles now author `visibility_range_buff_m`,
4. `VisibilityDisclosure` publishes `visibility_sources`,
5. old scanner-oriented component names were removed without compatibility shims.
