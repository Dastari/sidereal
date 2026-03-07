# DR-0019: Lua Asset Registry Authority and Gateway HTTP Asset Delivery

Status: Accepted
Date: 2026-03-06
Owners: Scripting + gateway + replication + client runtime

## Context

Sidereal asset delivery policy previously relied on replication-streamed chunk transport and still retained hardcoded Rust-side asset naming/mapping in some runtime paths. This conflicts with the scripting direction where content authority should be data-driven and modifiable without Rust code edits.

The project now requires:

1. Asset definitions to be script-authored.
2. Asset payload downloads to happen through gateway HTTP route `/assets/<asset_guid>`.
3. A pre-world client loading barrier for required assets.

## Decision

1. The Lua asset registry is the authoritative source for runtime asset definitions.
2. Rust runtime code must not hardcode concrete asset IDs, filenames, shader names, material names, audio names, or sprite names.
3. Server build/publish tooling generates authoritative catalog metadata (including checksum and immutable `asset_guid`) from Lua registry entries.
4. Asset payload bytes are delivered through authenticated gateway HTTP GET `/assets/<asset_guid>`.
5. Client world entry lifecycle includes a dedicated `AssetLoading` state before `InWorld`.
6. Client receives startup JSON manifest containing required asset list with `asset_id`, `asset_guid`, checksum, and URL.
7. Client performs cache checksum validation and downloads required assets before entering `InWorld`.
8. Runtime missing assets are lazily fetched by `asset_guid` when new `asset_id` references appear.

## Alternatives Considered

1. Keep replication-streamed asset chunks as primary runtime path: rejected (couples asset payload transport to gameplay replication and conflicts with gateway route requirement).
2. Keep Rust hardcoded baseline/critical asset lists: rejected (breaks data-driven scripting authority and introduces drift).
3. Use direct filename/path references in replicated components: rejected (breaks content abstraction and cache/version safety).

## Consequences

### Positive

1. Clear single source of truth for content asset definitions.
2. Asset transport path is explicit and easier to observe/secure at gateway boundary.
3. Cleaner client lifecycle with deterministic pre-world asset readiness.
4. Better mod/content iteration without Rust code churn for asset registration.

### Negative

1. Requires substantial migration from replication chunk protocol paths.
2. Adds gateway load and requires robust auth/rate-limiting/telemetry.
3. Requires coordinated updates across scripting, gateway, client state machine, and testing.

## Follow-Up

1. Implement phases and tests in `docs/features/asset_delivery_contract.md`.
2. Extend scripting contract in `docs/features/scripting_support.md` with registry schema/runtime integration.
3. Update source-of-truth architecture docs and AGENTS rules to remove old "no HTTP asset serving" policy.
4. Add integration and parity tests for `AssetLoading` and runtime lazy fetch.

## References

- `docs/features/asset_delivery_contract.md`
- `docs/features/scripting_support.md`
- `docs/sidereal_design_document.md`
- `AGENTS.md`
