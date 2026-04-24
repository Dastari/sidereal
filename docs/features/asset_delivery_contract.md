# Asset Delivery Contract

Status: Active implementation contract
Last updated: 2026-04-24
Owners: gateway + client runtime + asset pipeline
Scope: Lua-authored asset catalogs, gateway delivery, client cache/bootstrap, runtime dependency fetches
Primary architecture reference: `docs/sidereal_design_document.md`
Related contract: `docs/features/scripting_support.md`
Decision Register linkage: `DR-0004`, `DR-0005`, `DR-0006`, `DR-0019`
Related render-layer contract: `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`

## 0. Implementation Status

2026-04-24 status note:

1. Implemented: gateway asset routes expose startup manifests, authenticated bootstrap manifests, and `/assets/<asset_guid>` payload fetches from the runtime catalog.
2. Implemented: native and WASM client cache adapters load/save checksum-indexed assets and share the same startup/bootstrap state machine at the runtime boundary.
3. Implemented: runtime optional asset dependencies can be requested after world entry, persisted into the cache, and surfaced through debug/perf counters.
4. Implemented: audio and shader catalogs are now integrated with the same Lua-authored asset delivery model.
5. Open work: production packaging still needs the single `assets.pak` distribution shape and broader live WASM validation for all asset classes.

## 1. Purpose

Define the enforceable runtime contract for Sidereal asset delivery under a Lua-authored asset registry model.

Core outcomes:

1. Asset definitions (IDs, classes, dependencies, preload policy) are authored in Lua, not Rust.
2. Server builds authoritative asset metadata/checksums from the Lua registry.
3. Client enters a dedicated `StartupLoading` state before `Auth` and a dedicated `AssetLoading` state before `InWorld`, validating/downloading the required asset sets for each lifecycle stage.
4. Asset payloads are fetched through authenticated gateway HTTP route `/assets/<asset_guid>`.
5. Runtime cache is checksum-verified and reused across sessions.

## 2. Scope and Non-Goals

### In scope

- Lua registry schema and ownership rules.
- Server-side catalog build and checksum generation.
- Client startup manifest sync and cache validation.
- Pre-world required asset download flow.
- Runtime lazy asset fetch behavior for newly referenced `asset_id` values.
- Native/WASM parity requirements.

### Out of scope

- DCC authoring tooling UX details.
- CDN and launcher installer strategy.
- Binary format bikeshedding beyond checksum/caching guarantees.

## 3. Non-Negotiable Invariants

1. Asset authority is server-side and data-driven from Lua registry content.
2. Rust runtime code must not hardcode gameplay asset IDs, filenames, shader names, material names, sprite names, or audio clip names.
3. Asset identity crossing service boundaries uses logical `asset_id` and immutable `asset_guid` only.
4. Each published asset version has its own immutable generated `asset_guid`.
5. Asset payload bytes are not streamed over replication transport channels.
6. Asset payload downloads use authenticated gateway HTTP GET `/assets/<asset_guid>` only.
7. Missing/corrupt assets must fail soft; client must not crash due to missing content.
8. Cache trust decisions are deterministic via checksum (`sha256`) and version metadata.
9. Native and WASM clients must implement the same asset state machine and validation logic.
10. Browser/WASM runtime asset mounting is byte-backed from validated cache or gateway payload bytes; browser code must not depend on filesystem-style `AssetServer` paths.
11. Active script sources and asset source files must be polled for hot-reload changes on a bounded cadence (default 5 seconds via `SIDEREAL_ASSET_HOT_RELOAD_INTERVAL_S`), and authoritative catalog changes must invalidate connected clients through the existing manifest/bootstrap path.

## 4. Terminology

- `asset_id`: stable logical content identifier used by gameplay/scripts/components.
- `asset_guid`: immutable generated ID for one published payload version; used in gateway URL path.
- `lua asset registry`: runtime script-authored source of truth describing all known assets and policies.
- `bootstrap required assets`: assets that must be present before client transitions from `AssetLoading` to `InWorld`.
- `startup required assets`: assets that must be present before pre-auth/startup UI features may rely on them (for example login-screen music).
- `runtime optional assets`: non-blocking assets fetched on demand when referenced after world entry.
- `asset catalog`: generated server artifact derived from Lua registry + build pipeline metadata.

## 5. Lua Asset Registry Contract

The authoritative registry is defined in Lua under the scripts root (for example `data/scripts/assets/registry.lua`).

### 5.1 Required registry fields per asset entry

Minimum canonical shape:

```lua
return {
  schema_version = 1,
  assets = {
    {
      asset_id = "sprite.ship.rocinante",
      content_type = "image/png",
      source_path = "sprites/ships/rocinante.png",
      dependencies = { "shader.sprite.pixel_default" },
      bootstrap_required = true,
      startup_required = false,
      tags = { "ship", "starter" },
    },
  },
}
```

Rules:

1. `asset_id` is the script/gameplay-facing identifier.
2. `source_path` is authoring-time input only; it never crosses to client-facing runtime protocols.
3. Published `relative_cache_path` values are generated runtime metadata and must not reveal authoring/source-tree layout.
4. `dependencies` are logical `asset_id` references.
5. `bootstrap_required = true` marks assets that must be present before `InWorld`.
6. `startup_required = true` marks assets that must be present before pre-auth/startup UI features may rely on them. It is distinct from `bootstrap_required`.
7. All fields are validated by Rust loader/schema checks; invalid registry blocks activation.

2026-03-14 update:
1. The Lua asset registry and generated runtime catalog now carry `startup_required` as a first-class policy bit.
2. `startup_required` is authorable today and intended for pre-auth/login-screen content such as menu music.

2026-03-15 implementation update:
1. Gateway now exposes a public startup manifest route at `GET /startup-assets/manifest`.
2. Gateway now exposes a public startup payload route at `GET /startup-assets/<asset_guid>` and serves only assets authored with `startup_required = true`.
3. Client now enters `StartupLoading` before `Auth`, validates/downloads `startup_required` assets into the shared cache, applies the startup audio catalog subset, and only then transitions into the auth UI.
4. The authenticated `/assets/bootstrap-manifest` path remains the authoritative `bootstrap_required` lane for pre-world and in-world runtime content.

### 5.2 Forbidden runtime patterns

1. Hardcoded Rust maps such as `asset_id -> file path`.
2. Hardcoded Rust lists of "always preload" assets.
3. Per-feature Rust constants naming concrete shader/sprite/audio files.

Rust may define generic systems and schema validators only.

## 6. Catalog Build and Publish Contract

Server-side tooling must build a generated catalog from Lua registry entries:

1. Load Lua registry and validate schema/uniqueness/dependencies.
2. Resolve source files and compute payload checksum (`sha256`).
3. Generate immutable `asset_guid` for each published payload version.
4. Produce catalog metadata including:
   - `asset_id`
   - `asset_guid`
   - optional shader-family/domain/signature/schema compatibility metadata
   - `sha256_hex`
   - `byte_len`
   - `content_type`
   - dependency list
   - `bootstrap_required`
   - `startup_required`
5. Publish payload bytes to gateway-readable storage.
6. Publish an active catalog version pointer consumed by gateway/replication.

Current implementation note:
1. Gateway now builds runtime asset catalog entries through the shared `sidereal-asset-runtime` path.
2. Gateway asset manifest and payload routes resolve the active Lua asset registry from the cached in-memory script catalog / active SQL-backed catalog rather than reloading `assets/registry.lua` from disk on each request.
3. Gateway payload serving resolves `/assets/<asset_guid>` through shared runtime asset materialization into generated published storage (`<asset_root>/published_assets/...`) instead of route-local authoring path access.
4. Source-tree `source_path` remains authoring input only and is not exposed to the client-facing manifest.
5. Runtime catalog entries and manifest entries now carry dependency lists, and gateway expands bootstrap-required dependency closure before returning `required_assets`.
6. `catalog_version` is now derived deterministically from the generated runtime catalog contents instead of a fixed placeholder string.
7. Client bootstrap-required asset gating now fails closed for required assets; stalled required downloads surface dialogs but do not force `InWorld`.
8. Browser/WASM client runtime now mounts streamed shaders, images, and SVGs from validated cached bytes instead of relying on `AssetServer` filesystem paths.

## 7. Gateway Delivery Contract

2026-03-09 update:
1. Gateway script-catalog and runtime asset-catalog caches are no longer process-lifetime sticky.
2. Gateway must re-check the active script catalog and rebuild the runtime asset catalog after the hot-reload poll interval elapses, even when `assets/registry.lua` revision is unchanged, so source-byte edits for textures, shaders, SVGs, audio, and other payloads become visible without a process restart.

### 7.1 Payload route

- Route: `GET /assets/<asset_guid>`
- Auth: required; bound to session/account policy.
- Response: asset bytes with metadata headers (`content-type`, checksum header, cache headers).
- Unknown/unauthorized `asset_guid` returns fail-closed error.

### 7.2 Startup manifest route

- Route: `GET /startup-assets/manifest`
- Auth: not required.
- Scope: returns only `startup_required` assets plus startup-safe dependency closure and a startup-safe audio catalog subset.
- Response: startup catalog version, startup audio catalog version, required startup assets, and startup catalog entries.

### 7.3 Startup payload route

- Route: `GET /startup-assets/<asset_guid>`
- Auth: not required.
- Scope: serves only assets authored with `startup_required = true`.
- Unknown/non-startup `asset_guid` returns fail-closed error.

### 7.4 Bootstrap metadata payload

Before world entry, client receives a JSON manifest from server (via gateway API or session bootstrap response) containing at minimum:

```json
{
  "catalog_version": "2026-03-06T00:00:00Z",
  "required_assets": [
    {
      "asset_id": "sprite.ship.rocinante",
      "asset_guid": "2cf33ea8-3c79-4f24-97a9-72d971dc7f43",
      "shader_family": "world_sprite_generic",
      "dependencies": ["shader.sprite.pixel_default"],
      "sha256_hex": "ab12cd34...",
      "url": "/assets/2cf33ea8-3c79-4f24-97a9-72d971dc7f43"
    }
  ],
  "catalog": []
}
```

`catalog` may contain the full asset list for runtime lazy fetch optimization.

## 8. Client State Machine Contract

Client entry flow is:

`StartupLoading -> Auth -> CharacterSelect -> WorldLoading -> AssetLoading -> InWorld`

### 8.1 StartupLoading requirements

In `StartupLoading`, client must:

1. Fetch the public startup manifest from gateway.
2. Load the shared local cache index/metadata.
3. Verify startup-required assets by checksum.
4. Download missing/stale startup-required assets via `/startup-assets/<asset_guid>`.
5. Commit validated startup assets to the shared local cache and cache index.
6. Apply the startup audio catalog subset so pre-auth UI audio can resolve authored profile IDs.
7. Transition to `Auth` only after the startup-required asset pass has either completed successfully or failed-soft with surfaced error state.

### 8.2 AssetLoading requirements

In `AssetLoading`, client must:

1. Load local cache index/metadata.
2. Verify checksums for known cached entries (full scan at startup or deterministic rolling strategy with equivalent guarantee).
3. Compare required manifest entries by `asset_guid` + checksum.
4. Download missing/stale required assets via gateway `/assets/<asset_guid>`.
5. Commit validated assets to local cache.
6. Transition to `InWorld` only when all required assets validate.
7. Required-asset stalls may surface warning/error dialogs and retry behavior, but they must not force bootstrap completion in degraded mode.
8. Browser/WASM implementations may use platform storage primitives behind the cache adapter, but the gameplay/runtime layer must still consume validated asset bytes through the shared cache/index contract.

### 8.3 Runtime lazy fetch

2026-03-12 native impact: runtime optional-asset completion now persists cache payload bytes and saves the cache index off the main frame thread; dependency graph refresh and shader-assignment refresh are dirty-driven instead of full always-on polling every update.
2026-03-12 WASM impact: no contract change; browser/WASM clients keep the same shared asset state machine, and the native-only async cache persistence path does not change byte-backed WASM mounting requirements.

After entering world:

1. If entity/component references `asset_id` not yet locally available, client triggers background download.
2. Resolver maps `asset_id` -> `asset_guid` via received catalog metadata.
3. Shader-material systems resolve shader sources by authoritative `shader_asset_id` plus catalog domain/signature/schema metadata.
4. Visual/audio fallback stays active until asset validates and mounts.
5. Swap-in remains atomic; failed load keeps fallback and schedules retry.
6. Runtime shader install uses catalog/cache-provided shader bytes when available; built-in fallback is limited to one emergency shader per generic runtime family rather than a compiled-in WGSL source per named content case.
7. Runtime lazy fetch expands dependency closure from catalog metadata and fetches unresolved dependencies before attaching the root asset.
8. Browser/WASM shader/image/SVG attach paths mount from cached payload bytes through runtime loaders, not through direct `data/cache_stream/...` path loads.

### 8.4 Live asset hot reload

2026-03-09 native impact: native client now listens for authoritative catalog-version invalidations and refreshes manifest/cache state live while already in-world.
2026-03-09 WASM impact: no WASM-specific contract change; browser clients are expected to follow the same manifest invalidation and byte-backed cache refresh path when the shared runtime path is enabled.

After the initial bootstrap completes:

1. Replication sends authoritative asset-catalog version invalidations over the manifest channel when the server detects script or source-asset changes.
2. Client must re-fetch the gateway bootstrap manifest when the pushed `catalog_version` differs from the local catalog version.
3. Required assets still flow through the normal bootstrap validation/download path; do not invent a second authoritative download path for hot reload.
4. Changed optional assets may be eagerly refreshed in the background after the manifest refresh completes.
5. Runtime shader, image, and SVG attach paths must invalidate local handle caches and rebind from validated bytes when the catalog version changes.
6. If an asset disappears from the authoritative catalog, client must fall back safely rather than continue treating the removed asset as authoritative.

## 9. Runtime Entity Asset Resolution Contract

1. Replicated/persisted gameplay components carry logical `asset_id` values only.
2. Client runtime continuously evaluates entities/components for referenced `asset_id` values.
3. Missing local entries enqueue fetch jobs by resolved `asset_guid`.
4. Dependency closure must be honored; if asset `A` depends on `B`, both are fetched/validated before final attach.

## 10. Cache Contract

Target native-desktop cache shape remains:

```text
data/cache_stream/
  assets.pak
  assets.index
  assets.tmp
```

Minimum guarantees:

1. `assets.index` tracks `asset_id`, `asset_guid`, `sha256`, offsets/lengths, content type, and schema version.
2. Cache read path validates checksum before exposing a ready asset.
3. Interrupted writes recover safely via transactional temp/journal process.
4. Missing/corrupt cache state auto-recovers without manual user steps.

Browser/WASM note:

1. Browser storage adapters may not expose a literal `assets.pak` file.
2. Browser implementations must still preserve the same logical guarantees: authoritative checksum validation, deterministic index metadata, recovery on invalid cached payloads, and byte-backed runtime mounting.
3. Browser cache/storage layout is an adapter detail; gameplay/runtime systems must not depend on browser-visible file paths.
4. Current browser implementation uses IndexedDB as the persistent asset/index backend and mirrors validated payload bytes into runtime memory for synchronous render-path reads.

## 11. Security and Abuse Controls

1. Asset manifest and asset payload access must be session-authenticated.
2. Gateway denies unknown/non-active `asset_guid`.
3. Optional per-session/per-IP rate limits for asset route.
4. Manifest generation must not leak unauthorized world data through hidden asset references.
5. Checksum mismatch and repeated failures trigger telemetry and guardrails.

## 12. Observability Contract

Required metrics/logs:

- `asset_loading_duration_ms`
- required asset count/bytes
- cache hit/miss/stale rates
- checksum mismatch count
- runtime lazy fetch request count
- failed downloads and retry counts
- per-client asset-loading transition latency

## 13. Implementation Plan

### Phase A: Contracts and schema

1. Add Lua asset registry module and schema validation in `sidereal-scripting`.
2. Remove remaining hardcoded Rust asset lists/maps from active paths.
3. Add explicit validation errors for registry duplicates/missing deps/invalid fields.

### Phase B: Catalog builder and gateway metadata

1. Build generated asset catalog from Lua registry including checksum + generated `asset_guid`.
2. Add startup asset manifest JSON payload carrying required assets and optional full catalog.
3. Wire gateway `/assets/<asset_guid>` route to immutable published payloads.

Status update (2026-03-07):
1. Runtime manifest generation is shared through `sidereal-asset-runtime`.
2. Gateway `/assets/<asset_guid>` now serves through the shared runtime asset materialization/storage path.
3. Remaining work is moving from on-demand materialization to a cleaner published catalog/storage lifecycle with fewer runtime rebuilds.

### Phase C: Client `AssetLoading` state

1. Add dedicated `AssetLoading` client state between `WorldLoading` and `InWorld`.
2. Implement cache checksum verification pass and required-asset download barrier.
3. Block `InWorld` transition until required manifest validates.

### Phase D: Runtime lazy fetch

1. Add runtime asset resolver from replicated `asset_id` references.
2. Trigger background fetch for missing assets via catalog `asset_guid` mapping.
3. Keep fail-soft placeholders and atomic swap behavior.

### Phase E: Hardening and parity

1. Add native/WASM parity checks for startup and runtime fetch behavior.
2. Add retry/backoff and error UX for persistent download failures.
3. Add telemetry dashboards and alert thresholds.

## 14. Test Plan

### 14.1 Unit tests

- Lua registry schema validation and duplicate detection.
- Dependency closure and cycle detection.
- Catalog generation determinism (`asset_id`, checksum, generated `asset_guid`).
- Cache checksum validation and corruption recovery.

### 14.2 Integration tests

- Empty cache startup: required assets download then `InWorld` transition.
- Warm cache startup: checksum-valid required assets skip download.
- Checksum mismatch: stale entry redownload and replace.
- Runtime missing asset reference: lazy fetch + fallback swap.
- Unauthorized/unknown `asset_guid` fetch: fail closed without crash.

### 14.3 End-to-end tests

- Login -> Enter World -> AssetLoading -> InWorld lifecycle.
- Full catalog receipt + on-demand fetch during gameplay.
- Restart with persistent cache reuse.
- Native and WASM parity for manifest processing and gateway fetch paths.

## 15. Acceptance Criteria

All must be true:

1. Asset definitions come from Lua registry; no Rust hardcoded asset naming lists remain in runtime paths.
2. Server generates catalog entries with checksum and immutable `asset_guid`.
3. Client has an explicit `AssetLoading` state before `InWorld`.
4. Required assets are validated/downloaded before entering world.
5. Runtime missing assets are fetched lazily via gateway `/assets/<asset_guid>`.
6. Missing/corrupt assets fail soft with placeholders and no client crash.
7. Native and WASM behavior is equivalent at gameplay boundary.
