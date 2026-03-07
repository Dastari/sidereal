# Shader Editor/Preview Dashboard Implementation Spec

Status: Active partial implementation spec  
Date: 2026-03-05  
Owners: dashboard + rendering + runtime toolchain

Primary references:
- `docs/sidereal_design_document.md`
- `docs/features/asset_delivery_contract.md`
- `docs/features/shader_editor_wgsl_linting_and_diagnostics_plan.md`
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

## 1.1 Current Implementation State

Implemented today:

1. Route-backed shader workshop in `/shader-workshop`.
2. Dashboard-shell layout with:
   - left shader library tree,
   - center code editor + preview split,
   - right metadata/performance/uniform inspector.
3. Server APIs for shader list/load/upload.
4. Rust/WASM preview bridge crate and wasm build integration.
5. Browser WebGPU preview renderer with:
   - fullscreen triangle rendering,
   - float/vector uniforms,
   - struct uniform flattening for float/vector fields,
   - generated texture/sampler resources for common preview bindings.
6. Bottom-pane diagnostics under the preview canvas.
7. Simulation-speed driven advancement for time-like shader uniforms.
8. Shader dependency/role metadata merged from the Lua asset registry.

Not yet implemented:

1. Syntax-highlighted WGSL editor.
2. Full Bevy visual preview scene as the primary visible renderer.
3. Live runtime apply to a connected world/client.
4. Draft persistence/version history.
5. Full preview asset import workflow.

## 2. Scope

### 2.1 In scope (current implementation target)

1. Dashboard route: `shader-workshop`.
2. Server APIs to list/load shader sources and metadata.
3. WGSL editor surface with diagnostics pane.
4. Rust/WASM validation bridge plus browser WebGPU preview embedded in dashboard canvas.
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

Route: `/shader-workshop`

Panels:

1. Shader Library Panel
   - filter by class (`fullscreen`, `sprite`, `effect`, `post`).
   - search by ID/path/tags.
2. Code Editor Panel
   - current implementation: plain textarea editor.
   - target: Monaco or CodeMirror WGSL mode.
3. Uniform Inspector Panel
   - dynamic controls generated from schema.
4. Preview Canvas Panel
   - current implementation: browser WebGPU renderer over a fullscreen triangle.
   - current diagnostics pane lives directly below preview in the center stack.
   - target: hosted Bevy WASM runtime plugin / richer scene presets.
5. Revision/Actions Panel
   - current implementation: validate/apply/save.
   - target: draft/version/promotion flow.

## 4.2 Backend (Dashboard Server)

Current APIs:

1. `GET /api/shaders`
   - list all shader records discoverable on server.
2. `GET /api/shaders/:shaderId`
   - return source, metadata, schema, entry points.
3. `POST /api/shaders/upload`
   - upload/update canonical shader source and streamed cache counterpart.

Still pending:

1. `POST /api/shaders/:shaderId/validate`
2. `POST /api/shaders/:shaderId/preview-apply`
3. `POST /api/shaders/:shaderId/save-draft`

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

Current implementation note:

1. The Rust/WASM module exists primarily as a validation/apply bridge today.
2. The visible preview canvas is still driven by the browser WebGPU renderer.
3. Deterministic time control is partially implemented through simulation-speed advancement of matching uniforms.

## 5. Editor Requirements

## 5.1 Code Editor

Mandatory features:

1. Syntax highlight for WGSL.
2. Line/column diagnostics.
3. Format command (WGSL formatter service).
4. WGSL diagnostics architecture should follow `docs/features/shader_editor_wgsl_linting_and_diagnostics_plan.md`:
   - debounced text-in/diagnostics-out flow,
   - validator-backed syntax/semantic diagnostics,
   - project-specific Sidereal shader rules layered on top.
5. Keyboard shortcuts:
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
11. `dependencies` (resolved from the Lua asset registry `data/scripts/assets/registry.lua` for shader-linked asset IDs)

Current implementation note:

1. `shader_id`, `shader_class`, `source_path`, `cache_path`, dependency metadata, shader role, and bootstrap-required metadata are already surfaced.
2. `uniform_schema` is currently inferred from WGSL uniform declarations at preview time, not persisted as a canonical backend schema record yet.

Initial population:

1. Enumerate server-side shader assets from canonical source directory.
2. Merge with Lua asset registry metadata when available so declared texture/LUT dependencies and shader roles are visible in the workbench.
3. Merge with known metadata records when available.

## 7. Live Preview Flow

1. User edits WGSL.
2. Frontend derives preview uniforms from the WGSL.
3. Frontend calls Rust/WASM validation/apply bridge.
4. Frontend compiles/renders via browser WebGPU preview renderer.
5. Preview rerenders as uniform controls and simulation time advance.
6. Errors (validation/compile/bind/runtime) stream into the bottom diagnostics panel.

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

Current status:

1. `sidereal-shader-preview` unit tests cover valid WGSL, invalid apply behavior, and Bevy-import normalization.
2. Dashboard build/test currently validate the route and preview bundle build.
3. Full route-level integration coverage for preview controls and renderer behavior remains outstanding.

Minimum checks before merge:

1. `cargo check --workspace`
2. `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
3. `pnpm -C dashboard build`
4. `pnpm -C dashboard lint` (or targeted lint when repo-wide baseline is still being cleaned)

## 11. Delivery Phases

1. Phase A: Shader catalog + read/load APIs + static editor shell.
   Current status: complete.
2. Phase B: Validation pipeline + diagnostics UX.
   Current status: partial.
3. Phase C: Bevy WASM preview plugin integration.
   Current status: partial bridge implemented; full visible Bevy preview still pending.
4. Phase D: Schema-driven uniform inspector (full shadcn mapping).
   Current status: partial via inferred float/vector/struct uniform controls.
5. Phase E: Draft persistence/versioning.
   Current status: not started.
6. Phase F: Promotion pipeline + shader library (future).
   Current status: not started.
7. Phase G: Live in-game update + Lua integration (future).
   Current status: not started.

## 12. Open Decisions

1. Draft persistence backend: Postgres vs file-backed store.
2. Editor choice for WGSL syntax highlighting/intelligence: CodeMirror + WGSL mode/LSP vs Monaco-based integration.
3. Preview asset sourcing policy: when to switch from generated placeholder textures to explicit uploaded/selected preview assets.
4. Final publish governance (single approver vs multi-step review).
