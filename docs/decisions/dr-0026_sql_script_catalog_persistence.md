# DR-0026: SQL Tables for Durable Script Catalog Persistence

Status: Accepted  
Date: 2026-03-07  
Owners: Runtime + scripting + persistence

## Context

Once runtime script source moved into authoritative in-memory catalogs, the remaining durability question was where persisted script source should live.

Three options were on the table:

1. Persist script source in graph ECS records.
2. Invent direct Bevy-resource persistence.
3. Persist script source in relational SQL tables and hydrate runtime catalogs from there.

Scripts are not simulation entities. They do not need graph relationships, world hydration semantics, or ECS component replication. What they do need is stable content identity, revision history, active-version selection, and future publishing workflow support.

## Decision

Durable script persistence uses SQL tables, not graph records.

Layering:

1. Runtime execution authority is the in-memory script catalog.
2. Durable authority is the SQL-backed active script catalog.
3. Filesystem `.lua` content is seed/default input and explicit reload source only.

Current schema:

1. `script_catalog_documents`
   - one row per logical `script_path`
   - stores `script_family`
   - stores `active_revision`
   - stores create/update timestamps
2. `script_catalog_versions`
   - one row per `(script_path, revision)`
   - stores source text and origin metadata
3. `script_catalog_drafts`
   - one row per draft `script_path`
   - stores unpublished source text, origin, family, and update timestamp

Replication automatically flushes normalized in-memory catalog changes into the active catalog tables. Gateway loads from the same active SQL catalog and only falls back to disk when seeding an empty database or running helper-only/test scenarios without a reachable DB.

Gateway now also exposes authenticated admin-only draft/publish API routes on top of this schema:

1. save draft
2. inspect active + draft state
3. publish draft to a new immutable revision
4. discard draft
5. reload active catalog from disk

## Alternatives considered

1. **Graph persistence**
   - Rejected because scripts are content records, not gameplay entities.
   - Would mix tooling/content-authoring state into simulation persistence shape.
2. **Resource persistence**
   - Rejected because it would create a second persistence model outside the repo’s established persistence boundaries.
3. **Disk-only persistence**
   - Rejected because it does not support runtime editing, publishing, or durable multi-service authority.

## Consequences

### Positive

1. Durable script storage now matches the content-management problem, not the simulation-entity model.
2. Gateway and replication can share one durable catalog authority.
3. Future draft/publish/version workflows have a natural relational home.

### Negative

1. Adds another SQL schema surface that must be kept in sync across services.
2. Current implementation is still “active catalog replacement”, not a full publish workflow.

## Follow-up

1. Add explicit dashboard/API write flows for script edit/publish, rather than relying only on replication-local BRP edits.
2. Add richer publish semantics if needed: rollback selection, author metadata, multi-environment promotion.
3. Add integration coverage for SQL-backed script catalog bootstrapping, restart persistence, and publish-to-runtime activation.

## Feature doc

`docs/features/scripting_support.md`

## References

1. `crates/sidereal-persistence/src/lib.rs`
2. `bins/sidereal-replication/src/replication/scripting.rs`
3. `bins/sidereal-gateway/src/auth/starter_world_scripts.rs`
4. `docs/features/scripting_support.md`
