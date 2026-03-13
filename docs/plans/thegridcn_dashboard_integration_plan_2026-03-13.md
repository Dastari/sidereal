# The GridCN Dashboard Integration Plan

Status: Proposed implementation plan  
Date: 2026-03-13  
Owners: dashboard/frontend + design system

Primary references:
- `docs/frontend_ui_styling_guide.md`
- `dashboard/src/styles.css`
- `dashboard/src/components/layout/DashboardShell.tsx`
- `dashboard/src/features/explorer/ExplorerWorkspace.tsx`
- `dashboard/src/features/shaders/ShaderWorkshopPage.tsx`
- `https://github.com/educlopez/thegridcn-ui`
- `https://raw.githubusercontent.com/educlopez/thegridcn-ui/main/registry.json`
- `https://raw.githubusercontent.com/educlopez/thegridcn-ui/main/src/app/globals.css`
- `https://raw.githubusercontent.com/educlopez/thegridcn-ui/main/src/components/theme/theme-provider.tsx`

## 0. Status Notes

- 2026-03-13: Initial adoption plan created after reviewing the current dashboard codebase and the current upstream `thegridcn-ui` registry/theme sources.
- 2026-03-13: Upstream currently exposes 140 registry entries and 139 unique component names. There is at least one duplicate registry name (`alert`), and the public site/screenshot appears to showcase a few items not obviously represented in the raw registry. Implementation should treat the raw registry as the install/import source of truth and the public site as presentation/reference only.
- 2026-03-13: This is not a drop-in replacement plan. The dashboard is a TanStack Start + Vite app, not a Next.js app, and the current dashboard design guide still requires semantic theme tokens, dense technical readability, and existing local wrapper boundaries.

## 1. Goal

Adopt `thegridcn-ui` as a selective dashboard visual and component layer upgrade without:

1. breaking the existing TanStack route/data-loading architecture,
2. coupling the dashboard to Next.js-specific runtime assumptions,
3. replacing every local primitive blindly,
4. importing heavy 3D or showcase-only components into the eagerly loaded shell,
5. violating the current dashboard route-splitting and semantic-token standards.

The right outcome is:

1. keep local import boundaries under `dashboard/src/components/ui/`,
2. restyle/replace the most-used primitives with GridCN variants where they fit,
3. introduce GridCN-only operator widgets where they materially improve the dashboard,
4. preserve a Sidereal-specific theme/token layer rather than shipping the upstream demo wholesale.

## 2. Current Dashboard Baseline

## 2.1 Runtime and framework baseline

Current dashboard stack:

1. React 19.2 + TanStack Start/TanStack Router
2. Vite, not Next.js
3. Tailwind CSS 4
4. local `shadcn/ui` wrappers under `dashboard/src/components/ui/`
5. custom dark/light/system theme hook in `dashboard/src/hooks/use-theme.ts`

## 2.2 Current local wrapper inventory

Current local wrappers already present:

1. `alert`
2. `alert-dialog`
3. `badge`
4. `button`
5. `card`
6. `collapsible`
7. `confirm-dialog`
8. `data-table`
9. `dialog`
10. `dropdown-menu`
11. `input`
12. `label`
13. `scroll-area`
14. `separator`
15. `skeleton`
16. `slider`
17. `spinner`
18. `switch`
19. `table`
20. `tabs`
21. `tooltip`

## 2.3 Current high-traffic dashboard surfaces

Most important dashboard surfaces for early adoption:

1. Shell and navigation:
   - `dashboard/src/components/layout/DashboardShell.tsx`
2. Game World explorer:
   - `dashboard/src/features/explorer/ExplorerWorkspace.tsx`
   - `dashboard/src/components/sidebar/*`
   - `dashboard/src/components/grid/*`
3. Shader Workshop:
   - `dashboard/src/features/shaders/ShaderWorkshopPage.tsx`
4. Database tool:
   - `dashboard/src/routes-lazy/database-pages.tsx`
   - `dashboard/src/features/database/*`
5. Auth/settings/status affordances:
   - `dashboard/src/components/layout/DashboardAdminAccess.tsx`
   - `dashboard/src/components/ThemeToggle.tsx`

## 3. Upstream The GridCN Summary

## 3.1 What The GridCN is

Upstream positioning:

1. Tron: Ares-inspired theme system built on top of shadcn/ui
2. 6 public themes:
   - `tron`
   - `ares`
   - `clu`
   - `athena`
   - `aphrodite`
   - `poseidon`
3. 4 intensity levels:
   - `none`
   - `light`
   - `medium`
   - `heavy`
4. strong neon glow, HUD, scanline, and console-style presentation utilities
5. some 3D components via `three` and `@react-three/fiber`

## 3.2 Key compatibility observation

This is close to shadcn, but not a drop-in replacement:

1. the component registry is installable with `shadcn add`, which is useful,
2. many names directly overlap our current wrapper names,
3. the upstream theme provider and app shell are Next-oriented and must not be copied verbatim,
4. the upstream global CSS is much more visually opinionated than our current dashboard token set,
5. some showcase components are marketing/demo-oriented rather than operator-dashboard-oriented.

## 4. Non-Negotiable Integration Rules

Implementation should follow these rules:

1. Keep local wrapper imports stable.
   - The dashboard should continue importing from `@/components/ui/...` or equivalent local adapter surfaces, not directly from upstream registry paths in route code.
2. Do not replace TanStack route boundaries with component-library routing assumptions.
3. Do not import upstream Next.js `ThemeProvider` or layout code directly.
4. Keep major dashboard routes lazily split.
5. Do not move heavy Three.js/GridCN 3D effects into the eager shell.
6. Preserve semantic token usage in normal dashboard UI.
7. Treat typography changes as a deliberate design-system decision.
   - Upstream `Orbitron`/`Rajdhani` should not silently replace the current dashboard typography without an explicit guide update.
8. Preserve admin auth, validation, and route-owned data-loading patterns exactly as they are.

## 5. Theme and Design-System Strategy

## 5.1 Recommended direction

Do not fully replace the dashboard theme system with the upstream demo theme system.

Recommended V1 direction:

1. keep current semantic tokens and root theme ownership in `dashboard/src/styles.css`,
2. add a new GridCN-inspired token layer on top of the existing token system,
3. introduce a second visual axis:
   - `gridTheme`: `tron`, `poseidon`, `athena`, `clu`, `aphrodite`, `ares`
   - `gridIntensity`: `off`, `light`, `medium`, `heavy`
4. keep the existing dark/light/system color-scheme choice for accessibility and browser integration,
5. default the dashboard visual theme to `tron` or `poseidon`, not `ares` or `aphrodite`, because the current frontend guide still prefers cool blue operator styling.

## 5.2 Why not full upstream theme replacement

Full replacement would currently conflict with:

1. `docs/frontend_ui_styling_guide.md` palette direction,
2. current `dark/light/system` theme contract,
3. existing grid-canvas color hooks in `dashboard/src/hooks/use-grid-theme-colors.ts`,
4. the dashboard’s technical-tool readability bias.

## 5.3 Required theme implementation work

Phase-1 theme work should:

1. port relevant CSS utilities from upstream:
   - glow utilities
   - bracket/corner-frame utilities
   - scanline utilities
   - panel chrome helpers
2. map them onto Sidereal semantic variables rather than raw upstream color values,
3. add `data-grid-theme` and `data-grid-intensity` attributes at the document root,
4. update the current theme toggle into a combined visual-theme control surface,
5. keep a safe fallback where `gridIntensity=off` renders a restrained dashboard version.

## 6. Replacement Matrix for Existing Local Wrappers

These are the highest-confidence direct replacements because they already exist locally and have the same or near-same conceptual role upstream.

| Current local wrapper | Approx. current import count | Upstream match | Decision | Notes |
| --- | ---: | --- | --- | --- |
| `button` | 13 | `button` | Replace | Keep local API; port GridCN styling into local wrapper first. |
| `input` | 11 | `input` | Replace | High value because search/tool forms are everywhere. |
| `badge` | 10 | `badge` | Replace | Also useful for neon status tags. |
| `card` | 7 | `card` | Replace | Base panel skin for overview/database/settings. |
| `switch` | 7 | `switch` | Replace | Useful for settings/editor toggles. |
| `tooltip` | 6 | `tooltip` | Replace | Core shell and tool affordance surface. |
| `alert` | 5 | `alert` | Replace | Use restrained GridCN alert styling, not demo-only glow spam. |
| `collapsible` | 5 | `collapsible` | Replace | Used in explorer/detail contexts. |
| `slider` | 5 | `slider` | Replace | Especially useful in shader workbench/editor surfaces. |
| `scroll-area` | 4 | `scroll-area` | Replace | Important for explorer tree and shader lists. |
| `button`-adjacent grouped actions | n/a | `button-group` | Add alongside replacement | Useful for toolbars and theme/intensity controls. |
| `tabs` | 2 | `tabs` | Replace | Database and routed tool tabs. |
| `dialog` | 2 | `dialog` | Replace | Admin unlock + rename flows. |
| `data-table` | 2 | `data-table` | Evaluate, then selectively replace | Our local data table has custom selection/action behavior; use upstream styling ideas before swapping internals. |
| `label` | 2 | `label` | Replace | Safe low-risk port. |
| `spinner` | 2 | `spinner` | Replace | Use upstream operator-style spinner/loading chrome. |
| `dropdown-menu` | 1 | `dropdown-menu` | Replace | Low risk. |
| `table` | 1 | `table` | Replace | Mostly a styling pass. |
| `alert-dialog` | 1 | `alert-dialog` | Replace | Keep local confirm semantics. |
| `separator` | 0 | `separator` | Replace | Safe base utility. |
| `skeleton` | 1 | `skeleton` | Replace | Good candidate for CRT/scanline-styled loading blocks. |

Implementation rule:

1. for direct replacements, keep the local file paths and migrate implementation under those paths first,
2. only after local compatibility is stable should we consider installing additional upstream variants.

## 7. New Components We Should Add and Use

These are the highest-value additive components for the dashboard, including surfaces that do not currently use shadcn directly.

## 7.1 Shell and navigation

Recommended additions:

1. `uplink-header`
2. `sidebar`
3. `breadcrumb`
4. `button-group`
5. `notification`
6. `chip`
7. `tag`
8. `status-bar`
9. `beam-marker`
10. `glow-container`

Planned usage:

1. Replace the current top header in `DashboardShell` with a Sidereal-adapted `uplink-header` pattern.
2. Restyle the left icon rail with GridCN sidebar chrome instead of plain ghost buttons.
3. Use `button-group` for theme/intensity controls and grouped action clusters.
4. Use `notification` for shell-level mutation success/failure summaries instead of plain text blocks.

## 7.2 Explorer / Game World route

Recommended additions:

1. `status-bar`
2. `diagnostics-panel`
3. `coordinate-display`
4. `location-display`
5. `radar`
6. `map-marker`
7. `reticle`
8. `floating-panel`
9. `hud-frame`
10. `hud-corner-frame`
11. `context-menu`
12. `command-menu`
13. `empty-state`
14. `comparison-table`

Planned usage:

1. Replace the current custom bottom status bar chrome in the explorer with a GridCN status-bar treatment.
2. Re-skin entity details, visibility debugging, and BRP diagnostics as `diagnostics-panel`/`hud-frame` surfaces.
3. Add coordinate/location displays around `GridCanvas` without rewriting the core renderer.
4. Use `reticle` and `map-marker` for selection/focus overlays.
5. Use `command-menu` for fast entity/resource navigation beyond the current tree-only workflow.

## 7.3 Shader Workshop

Recommended additions:

1. `terminal`
2. `data-stream`
3. `progress-ring`
4. `timeline-bar`
5. `number-input`
6. `text-input`
7. `field`
8. `popover`
9. `sheet`
10. `waveform`
11. `sparkline`
12. `countdown`
13. `timer`

Planned usage:

1. Present shader diagnostics and compile output in a `terminal`/`data-stream` style rather than plain card blocks.
2. Use `progress-ring` for preview/compile state and shader validation status.
3. Use `sparkline` for frame/perf metrics instead of text-only perf rows.
4. Use `field` and specialized numeric/text inputs for uniform editing.
5. Use `sheet` for compact side workflows such as dependency metadata or preset details.

## 7.4 Database and admin tools

Recommended additions:

1. `command`
2. `command-menu`
3. `empty`
4. `empty-state`
5. `pagination`
6. `select`
7. `textarea`
8. `toast` or `sonner`
9. `comparison-table`
10. `changelog`
11. `file-upload`

Planned usage:

1. Use `command`/`command-menu` for table/account quick search and future entity jump menus.
2. Use `empty` or `empty-state` where tables or script results are empty.
3. Use `toast`/`sonner` for non-blocking success feedback from mutation routes.
4. Use `changelog` for future script/shader/document history views.
5. Use `file-upload` for future shader/script import tools instead of bespoke drop zones.

## 7.5 Settings and future tools

Recommended additions:

1. `select`
2. `toggle`
3. `toggle-group`
4. `radio-group`
5. `checkbox`
6. `form`
7. `calendar`
8. `avatar`
9. `agent-avatar`
10. `avatar-group`
11. `identity-disc`
12. `stepper`

Planned usage:

1. Future settings screens should use GridCN form primitives rather than bespoke composition.
2. Script-editor and operator identity surfaces can use `agent-avatar`/`identity-disc` if we later expose operator/session identity more visually.
3. None of these should block the first dashboard integration slice.

## 8. Components to Defer or Avoid for Dashboard V1

## 8.1 Defer because they are heavy or niche

Defer until there is a real use case:

1. `grid`
2. `grid-floor`
3. `grid-scan-overlay`
4. `tunnel`
5. `video-player`
6. `video-progress`
7. `kanban-board`
8. `carousel`
9. `chart`
10. `heatmap`
11. `bento-grid`
12. `marquee`
13. `rating`

Reason:

1. these are either 3D-heavy,
2. showcase-oriented,
3. or not aligned with the current operator-dashboard priorities.

## 8.2 Avoid for now because they are duplicate showcase variants

Do not target these first:

1. `thegridcn-alert`
2. `thegridcn-badge`
3. `thegridcn-pagination`
4. `thegridcn-select`
5. `thegridcn-skeleton`
6. `thegridcn-slider`
7. `thegridcn-tabs`
8. `thegridcn-toggle`
9. `thegridcn-tooltip`
10. `dropdown`
11. `modal`
12. `breadcrumb-nav`
13. `thegridcn-timeline`

Reason:

1. these appear to be alternate showcase compositions rather than the primary primitive we should standardize on,
2. local wrapper clarity matters more than collecting every visual variant.

## 8.3 Marketing/demo-first components

Do not adopt for dashboard V1:

1. `cta-banner`
2. `feature-card`
3. `pricing-card`
4. `testimonial-card`
5. `logo-cloud`
6. `faq`
7. `stats-counter`

These may be useful for a public marketing site later, but not for the current operator dashboard.

## 9. Dependency and Compatibility Plan

## 9.1 Minimal first-wave dependency set

First-wave likely additions:

1. `cmdk`
2. `@radix-ui/react-accordion`
3. `@radix-ui/react-avatar`
4. `@radix-ui/react-checkbox`
5. `@radix-ui/react-context-menu`
6. `@radix-ui/react-hover-card`
7. `@radix-ui/react-navigation-menu`
8. `@radix-ui/react-progress`
9. `@radix-ui/react-radio-group`
10. `sonner`

## 9.2 Conditional second-wave dependencies

Only add when the mapped surface is actually being implemented:

1. `react-hook-form`
2. `react-day-picker`
3. `input-otp`
4. `vaul`
5. `recharts`
6. `embla-carousel-react`

## 9.3 Avoid in phase 1

Avoid adding these in the first dashboard adoption slice:

1. `three`
2. `@react-three/fiber`
3. other 3D-only GridCN dependencies

## 10. Route-by-Route Adoption Map

| Dashboard surface | Replace now | Add now | Defer |
| --- | --- | --- | --- |
| Shell (`DashboardShell`) | `button`, `badge`, `tooltip`, `card` skin | `uplink-header`, `sidebar`, `notification`, `beam-marker`, `button-group` | 3D/grid effects in eager shell |
| Overview route | `card`, `badge`, `button` | `stat-card`, `diagnostics-panel`, `anomaly-banner`, `announcement-bar`, `notification`, `sparkline` | marketing cards |
| Database routes | `tabs`, `data-table`, `dialog`, `alert`, `input`, `badge`, `button` | `command-menu`, `empty-state`, `pagination`, `toast/sonner`, `comparison-table`, `file-upload` | chart/carousel/kanban |
| Game World explorer | `scroll-area`, `collapsible`, `switch`, `button`, `tooltip` | `status-bar`, `diagnostics-panel`, `coordinate-display`, `location-display`, `radar`, `reticle`, `hud-frame`, `command-menu`, `context-menu` | 3D tunnel/grid |
| Shader Workshop | `input`, `slider`, `scroll-area`, `button`, `badge`, `tooltip` | `terminal`, `data-stream`, `progress-ring`, `timeline-bar`, `field`, `sheet`, `sparkline`, `timer` | media/video widgets |
| Settings / future script editor | `dialog`, `input`, `label`, `switch` | `select`, `toggle-group`, `radio-group`, `checkbox`, `form`, `identity-disc`, `agent-avatar` | public-site marketing widgets |

## 11. Implementation Phases

## 11.1 Phase 0: Validation spike

Deliverables:

1. install a minimal subset of upstream-compatible dependencies,
2. create a local experimental branch/surface under the dashboard only,
3. port:
   - `button`
   - `card`
   - `badge`
   - `tabs`
   - `tooltip`
4. prove the styles work in Vite/TanStack without Next.js assumptions,
5. prove theme + intensity attributes can be driven by the existing dashboard theme hook.

Exit criteria:

1. one route renders correctly with GridCN-inspired chrome,
2. no routing, hydration, or bundle regressions,
3. no guide-breaking typography change sneaks in accidentally.

## 11.2 Phase 1: Theme/token adapter

Deliverables:

1. add `gridTheme` and `gridIntensity` state locally,
2. port selected upstream glow/scanline/panel utility classes,
3. introduce a local theme selector UI,
4. keep `gridIntensity=off` as the safe default during rollout.

## 11.3 Phase 2: Primitive wrapper migration

Deliverables:

1. migrate high-traffic local wrappers first,
2. keep all imports stable under `@/components/ui/*`,
3. update snapshots/tests where local class output changes,
4. re-skin `confirm-dialog` through the migrated local `alert-dialog`.

Priority order:

1. `button`
2. `input`
3. `badge`
4. `card`
5. `tabs`
6. `tooltip`
7. `dialog`
8. `alert`
9. `scroll-area`
10. `slider`

## 11.4 Phase 3: Shell and explorer pass

Deliverables:

1. new dashboard shell chrome,
2. GridCN status/header treatment,
3. explorer HUD framing and diagnostics styling,
4. command-menu fast navigation.

## 11.5 Phase 4: Shader workshop and database pass

Deliverables:

1. shader diagnostics terminalization,
2. upgraded perf/status widgets,
3. database search/action surfaces using command-menu and improved empty/loading states,
4. non-blocking toast feedback for mutations.

## 11.6 Phase 5: Optional advanced widgets

Candidate later work:

1. radar overlays for explorer,
2. heatmap/sparkline telemetry,
3. boot-sequence/game-client loading treatments,
4. changelog/file-upload/script-editor workflows.

## 12. Risks and Mitigations

### Risk: visual drift from the current frontend UI guide

Mitigation:

1. keep default theme blue/cool-toned,
2. keep typography unchanged in phase 1,
3. update `docs/frontend_ui_styling_guide.md` only once the new visual direction is intentionally approved.

### Risk: importing too much showcase chrome into operator workflows

Mitigation:

1. adopt by route and by workflow,
2. prefer subtle intensity levels for data-dense screens,
3. keep `heavy` intensity optional, not default.

### Risk: breaking local wrapper API compatibility

Mitigation:

1. migrate implementation behind existing local wrapper paths,
2. avoid direct upstream imports in route code,
3. validate the database/game-world/shader routes individually.

### Risk: bundle growth from optional dependencies

Mitigation:

1. keep first-wave dependency set minimal,
2. defer 3D/media libraries,
3. preserve lazy route boundaries.

## 13. Recommended First Implementation Slice

Recommended first slice:

1. add local GridCN theme/intensity adapter,
2. port `button`, `card`, `badge`, `tabs`, and `tooltip`,
3. rework `DashboardShell` with an `uplink-header`-inspired top bar and stronger sidebar chrome,
4. apply `status-bar`, `diagnostics-panel`, and `hud-frame` patterns to the Game World explorer,
5. add `command-menu` and `sonner` only after the shell and primitive migration is stable.

This gets visible value quickly without committing the dashboard to every upstream showcase component.

## 14. Appendix A: Upstream Component Inventory Snapshot

Current upstream unique component names reviewed for this plan:

### 14.1 Base and shadcn-adjacent primitives

`accordion`, `alert`, `alert-dialog`, `aspect-ratio`, `avatar`, `badge`, `breadcrumb`, `button`, `button-group`, `calendar`, `card`, `carousel`, `chart`, `checkbox`, `collapsible`, `command`, `context-menu`, `data-table`, `dialog`, `drawer`, `dropdown-menu`, `empty`, `field`, `form`, `hover-card`, `input`, `input-group`, `input-otp`, `item`, `kbd`, `label`, `menubar`, `navigation-menu`, `pagination`, `popover`, `progress`, `radio-group`, `scroll-area`, `select`, `separator`, `sheet`, `sidebar`, `skeleton`, `slider`, `sonner`, `spinner`, `switch`, `table`, `tabs`, `textarea`, `toast`, `toggle`, `toggle-group`, `tooltip`

### 14.2 GridCN operator and HUD widgets

`agent-avatar`, `announcement-bar`, `anomaly-banner`, `arrival-panel`, `beam-marker`, `boot-sequence`, `breadcrumb-nav`, `changelog`, `chip`, `circuit-background`, `command-menu`, `comparison-table`, `coordinate-display`, `countdown`, `crt-effect`, `data-card`, `data-stream`, `derez-timer`, `diagnostics-panel`, `divider`, `empty-state`, `energy-meter`, `floating-panel`, `gauge`, `glow-container`, `heatmap`, `hud`, `hud-corner-frame`, `hud-frame`, `identity-disc`, `kanban-board`, `location-display`, `map`, `map-marker`, `modal`, `notification`, `number-input`, `progress-bar`, `progress-ring`, `radar`, `rating`, `regen-indicator`, `reticle`, `signal-indicator`, `sparkline`, `speed-indicator`, `stat`, `stat-card`, `status-bar`, `stepper`, `tag`, `terminal`, `text-input`, `thegridcn-alert`, `thegridcn-badge`, `thegridcn-pagination`, `thegridcn-select`, `thegridcn-skeleton`, `thegridcn-slider`, `thegridcn-tabs`, `thegridcn-timeline`, `thegridcn-toggle`, `thegridcn-tooltip`, `timeline-bar`, `timer`, `uplink-header`, `waveform`

### 14.3 3D, media, and showcase-heavy extras

`bento-grid`, `cta-banner`, `faq`, `feature-card`, `file-upload`, `grid`, `grid-floor`, `grid-scan-overlay`, `logo-cloud`, `marquee`, `pricing-card`, `stats-counter`, `testimonial-card`, `tunnel`, `video-player`, `video-progress`
