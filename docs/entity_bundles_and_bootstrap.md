# Entity Bundles and Bootstrap (Historical Note)

Status: Historical/redirect  
Last updated: February 24, 2026

This document previously contained exploratory notes about bundle structure and bootstrap flows.
Its useful guidance has been consolidated into:

- `docs/component_authoring_guide.md` (entity archetype layout, bundle/spawn rules, bootstrap graph-template guidance)

Current implementation and source-of-truth references:

- `crates/sidereal-game/src/entities/`
- `crates/sidereal-runtime-sync/src/entity_templates.rs`
- `bins/sidereal-gateway/src/auth.rs` (direct bootstrap dispatch)
- `bins/sidereal-replication/src/bootstrap_runtime.rs`

Do not treat this file as an active design contract.
