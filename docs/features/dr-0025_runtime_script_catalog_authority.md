# DR-0025: Runtime Script Catalog Authority

Status: Accepted  
Date: 2026-03-07  
Owners: Runtime + scripting + dashboard/tooling

## Context

Sidereal is moving toward live script authoring and inspection through Bevy Remote Protocol and dashboard tooling. The current scripting implementation still mixes two models:

1. filesystem Lua files as the de facto execution source,
2. partial Bevy resource mirrors for registry metadata (`EntityRegistryResource`, `AssetRegistryResource`) on the replication host.

That is not sufficient for the intended workflow:

1. edit scripts from dashboard without restarting services,
2. have the next execution use the edited source,
3. support explicit `reload from disk` for seed/default scripts,
4. eventually persist scripts to the database as published runtime content.

If runtime execution continues to read directly from disk, BRP-visible resources are only an inspection surface and not the actual source of truth. That breaks live editing and makes reload semantics ambiguous.

## Decision

The intended authoritative script model is:

1. Runtime script source lives in Bevy-managed in-memory resources.
2. Script execution always resolves source from those resources, not directly from filesystem files.
3. Filesystem `.lua` content is bootstrap seed input and explicit reload source only.
4. BRP/dashboard edits mutate the in-memory script catalog and must take effect on the next script execution after cache invalidation/revision bump.
5. A runtime `reload from disk` operation replaces in-memory script content from the filesystem seed set and invalidates any derived compiled caches.
6. Long term, persisted/published scripts stored in the database become the durable authoritative source, with disk remaining the pre-seed/default layer.

This applies across script families, while respecting their different execution semantics:

1. `world_init.lua` edits affect the next invocation only; they do not retroactively re-run guarded bootstrap content.
2. bundle scripts and bundle registry edits affect future spawn requests, not already persisted entities.
3. asset registry edits affect future asset resolution/catalog rebuild behavior according to asset pipeline rules.
4. runtime AI/event scripts affect future callback execution after resource revision change.

## Alternatives considered

1. Keep filesystem files as the direct runtime authority and use BRP only for viewing:
   - Rejected because it does not support live editing or deterministic reload semantics.
2. Allow dashboard to mutate files on disk directly and rely on file polling:
   - Rejected because it couples tooling to filesystem deployment details and does not provide a clean path to DB-backed script publishing.
3. Move immediately to DB-only scripts with no disk seed support:
   - Rejected because disk-based seed content is still valuable for local development, bootstrap, and version-controlled defaults.

## Consequences

### Positive

1. Makes dashboard-based live Lua editing viable without service restart.
2. Gives BRP-visible script state real authority instead of being observational only.
3. Cleanly separates bootstrap seed sources from runtime authoritative source.
4. Provides a direct migration path toward DB-backed script publishing.

### Negative

1. Requires explicit script catalog resources, revisioning, and invalidation rules.
2. Requires careful per-script-family lifecycle semantics.
3. Adds more runtime state that must be observed, validated, and eventually persisted.

## Current implementation status

Implemented now:

1. Replication host exposes BRP-visible `ScriptCatalogResource` and `ScriptCatalogControlResource`.
2. Replication runtime script execution, world init, bundle spawning, and derived registry resources all resolve source from the in-memory catalog rather than directly from disk.
3. Replication supports explicit `reload_all_from_disk_requested` to replace the active in-memory catalog from disk seed content.
4. Gateway also executes from a cached in-memory script catalog rather than directly from disk.
5. Gateway supports explicit `reload_script_catalog_from_disk(root)` to replace its active cached catalog.

Still pending:

1. first-class dashboard publish/edit APIs for gateway-side script mutation,
2. richer script publishing workflow semantics (drafts/publish/promote), beyond the current active-catalog replacement model.

## Follow-up

1. Introduce a reflected `ScriptCatalogResource` keyed by logical script path/id with source text, revision, and origin metadata.
2. Change runtime script execution surfaces to load/evaluate from `ScriptCatalogResource`.
3. Add `reload_all_scripts_from_disk` and targeted reload controls.
4. Add BRP/dashboard-safe mutation path for script source updates.
5. Extend the active-catalog model into richer publish workflow semantics (drafts/promote/rollback) if tooling needs it.

## Feature doc

`docs/features/scripting_support.md`

## References

1. `bins/sidereal-replication/src/replication/scripting.rs`
2. `bins/sidereal-replication/src/replication/runtime_scripting.rs`
3. `docs/features/scripting_support.md`
