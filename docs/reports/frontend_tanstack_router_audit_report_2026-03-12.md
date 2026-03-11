# Frontend TanStack Router Audit Report

**Date:** 2026-03-12
**Scope:** `dashboard/`
**Audited stack:** TanStack Start, TanStack Router, React 19, Vite 7, Tailwind 4, shadcn/ui, Radix UI, `nuqs`

## Executive Summary

The dashboard route tree is directionally correct: top-level tools are separated into `database`, `game-world`, `game-client`, and `shader-workshop`, and file-based routing is being used consistently. The main problems are underneath that surface.

The most serious issue is security. Several mutation and admin endpoints are effectively unauthenticated and some of them proxy privileged backend actions. The next biggest issue is bundle structure: the built client currently ships a `main-DRmwNXeB.js` shared chunk at roughly `1120 KB`, while many route chunks are only `4 KB`, which means the route graph is split more cleanly than the actual code graph. The third recurring problem is pattern drift: data loading, validation, error handling, and destructive actions are all implemented ad hoc instead of using consistent TanStack Router and shadcn patterns.

## Architecture and Code Organization Findings

### 1. Unauthenticated mutation routes expose privileged and destructive actions

- Severity: Critical
- Category: security
- Priority: must fix
- Files:
  - [dashboard/src/routes/api.admin.spawn-entity.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.admin.spawn-entity.tsx#L31)
  - [dashboard/src/routes/api.delete-entity.$entityId.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.delete-entity.$entityId.tsx#L65)
  - [dashboard/src/routes/api.graph.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.graph.tsx#L189)
  - [dashboard/src/routes/api.brp.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.brp.tsx#L66)
  - [dashboard/src/routes/api.database.accounts.$accountId.password-reset.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.database.accounts.$accountId.password-reset.tsx#L39)
  - [dashboard/src/routes/api.database.characters.$playerEntityId.display-name.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.database.characters.$playerEntityId.display-name.tsx#L42)
- Why it matters:
  - `POST /api/admin/spawn-entity` forwards a server-side bearer token to the gateway, but the dashboard route itself does not authenticate or authorize the caller first.
  - `DELETE /api/delete-entity/$entityId`, `POST /api/graph`, and `POST /api/brp` all mutate live or persisted state without any visible auth or CSRF boundary.
  - `POST /api/database/accounts/$accountId/password-reset` can trigger account recovery flow for arbitrary accounts without any route-level authorization.
- Concrete recommendation:
  - Add a shared server-side guard for all mutation routes before adding more admin features.
  - Require an authenticated dashboard session plus role/permission checks.
  - Add CSRF protection for cookie-backed auth and reject cross-origin mutation requests by default.
  - Split read-only inspection routes from mutation routes so permissions can be enforced per capability.
- Implementation sketch:

```ts
async function requireDashboardAdmin(request: Request) {
  const session = await getDashboardSession(request)
  if (!session || !session.permissions.includes('dashboard:admin')) {
    throw new Response('Forbidden', { status: 403 })
  }
  return session
}

POST: async ({ request }) => {
  await requireDashboardAdmin(request)
  // mutation logic
}
```

### 2. Password reset tokens are returned to the browser and rendered in-page

- Severity: High
- Category: security
- Priority: must fix
- Files:
  - [dashboard/src/routes/api.database.accounts.$accountId.password-reset.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.database.accounts.$accountId.password-reset.tsx#L90)
  - [dashboard/src/features/database/useDatabaseAdminData.ts](/home/toby/dev/sidereal_v3/dashboard/src/features/database/useDatabaseAdminData.ts#L42)
  - [dashboard/src/features/database/AccountsPanel.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/database/AccountsPanel.tsx#L211)
- Why it matters:
  - The API route returns `resetToken` to the client.
  - The UI then displays that token inline in a message string.
  - Even in an internal admin tool, recovery tokens should not be casually exposed to browser memory, UI surfaces, screenshots, logs, or accidental copy/paste paths.
- Concrete recommendation:
  - Keep the reset flow server-side.
  - Return only an accepted/sent status to the client unless there is a documented operational reason to reveal raw tokens.
  - If token display is required for local-only development, gate it behind an explicit development-only switch and never enable it by default.

### 3. The route tree is split logically, but the bundle graph is not

- Severity: High
- Category: bundle-size
- Priority: must fix
- Files:
  - [dashboard/dist/client/assets/main-DRmwNXeB.js](/home/toby/dev/sidereal_v3/dashboard/dist/client/assets/main-DRmwNXeB.js)
  - [dashboard/dist/client/assets/_dashboard.database-RNyNae9R.js](/home/toby/dev/sidereal_v3/dashboard/dist/client/assets/_dashboard.database-RNyNae9R.js)
  - [dashboard/dist/client/assets/_dashboard.game-client-Bniud_ar.js](/home/toby/dev/sidereal_v3/dashboard/dist/client/assets/_dashboard.game-client-Bniud_ar.js)
  - [dashboard/src/routes/_dashboard.database.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.database.tsx#L1)
  - [dashboard/src/routes/_dashboard.game-world.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.game-world.tsx#L1)
  - [dashboard/src/routes/_dashboard.shader-workshop.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.shader-workshop.tsx#L1)
  - [dashboard/src/routes/_dashboard.game-client.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.game-client.tsx#L1)
- Why it matters:
  - The build output shows a very large shared main chunk and tiny route wrappers.
  - The route files import heavy feature modules eagerly, so route wrappers are split but their real payload is hoisted into the shared chunk.
  - This weakens route-based code splitting and makes first-load cost much higher than necessary for a dashboard where many screens are niche.
- Concrete recommendation:
  - Move tool routes to lazy boundaries using `lazyRouteComponent` or route-level lazy modules.
  - Split large features into route-level entry files so `ExplorerWorkspace`, `ShaderWorkshopPage`, and the WASM boot paths do not land in the initial shared chunk.
  - Keep only shell/navigation code in the eagerly loaded dashboard path.
- Implementation sketch:

```tsx
import { createFileRoute, lazyRouteComponent } from '@tanstack/react-router'

export const Route = createFileRoute('/_dashboard/shader-workshop')({
  pendingComponent: ShaderWorkshopSkeleton,
  errorComponent: RouteErrorBoundary,
  component: lazyRouteComponent(
    () => import('@/routes-lazy/ShaderWorkshopRoutePage'),
  ),
})
```

### 4. Data loading is client-side effect driven instead of route-driven

- Severity: High
- Category: architecture
- Priority: should fix
- Files:
  - [dashboard/src/features/database/useDatabaseAdminData.ts](/home/toby/dev/sidereal_v3/dashboard/src/features/database/useDatabaseAdminData.ts#L16)
  - [dashboard/src/features/explorer/ExplorerWorkspace.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/explorer/ExplorerWorkspace.tsx#L572)
  - [dashboard/src/features/shaders/ShaderWorkshopPage.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/shaders/ShaderWorkshopPage.tsx#L375)
  - [dashboard/src/features/shaders/ShaderWorkshopPage.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/shaders/ShaderWorkshopPage.tsx#L539)
- Why it matters:
  - The dashboard is using TanStack Router and Start, but major screens fetch after mount with `useEffect` and local state.
  - That loses route-aware pending states, route-aware errors, preloading, SSR opportunities, and reuse of cached data across navigation.
  - It also forces each feature to reinvent loading/error behavior.
- Concrete recommendation:
  - Promote top-level screen data fetching into route loaders or server functions and use route-owned pending/error boundaries.
  - Use TanStack Query only where persistent client cache is genuinely useful; otherwise use route loaders with typed results.
  - Keep component-local fetches only for highly interactive operations, not initial page data.

### 5. Large monolithic modules are carrying too many concerns

- Severity: High
- Category: maintainability
- Priority: should fix
- Files:
  - [dashboard/src/features/explorer/ExplorerWorkspace.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/explorer/ExplorerWorkspace.tsx)
  - [dashboard/src/features/shaders/ShaderWorkshopPage.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/shaders/ShaderWorkshopPage.tsx)
  - [dashboard/src/components/sidebar/DetailPanel.tsx](/home/toby/dev/sidereal_v3/dashboard/src/components/sidebar/DetailPanel.tsx)
  - [dashboard/src/components/grid/GridCanvas.tsx](/home/toby/dev/sidereal_v3/dashboard/src/components/grid/GridCanvas.tsx)
- Why it matters:
  - `ExplorerWorkspace.tsx` is 1761 lines.
  - `ShaderWorkshopPage.tsx` is 1643 lines.
  - `DetailPanel.tsx` is 1106 lines.
  - `GridCanvas.tsx` is 947 lines.
  - These files mix route state, fetch orchestration, business rules, rendering, optimistic updates, and UI composition, which makes code splitting, testing, and local changes harder than they need to be.
- Concrete recommendation:
  - Split by responsibility, not by arbitrary size.
  - For explorer: separate route state, remote data adapter, graph transformation, mutation actions, and visual composition.
  - For shader workshop: separate catalog loading, selected-shader lifecycle, preview engine, uniform controls, and upload workflow.

## Routing and Boundary Findings

### 6. Route-level error and pending boundaries are mostly missing

- Severity: Medium
- Category: UX/resilience
- Priority: should fix
- Files:
  - [dashboard/src/routes/__root.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/__root.tsx#L17)
  - [dashboard/src/routes/_dashboard.database.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.database.tsx#L14)
  - [dashboard/src/routes/_dashboard.game-world.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.game-world.tsx#L4)
  - [dashboard/src/routes/_dashboard.game-client.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.game-client.tsx#L6)
  - [dashboard/src/routes/_dashboard.shader-workshop.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.shader-workshop.tsx#L4)
- Why it matters:
  - There is a root `errorComponent` and `notFoundComponent`, which is good.
  - The feature routes do not define their own `errorComponent` or `pendingComponent`, even though they own large, failure-prone async flows.
  - As a result, route-specific failures fall back to inline banners, console logging, or the root shell instead of focused recovery UI.
- Concrete recommendation:
  - Add route-level `errorComponent` and `pendingComponent` for `database`, `game-world`, `game-client`, and `shader-workshop`.
  - Move screen-level error banners into reusable route-aware boundary components.

### 7. Search params and route params are not using TanStack validation

- Severity: Medium
- Category: correctness
- Priority: should fix
- Files:
  - [dashboard/src/routes/_dashboard.database.accounts.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.database.accounts.tsx#L1)
  - [dashboard/src/routes/_dashboard.database.tables.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.database.tables.tsx#L1)
  - [dashboard/src/routes/_dashboard.game-world.$entityGuid.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.game-world.$entityGuid.tsx#L4)
  - [dashboard/src/routes/_dashboard.shader-workshop.$shaderId.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.shader-workshop.$shaderId.tsx#L4)
  - [dashboard/src/features/explorer/ExplorerWorkspace.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/explorer/ExplorerWorkspace.tsx#L221)
  - [dashboard/src/features/shaders/ShaderWorkshopPage.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/shaders/ShaderWorkshopPage.tsx#L132)
- Why it matters:
  - Params are read directly with `Route.useParams()`.
  - Query state is managed via `nuqs`, but the route definitions themselves do not validate route params or search shapes.
  - The result is fragmented parsing rules and weaker guarantees at the route boundary.
- Concrete recommendation:
  - Add `validateSearch` and typed param validation to the route definitions.
  - Use Zod schemas where there are real invariants.
- Implementation sketch:

```ts
const searchSchema = z.object({
  search: z.string().catch(''),
  sort: z.enum(['email', 'characters', 'created']).catch('email'),
})

export const Route = createFileRoute('/_dashboard/database/accounts')({
  validateSearch: searchSchema,
  component: AccountsRoute,
})
```

## Error Handling and Resilience Findings

### 8. Destructive and edit flows bypass the existing dialog primitives

- Severity: Medium
- Category: UX/resilience
- Priority: should fix
- Files:
  - [dashboard/src/features/database/AccountsPanel.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/database/AccountsPanel.tsx#L121)
  - [dashboard/src/components/sidebar/EntityTree.tsx](/home/toby/dev/sidereal_v3/dashboard/src/components/sidebar/EntityTree.tsx#L557)
  - [dashboard/src/components/ui/confirm-dialog.tsx](/home/toby/dev/sidereal_v3/dashboard/src/components/ui/confirm-dialog.tsx#L24)
- Why it matters:
  - Character rename uses `window.prompt`.
  - Entity deletion is one click with no confirmation dialog and only logs failures to the console.
  - A reusable `ConfirmDialog` already exists but is not used in the destructive flows, and it also swallows failures by logging them instead of surfacing them to the UI.
- Concrete recommendation:
  - Replace `window.prompt` with a real shadcn dialog.
  - Add `AlertDialog` confirmation for destructive actions.
  - Convert `ConfirmDialog` to take an `onError` callback or render inline failure state rather than only `console.error`.

### 9. Root fallback UIs bypass the design system and styling conventions

- Severity: Low
- Category: maintainability
- Priority: optional improvement
- Files:
  - [dashboard/src/router.tsx](/home/toby/dev/sidereal_v3/dashboard/src/router.tsx#L5)
  - [dashboard/src/routes/__root.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/__root.tsx#L42)
- Why it matters:
  - `defaultNotFoundComponent` is still just `<p>Not Found</p>`.
  - The root `notFoundComponent` and `errorComponent` use inline style objects rather than the dashboard’s Tailwind/shadcn conventions.
  - This creates visible inconsistency in exactly the states users see when something goes wrong.
- Concrete recommendation:
  - Replace inline fallback UI with shared shell-safe components using the same tokens as the rest of the app.

## Bundle Size and Code-Splitting Findings

### 10. Specialized tools are likely inflating the initial client path more than necessary

- Severity: Medium
- Category: bundle-size
- Priority: should fix
- Files:
  - [dashboard/src/components/shader-workbench/ShaderCodeEditor.tsx](/home/toby/dev/sidereal_v3/dashboard/src/components/shader-workbench/ShaderCodeEditor.tsx#L1)
  - [dashboard/src/lib/game-client-wasm.ts](/home/toby/dev/sidereal_v3/dashboard/src/lib/game-client-wasm.ts)
  - [dashboard/src/lib/shader-preview-wasm.ts](/home/toby/dev/sidereal_v3/dashboard/src/lib/shader-preview-wasm.ts)
  - [dashboard/dist/client/assets/main-DRmwNXeB.js](/home/toby/dev/sidereal_v3/dashboard/dist/client/assets/main-DRmwNXeB.js)
- Why it matters:
  - Shader editing pulls in `prismjs` and `use-editable`.
  - Game client and shader preview both have WASM bootstrap paths.
  - These are tool-specific costs and should stay behind route boundaries as much as possible.
- Concrete recommendation:
  - Keep code editor, preview engine, and WASM bootstraps behind lazy route entrypoints.
  - Consider dynamic-importing `ShaderCodeEditor` itself if the workshop layout can render before the editor is ready.

### 11. Dependency drift is making bundle and maintenance discipline weaker

- Severity: Medium
- Category: bundle-size
- Priority: should fix
- Files:
  - [dashboard/package.json](/home/toby/dev/sidereal_v3/dashboard/package.json#L17)
  - [dashboard/vite.config.ts](/home/toby/dev/sidereal_v3/dashboard/vite.config.ts#L18)
  - [dashboard/src/components/ui/slider.tsx](/home/toby/dev/sidereal_v3/dashboard/src/components/ui/slider.tsx#L1)
- Why it matters:
  - The codebase currently references `@tanstack/devtools-vite`, but `@tanstack/react-devtools` and `@tanstack/react-router-devtools` are present in `package.json` without usage in `src/`.
  - `@tanstack/react-router-ssr-query` is installed but not used.
  - `@radix-ui/react-popover`, `@radix-ui/react-select`, and `@radix-ui/react-dialog` are installed but not used in `src/`.
  - `slider.tsx` imports from the umbrella `radix-ui` package instead of `@radix-ui/react-slider`, which is inconsistent with the rest of the UI layer and keeps a redundant dependency in the graph.
- Concrete recommendation:
  - Remove unused runtime dependencies.
  - Standardize on the scoped Radix packages already used elsewhere.
  - Keep TanStack devtools dependencies only if there is an explicit roadmap to wire them in.

## UI Library / shadcn Findings

### 12. The `components/ui/` layer is mostly coherent, but important primitives are missing from real workflows

- Severity: Medium
- Category: maintainability
- Priority: should fix
- Files:
  - [dashboard/src/components/ui](/home/toby/dev/sidereal_v3/dashboard/src/components/ui)
  - [dashboard/src/features/database/AccountsPanel.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/database/AccountsPanel.tsx#L121)
  - [dashboard/src/components/sidebar/EntityTree.tsx](/home/toby/dev/sidereal_v3/dashboard/src/components/sidebar/EntityTree.tsx#L636)
- Why it matters:
  - Buttons, tabs, tooltips, badges, cards, dropdowns, and alert-dialog wrappers are in good shape.
  - But real edit/delete flows are still falling back to `window.prompt`, direct action buttons, and banner messages.
  - The local component inventory is missing the pieces needed to make these flows consistent, such as dialog-based forms, labels, textarea/form wiring, and better inline status primitives.
- Concrete recommendation:
  - Add only the shadcn components that map to existing pain points:
    - `dialog`
    - `label`
    - `textarea`
    - `form` if Zod validation is adopted
  - Do not add the full library blindly; add the minimum set that removes current bespoke UI behavior.

## Validation and Forms Findings

### 13. Validation is manual, duplicated, and weakly composable

- Severity: Medium
- Category: correctness
- Priority: should fix
- Files:
  - [dashboard/src/routes/api.graph.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.graph.tsx#L189)
  - [dashboard/src/routes/api.admin.spawn-entity.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.admin.spawn-entity.tsx#L34)
  - [dashboard/src/routes/api.shaders.upload.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.shaders.upload.tsx#L13)
  - [dashboard/src/routes/api.database.characters.$playerEntityId.display-name.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.database.characters.$playerEntityId.display-name.tsx#L47)
  - [dashboard/src/routes/api.database.accounts.$accountId.password-reset.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.database.accounts.$accountId.password-reset.tsx#L44)
- Why it matters:
  - Every route manually parses JSON, checks strings, and shapes responses.
  - That duplicates logic and makes it hard to share or audit validation rules.
  - The codebase is a good fit for Zod because it has lots of route params, query state, and JSON request bodies.
- Concrete recommendation:
  - Introduce a small `schemas/` area for route params, search shapes, and mutation bodies.
  - Use Zod first where the value is immediate:
    - account password reset params
    - character rename body
    - graph update body
    - shader upload body
    - database account/table search params

## API Route and Server Boundary Findings

### 14. API routes repeat the same transport and parsing patterns instead of sharing a server utility layer

- Severity: Medium
- Category: maintainability
- Priority: should fix
- Files:
  - [dashboard/src/routes/api.graph.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.graph.tsx#L189)
  - [dashboard/src/routes/api.database.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.database.tsx#L62)
  - [dashboard/src/routes/api.admin.spawn-entity.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.admin.spawn-entity.tsx#L34)
  - [dashboard/src/routes/api.delete-entity.$entityId.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.delete-entity.$entityId.tsx#L20)
  - [dashboard/src/server/postgres.ts](/home/toby/dev/sidereal_v3/dashboard/src/server/postgres.ts#L18)
- Why it matters:
  - JSON parsing, response shaping, error mapping, and connection helpers are repeated across routes.
  - `api.delete-entity.$entityId.tsx` even reimplements its own Postgres pool instead of using `src/server/postgres.ts`.
  - This makes it harder to add consistent auth, metrics, tracing, and error contracts later.
- Concrete recommendation:
  - Centralize:
    - auth guard
    - request parsing helpers
    - typed `ok/error` response helpers
    - shared Postgres access
    - gateway proxy helpers

### 15. `api/graph` accepts arbitrarily shaped values and writes them directly into Cypher

- Severity: Medium
- Category: security
- Priority: should fix
- Files:
  - [dashboard/src/routes/api.graph.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.graph.tsx#L62)
  - [dashboard/src/routes/api.graph.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.graph.tsx#L220)
- Why it matters:
  - The code does sanitize string literals reasonably, but it still allows loosely shaped `value` payloads and maps `typePath` to a dynamic property key.
  - That is fragile for a long-term admin surface and harder to reason about than a schema-driven whitelist.
  - This is more of a future hardening issue than a demonstrated injection bug today, but it is still the wrong direction for an eventually authenticated admin tool.
- Concrete recommendation:
  - Replace generic `unknown` body writes with schema-validated payloads per editable component family, or at least by a stricter editor registry contract.

## Security and Auth-Readiness Findings

### 16. The current structure will make later auth retrofitting harder than it needs to be

- Severity: Medium
- Category: security
- Priority: should fix
- Files:
  - [dashboard/src/features/explorer/ExplorerWorkspace.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/explorer/ExplorerWorkspace.tsx#L1029)
  - [dashboard/src/features/database/useDatabaseAdminData.ts](/home/toby/dev/sidereal_v3/dashboard/src/features/database/useDatabaseAdminData.ts#L21)
  - [dashboard/src/features/shaders/ShaderWorkshopPage.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/shaders/ShaderWorkshopPage.tsx#L408)
- Why it matters:
  - Client code talks to raw route URLs directly from many places.
  - There is no central API client layer to attach auth headers, CSRF tokens, request tracing, or standardized error handling.
  - Once auth is introduced, the migration will be broad and noisy because these calls are distributed across features.
- Concrete recommendation:
  - Introduce a small typed dashboard API client now, even before full auth.
  - Use that layer for all route requests so auth/session concerns land in one place later.

## Naming / Consistency Findings

### 17. Naming and folder structure are mostly sound, but there is visible style drift

- Severity: Low
- Category: maintainability
- Priority: optional improvement
- Files:
  - [dashboard/src/components/ui/slider.tsx](/home/toby/dev/sidereal_v3/dashboard/src/components/ui/slider.tsx#L1)
  - [dashboard/src/routes/__root.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/__root.tsx#L42)
  - [dashboard/src/routes/_dashboard.index.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.index.tsx#L37)
- Why it matters:
  - Most files use the same naming conventions and route layout.
  - But `slider.tsx` is formatted differently from the rest of the codebase and imports from a different dependency family.
  - Root error/not-found views use inline styles while the rest of the app uses Tailwind and shadcn wrappers.
  - Placeholder copy and health cards are mixed into the same navigation as real tools, which makes “current” versus “future” states a little blurry.
- Concrete recommendation:
  - Normalize the few outliers rather than doing a broad style rewrite.

## Route Architecture Map

### Current route tree

- `__root`
- `/_dashboard`
- `/_dashboard/`
- `/_dashboard/database`
- `/_dashboard/database/`
- `/_dashboard/database/accounts`
- `/_dashboard/database/tables`
- `/_dashboard/database/$entityGuid`
- `/_dashboard/game-world`
- `/_dashboard/game-world/$entityGuid`
- `/_dashboard/game-client`
- `/_dashboard/shader-workshop`
- `/_dashboard/shader-workshop/$shaderId`
- `/_dashboard/script-editor`
- `/_dashboard/settings`
- `/shader-workbench` -> redirect to `/shader-workshop`
- API routes under `/api/*`

### Assessment

- Good:
  - Top-level tool routing is logical and matches user workflows.
  - Slug routes for selected entity/shader are sensible.
  - `database` and `game-world` are correctly not sharing one route module anymore.
- Weak:
  - Real feature boundaries still live inside very large shared components.
  - Route modules are thin wrappers, but the heavy work is not isolated behind true lazy entrypoints.
  - Route-specific error/pending boundaries are mostly absent.

## Code-Splitting and Bundle-Reduction Plan

### Quick wins

- Make `game-world`, `database`, `shader-workshop`, and `game-client` route components lazy.
- Move `ExplorerWorkspace` and `ShaderWorkshopPage` behind route-local lazy imports.
- Remove unused dependencies from [dashboard/package.json](/home/toby/dev/sidereal_v3/dashboard/package.json#L17).
- Replace `radix-ui` umbrella import in [dashboard/src/components/ui/slider.tsx](/home/toby/dev/sidereal_v3/dashboard/src/components/ui/slider.tsx#L1).

### Medium-effort improvements

- Split `ExplorerWorkspace` into:
  - route state adapter
  - data loader/mutation hook
  - map/tree/detail composition
- Split `ShaderWorkshopPage` into:
  - catalog route loader
  - selected shader loader
  - preview engine
  - upload flow
  - editor shell
- Add a shared dashboard API client to reduce duplicated fetch code.

### Architectural refactors

- Move initial data ownership to route loaders or server functions.
- Add feature-local error and pending boundaries.
- Treat WASM-heavy tools as opt-in routes, not shared application cost.

## shadcn/ui Adoption Review

### Current strengths worth keeping

- The existing `button`, `badge`, `card`, `tabs`, `tooltip`, `dropdown-menu`, `input`, and `alert-dialog` wrappers are coherent.
- The shell and most feature pages already use the local UI layer instead of raw primitives.

### Components that should be added or used more consistently

- `dialog`
- `label`
- `textarea`
- `form` if Zod-backed validation is introduced

### Custom components that should remain custom

- `DataTable`
  - Keep only if you want the current bespoke interaction model.
  - Otherwise evaluate replacing it with the standard shadcn TanStack Table pattern later.
- `GridCanvas`
- BRP editor component family
- shader preview controls

## Validation Strategy

### Introduce Zod first in these places

- `api.admin.spawn-entity`
- `api.graph`
- `api.shaders.upload`
- `api.database.characters.$playerEntityId.display-name`
- `api.database.accounts.$accountId.password-reset`

### Add TanStack Router validation here

- database account/table search params
- explorer query-state params
- shader workshop search params
- `$entityGuid` and `$shaderId` route params

### Manual parsing is still acceptable for

- thin coercion of trivial one-field query strings
- low-level server utility internals after a validated outer boundary already exists

## Security / Auth-Readiness Hardening Sequence

1. Add a shared auth/authorization guard for all mutation routes.
2. Remove reset-token exposure from client responses.
3. Add CSRF protection for mutation endpoints.
4. Centralize dashboard API calling so auth/session logic has one insertion point.
5. Split read-only and mutating capabilities into separate route groups or handler helpers.
6. Add request logging/tracing for admin mutations once the auth boundary exists.

## API Route Optimization Review

### Current inventory

- `GET|POST /api/brp`
- `GET|POST /api/graph`
- `GET /api/database`
- `POST /api/database/accounts/$accountId/password-reset`
- `POST /api/database/characters/$playerEntityId/display-name`
- `POST /api/admin/spawn-entity`
- `DELETE /api/delete-entity/$entityId`
- `GET /api/shaders`
- `GET /api/shaders/$shaderId`
- `POST /api/shaders/upload`

### Repeated patterns worth centralizing

- JSON request parsing
- error-to-response mapping
- auth/permission checks
- Postgres access
- gateway proxying
- typed response helpers

### Performance and boundary recommendations

- Move shared database helpers out of route files.
- Reuse `src/server/postgres.ts` everywhere.
- Use route loaders for initial read models instead of per-feature client fetch orchestration.

## Naming / Style Consistency Review

### Strengths worth keeping

- File-based route naming is consistent.
- Feature folders are understandable.
- Shared UI imports use `@/components/ui/*` consistently.

### Conventions to standardize

- No browser-native prompt/confirm flows for real product interactions.
- No inline-styled error/not-found pages in a Tailwind/shadcn app.
- No mixed Radix package styles.
- No feature routes that eagerly import entire tool implementations by default.

## Recommendations List

### 1. Lock down all mutation routes first

- Files:
  - [dashboard/src/routes/api.admin.spawn-entity.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.admin.spawn-entity.tsx)
  - [dashboard/src/routes/api.delete-entity.$entityId.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.delete-entity.$entityId.tsx)
  - [dashboard/src/routes/api.graph.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.graph.tsx)
  - [dashboard/src/routes/api.brp.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.brp.tsx)
  - [dashboard/src/routes/api.database.accounts.$accountId.password-reset.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.database.accounts.$accountId.password-reset.tsx)
  - [dashboard/src/routes/api.database.characters.$playerEntityId.display-name.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.database.characters.$playerEntityId.display-name.tsx)
- Type: structural
- Recommendation:
  - Add `requireDashboardAdmin()` and use it everywhere before mutating state.

### 2. Turn top-level tools into real lazy routes

- Files:
  - [dashboard/src/routes/_dashboard.database.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.database.tsx)
  - [dashboard/src/routes/_dashboard.game-world.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.game-world.tsx)
  - [dashboard/src/routes/_dashboard.shader-workshop.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.shader-workshop.tsx)
  - [dashboard/src/routes/_dashboard.game-client.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.game-client.tsx)
- Type: structural
- Recommendation:
  - Introduce lazy route entrypoints so the shell is eager but tool implementations are not.

### 3. Move initial screen data into route-owned loading

- Files:
  - [dashboard/src/features/database/useDatabaseAdminData.ts](/home/toby/dev/sidereal_v3/dashboard/src/features/database/useDatabaseAdminData.ts)
  - [dashboard/src/features/explorer/ExplorerWorkspace.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/explorer/ExplorerWorkspace.tsx)
  - [dashboard/src/features/shaders/ShaderWorkshopPage.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/shaders/ShaderWorkshopPage.tsx)
- Type: structural
- Recommendation:
  - Shift initial read data to route loaders or server functions and let route boundaries own pending/error states.

### 4. Add Zod schemas at the route boundary

- Files:
  - [dashboard/src/routes/api.graph.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.graph.tsx)
  - [dashboard/src/routes/api.shaders.upload.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.shaders.upload.tsx)
  - [dashboard/src/routes/api.admin.spawn-entity.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/api.admin.spawn-entity.tsx)
  - [dashboard/src/routes/_dashboard.database.accounts.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.database.accounts.tsx)
  - [dashboard/src/routes/_dashboard.database.tables.tsx](/home/toby/dev/sidereal_v3/dashboard/src/routes/_dashboard.database.tables.tsx)
- Type: structural
- Recommendation:
  - Add a shared `src/lib/schemas/` or `src/schemas/` folder and validate params, search, and mutation bodies there.

### 5. Replace prompt/delete ad hoc flows with real dialogs

- Files:
  - [dashboard/src/features/database/AccountsPanel.tsx](/home/toby/dev/sidereal_v3/dashboard/src/features/database/AccountsPanel.tsx)
  - [dashboard/src/components/sidebar/EntityTree.tsx](/home/toby/dev/sidereal_v3/dashboard/src/components/sidebar/EntityTree.tsx)
  - [dashboard/src/components/ui/confirm-dialog.tsx](/home/toby/dev/sidereal_v3/dashboard/src/components/ui/confirm-dialog.tsx)
- Type: local cleanup
- Recommendation:
  - Use `AlertDialog` for delete and a proper `Dialog` for rename/edit flows.

## Prioritized Remediation Plan

1. Add auth/authorization and CSRF boundaries for all mutation routes.
2. Stop returning reset tokens to the browser.
3. Lazy-load the top-level tool routes and verify bundle reduction against the current `main-DRmwNXeB.js` baseline.
4. Split `ExplorerWorkspace` and `ShaderWorkshopPage` into route-level entrypoints plus smaller internal modules.
5. Introduce Zod schemas for route params, search params, and JSON bodies.
6. Move initial data loading into route loaders or server functions with route-level `pendingComponent` and `errorComponent`.
7. Replace prompt/delete ad hoc interactions with shadcn dialogs.
8. Remove unused dependencies and normalize the UI layer imports.

## Specific Confirm / Refute Summary

- The route tree is logically split by feature and workflow: confirmed.
- Route-level error handling is consistently applied where it should be: refuted.
- The app is leaving meaningful bundle-size savings on the table: confirmed.
- Specialized dashboard screens should be split more aggressively: confirmed.
- The current `components/ui/` layer is being used consistently: mostly confirmed, with important workflow gaps.
- There are custom components that should be replaced by shadcn primitives: partially confirmed.
- Search params, route params, and forms need a stronger validation story with Zod: confirmed.
- API routes are doing more work than necessary or are shaped inconsistently: confirmed.
- The server/client boundary is clean enough to prevent accidental bundle bloat: refuted.
- Current patterns would make future auth hardening more difficult: confirmed.
- The lint/format/type setup is sufficient to enforce intended conventions: partially refuted.
- The app is underusing modern TanStack Router / Start capabilities in ways that materially matter: confirmed.
