# Dashboard Route Shell Refactor Note

Status: Active feature reference
Last updated: 2026-04-24
Owners: dashboard + tooling
Scope: dashboard route/shell layout direction after the route-shell refactor

## 0. Implementation Status

2026-04-24 status note:

1. Implemented: dashboard major tools are TanStack Router routes under a shared dashboard shell rather than one monolithic conditional page.
2. Implemented: shader workshop and sound studio use lazily split route-owned feature modules; explorer/database/game-world routes share shell patterns.
3. Open work: more panel/tree primitives can still be extracted, but the route-shell direction is current behavior rather than a pending plan.

## Objective

Refactor the dashboard so major tools are first-class TanStack Router routes that reuse one consistent shell instead of growing as one large routed page with internal conditional UI state.

## Target Shell Layout

All major dashboard tools should converge on a shared routed shell built from the existing dashboard layout primitives:

1. Left sidebar:
   - tool-specific library/tree/navigation
   - collapsible groups
   - consistent search/filter affordances
2. Center workspace:
   - tool-primary canvas/editor/content
   - split panes when needed
3. Right detail panel:
   - metadata
   - performance
   - inspectors/controls
   - diagnostics/logs

The shader workshop route is the first explicit example of this shape:

1. Left sidebar: shader library tree
2. Center: split code editor + preview/diagnostics stack
3. Right detail panel: metadata, performance, uniform controls

## Why This Refactor Is Wanted

The current dashboard route structure still leaves too much tool behavior inside large route-local state trees. That makes:

1. feature growth harder,
2. performance analysis harder,
3. code ownership boundaries blurrier,
4. future route-specific deep linking weaker.

The implemented route set is:

1. `/` for dashboard overview/health placeholders
2. `/database` and `/database/$entityGuid` for persisted explorer selections
3. `/game-world` and `/game-world/$entityGuid` for live BRP selections
4. `/shader-workshop` and `/shader-workshop/$shaderId` for shader authoring
5. `/script-editor` and `/settings` as routed placeholders
6. `/shader-workbench` redirected to `/shader-workshop` for compatibility

## Explicit Design Direction

Future work should prefer:

1. route-per-tool instead of conditional render branches inside one screen,
2. slugs for durable primary selections (`$entityGuid`, `$shaderId`) and query params for secondary tool state,
3. shared layout primitives for panel chrome/resizing,
4. reuse of sidebar tree and detail panel patterns across tools.

## Implemented Notes

1. The dashboard now uses a shared routed shell with one top navbar and one icon-only left rail for major tools.
2. `nuqs` backs secondary tool state such as panel widths, filters, selected tool sections, and shader search.
3. `Database` and `Game World` intentionally diverge at the route boundary even though both currently reuse the explorer shell primitives.
4. The shader route now uses resizable editor/preview and preview/diagnostics panes while keeping the right inspector panel for metadata and performance.

Future work should avoid:

1. building another one-off full-page layout for each tool,
2. hiding route-worthy state inside large component-local state blobs,
3. duplicating panel resizing/shell wiring per tool.

## Likely Follow-Up Tasks

1. Extract a reusable routed dashboard shell component from the current explorer/workbench layouts.
2. Move the world explorer out of an oversized single route into a clearer route/module boundary.
3. Introduce route search params for selected item, tabs, and view modes where deep linking is useful.
4. Normalize left-panel tree APIs so entity trees, shader trees, and future asset/script trees share one model.
5. Keep performance instrumentation visible in each route’s right-side inspector panel.
6. Allow route-specific exceptions for where high-frequency diagnostics belong; shader compile/runtime diagnostics currently live under the preview pane rather than the right panel because that keeps errors attached to the rendered output.

## Note For Future Agents

When extending the dashboard, assume the intended direction is a proper routed tool suite using TanStack Router, not a single monolithic screen. The shader workshop layout is intended as a concrete stepping stone toward that structure, not a special exception.
