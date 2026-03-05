# Shader Editor/Preview Dashboard Implementation Spec

Status: Proposed implementation spec  
Date: 2026-03-05  
Owners: dashboard + rendering + runtime toolchain

Primary references:
- `docs/sidereal_design_document.md`
- `docs/features/asset_delivery_contract.md`
- `docs/features/visibility_replication_contract.md`
- `docs/ui_design_guide.md`
- `AGENTS.md`

## 1. Objective

Deliver a production-ready shader editor + live preview workflow in `/dashboard` with:

1. Full WGSL code editing (syntax highlighting, formatting, diagnostics).
2. Live visual preview powered by a Bevy WASM preview runtime plugin.
3. Uniform/input parameter controls auto-rendered as shadcn inputs.
4. Server-backed shader listing/load for all known shaders (initial phase).
5. Clear upgrade path to live in-game shader updates and reusable shader library assets for gameplay + Lua.

## 2. Scope

### 2.1 In scope (current implementation target)

1. Dashboard route: `shader-workbench`.
2. Server APIs to list/load shader sources and metadata.
3. Monaco-based WGSL editor with format action + diagnostics pane.
4. Bevy WASM preview plugin embedded in dashboard canvas.
5. Uniform inspector with shadcn controls mapped by schema type.
6. Preview-only live apply loop (edit -> validate -> preview compile -> render).

### 2.2 Deferred (future phases)

1. In-game live apply to running native/WASM clients.
2. Shader publish/promote workflow with versioned library.
3. New shader creation workflow from templates.
4. Lua-facing shader registry and script-level shader assignment APIs.

## 3. Non-Negotiable Constraints

1. No architecture bypass of server-authoritative flow: dashboard never mutates game runtime assets directly without backend mediation.
2. Source/cache parity required whenever shader source changes:
   - `data/shaders/*.wgsl`
   - `data/cache_stream/shaders/*.wgsl`
3. Native/WASM parity: preview/runtime shader contracts must compile under both.
4. Platform branching rules remain unchanged (`cfg(target_arch = "wasm32")` only).
5. UI implementation must use design-system-consistent shadcn components.

## 4. System Architecture

## 4.1 Dashboard Frontend

Route: `/shader-workbench`

Panels:

1. Shader Library Panel
   - filter by class (`fullscreen`, `sprite`, `effect`, `post`).
   - search by ID/path/tags.
2. Code Editor Panel
   - Monaco WGSL language mode.
   - diagnostics gutter + problems list.
   - format action.
3. Uniform Inspector Panel
   - dynamic controls generated from schema.
4. Preview Canvas Panel
   - hosted Bevy WASM runtime plugin.
   - mode: fullscreen quad, sprite bench, effect bench.
5. Revision/Actions Panel
   - validate, preview-apply, save draft, copy JSON.

## 4.2 Backend (Dashboard Server)

Add APIs:

1. `GET /api/shaders`
   - list all shader records discoverable on server.
2. `GET /api/shaders/:shaderId`
   - return source, metadata, schema, entry points.
3. `POST /api/shaders/:shaderId/validate`
   - WGSL parse + binding schema lint.
4. `POST /api/shaders/:shaderId/preview-apply`
   - returns validated payload to preview runtime bridge.
5. `POST /api/shaders/:shaderId/save-draft`
   - persist draft record (DB/file-backed).

## 4.3 Bevy WASM Preview Plugin

Create a dedicated WASM preview target/plugin:

1. Receives shader source + uniform updates via typed message bridge.
2. Compiles shader using actual Bevy material path.
3. Renders preview scene in canvas.
4. Emits compile/runtime errors back to dashboard.

Plugin responsibilities:

1. Preview scene setup (camera, quad/sprite fixtures).
2. Dynamic material registration + shader hot-reload.
3. Uniform buffer updates from inspector state.
4. Deterministic time controls (play/pause/scrub).

## 5. Editor Requirements

## 5.1 Code Editor

Mandatory features:

1. Syntax highlight for WGSL.
2. Line/column diagnostics.
3. Format command (WGSL formatter service).
4. Keyboard shortcuts:
   - `Ctrl/Cmd+S`: save draft
   - `Ctrl/Cmd+Enter`: validate
   - `Ctrl/Cmd+Shift+Enter`: apply preview

## 5.2 Uniform Controls (shadcn mapping)

Schema-driven mapping:

1. `bool` -> `Switch`
2. `i32/u32 enum` -> `Select`
3. `f32` with range -> `Slider` + numeric `Input`
4. unconstrained scalar text/numeric -> `Input`
5. `vec2/vec3/vec4` numeric -> grouped `Input` or per-axis `Slider`
6. color (`rgb/rgba`) -> grouped channel controls (existing pattern) + optional swatch
7. multiline metadata text -> `Textarea`

Rule: every schema field must produce a concrete shadcn control type. No ad-hoc custom widgets unless documented.

## 6. Shader Registry and Metadata

Registry record fields:

1. `shader_id`
2. `shader_class`
3. `source_path`
4. `cache_path`
5. `entry_points`
6. `bind_layout_schema`
7. `uniform_schema`
8. `tags`
9. `version/hash`
10. `updated_at/updated_by`

Initial population:

1. Enumerate server-side shader assets from canonical source directory.
2. Merge with known metadata records when available.

## 7. Live Preview Flow

1. User edits WGSL.
2. Frontend calls `validate`.
3. If valid, frontend calls `preview-apply`.
4. Preview bridge pushes source + params into Bevy WASM plugin.
5. Plugin recompiles/rebinds material and re-renders.
6. Errors (compile/bind/runtime) stream back to diagnostics panel.

## 8. Future Live Game Integration

Phase extension (not in initial delivery):

1. Add controlled backend endpoint to push approved shader revisions to connected game runtimes.
2. Runtime receives asset delta via authoritative channel.
3. Runtime performs safe hot-reload with rollback on failure.
4. Persist promoted shaders into shader library records.
5. Expose shader library IDs for Lua data/scripts usage.

## 9. Security and Access

1. All shader mutation endpoints require authenticated admin/operator role.
2. Preview apply is sandboxed to dashboard preview runtime only.
3. Live runtime apply (future) must be explicitly gated and auditable.
4. Shader source edits are revisioned with actor identity + timestamp.

## 10. Testing and Quality Gates

Backend:

1. Unit tests for schema parsing and control mapping contracts.
2. API tests for list/load/validate/preview-apply.

Frontend:

1. Component tests for inspector mapping by schema type.
2. Integration tests for edit->validate->preview loop.

Runtime:

1. WASM preview compile smoke tests for supported shader classes.
2. Native/WASM parity checks for shared shader/material contracts.

Minimum checks before merge:

1. `cargo check --workspace`
2. `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
3. `pnpm -C dashboard build`
4. `pnpm -C dashboard lint` (or targeted lint when repo-wide baseline is still being cleaned)

## 11. Delivery Phases

1. Phase A: Shader catalog + read/load APIs + static editor shell.
2. Phase B: Validation pipeline + diagnostics UX.
3. Phase C: Bevy WASM preview plugin integration.
4. Phase D: Schema-driven uniform inspector (full shadcn mapping).
5. Phase E: Draft persistence/versioning.
6. Phase F: Promotion pipeline + shader library (future).
7. Phase G: Live in-game update + Lua integration (future).

## 12. Open Decisions

1. Draft persistence backend: Postgres vs file-backed store.
2. Formatter engine choice for WGSL (embedded vs service call).
3. Preview asset sourcing policy (allow uploads vs constrained built-ins).
4. Final publish governance (single approver vs multi-step review).
