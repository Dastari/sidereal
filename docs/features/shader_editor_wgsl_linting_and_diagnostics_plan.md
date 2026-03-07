# Shader Editor WGSL Linting and Diagnostics Plan

Status: Proposed implementation plan  
Date: 2026-03-07  
Owners: dashboard + rendering + tooling

Primary references:
- `docs/features/shader_editor_dashboard_implementation_spec.md`
- `docs/features/shader_editor_dashboard_plan.md`
- `docs/features/asset_delivery_contract.md`
- `docs/ui_design_guide.md`
- `AGENTS.md`

## 1. Objective

Add practical WGSL linting and diagnostics to the dashboard shader editor in a way that fits a custom editor surface built on `use-editable`.

The goal is not to chase a nonexistent "WGSL ESLint ecosystem." The goal is to deliver:

1. fast syntax diagnostics,
2. real validator-backed semantic errors,
3. editor-visible ranges/underlines/messages,
4. a path for project-specific shader rules,
5. a clean upgrade path to richer language tooling later.

## 2. Summary

WGSL linting is very feasible for the dashboard shader editor, but the term "linting" needs to be scoped correctly.

What is mature today:

1. syntax and semantic diagnostics,
2. compiler-like validation,
3. language-server-backed editor intelligence,
4. parser/reflector-driven structural inspection.

What is not mature today:

1. a deep npm-style WGSL style-rule ecosystem comparable to ESLint,
2. a widely adopted pack of reusable named style rules for formatting and best-practice enforcement.

Practical conclusion:

1. syntax + semantic diagnostics: high confidence,
2. good custom-editor UX: medium effort but feasible,
3. reusable off-the-shelf style-rule packs: limited,
4. project-specific lint rules layered on top of real validation: the right approach for Sidereal.

## 3. Design Constraints

1. The dashboard editor is not a full Monaco/CodeMirror/LSP IDE by default.
2. `use-editable` gives us an editable surface, not built-in compiler/language tooling.
3. Diagnostics must therefore follow a "text in, diagnostics out" architecture.
4. The same shader diagnostics model should be reusable for:
   - local editor warnings,
   - preview validation failures,
   - backend validation endpoints,
   - future live-apply safety checks.
5. Diagnostics must remain understandable to non-engineers using the shader workshop.

## 4. What "Linting" Means Here

For Sidereal, WGSL linting should be defined as four layers:

## 4.1 Layer A: Parse diagnostics

Detect:

1. invalid WGSL syntax,
2. malformed attributes,
3. invalid declarations,
4. parse-time range/structure errors.

## 4.2 Layer B: Semantic/validator diagnostics

Detect:

1. undeclared identifiers,
2. type mismatches,
3. invalid entry points,
4. bind-group/layout mismatches where the validator can prove them,
5. invalid address-space usage,
6. uniformity-related diagnostics where supported by WGSL tooling.

## 4.3 Layer C: Structural/project rules

Detect repo-specific issues such as:

1. required `@group/@binding` ordering conventions,
2. entry-point naming conventions,
3. missing explicit address spaces where Sidereal wants them,
4. forbidden hardcoded patterns,
5. shader-role/class mismatches,
6. source/cache parity violations,
7. unsupported preview/runtime binding usage.

## 4.4 Layer D: Style/best-practice rules

Warn about:

1. magic numbers,
2. large functions,
3. excessive nesting,
4. ambiguous naming,
5. unused declarations where detectable.

This layer should be treated as optional/polish, not the initial dependency.

## 5. Recommended Tooling Strategy

## 5.1 Primary diagnostic source: validator-backed WGSL diagnostics

Best fit:

1. Naga-backed validation exposed to JS/WASM,
2. or a service path that uses Naga under the hood.

Why:

1. this gives real compiler-ish WGSL feedback,
2. it aligns with actual runtime validation expectations,
3. it is more valuable than shallow regex/style checks.

Use cases:

1. validate on debounced edit,
2. validate before preview apply,
3. validate before save/promote,
4. power the diagnostics pane with real semantic errors.

## 5.2 Optional structural parser/reflection layer

Useful fit:

1. `wgsl_reflect`-style parsing/reflection in JS/TS or equivalent tooling.

Why:

1. cheap access to uniforms/bindings/entry points,
2. easy structural rule checks,
3. useful for auto-generating inspector metadata and cheap lint passes.

Use cases:

1. extract uniform schema,
2. inspect binding declarations,
3. detect duplicate bindings,
4. infer structural metadata for project rules.

## 5.3 Optional future language-intelligence source

Best fit:

1. `wgsl-analyzer` or equivalent LSP-backed intelligence.

Why:

1. richer IDE behaviors,
2. type-aware navigation and completions,
3. more mature editor-like diagnostics over time.

Why not first:

1. wiring LSP semantics into a `use-editable` contenteditable editor is materially more work,
2. diagnostics-only delivery does not require LSP integration,
3. the dashboard can get real value earlier from validator-backed diagnostics alone.

## 6. Editor Integration Architecture

## 6.1 Core model

Treat the editor as:

1. source text input,
2. debounced validation pipeline,
3. normalized diagnostics output,
4. rendered decorations/markers.

This is the right fit for `use-editable`.

## 6.2 Recommended data flow

1. User edits WGSL in the `use-editable` surface.
2. Frontend updates a canonical text buffer.
3. A debounce timer fires after a short idle window.
4. The buffer is sent through:
   - parser/reflection pass,
   - validator pass,
   - Sidereal project-rule pass.
5. All issues are normalized into one diagnostics model.
6. The editor renders:
   - line/column markers,
   - squiggles/underlines,
   - gutter badges,
   - bottom diagnostics panel entries.
7. Preview apply/save actions reuse the same diagnostics engine but may run a stricter profile.

## 6.3 Diagnostic model

Recommended normalized diagnostic shape:

```ts
type ShaderDiagnosticSeverity = 'error' | 'warning' | 'info'

type ShaderDiagnosticSource =
  | 'parser'
  | 'validator'
  | 'project_rule'
  | 'preview_runtime'

interface ShaderDiagnostic {
  id: string
  source: ShaderDiagnosticSource
  severity: ShaderDiagnosticSeverity
  message: string
  line: number
  column: number
  endLine?: number
  endColumn?: number
  ruleId?: string
  relatedText?: string
}
```

Rationale:

1. one model for frontend and backend,
2. one model for parse/validate/project-rule errors,
3. easy diagnostics pane rendering,
4. easy future export over API.

## 6.4 `use-editable` implications

`use-editable` is viable, but there are real UX costs:

1. DOM-range to text-range mapping is your job,
2. underlines/decorations need careful synchronization,
3. contenteditable selection behavior is less forgiving than Monaco/CodeMirror,
4. hover popovers and clickable diagnostics need explicit anchor/range handling.

Conclusion:

1. diagnostics are feasible,
2. rich IDE ergonomics are possible but take deliberate editor infrastructure work,
3. `use-editable` is best if the team wants a lightweight custom editor and accepts owning range/decorations.

## 7. Recommended Rule Sources

## 7.1 Reusable diagnostics

Use:

1. validator/compiler diagnostics as the primary reusable diagnostics source,
2. WGSL reflection/parsing as the primary reusable structural-inspection source.

## 7.2 Sidereal-owned project rules

Implement locally in the dashboard/backend. Recommended first-pass rules:

1. require deterministic `@group/@binding` ordering conventions,
2. warn on hardcoded asset-role names where project metadata should drive behavior,
3. require entry-point naming conventions per shader class,
4. require explicit address spaces when applicable,
5. warn on magic numbers above a project-defined threshold,
6. cap function size/nesting for readability,
7. warn on duplicate or suspicious binding usage,
8. validate declared shader class against expected bind-layout/profile rules,
9. warn when preview-only bindings leak into runtime shader paths,
10. warn when source/cache parity contract looks violated.

These rules are likely more valuable than waiting for external WGSL style packs.

## 7.3 WGSL spec-level diagnostics

The WGSL spec has a limited named diagnostics model compared with JS lint ecosystems. That is useful, but narrow.

This means:

1. spec-defined diagnostics are important,
2. but they are not sufficient as a standalone "linter ecosystem."

## 8. Validation Modes

The editor should support multiple validation modes.

## 8.1 Fast edit mode

Runs on debounce while typing:

1. parse diagnostics,
2. lightweight semantic diagnostics,
3. cheap structural/project-rule checks.

Target:

1. low latency,
2. non-blocking UX.

## 8.2 Preview-apply mode

Runs on explicit validate/apply:

1. full validator pass,
2. preview compatibility checks,
3. inspector/uniform-schema reconciliation,
4. render-class compatibility checks.

## 8.3 Save/promote mode

Runs on draft save or promotion:

1. full validator pass,
2. full project-rule pass,
3. source/cache parity checks,
4. metadata completeness checks,
5. backend-side authoritative validation.

## 9. UI/UX Requirements

## 9.1 Mandatory editor diagnostics UX

1. inline squiggles or equivalent visible markers,
2. line/column resolution,
3. clickable diagnostics list,
4. current-line problem summary,
5. source label on each diagnostic (`parser`, `validator`, `project rule`, `preview runtime`),
6. severity coloring consistent with the dashboard design system.

## 9.2 Strongly recommended UX

1. debounce indicator while validation is pending,
2. stale diagnostics clearing when source changes,
3. hover message over highlighted ranges,
4. "jump to next error" shortcut,
5. diagnostics count in the editor toolbar,
6. separate tabs or filters for errors/warnings/info.

## 9.3 Nice-to-have later

1. quick fixes for simple project rules,
2. completions,
3. go-to-definition,
4. symbol outline,
5. hover type information.

Those move the system toward LSP territory and should not block initial linting.

## 10. Suggested Implementation Plan

## Phase A: Basic diagnostics pipeline

1. Add canonical editor text buffer separate from DOM rendering.
2. Add debounced validation loop.
3. Add parser/validator bridge returning normalized diagnostics.
4. Render diagnostics pane with line/column/source/severity.

Success criteria:

1. broken WGSL shows inline and panel diagnostics,
2. valid WGSL clears diagnostics quickly,
3. preview apply is blocked on validator errors.

## Phase B: Inline editor decorations

1. Add text-range mapping layer for `use-editable`.
2. Render per-range highlights/squiggles.
3. Add click-through from diagnostics list to source location.

Success criteria:

1. editor and diagnostics panel stay in sync,
2. cursor/selection behavior remains stable under frequent updates.

## Phase C: Sidereal project rules

1. Add local project-rule engine over reflected/parsed shader structure.
2. Add repo-specific warnings/errors for shader classes and conventions.
3. Surface rule IDs in the diagnostics panel.

Success criteria:

1. diagnostics go beyond parser/compiler output,
2. project constraints are visible before preview/runtime failures.

## Phase D: Richer language tooling

1. Evaluate whether `wgsl-analyzer`-style LSP integration is worth the cost.
2. Add richer code intelligence only if the custom editor still justifies it.

Success criteria:

1. completions/navigation are added without destabilizing editor correctness.

## 11. Recommendation

For Sidereal, the best path is a hybrid model:

1. validator-backed WGSL diagnostics as the foundation,
2. optional reflection/parsing for cheap structural checks,
3. Sidereal-owned project rules layered on top,
4. LSP integration only later if the custom editor still needs richer IDE features.

This is better than waiting for a mature WGSL ESLint ecosystem that does not currently exist in the same form as JS/TS tooling.

## 12. Direct Answer

1. Implementing linting in the dashboard WGSL editor with `use-editable`: very doable.
2. Getting real syntax/semantic diagnostics: high confidence.
3. Building a good inline-editor UX on top of `use-editable`: feasible, but this is the main implementation cost.
4. Reusing mature off-the-shelf WGSL style-rule packs: limited.
5. Best practical building blocks: validator-backed WGSL diagnostics, optional reflection parsing, and Sidereal-specific rules layered above them.
