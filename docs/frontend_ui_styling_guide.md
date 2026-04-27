# Sidereal Frontend UI Styling Guide

**Status:** Active Frontend Design System  
**Date:** 2026-03-12  
**Audience:** Dashboard/frontend developers, AI agents, UI contributors

Update note (2026-03-12):

- 2026-04-26 update: dashboard routes and API handlers are expected to flow through the gateway-backed account session boundary exposed by `/api/dashboard-session` and enforced by the shared server-side guard in `dashboard/src/server/dashboard-auth.ts`. Do not add new dashboard admin-password flows.
- Temporary local auth uses an HttpOnly SameSite=Strict cookie plus same-origin mutation checks; this is an interim operator auth point, not a replacement for future role-based session auth.
- Password reset workflows must return acceptance state only; raw reset tokens must not be rendered back into the browser by default.
- The database tool now uses route-owned initial data loading through a shared server loader/server function path instead of effect-owned first-render fetches.

Update note (2026-03-13):

- The dashboard now supports a GridCN-aligned visual theme layer on top of the existing semantic token system. `data-theme` is now reserved for the GridCN visual theme family (`tron`, `ares`, `clu`, `athena`, `aphrodite`, `poseidon`), while browser color-scheme continues to be controlled through the existing `light`/`dark` classes and `system` resolution.
- GridCN intensity (`off`, `light`, `medium`, `heavy`) is now an allowed dashboard visual control, but components must still consume semantic tokens rather than hardcoded per-component colors.
- The default operator direction remains cool-toned (`tron`-style / blue-cyan family) even though warmer or pink accent themes are technically supported.
- GridCN frame adoption should stay sharp rather than soft: explorer side panels should prefer the local sidebar-nav shell, bounded technical surfaces should prefer the local HUD-frame wrapper, and large `rounded-xl` panel chrome should not be reintroduced on those GridCN-framed surfaces.
- 2026-03-14: explorer/tactical grid overlays should use the local GridCN wrapper layer and respect dashboard theme intensity. Decorative scan/grid effects belong on the grid canvas container, not on shell panels or route backgrounds.
- 2026-03-14: bottom-of-grid HUD readouts should prefer GridCN display components fed by live camera/selection state rather than ad hoc text pills. Keep them in the grid canvas overlay lane and style them from semantic tokens.
- Modal and confirmation flows should keep using the shared local `Dialog` and `AlertDialog` wrappers, but those wrappers should present as GridCN-framed overlays rather than default rounded card dialogs.

Update note (2026-03-14):

- Segmented selectors should prefer the shared local GridCN-aligned `Tabs` wrapper, and compact grouped controls should prefer the shared local `ButtonGroup` wrapper rather than ad hoc adjacent bordered buttons.
- Explorer/tool header toggle controls and schema-editor boolean fields should use the shared local `Switch` wrapper inside those grouped control surfaces instead of bespoke label-plus-toggle rows.
- Shared dashboard buttons, inputs, badges, dropdown menus, and table primitives should continue to route through the local wrapper layer under `dashboard/src/components/ui/`, and those wrappers should stay aligned to the GridCN interaction model rather than falling back to plain shadcn defaults or route-local one-off styling.
- Single-value editor sliders should prefer the local GridCN-style slider component; keep the shared Radix slider wrapper for true range sliders and other multi-thumb cases that the GridCN custom slider does not cover cleanly.
- Numeric inputs in the component-editing surfaces should prefer the local GridCN number-input wrapper instead of plain text/number fields when the control is editing numeric gameplay/component values directly.

Update note (2026-04-24):

- Dashboard world-coordinate API and BRP parsing surfaces consume authoritative Rust f64 coordinate payloads as JSON numbers and TypeScript `number` values. Do not downcast, round, stringify, or integer-coerce world positions/velocities in loaders, schemas, or explorer state; only rendering layers may project to f32/canvas space.

Update note (2026-04-26):

- The target dashboard auth direction is gateway-backed account auth, documented in `docs/plans/gateway_dashboard_auth_character_flow_plan_2026-04-26.md` and `docs/decisions/dr-0036_gateway_account_auth_dashboard_and_character_creation.md`.
- The existing password-backed `/api/dashboard-session` admin boundary is superseded by the `DR-0036` target once implementation lands.
- Public registration/login, email one-time code and magic-link completion, TOTP setup, account character management, and admin dashboard routes should use the same semantic theme tokens and local UI wrappers.
- Admin dashboard routes and server data loaders must be guarded by gateway account session, route-specific scopes, and verified MFA after the migration.

## 1. Scope

This guide defines the enforceable UI, theming, routing, validation, and frontend architecture standards for the web frontend under `dashboard/`.

This guide does **not** replace [docs/ui_design_guide.md](/home/toby/dev/sidereal_v3/docs/ui_design_guide.md). The native Bevy UI guide remains the source of truth for in-game/native client UI. This document exists to give the dashboard/web frontend its own operational standard while keeping brand and theme consistency across both surfaces.

## 2. Shared Brand Invariants

The dashboard and native client must feel like the same product family.

Shared non-negotiable brand traits:

- Dark space-themed presentation with cool neutral surfaces and restrained blue accents.
- High legibility over spectacle; readable density matters more than decoration.
- Rounded but not soft UI:
  - controls use compact radii,
  - panels/dialogs use larger but still restrained radii.
- Strong information hierarchy:
  - obvious page title,
  - clear panel ownership,
  - minimal ambiguous chrome.
- Status semantics must stay consistent:
  - destructive/error = red,
  - warning = amber/yellow,
  - success = green,
  - info = blue.
- Dense technical areas should feel precise and tool-like, not consumer-app casual.

## 3. Theme and Token Standards

### 3.1 Use semantic theme tokens only

Dashboard UI must use semantic theme tokens and CSS variables, not ad hoc per-component colors.

Required:

- Use Tailwind classes backed by theme variables such as:
  - `bg-background`
  - `bg-card`
  - `text-foreground`
  - `text-muted-foreground`
  - `border-border`
  - `bg-primary`
  - `text-primary`
  - `bg-destructive`
- Keep component styling aligned to the shared semantic palette.

Forbidden except for tightly scoped bootstrapping or rendering cases:

- raw hex colors inside React components,
- inline `style` color declarations for normal app UI,
- one-off gradients or shadows that are not part of the design system,
- introducing a new accent color without updating this guide.

GridCN-specific clarification:

- GridCN theme support is allowed only through root theme attributes and semantic token remapping.
- Do not hardcode `tron`, `ares`, or other theme colors inside component logic when a semantic token or shared utility class can be used instead.

### 3.2 Dashboard palette direction

The dashboard should stay aligned to the native client mood:

- Base background: blue-black / deep neutral dark.
- Surface background: slightly lifted blue-gray dark panels.
- Accent: cool blue.
- Secondary emphasis: muted steel/neutral.
- Success/warning/error should stay close to the native guide’s severity direction.

GridCN theme note:

- Alternate GridCN themes may exist for operator preference, but the default dashboard presentation should stay closest to the `tron` / `poseidon` family rather than the warmer showcase themes.
- New dashboard work should still look correct with GridCN intensity set to `off` or `light`; do not require `heavy` intensity for basic affordance clarity.

### 3.3 Typography

Preferred dashboard stack:

- Sans: `Rajdhani`
- Display: `Orbitron`
- Mono: `JetBrains Mono`

Use mono only for:

- IDs,
- entity GUIDs,
- ports,
- paths,
- code,
- diagnostic payloads,
- tabular numeric views.

Use `Orbitron` sparingly for:

- page/surface titles,
- compact HUD-like labels,
- top-level shell chrome.

Do not apply the display font to long-form body copy, dense tables, or normal form fields.

### 3.4 Spacing and shape

Dashboard spacing should stay compact and regular:

- page/panel padding: 16, 20, 24
- control padding: 8, 10, 12
- gaps: 8, 12, 16, 20
- compact toolbars should be visibly tighter than content panels

Preferred radii:

- GridCN-aligned controls and HUD surfaces should stay sharp: prefer `rounded-none`
- Do not reintroduce `rounded-md`, `rounded-xl`, or pill-shaped chrome on GridCN-framed buttons, inputs, panels, tables, dialogs, or shell navigation

## 4. shadcn/ui Policy

### 4.1 Default rule

For dashboard UI, use existing local `shadcn/ui` wrappers first.

Decision order:

1. Use an existing component from `dashboard/src/components/ui/`.
2. If the pattern exists in shadcn but is missing locally, add the local wrapper and use it.
3. Build a custom component only if the interaction is genuinely domain-specific and not well served by the available shadcn components.

### 4.2 Available component inventory

The available shadcn/UI component inventory for dashboard work includes:

- Accordion
- Alert
- Alert Dialog
- Aspect Ratio
- Avatar
- Badge
- Breadcrumb
- Button
- Button Group
- Calendar
- Card
- Carousel
- Chart
- Checkbox
- Collapsible
- Combobox
- Command
- Context Menu
- Data Table
- Date Picker
- Dialog
- Direction
- Drawer
- Dropdown Menu
- Empty
- Field
- Hover Card
- Input
- Input Group
- Input OTP
- Item
- Kbd
- Label
- Menubar
- Native Select
- Navigation Menu
- Pagination
- Popover
- Progress
- Radio Group
- Resizable
- Scroll Area
- Select
- Separator
- Sheet
- Sidebar
- Skeleton
- Slider
- Sonner
- Spinner
- Switch
- Table
- Tabs
- Textarea
- Toast
- Toggle
- Toggle Group
- Tooltip
- Typography

### 4.3 Required component choices for common cases

Use these defaults unless there is a clear reason not to:

- blocking confirmation: `Alert Dialog`
- editable modal form: `Dialog`
- supplemental side workflow: `Sheet`
- short transient success/info feedback: `Sonner` or `Toast`
- inline error or warning block: `Alert`
- empty collection or empty panel state: `Empty`
- loading placeholder for routed content: `Skeleton`
- indeterminate loading for compact actions: `Spinner`
- long structured forms: `Field`, `Label`, `Input`, `Textarea`, `Select`, `Checkbox`, `Radio Group`, `Switch`
- command/searchable picker: `Command` or `Combobox`
- destructive action menus: `Dropdown Menu` or `Context Menu` plus `Alert Dialog`
- dense tool navigation: `Sidebar`, `Tabs`, `Breadcrumb`, `Navigation Menu`, `Menubar`

### 4.4 Forbidden dashboard UI patterns

Do not introduce these patterns in the dashboard:

- `window.prompt`
- `window.confirm`
- `window.alert`
- console-only error handling for user-visible failures
- inline-styled fallback pages when a themed component should exist
- bespoke primitives for controls that already exist in shadcn/ui

## 5. Routing, Code Splitting, and Screen Boundaries

### 5.1 Route ownership

Route modules must own:

- route params/search validation,
- pending/loading boundary,
- error boundary,
- not-found handling where relevant,
- initial data loading boundary.

Route modules should stay thin. Heavy screen logic belongs in feature modules loaded by the route.

### 5.2 Major tool routes must be lazily loaded

All major dashboard tool areas must be route-split and lazy by default:

- `database`
- `game-world`
- `game-client`
- `shader-workshop`
- any future editor/workbench/admin-heavy tool

Do not eagerly import heavy tool implementations into the shell route or other always-loaded modules.

### 5.3 Pending and error boundaries

Data-owning routes must define route-level boundaries:

- `pendingComponent`
- `errorComponent`
- `notFoundComponent` where route-local not-found states are meaningful

The root boundary is a last resort, not the primary UX for feature failures.

## 6. Data Loading and API Usage Standards

### 6.1 Initial data should be route-owned, not effect-owned

Default rule:

- initial screen data belongs in route loaders, server functions, or equivalent route-owned loading mechanisms,
- not in ad hoc `useEffect` fetches after mount.

Allowed exception:

- local interactive refresh/update behavior that is explicitly incremental and not the first-render data path.

### 6.2 Centralized API access

Dashboard API calls should flow through a shared typed client/helper layer rather than raw `fetch()` scattered throughout feature code.

That shared layer must standardize:

- request shaping,
- error mapping,
- auth/session headers when introduced,
- CSRF handling when introduced,
- typed response parsing.

### 6.3 Server/client separation

Server-only modules must stay isolated from client bundles.

Required:

- keep server utilities under server-only modules,
- do not import server-only helpers into client-rendered components,
- keep database/gateway/proxy code out of eagerly loaded client modules.

## 7. Validation and Security Standards

### 7.1 Zod is the default validation layer

Use Zod for:

- route params with real invariants,
- route search params,
- form input validation,
- API request bodies,
- API query strings,
- mutation payload validation,
- shared normalization/parsing at the route boundary.

Do not keep expanding one-off manual validators for routine dashboard inputs.

### 7.2 Mutation routes must be security-ready

Any dashboard mutation route must be written as if future auth is mandatory.

Required:

- explicit auth/authorization guard point,
- no destructive mutation endpoint without a clear authorization path,
- no sensitive token exposure back to the browser unless explicitly justified and documented,
- no reliance on “internal tool” status as a security model,
- design routes so CSRF protection can be attached cleanly.

### 7.3 Error exposure

User-facing errors should be:

- safe,
- actionable,
- non-secret leaking,
- consistent in shape.

Do not pass through raw infrastructure details to the browser unless the route is intentionally debug-only and access-controlled.

## 8. Component and File Organization Standards

### 8.1 Feature boundaries

Use feature-based organization:

- `components/` for reusable UI building blocks,
- `features/` for screen/domain logic,
- `routes/` for route ownership and wiring,
- `server/` for backend-only adapters,
- `lib/` for small cross-feature utilities, not dumping grounds.

### 8.2 Module size and responsibility

Avoid continuing growth of large mixed-responsibility files.

If a file mixes several of these, split it:

- route state,
- remote loading,
- mutation orchestration,
- rendering,
- editor behavior,
- domain transformation,
- layout composition.

Route wrappers should be small. Feature entrypoints may be larger, but long files should be split before they become the only place the feature can be understood.

### 8.3 Naming

Use names that expose ownership and purpose.

Prefer:

- `ShaderWorkshopRoutePage`
- `useDatabaseAdminData`
- `DatabaseAccountsPage`

Avoid vague catch-all names like:

- `helpers`
- `data`
- `misc`
- `manager`
- `utils` for domain-specific logic

## 9. Feedback, Loading, and Empty-State Standards

### 9.1 Loading

Use:

- `Skeleton` for first-load panels and routed content,
- `Spinner` for compact action/loading states,
- `Progress` when there is meaningful completion progress.

Do not leave routed screens blank while loading.

### 9.2 Empty states

Use an explicit `Empty` pattern or equivalent local wrapper for:

- no results,
- no selected item,
- no configured connection,
- no data available.

Empty states should explain the condition and, where possible, the next action.

### 9.3 Success, warning, and error messaging

Use:

- `Toast`/`Sonner` for transient non-blocking success/info,
- `Alert` for inline durable warnings/errors,
- `Alert Dialog` for destructive confirmation or blocking failure acknowledgment.

Do not rely on:

- console logs,
- status text hidden far from the triggering action,
- native browser dialogs.

## 10. Dashboard Acceptance Checklist

Any substantive dashboard UI change should be checked against this list:

1. Uses semantic tokens instead of raw colors.
2. Uses existing local shadcn/ui wrappers or adds missing wrappers before custom components.
3. Keeps major tool screens lazily split.
4. Provides route-level pending/error handling for data-owning routes.
5. Uses Zod for route/form/API validation where inputs cross boundaries.
6. Avoids browser-native prompt/confirm/alert flows.
7. Keeps server-only logic out of client bundles.
8. Uses consistent empty/loading/error states.
9. Preserves shared Sidereal brand direction with the native client.
10. Updates this guide and `AGENTS.md` in the same change if a new enforceable frontend rule is introduced.

## 11. References

- [docs/ui_design_guide.md](/home/toby/dev/sidereal_v3/docs/ui_design_guide.md)
- [AGENTS.md](/home/toby/dev/sidereal_v3/AGENTS.md)

## 12. Change Log

- 2026-03-12: Initial frontend-specific dashboard styling, routing, validation, and code-splitting guide added. Native Bevy UI guidance remains in `docs/ui_design_guide.md`.
