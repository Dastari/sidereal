# Generic Visibility Range Migration Plan

Status: Implemented reference note  
Date: 2026-03-07

Primary references:
- `docs/features/dr-0028_generic_visibility_range_components.md`
- `docs/features/visibility_replication_contract.md`
- `AGENTS.md`

## Goal

This migration has been implemented. The plan remains as a reference for the adopted generic root-effective visibility-range model:

1. `VisibilityRangeM`
2. `VisibilityRangeBuffM`
3. no implicit `ShipTag` baseline

## Target Runtime Shape

1. Root entity carries final effective `VisibilityRangeM`.
2. Any root, child, module, or temporary effect may carry `VisibilityRangeBuffM`.
3. Aggregation system computes the final root visibility range from generic contributors.
4. Hot-path visibility checks read only final root `VisibilityRangeM`.

## Migration Steps

1. Add generic components in `crates/sidereal-game`:
   - `visibility_range_m.rs`
   - `visibility_range_buff_m.rs`
2. Update `components/mod.rs` exports and registry coverage.
3. Replace legacy scanner-oriented naming in runtime systems:
   - replication visibility
   - replication runtime-state aggregation
   - any gameplay helpers/tests/docs that use `ScannerRange*`
4. Remove `ShipTag` baseline visibility behavior.
5. Update Lua-authored bundles/content to grant generic visibility-range components explicitly.
6. Update docs and script examples to use the generic names.
7. Remove old scanner-oriented component kinds once all producers/consumers are migrated.

## Constraints

1. Do not introduce compatibility aliases/shims.
2. Do not call Lua during hot visibility checks.
3. Do not deeply rescan hierarchy in the visibility hot path.
4. Keep the visibility system generic over entities.

## Examples To Support

1. ship with a mounted scanner module:
   - child/module contributes `VisibilityRangeBuffM`
2. temporary scanner ping:
   - root or temporary effect entity grants short-lived `VisibilityRangeBuffM`
3. non-space content:
   - watchtower / ward / detection aura / sonar beacon all use the same generic components

## Follow-On Questions

1. Should the aggregation formula stay additive + multiplicative only?
2. Do we later want a separate generic `VisibilitySignatureM` for detectability/stealth?
3. Should root entities without any effective range omit `VisibilityRangeM` entirely, or keep `0.0` explicitly?
