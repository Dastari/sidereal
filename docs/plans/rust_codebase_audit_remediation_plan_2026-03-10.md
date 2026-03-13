# Rust Codebase Audit Remediation Plan

Date: 2026-03-10
Status: New plan created from `docs/reports/rust_codebase_audit_report_2026-03-10.md`
Scope: Workspace-wide remediation plan for audit findings, coding-standard alignment, and architecture cleanup

## 1. Purpose

This plan converts the findings in `docs/reports/rust_codebase_audit_report_2026-03-10.md` into a concrete remediation sequence.

The goals are:

1. Restore compliance with the repository's stated coding standards and quality gates.
2. Eliminate correctness drift between code, docs, and runtime behavior.
3. Reduce architectural duplication and transitional compatibility code.
4. Finish or explicitly re-scope partial migrations so the repo has one clear story for each runtime contract.
5. Make future work cheaper by reducing oversized modules, duplicated scheduling, and split ownership boundaries.

This plan is intentionally detailed. It is meant to be execution-ready, not just directional.

## 2. Guiding Constraints

All remediation work under this plan must continue to respect the repository rules in `AGENTS.md`, especially:

1. Server authority remains one-way: client input -> shard sim -> replication/distribution -> persistence.
2. Clients do not authoritatively own transforms or gameplay state.
3. Shared simulation/prediction/runtime logic must remain in shared crates or shared client code where possible.
4. Native client stabilization remains the delivery priority, but remediation must not make later WASM parity harder.
5. Critical runtime behavior or contract changes require same-change doc updates.
6. The minimum quality gates remain:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo check --workspace`
   - plus client WASM and Windows checks when client code changes

## 3. Remediation Strategy

The work should not be attempted as one giant PR. The audit findings cluster into a small number of coherent tracks:

1. Quality-gate restoration
2. Shared runtime constant and contract correction
3. Client bootstrap and plugin graph decomposition
4. Shared scripting/world-init extraction
5. Rendering/fullscreen migration completion
6. Asset cache contract resolution
7. Residual cleanup and consistency follow-through

The correct sequence is important:

1. Restore Clippy compliance first so the workspace is back under control.
2. Fix shared correctness drift next, especially the simulation tick constant.
3. Remove duplication and mixed ownership only after the workspace is compiling cleanly.
4. Finish migrations and doc alignment after architecture boundaries are clearer.

Trying to do this out of order will make regressions harder to isolate.

## 4. Workstream A: Restore Quality-Gate Compliance

### Objective

Bring the workspace back into compliance with `cargo clippy --workspace --all-targets -- -D warnings` without papering over problems with broad `allow` attributes.

### Why this is first

The repo explicitly claims this gate is mandatory. Until it passes again, the codebase is out of alignment with its own coding standard. Also, the active Clippy failures already point to the worst complexity hotspots, so this work directly reduces review friction for the remaining tracks.

### Targeted failures to resolve

#### A1. Dead code in replication scripting

Current issue:

- `bins/sidereal-replication/src/replication/scripting.rs:1005`
  - `load_world_init_config` is flagged as unused.

Action:

1. Confirm whether the helper is truly dead or only indirectly intended for future use.
2. If dead, delete it.
3. If needed by gateway/replication shared extraction, move it into the shared module introduced in Workstream D and consume that shared implementation from call sites.
4. Do not leave it exported-but-unused in the replication module.

Definition of done:

- No dead-code warning remains for this function.
- The resulting ownership model is clearer than before.

#### A2. Too-many-arguments in replication input

Current issue:

- `bins/sidereal-replication/src/replication/input.rs:373`

Action:

1. Inspect `drain_native_player_inputs_to_action_queue`.
2. Separate pure data normalization from ECS-facing orchestration.
3. Introduce a `SystemParam` or helper struct for repeated query/resource bundles.
4. If there is an obvious domain split, split one large system into two adjacent ordered systems.
5. Keep the authenticated player binding and anti-spoofing behavior unchanged.

Definition of done:

- The function falls below Clippy's argument threshold.
- Input handling behavior is unchanged.
- Tests cover accepted, dropped, stale, and mismatched control paths if not already present.

#### A3. Type-complexity in replication runtime state

Current issue:

- `bins/sidereal-replication/src/replication/runtime_state.rs:32`

Action:

1. Wrap complex query signatures in named type aliases or `SystemParam` wrappers where they improve readability.
2. Prefer names that describe ownership or domain meaning, not just type mechanics.
3. If the query is doing more than one conceptual job, split the system.

Definition of done:

- The query signature is understandable without scanning a giant generic block.
- Clippy warning is removed.

#### A4. Needless option deref in visibility

Current issues:

- `bins/sidereal-replication/src/replication/visibility.rs:562`
- `bins/sidereal-replication/src/replication/visibility.rs:1609`

Action:

1. Apply the direct simplification Clippy is asking for.
2. Re-run targeted visibility tests.
3. Confirm behavior is unchanged around faction visibility and public visibility handling.

Definition of done:

- The warnings are gone.
- Visibility logic remains policy-correct.

#### A5. Client backdrop complexity

Current issues:

- `bins/sidereal-client/src/runtime/backdrop.rs:502`
- `bins/sidereal-client/src/runtime/backdrop.rs:580`
- `bins/sidereal-client/src/runtime/backdrop.rs:1904`

Action:

1. Introduce helper structs for material asset collections and fullscreen renderable sync context.
2. Split fullscreen material attachment from fullscreen entity/render-layer selection.
3. Narrow any long async future type through a named boxed future alias.
4. Avoid introducing additional branching complexity while shrinking signatures.

Definition of done:

- Each helper or system has one clear responsibility.
- Clippy warnings are removed.
- Fullscreen layer sync behavior remains intact.

#### A6. Client replication complexity

Current issues:

- `bins/sidereal-client/src/runtime/replication.rs:69`
- `bins/sidereal-client/src/runtime/replication.rs:178`
- `bins/sidereal-client/src/runtime/replication.rs:860`
- `bins/sidereal-client/src/runtime/replication.rs:869`

Action:

1. Introduce domain-named query aliases or `SystemParam` wrappers for:
   - replicated-entity transform repair
   - hierarchy adoption/sync
   - controlled-tag synchronization
2. Split `sync_controlled_entity_tags_system` if it is doing both state derivation and mutation.
3. Preserve the ordering around prediction markers, interpolation markers, and controlled-tag ownership.

Definition of done:

- Clippy passes.
- Replication adoption, interpolation, and controlled-entity tagging behavior are unchanged.

#### A7. Client UI complexity

Current issues:

- `bins/sidereal-client/src/runtime/ui.rs:158`
- `bins/sidereal-client/src/runtime/ui.rs:166`

Action:

1. Reduce the argument count of `update_debug_overlay_text_ui_system`.
2. Replace the large `ParamSet` if possible with smaller domain-specific text-update helpers.
3. Keep the system's visible UI behavior unchanged.

Definition of done:

- Clippy passes.
- Debug overlay text still updates correctly in both normal and debug-heavy sessions.

#### A8. Client visuals complexity

Current issues:

- `bins/sidereal-client/src/runtime/visuals.rs:1833`
- `bins/sidereal-client/src/runtime/visuals.rs:2589`

Action:

1. Wrap the plume-attachment state in a narrower context object.
2. Alias or restructure the complex spark query.
3. If needed, split attachment/maintenance/cleanup into separate systems.
4. Be careful not to introduce frame-order regressions in the visual pipeline.

Definition of done:

- Clippy passes.
- Existing visual behavior and tests remain stable.

### Validation for Workstream A

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Also run targeted tests for each touched crate/module while fixing warnings, not only at the end.

### Deliverables

1. Code changes only.
2. No new architecture docs required unless the refactor materially changes ownership boundaries.
3. If ownership boundaries do change, update relevant docs in the same change.

## 5. Workstream B: Fix Shared Runtime Constant and Correctness Drift

### Objective

Eliminate the mismatch between the documented/runtime simulation rate and the shared core constant.

### Problems being fixed

1. `crates/sidereal-core/src/lib.rs:11` still defines `SIM_TICK_HZ = 30`.
2. `docs/sidereal_design_document.md:204` defines fixed simulation tick as 60 Hz.
3. Client and replication both explicitly insert `Time::<Fixed>::from_hz(60.0)`.
4. `crates/sidereal-core/tests/id_helpers.rs:19` institutionalizes the stale value.

### Actions

1. Update `SIM_TICK_HZ` from 30 to 60.
2. Update tests accordingly.
3. Replace direct literal `60.0` uses in runtime setup with derivation from the shared constant where practical.
4. Search for any other stale assumptions around tick windows, timeouts, interpolation settings, or UI text that implicitly assumes 30 Hz.
5. Verify Lightyear server/client plugin tick durations remain consistent with the shared constant.

### Additional review required

Because this value may affect behavior indirectly, audit the following after the constant change:

1. Input buffering windows
2. Time-based logging and cooldown assumptions
3. Interpolation or prediction tuning derived from fixed-step duration
4. Any tests with hardcoded tick counts that assumed 30 Hz semantics

### Validation

Run:

```bash
cargo test -p sidereal-core
cargo check --workspace
```

If any prediction or simulation tuning becomes visibly wrong after unifying constants, adjust tuning explicitly rather than reintroducing drift.

### Deliverables

1. Code updates in `sidereal-core` and any runtime literal users.
2. If docs mention historical drift or old values elsewhere, update them in the same change.

## 6. Workstream C: Decompose Client Bootstrap and Plugin Scheduling

### Objective

Make client runtime ownership legible and reduce scheduling duplication without changing the server-authoritative model or regressing native/WASM sharing.

### Problems being fixed

1. `bins/sidereal-client/src/runtime/mod.rs` centralizes too many resource insertions and runtime concerns.
2. `bins/sidereal-client/src/runtime/plugins.rs` duplicates large headless/non-headless system chains.
3. The current layout makes lifecycle resets, bootstrap ordering, and ownership boundaries harder to reason about.

### Design intent

The client should read like a composition of domain plugins with explicit ownership:

1. transport/auth
2. asset bootstrap/cache/catalog sync
3. replication adoption/state sync
4. prediction and local control
5. scene/render/fullscreen
6. UI/tactical/diagnostics

The app entrypoint should configure cross-cutting runtime primitives and then hand off to domain plugins.

2026-03-13 status note:
The shared client runtime extraction is now partially landed. The remaining shared app wiring that had still been sitting in `bins/sidereal-client/src/runtime/mod.rs` has been split into `bins/sidereal-client/src/runtime/app_setup.rs` and `bins/sidereal-client/src/runtime/app_builder.rs`, while native-only startup remains under `bins/sidereal-client/src/platform/native/entry.rs`.

2026-03-13 follow-up note:
The shared plugin composition has also been split by domain under `bins/sidereal-client/src/runtime/plugins/` (`bootstrap_plugins.rs`, `replication_plugins.rs`, `presentation_plugins.rs`, `ui_plugins/`). `bins/sidereal-client/src/runtime/plugins.rs` is now a thin re-export layer instead of the direct implementation for every client plugin, and the `ui_plugins` domain has been split again into focused submodules for menu/loading, in-world UI, post-update/debug render wiring, and logout flow.

### Proposed refactor structure

#### C1. Split resource initialization by domain

Create focused initialization helpers or small plugins such as:

1. `init_transport_resources(app)`
2. `init_asset_runtime_resources(app)`
3. `init_control_and_prediction_resources(app)`
4. `init_debug_and_diagnostics_resources(app)`
5. `init_tactical_resources(app)`
6. `init_scene_and_render_resources(app)`

These can live in the existing module layout initially if creating new files would add too much churn, but the ownership split should be visible in the code.

#### C2. Collapse duplicated replication chains

Refactor `ClientReplicationPlugin` so that:

1. shared replication-update chains are built once,
2. headless/non-headless mode only adds the truly different systems,
3. state-transition systems such as `transition_world_loading_to_in_world` remain gated to the non-headless path,
4. shared ordering around adoption, transform sync, control sync, asset manifest/catalog updates, and tactical snapshot handling remains identical.

#### C3. Narrow app-entrypoint responsibility

The client entrypoint should keep:

1. top-level Bevy/Lightyear/Avian plugin composition,
2. fixed-step insertion,
3. transport/cache adapter injection,
4. platform-specific wiring.

It should not own a giant list of unrelated domain resources long-term.

#### C4. Preserve WASM boundary discipline

Any refactor here must avoid reintroducing a native-vs-WASM fork. Shared runtime logic should remain shared; only HTTP/cache/window/transport adapters should differ by target.

### Recommended execution order

1. Extract resource init helpers first.
2. Then collapse duplicated plugin chains.
3. Then move any remaining large tuples into named helper functions.
4. Only after that consider file-level module splits if the code is still hard to review.

### Testing and validation

Run:

```bash
cargo test -p sidereal-client
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

Also manually confirm:

1. native auth -> character select -> world loading -> in-world flow still works
2. headless transport mode still boots cleanly
3. session-ready timeout and disconnect handling still function

### Deliverables

1. Client code refactor
2. Doc note if bootstrap/plugin ownership changes materially
3. No behavior drift in app-state flow

## 7. Workstream D: Extract Shared World-Init and Script Validation Logic

### Objective

Eliminate duplicated authoritative bootstrap logic between gateway and replication.

### Problems being fixed

Gateway and replication both define or duplicate:

1. graph entity record decoding
2. runtime render graph validation
3. `WorldInitScriptConfig`
4. script-root resolution
5. world-init catalog loading helpers

This duplication already appears in docs as known debt.

### Target destination

The most likely correct home is `crates/sidereal-scripting`, assuming dependencies remain clean. If `sidereal-scripting` would pick up invalid service-specific dependencies, introduce a smaller shared helper module/crate instead.

### Proposed extraction layers

#### D1. Pure shared helpers

Move into shared code:

1. `WorldInitScriptConfig`
2. graph-record decode helpers
3. runtime render-layer validation helpers
4. script-root/path resolution helpers if they are truly shared and not service-specific

These should return shared neutral errors, not gateway-specific `AuthError`.

#### D2. Service-specific translation layers

Keep local to each binary:

1. gateway-specific auth/bootstrap error translation
2. replication-specific operational/logging context
3. service-specific orchestration and DB/disk selection logic if behavior differs

### Implementation sequence

1. Identify the smallest common subset.
2. Move that subset first without changing behavior.
3. Update gateway to consume the shared implementation.
4. Update replication to consume the same implementation.
5. Remove duplicated local copies.
6. Add or update tests in the shared location to cover schema decoding and validation.

### Key risk

Do not accidentally create a shared crate dependency cycle or pull gateway auth concerns into scripting. The extracted layer must stay data/schema/validation focused.

### Validation

Run:

```bash
cargo test -p sidereal-scripting
cargo test -p sidereal-gateway
cargo test -p sidereal-replication
```

### Doc updates

Update:

1. `docs/features/scripting_support.md`
2. Any related plan or status notes that currently describe this duplication as still active

Add a dated note rather than silently overwriting previous context.

## 8. Workstream E: Finish the Fullscreen / Runtime Render-Layer Migration

### Objective

Resolve the current ambiguous state where Lua-authored runtime render layers exist, but the client still carries legacy `FullscreenLayer` fallback behavior and the docs still describe migration as pending.

### Problems being fixed

1. `data/scripts/world/world_init.lua` already authors fullscreen runtime render layers.
2. `bins/sidereal-client/src/runtime/backdrop.rs` still falls back to legacy `FullscreenLayer`.
3. `docs/features/scripting_support.md` still describes fullscreen bootstrap migration as pending.

### Decision required

Before implementing this workstream, make one explicit decision:

1. **Preferred path:** legacy fullscreen compatibility is no longer needed and should be removed.
2. **Fallback path:** legacy compatibility is still needed for a specific runtime/bootstrap path and must remain temporarily.

The plan should assume the preferred path unless new evidence says otherwise.

### Preferred-path actions

1. Audit all remaining producers of `FullscreenLayer`.
2. Confirm active runtime paths can rely exclusively on `RuntimeRenderLayerDefinition`.
3. Remove the fallback branch in `resolve_fullscreen_layer_selection`.
4. Delete obsolete legacy helpers/components/tests if they no longer serve active runtime paths.
5. Update docs to state that fullscreen rendering is now driven by runtime render-layer definitions authored through Lua/bootstrap data.

### Fallback-path actions

If compatibility must remain:

1. Document the exact surviving producer/consumer path.
2. Add a dated note explaining why the compatibility path still exists.
3. Add an explicit removal condition so it does not linger indefinitely.

### Validation

Run:

```bash
cargo test -p sidereal-client backdrop render_layers
cargo test -p sidereal-replication scripting
```

Manual validation:

1. fullscreen background layers still appear in-world
2. authored render-layer ordering still works
3. no legacy-only world bootstrap path remains hidden

### Doc updates

Update:

1. `docs/features/scripting_support.md`
2. Any design/decision doc sections that imply the migration is still pending if it is now complete

## 9. Workstream F: Resolve the Asset Cache Contract Mismatch

### Objective

Bring the code and docs back into agreement on what the client asset cache actually is.

### Problems being fixed

1. Docs describe an MMO-style cache centered around `assets.pak` and `assets.index`.
2. Current implementation uses loose files by content type plus `index.json`.
3. The repo should not keep both as simultaneously active truths.

### Required first decision

This workstream starts with an explicit product/engineering decision:

1. **Implement the documented pak/index cache**
2. **Revise the docs to bless the current loose-file cache as the intended near-term design**

The correct choice depends on whether pak/index is still considered an active near-term requirement or an outdated aspirational contract.

### Option F1: Implement pak/index

Use this path if the documented cache contract is still the intended product direction.

Actions:

1. Design the on-disk transactional model:
   - pack file layout
   - index schema
   - temp file strategy
   - integrity verification flow
2. Update `sidereal-asset-runtime` to read/write pack-backed entries rather than loose files.
3. Update native client asset bootstrap and materialization paths.
4. Ensure browser/WASM storage abstraction still exposes the same logical cache contract even if browser storage does not present literal files.
5. Add migration/reset handling for local dev caches if necessary.

Risks:

1. Larger implementation cost
2. More interaction with runtime asset attach paths
3. Higher chance of introducing asset bootstrap regressions

### Option F2: Revise docs to the loose-file cache

Use this path if loose-file cache is acceptable for the current project stage.

Actions:

1. Rewrite `docs/features/asset_delivery_contract.md` to describe the actual cache structure:
   - `index.json`
   - generated relative cache paths
   - per-content-type loose files
   - checksum validation behavior
2. Explain the intended long-term evolution if pak/index is still a future optimization rather than a current contract.
3. Make sure browser/WASM notes still describe byte-backed mounting and no filesystem-path dependency on the web.

Risks:

1. This reduces architectural tension, but only if the current design is genuinely acceptable.
2. If pak/index is still expected soon, this may just defer the real work.

### Recommendation

Unless there is a strong near-term need for pak/index, the pragmatic path is likely:

1. Document the current loose-file cache as canonical for now.
2. Explicitly mark pak/index as a future optimization if still desired.

This avoids large churn while restoring code/doc alignment.

### Validation

For either path:

```bash
cargo test -p sidereal-asset-runtime
cargo test -p sidereal-client
```

Manual validation:

1. bootstrap manifest fetch succeeds
2. required assets are cached correctly
3. cache hits skip redundant downloads
4. runtime asset materialization still works for shaders and textures
5. WASM cache adapter still compiles and behaves consistently

### Doc updates

At minimum update:

1. `docs/features/asset_delivery_contract.md`

Potentially also:

1. `AGENTS.md` if contributor-facing enforcement language changes materially

## 10. Workstream G: Continue De-Hardcoding Render Content Ownership

### Objective

Reduce the amount of content-specific shader/material routing still embedded in Rust.

### Problems being fixed

1. `bins/sidereal-client/src/runtime/shaders.rs` still names current content-specific shader slots and handles.
2. Runtime rendering still knows about concrete concepts such as starfield, asteroid sprite, and tactical map overlay in hardcoded Rust routing.

### Scope caution

This workstream is important, but it should not block the more urgent quality-gate, duplication, and docs-alignment work. It is a medium-priority cleanup with architectural value, not the first fire.

### Actions

1. Inventory all remaining content-specific shader/material routing.
2. Group each case into:
   - runtime-family behavior that belongs in Rust
   - concrete content identity that should move to data
3. Move concrete content selection behind authored render-layer/catalog metadata where feasible.
4. Retain only generic family/domain fallback logic in Rust.
5. Keep platform fallback shaders isolated from content ownership if possible.

### Validation

1. Fullscreen shaders still load and render.
2. Sprite/planet/tactical materials still bind correctly.
3. No required content is silently depending on deleted hardcoded IDs.

### Deliverables

1. Client render/runtime code cleanup
2. Potential doc updates to the render-layer/shader pipeline docs if the ownership boundary changes materially

## 11. Workstream H: Residual Cleanup and Consistency Follow-Through

### Objective

Clean up small but persistent evidence of transitional state after the major workstreams are complete.

### Items to review

1. `bins/sidereal-client/src/runtime/visuals.rs:323`
   - TODO for symmetric fade-out behavior
2. `bins/sidereal-client/src/runtime/pause_menu.rs:1`
   - explicit placeholder note
3. `bins/sidereal-client/src/runtime/render_layers.rs:672`
4. `bins/sidereal-client/src/runtime/render_layers.rs:675`
   - temporary test/scaffold naming that may or may not still be appropriate

### Actions

1. Triage each item:
   - implement now
   - keep and document
   - delete
2. Prefer deletion where code serves no active purpose.
3. If an item remains, ensure it has a real owner and a removal condition.

### Deliverables

1. Small cleanup changes
2. No open-ended “temporary” markers left without justification

## 12. Recommended Execution Breakdown

This is the recommended PR/task sequence.

### PR 1: Clippy compliance restoration

Scope:

1. Workstream A only

Why first:

1. Restores the repo's baseline discipline
2. Shrinks future diff noise
3. Makes later refactors easier to review

### PR 2: Shared tick constant correction

Scope:

1. Workstream B

Why separate:

1. Small correctness-focused change
2. Easy to validate
3. Avoids burying a fundamental constant fix inside a larger refactor

### PR 3: Client bootstrap/plugin decomposition

Scope:

1. Workstream C

Why separate:

1. High-churn client refactor
2. Needs focused review on scheduling and lifecycle behavior

### PR 4: Shared scripting/world-init extraction

Scope:

1. Workstream D

Why separate:

1. Cross-binary ownership refactor
2. Likely touches gateway, replication, and scripting crates together

### PR 5: Fullscreen migration completion

Scope:

1. Workstream E

Why separate:

1. Mixed code + docs + migration cleanup
2. Easier to review once shared scripting/bootstrap logic is cleaner

### PR 6: Asset cache contract resolution

Scope:

1. Workstream F

Why separate:

1. Either a major code change or a major docs contract correction
2. Needs a conscious decision before execution

### PR 7: Render de-hardcoding and residual cleanup

Scope:

1. Workstreams G and H

Why last:

1. Lower urgency
2. Better done after the higher-risk ownership work settles

## 13. Validation Matrix

Each workstream should run a tailored subset of checks, but the final completion bar for the full plan is:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

In addition, run targeted tests per touched area:

```bash
cargo test -p sidereal-core
cargo test -p sidereal-scripting
cargo test -p sidereal-asset-runtime
cargo test -p sidereal-gateway
cargo test -p sidereal-replication
cargo test -p sidereal-client
```

Where full crate test runs are too slow during iteration, narrower targeted tests are acceptable during development, but full touched-crate validation should be done before merging each workstream.

## 14. Documentation Update Checklist

When executing this plan, update docs in the same change whenever a contract or migration state changes.

Most likely docs requiring updates:

1. `docs/features/scripting_support.md`
2. `docs/features/asset_delivery_contract.md`
3. `docs/sidereal_design_document.md`
4. `AGENTS.md` if contributor-facing enforcement expectations change
5. Related plans/reports that should carry a dated status note

Documentation rules to follow:

1. Add dated notes using `YYYY-MM-DD`.
2. Append new dated status context rather than silently rewriting historical notes unless the old text is directly wrong and being superseded.
3. Do not leave a migration described as “pending” if code has already crossed that line.

## 15. Recommended Ownership and Review Focus

For each workstream, reviewers should focus on different risks:

### Workstream A review focus

1. No behavior regressions hidden inside “cleanup”
2. No broad `allow(clippy::...)` escape hatches without a strong reason
3. Simpler ownership, not just shorter signatures

### Workstream B review focus

1. Consistent tick rate across client, replication, tests, and docs
2. No silent tuning regressions

### Workstream C review focus

1. App-state flow correctness
2. Prediction/replication scheduling order
3. Native/WASM shared-boundary discipline

### Workstream D review focus

1. No crate dependency inversion
2. Correct ownership of shared vs service-specific logic
3. Test coverage for shared schema/validation helpers

### Workstream E review focus

1. Whether legacy fullscreen compatibility is truly removable
2. Code/doc alignment on migration state

### Workstream F review focus

1. One canonical cache contract
2. Practicality of implementation vs documentation truthfulness

### Workstream G/H review focus

1. Real de-hardcoding progress
2. Deletion of low-value transitional code

## 16. Success Criteria

This remediation plan is complete when all of the following are true:

1. The workspace passes the mandatory quality gates again.
2. `SIM_TICK_HZ` and the active runtime/docs agree on 60 Hz.
3. Client bootstrap and scheduling ownership is materially clearer and less duplicated.
4. Gateway and replication no longer maintain duplicate world-init/script validation logic.
5. Fullscreen/render-layer migration status is unambiguous in both code and docs.
6. Asset cache contract is described the same way in code and docs.
7. Low-value placeholders and stale transitional paths have been either finished, documented precisely, or deleted.

## 17. 2026-03-10 Status Note

This plan was created immediately after the 2026-03-10 codebase audit. At plan creation time:

1. `cargo clippy --workspace --all-targets -- -D warnings` was failing in active client and replication code.
2. Shared core still declared `SIM_TICK_HZ = 30` while runtime/docs used 60 Hz.
3. Lua-authored fullscreen runtime render layers already existed, but the client still retained legacy fallback logic and docs still described migration as pending.
4. Asset cache implementation and asset-cache docs were not describing the same canonical contract.

This note should remain as baseline context; future updates should append dated execution status rather than replacing it silently.
