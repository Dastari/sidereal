# Dashboard Shader Editor and Preview Workbench Plan

Status: Partially implemented  
Date: 2026-03-03  
Owners: dashboard + client rendering + asset streaming + replication

Primary references:
- `docs/sidereal_design_document.md`
- `docs/features/asset_delivery_contract.md`
- `docs/features/wasm_parity_implementation_plan.md`
- `docs/ui_design_guide.md`
- `AGENTS.md`

## 1. Objective

Build a Bevy-compatible shader authoring and preview tool inside `/dashboard` that enables:

1. Full shader source viewing/editing with syntax highlighting and code intelligence.
2. Hot-reload into running client render paths (sprite shaders, thruster shaders, fullscreen shaders).
3. Visual preview workbench for:
   - fullscreen materials (starfield, space background),
   - sprite/material shaders on test quads/sprites,
   - optional module preview via Bevy-WASM runtime.
4. Parameter editing via shadcn controls, with JSON copy/export workflow.
5. Asset-stream aware authoring flow so server/client runtime paths stay consistent.
6. Safety rails (validation, compile diagnostics, rollback, versioning) to avoid broken live shaders.

Target experience: Unity-style 2D shader authoring loop adapted for Sidereal’s architecture and Bevy WGSL pipeline.

## 1.1 Current Status Snapshot

As of 2026-03-07, the shader workbench is no longer just a plan. The following slices are implemented:

1. Dedicated dashboard route: `/shader-workshop` (with `/shader-workbench` redirected for compatibility).
2. Shared dashboard shell usage:
   - left sidebar: shader library tree
   - center: code editor + preview stack
   - right panel: metadata, performance, and uniform controls
3. Server-backed shader catalog/load/upload flow for `data/shaders/` with source/cache parity updates into `data/cache_stream/shaders/`.
4. Rust/WASM shader preview bridge crate: `crates/sidereal-shader-preview`.
5. Browser WebGPU preview renderer for:
   - fullscreen fragment shaders,
   - float/vector uniforms,
   - struct uniform blocks containing float/vector fields,
   - texture + sampler bindings with generated preview assets.
6. Derived uniform inspector controls in the dashboard.
7. Simulation-time advancement for uniforms that represent `time`, `age`, `life`, or `progress`.
8. Diagnostics pane embedded under the preview canvas.
9. Lua asset registry metadata merge for shader dependencies/roles from `data/scripts/assets/registry.lua`.

The following major pieces are still not implemented:

1. Proper syntax-highlighted WGSL editor; current editor is still a plain textarea.
2. True embedded Bevy scene/material preview; the browser preview path is still the main visible renderer.
3. Live apply into a connected game runtime.
4. Draft/version history and promotion workflow.
5. Real preview asset import workflow for arbitrary textures.
6. Deep shader-class preset libraries for sensible default fullscreen/sprite parameters.

## 2. Scope and Non-Goals

### 2.1 In scope (v1-v2)

1. Shader source editor in dashboard with:
   - code coloring,
   - line numbers,
   - search/replace,
   - diagnostics panel.
2. Runtime preview scene(s):
   - fullscreen preview canvas,
   - sprite preview canvas,
   - test texture/sprite imports.
3. Live update pipeline:
   - save draft,
   - compile/validate,
   - apply to preview,
   - push to staged runtime.
4. Preset and uniform controls (sliders/toggles/color pickers) using shadcn UI primitives.
5. Direct integration with current streamed shader file conventions.

### 2.2 Out of scope (v1)

1. Full node-graph shader authoring UI.
2. 3D PBR material graph tooling.
3. Automatic optimization/rewrite of user-authored WGSL.
4. Production final-publish workflow bypassing source control/PR process.

## 3. Design Principles

1. Bevy-first compatibility: authored shaders must align with Bevy material bind group layouts.
2. Server-authoritative content flow: dashboard edits are managed through backend-controlled APIs, not direct client mutation hacks.
3. Fast iteration loop: compile-feedback-preview in seconds.
4. Safe-by-default: syntax/contract checks before runtime apply.
5. Source/cache parity: shader source and streamed cache variants stay aligned.
6. Native/WASM parity: shader behavior previewed across target constraints.

## 4. User Personas and Workflows

### 4.1 VFX Artist / Technical Designer

1. Opens shader in dashboard.
2. Adjusts code and exposed parameters in split view.
3. Previews effect on:
   - sprite test target,
   - fullscreen target.
4. Copies JSON defaults to gameplay component constant when needed.
5. Saves versioned draft and shares.

### 4.2 Gameplay/Rendering Engineer

1. Inspects compile diagnostics and binding compatibility.
2. Validates hot reload against running client scene.
3. Promotes approved shader draft to streamable asset source.
4. Runs parity checks for native + wasm.

## 5. High-Level Architecture

## 5.1 Dashboard Frontend (React + shadcn)

Primary panels:

1. `ShaderLibraryPanel`
   - shader list/filter/tagging by class (`fullscreen`, `sprite`, `effect`).
2. `ShaderCodeEditorPanel`
   - Monaco (preferred) or CodeMirror with WGSL grammar highlighting.
3. `ShaderPreviewPanel`
   - Preview mode switch: `Sprite`, `Fullscreen`, `Thruster`, `Custom Scene`.
4. `UniformInspectorPanel`
   - shadcn slider/input/switch/color controls bound to preview uniforms.
5. `DiagnosticsPanel`
   - parser/validation messages, runtime compile errors, bind-group mismatch hints.
6. `AssetImportPanel`
   - upload/select sprite textures and masks for preview.
7. `RevisionPanel`
   - draft history, compare, rollback.

All chrome/components follow shadcn patterns for inputs, tabs, dialogs, sheets, and tables.

## 5.2 Dashboard Backend API Layer

Add server routes in dashboard backend for:

1. `GET /api/shaders`
   - list shader records and metadata.
2. `GET /api/shaders/:id`
   - source + metadata + known uniform schema.
3. `POST /api/shaders/:id/validate`
   - static WGSL checks + project-specific binding lint.
4. `POST /api/shaders/:id/preview-apply`
   - apply to preview runtime only.
5. `POST /api/shaders/:id/save-draft`
   - persist draft/version metadata.
6. `POST /api/shaders/:id/promote`
   - publish to canonical source + cache path update flow.
7. `POST /api/shaders/import-texture`
   - upload texture for preview sprites.

## 5.3 Runtime Preview Engine (Two options)

### Option A: Shared WebGPU Preview Renderer in Dashboard (implemented baseline)

1. Render preview with WGSL in dashboard canvas using WebGPU pipeline.
2. Simulate Bevy-style uniform bindings with explicit schemas.
3. Faster to integrate initially, no Bevy ECS startup overhead.

### Option B: Dedicated Bevy-WASM Preview Module (partially implemented target)

1. New workspace target dedicated to shader preview runtime.
2. Embedded as a module/canvas in dashboard.
3. Uses real Bevy material/shader compilation path.
4. Best fidelity for bind group/material compatibility.

Current reality:

1. Option A is the active preview renderer and user-visible path.
2. Option B exists today as a Rust/WASM validation/apply bridge, but not yet as the primary visible Bevy scene renderer.

## 6. Data and Contract Model

## 6.1 Shader Registry Entry

Each editable shader record includes:

1. `shader_id` (logical ID, e.g. `thruster_plume_wgsl`)
2. `shader_class` (`fullscreen`, `sprite`, `effect`)
3. `source_path` (canonical path under `data/shaders/`)
4. `streamed_cache_path` (target under `data/cache_stream/shaders/`)
5. `entry_points` (fragment/vertex labels as relevant)
6. `expected_bind_layout` (group/binding definitions)
7. `uniform_schema` (typed control metadata)
8. `dependencies` (textures/noise LUTs)
9. `version`, `hash`, `last_modified_by`, `last_modified_at`

## 6.2 Uniform Schema

Use a schema format that drives both:

1. dashboard controls,
2. runtime validation hints.

Schema field examples:

1. `name`
2. `type` (`f32`, `vec2`, `vec3`, `vec4`, `bool`, `color`)
3. `range`, `step`, `default`
4. `group`, `label`, `description`
5. `runtime_mapping` (optional link to gameplay component field)

## 6.3 Validation Contract

Validation layers:

1. WGSL parse/compile validation.
2. Binding layout validation against declared material schema.
3. Sidereal lint checks:
   - banned path usage,
   - unsupported target features,
   - mismatch between source/cache class contract.

## 7. Editor UX Requirements

## 7.1 Code Editor

1. Syntax highlighting for WGSL.
2. Line diagnostics with clickable error navigation.
3. Diff view against previous revision.
4. Formatting command (if stable WGSL formatter available).
5. Keyboard shortcuts:
   - save draft,
   - validate,
   - apply preview,
   - revert.

Implementation note:

1. Diagnostics are implemented.
2. Syntax highlighting and richer code intelligence remain outstanding.

## 7.2 Preview UX

Preview modes:

1. `Fullscreen`
   - fullscreen quad with camera/time controls.
2. `Sprite`
   - one or many sprites with transform controls.
3. `Thruster Bench`
   - ship-tail style flame harness with thrust/afterburner sliders.
4. `Split Compare`
   - A/B compare old/new shader.

Controls:

1. pause/play time,
2. scrub time,
3. FPS display,
4. camera zoom/pan for sprite mode,
5. background selector (solid, starfield, scene capture).

Current implementation note:

1. A simulation-speed control exists and auto-advances matching time-like uniforms.
2. Derived sliders/inputs exist for preview uniforms.
3. Diagnostics are displayed in a bottom split pane below the preview canvas.
4. Dedicated preview-mode switching, sprite benches, and background selection are still pending.

## 7.3 Asset Import for Testing

1. Drag/drop textures for preview-only sessions.
2. Persist selected test assets under dedicated preview cache namespace.
3. Include sprite slicing/alpha-channel preview.

## 8. Hot Reload Strategy

## 8.1 Preview-Only Hot Reload

1. User edits WGSL.
2. Dashboard backend validates.
3. Preview runtime recompiles shader module in-place.
4. Diagnostics shown immediately if compile fails.

## 8.2 Live Runtime Hot Reload (Controlled)

1. Optional “Apply to connected client scene” action.
2. Sends shader update command through controlled debug interface.
3. Client replaces shader asset handle content in memory.
4. If compile fails, rollback to prior known-good revision.

Guardrails:

1. gated behind debug/developer role.
2. never silently overwrites canonical files without explicit save/promote.

## 9. Fullscreen and Sprite Shader Support

For Sidereal parity, first-class support must include:

1. current fullscreen shaders:
   - starfield,
   - space background.
2. sprite/effect shaders:
   - sprite pixel shader,
   - thruster plume shader.
3. future lighting/post passes.

Workbench should allow previewing same shader code on multiple targets when relevant.

## 10. Bevy-WASM Preview Module Plan (Detailed)

## 10.1 Why add Bevy-WASM preview

1. Matches actual Bevy shader/material compiler path.
2. Reveals bind group mismatches early.
3. Reduces “works in custom preview but fails in Bevy pipeline” regressions.

## 10.2 Proposed module crate

1. Add a small preview crate/target:
   - `bins/sidereal-shader-preview` (or dedicated lib target in dashboard context).
2. Exposes JS bridge functions:
   - load shader source,
   - set uniform values,
   - set preview mode,
   - import texture bytes.
3. Renders into canvas controlled by dashboard panel.

## 10.3 Integration mode

1. Dashboard mounts canvas.
2. Sends messages over JS bridge.
3. Receives diagnostics/events over callback channel.

## 11. File and Asset Management Rules

1. Canonical authored shader source lives in `data/shaders/`.
2. Streamed runtime path stays under `data/cache_stream/shaders/`.
3. Promotion workflow must update parity and metadata together.
4. Drafts can be stored separately from canonical runtime shader files.
5. Asset IDs remain logical; code paths should not hardcode ad-hoc filesystem paths in gameplay components.

## 12. Security and Permissions

1. Editing/promotion endpoints require authenticated developer role.
2. Production promotion requires explicit confirmation and audit trail.
3. Keep `bevy_remote` or similar debug hooks auth-gated per project rules.
4. Add change logs with editor identity and timestamp.

## 13. Phased Implementation Plan

## Phase 0: Discovery and Contracts

1. Inventory current shader assets and material binding schemas.
2. Define shader registry model and uniform schema format.
3. Define draft storage strategy and promotion policy.

Deliverables:

1. contract doc updates,
2. API spec draft,
3. initial schema examples for existing shaders.

Status: substantially complete

## Phase 1: Dashboard Shader Library + Read-Only Viewer

1. Build shader list panel.
2. Add code viewer with highlighting and diagnostics placeholder.
3. Add metadata view (asset ID, paths, hash, class).

Deliverables:

1. list/filter/search UX,
2. shader metadata endpoints.

Status: complete

## Phase 2: Editable Code + Validation

1. Enable code editing and draft save.
2. Implement validate API and diagnostics rendering.
3. Add shadcn dialog for save/apply/rollback flows.

Deliverables:

1. compile + lint feedback loop,
2. draft persistence and revision list.

Status: partial

Implemented:

1. editable shader source
2. Rust/WASM validation bridge
3. browser WebGPU compile diagnostics
4. diagnostics UI

Remaining:

1. draft persistence
2. revision history
3. richer lint/schema validation endpoints

## Phase 3: Preview Workbench (WebGPU baseline)

1. Add fullscreen and sprite preview canvases.
2. Add uniform inspector and time controls.
3. Add texture import and assignment tools.

Deliverables:

1. real-time preview loop,
2. A/B compare mode.

Status: partial

Implemented:

1. WebGPU fullscreen preview path
2. uniform controls derived from WGSL
3. simulated time progression
4. generated preview textures/samplers for common shader resources

Remaining:

1. richer preview scene presets
2. A/B compare
3. texture import UI
4. better preset/default handling for complex fullscreen shaders

## Phase 4: Live Runtime Apply + Hot Reload

1. Controlled apply to connected client session.
2. Add rollback on runtime compile failure.
3. Add activity/audit logging.

Deliverables:

1. safe live-edit workflow for development sessions.

Status: not started

## Phase 5: Bevy-WASM Preview Module

1. Build dedicated Bevy-WASM preview runtime.
2. Integrate with dashboard canvas bridge.
3. Add parity test suite vs native runtime behavior.

Deliverables:

1. high-fidelity Bevy preview mode,
2. compatibility confidence before promotion.

Status: partial

Implemented:

1. `sidereal-shader-preview` Rust crate
2. wasm build pipeline into dashboard public assets
3. Rust/WASM validation/apply bridge
4. panic-hooked diagnostics and wasm-safe timing path

Remaining:

1. persistent Bevy-driven visual preview scene
2. actual Bevy material/bind-group rendering as the main preview mode
3. parity coverage beyond the current validation bridge

## 14. Testing and Quality Gates

For each phase:

1. Unit tests for schema parsing/validation.
2. Integration tests for API workflow (save/validate/promote).
3. UI tests for editor interactions and diagnostics rendering.
4. Shader smoke tests against known sample shaders.
5. Build checks:
   - dashboard build/lint/typecheck,
   - client native build,
   - wasm build with `bevy/webgpu`.

Required command set before completion:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo check --workspace`
4. `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
5. `cargo check -p sidereal-client --target x86_64-pc-windows-gnu`
6. `pnpm -C dashboard build`

## 15. Risks and Mitigations

1. Risk: bind group drift between editor preview and Bevy runtime.
   - Mitigation: schema contract + Bevy-WASM preview parity phase.
2. Risk: hot reload destabilizes runtime sessions.
   - Mitigation: preview-only first, guarded live apply, rollback.
3. Risk: stale cache/source mismatch.
   - Mitigation: parity checks on promote; hash validation.
4. Risk: dashboard complexity and UX overload.
   - Mitigation: progressive disclosure UI and phased release.

## 16. Open Decisions

1. Monaco vs CodeMirror as primary editor engine for WGSL ergonomics and bundle size.
2. Draft storage backend:
   - filesystem in repo,
   - DB-backed revisions,
   - hybrid.
3. Promotion policy:
   - direct file write on main dev env,
   - PR artifact generation workflow.
4. Bevy-WASM module packaging model and ownership.

## 17. Initial Backlog Checklist

1. Create `ShaderRegistry` definitions for existing shader assets.
2. Build `/dashboard` route: `shader-workshop`.
3. Implement shadcn layout shell with resizable panes.
4. Add WGSL editor with diagnostics gutter.
5. Implement validate endpoint and parser integration.
6. Add sprite/fullscreen preview canvases.
7. Add uniform inspector from schema.
8. Add draft save/version list/restore.
9. Add controlled live-apply command path.
10. Add documentation updates and contributor workflow notes.
