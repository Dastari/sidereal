# Frontend Dashboard Remediation Plan

Status: New plan created from `docs/reports/frontend_tanstack_router_audit_report_2026-03-12.md`  
Date: 2026-03-12  
Scope: `dashboard/` full frontend remediation, redesign alignment, security hardening, and architecture cleanup

Primary references:
- `docs/reports/frontend_tanstack_router_audit_report_2026-03-12.md`
- `docs/frontend_ui_styling_guide.md`
- `docs/ui_design_guide.md`
- `AGENTS.md`
- `dashboard/package.json`

## 1. Purpose

This plan turns the frontend audit findings into an execution-ready remediation sequence for the dashboard/web frontend.

The goals are:

1. Bring the dashboard under one coherent frontend standard for theming, routing, validation, shadcn/ui usage, and bundle discipline.
2. Close the current security gaps around mutation/admin routes and future auth-readiness.
3. Reduce first-load cost by fixing route/code splitting and removing unnecessary bundle weight.
4. Replace inconsistent or fragile interaction patterns with durable shadcn-based workflows.
5. Break up oversized feature modules so future work becomes cheaper and safer.
6. Align the dashboard with the new frontend styling guide while preserving shared Sidereal branding with the native client.

## 2. Constraints and Non-Negotiables

All work under this plan must respect:

1. `docs/frontend_ui_styling_guide.md` for dashboard/frontend UI standards.
2. `docs/ui_design_guide.md` for shared brand/theme direction with the native client.
3. `AGENTS.md` for contributor rules and same-change documentation updates.
4. Existing server-authoritative gameplay and replication invariants; dashboard work must not bypass them.
5. The fact that the dashboard is an internal/admin surface today but must be built as if future auth and public exposure are real.

## 3. Remediation Strategy

Do not attempt this as a single large PR.

The work should be delivered in this order:

1. Standards and infrastructure guardrails
2. Security and mutation-route hardening
3. Route/data-loading architecture
4. Bundle splitting and dependency cleanup
5. Shared UI system and theme cleanup
6. Feature-by-feature dashboard refactors
7. Tests, verification, and residual cleanup

This order matters:

1. If security is delayed, new UI work may pile more unsafe routes on top of current debt.
2. If route/data boundaries are not fixed early, redesign work will continue embedding fetch and error logic in the wrong places.
3. If bundle splitting is delayed until the end, feature refactors can accidentally preserve the current main-chunk problem.

## 4. Workstream A: Standards Foundation

### Objective

Make the new frontend standards operational before large implementation changes land.

### Actions

1. Establish the dashboard UI guide as the web source of truth:
   - `docs/frontend_ui_styling_guide.md`
2. Mirror enforceable rules in `AGENTS.md`:
   - shadcn-first component policy,
   - semantic theme token usage,
   - lazy major tool routes,
   - route-level boundaries,
   - Zod validation at route/API boundaries,
   - no browser-native prompt/confirm flows,
   - auth-ready mutation route structure.
3. Create a small implementation checklist for dashboard PRs, derived from the new guide.
4. Decide on the canonical location for:
   - shared dashboard API client,
   - shared Zod schemas,
   - route boundary components,
   - loading/empty/error states.

### Deliverables

1. Guide and AGENTS updates.
2. New shared frontend folders or modules if needed:
   - `dashboard/src/lib/api/`
   - `dashboard/src/lib/schemas/`
   - `dashboard/src/components/feedback/`
   - `dashboard/src/routes-lazy/`

## 5. Workstream B: Security and Auth-Readiness

### Objective

Close the highest-severity audit findings first.

### Findings addressed

1. Unauthenticated mutation routes.
2. Password reset token exposure.
3. Mutation routes not structured around a shared guard point.
4. No clean insertion point for future auth/CSRF.

### Actions

#### B1. Add a shared dashboard auth/authorization guard

Target files:

- `dashboard/src/routes/api.admin.spawn-entity.tsx`
- `dashboard/src/routes/api.delete-entity.$entityId.tsx`
- `dashboard/src/routes/api.graph.tsx`
- `dashboard/src/routes/api.brp.tsx`
- `dashboard/src/routes/api.database.accounts.$accountId.password-reset.tsx`
- `dashboard/src/routes/api.database.characters.$playerEntityId.display-name.tsx`

Tasks:

1. Introduce a shared server-side guard/helper for dashboard mutations.
2. Centralize permission checks instead of inline ad hoc logic.
3. Make the contract CSRF-ready even if full session auth is not implemented in the same change.

#### B2. Remove sensitive token exposure

Target files:

- `dashboard/src/routes/api.database.accounts.$accountId.password-reset.tsx`
- `dashboard/src/features/database/useDatabaseAdminData.ts`
- `dashboard/src/features/database/AccountsPanel.tsx`

Tasks:

1. Stop returning raw reset tokens to the browser by default.
2. Return accepted/sent state only.
3. Replace token-display messaging with safe confirmation feedback.

#### B3. Separate read and write capabilities

Tasks:

1. Classify dashboard routes and API routes as:
   - read-only,
   - operator write,
   - admin write.
2. Use that classification to guide route guards and later navigation visibility.

### Definition of done

1. No privileged mutation route remains unauthenticated by design.
2. No password-reset token is rendered into browser UI by default.
3. New mutation work has one obvious place to attach auth and CSRF.

## 6. Workstream C: Route Ownership, Data Loading, and Validation

### Objective

Move the dashboard onto consistent TanStack Router/TanStack Start patterns.

### Findings addressed

1. Initial data fetched in client `useEffect`.
2. Missing route-level pending/error boundaries.
3. Missing route/search/body validation.
4. Distributed raw `fetch` usage.

### Actions

#### C1. Move initial read models to route-owned loading

Target areas:

- database screens
- shader workshop
- game world explorer
- game client boot status where appropriate

Tasks:

1. Convert first-render data paths from feature-local `useEffect` fetches to route loaders or server functions.
2. Keep local mutations and refreshes as feature hooks only when justified.
3. Introduce route-owned `pendingComponent` and `errorComponent` for tool routes.

#### C2. Introduce a shared dashboard API client

Tasks:

1. Wrap request execution, error mapping, and typed JSON parsing.
2. Ensure all client features move off scattered raw `fetch`.
3. Reserve a central place for future auth/session/CSRF handling.

#### C3. Add Zod schemas at boundaries

Priority targets:

- `api.admin.spawn-entity`
- `api.graph`
- `api.shaders.upload`
- `api.database.characters.$playerEntityId.display-name`
- `api.database.accounts.$accountId.password-reset`
- database search params
- shader workshop search params
- explorer query state and slug parsing where practical

### Definition of done

1. Initial screen data is route-owned for the main tools.
2. Data-owning routes have route-level pending/error boundaries.
3. New or updated route/API boundaries use Zod instead of repeated manual validation.

## 7. Workstream D: Code Splitting and Bundle Discipline

### Objective

Reduce the oversized shared main chunk and restore meaningful route-level splitting.

### Findings addressed

1. `dashboard/dist/client/assets/main-DRmwNXeB.js` is roughly `1120 KB`.
2. Tool route wrappers are split, but heavy feature code is still shared eagerly.
3. Specialized tools likely inflate first load more than necessary.

### Actions

#### D1. Make major tools true lazy route boundaries

Target routes:

- `dashboard/src/routes/_dashboard.database.tsx`
- `dashboard/src/routes/_dashboard.game-world.tsx`
- `dashboard/src/routes/_dashboard.shader-workshop.tsx`
- `dashboard/src/routes/_dashboard.game-client.tsx`

Tasks:

1. Move heavy tool screens behind route-level lazy entrypoints.
2. Keep the shell, nav, and theme bootstrap in the eager path only.
3. Add route-level skeletons for lazy transitions.

#### D2. Dynamic-import the heaviest sub-features where justified

Target candidates:

- shader editor code surface
- shader preview bridge/bootstrap
- game client WASM bootstrap
- BRP editor registries and specialized editors

Tasks:

1. Audit which route-local modules are still too heavy after route splitting.
2. Add sub-splits only where they produce real savings.

#### D3. Remove unused or inconsistent dependencies

Likely candidates from audit:

- `@tanstack/react-devtools`
- `@tanstack/react-router-devtools`
- `@tanstack/react-router-ssr-query`
- unused Radix packages
- `radix-ui` umbrella package if fully replaced by scoped imports

### Definition of done

1. Main shared client chunk is materially smaller than the audit baseline.
2. Entering specialized tools does not force their entire runtime into first load.
3. Dependency graph better matches actual usage.

## 8. Workstream E: Shared UI System and Theme Cleanup

### Objective

Standardize the dashboard around semantic theme tokens and shadcn-based workflows.

### Findings addressed

1. Native browser prompt/confirm flows.
2. Inconsistent fallback pages.
3. Missing use of available shadcn components in real workflows.
4. Style and dependency drift.

### Actions

#### E1. Add missing local shadcn wrappers

Priority additions:

- `dialog`
- `label`
- `textarea`
- `field`
- `empty`
- `spinner`
- `skeleton`
- `toast` or `sonner`

Only add wrappers the dashboard actually needs in near-term work.

#### E2. Create standard dashboard feedback primitives

Tasks:

1. Shared inline error/warning/info blocks using `Alert`.
2. Shared blocking confirmation flow using `Alert Dialog`.
3. Shared empty state.
4. Shared route skeletons for tool routes.

#### E3. Remove inline style fallback UI

Target files:

- `dashboard/src/router.tsx`
- `dashboard/src/routes/__root.tsx`

Tasks:

1. Replace inline-styled not-found and error pages with theme-token-driven components.
2. Keep SSR-safe theme bootstrap only where actually necessary.

### Definition of done

1. Dashboard UI uses theme tokens consistently.
2. Prompt/confirm/alert browser APIs are gone from real flows.
3. Loading, empty, warning, and error states look and behave consistently.

## 9. Workstream F: Feature Refactors

### Objective

Refactor the highest-complexity features into smaller, safer modules.

### Findings addressed

1. Monolithic feature files.
2. Mixed route/data/render/mutation logic.
3. Hard-to-split code blocking bundle and UX improvements.

### Actions

#### F1. Database tool

Targets:

- `dashboard/src/routes/_dashboard.database.tsx`
- `dashboard/src/features/database/useDatabaseAdminData.ts`
- `dashboard/src/features/database/AccountsPanel.tsx`
- `dashboard/src/features/database/TablesPanel.tsx`

Tasks:

1. Split route ownership from feature UI composition.
2. Move initial read loading to route layer.
3. Replace rename prompt with dialog workflow.
4. Replace inline token display with safe toasts/alerts.

#### F2. Explorer / game world tool

Targets:

- `dashboard/src/features/explorer/ExplorerWorkspace.tsx`
- `dashboard/src/components/sidebar/EntityTree.tsx`
- `dashboard/src/components/sidebar/DetailPanel.tsx`
- `dashboard/src/components/grid/GridCanvas.tsx`

Tasks:

1. Split explorer state orchestration from rendering/layout.
2. Create dedicated mutation helpers for:
   - graph updates,
   - BRP updates,
   - deletion,
   - spawn,
   - owner assignment.
3. Add confirmation UX for destructive operations.
4. Keep BRP/server-only concerns isolated from client rendering concerns.

#### F3. Shader workshop

Targets:

- `dashboard/src/features/shaders/ShaderWorkshopPage.tsx`
- `dashboard/src/components/shader-workbench/ShaderCodeEditor.tsx`
- `dashboard/src/lib/shader-preview-wasm.ts`
- `dashboard/src/lib/shader-workbench.server.ts`

Tasks:

1. Split route load, catalog state, shader load state, preview engine, and upload flows.
2. Keep Prism/editor cost inside the shader route boundary.
3. Add proper skeleton and error boundaries.

#### F4. Game client route

Targets:

- `dashboard/src/routes/_dashboard.game-client.tsx`
- `dashboard/src/lib/game-client-wasm.ts`

Tasks:

1. Keep game client boot isolated from the shared shell bundle.
2. Provide a proper loading/error surface using shared feedback primitives.

### Definition of done

1. No major tool is still anchored by one oversized multi-concern module.
2. Feature workflows match the new frontend guide.

## 10. Workstream G: Testing and Verification

### Objective

Raise confidence while refactoring.

### Actions

1. Add targeted tests for:
   - route param/search validation,
   - shared API client behavior,
   - mutation error handling,
   - dialog/toast/empty-state rendering where practical,
   - route loader error/pending behavior where practical.
2. Add build verification for bundle splitting:
   - compare main/shared chunk size before and after major route splitting work.
3. Run dashboard lint/test/build verification after each major workstream.

Suggested dashboard commands:

```bash
pnpm --dir dashboard lint
pnpm --dir dashboard test
pnpm --dir dashboard build
```

If additional checks are introduced, document them in the same change.

## 11. Execution Sequence

Recommended PR sequence:

1. Standards/docs foundation:
   - new frontend guide
   - AGENTS update
   - plan document
2. Security hardening:
   - mutation route guard point
   - token exposure removal
3. Shared frontend infrastructure:
   - API client
   - Zod schema layer
   - route boundary primitives
4. Route splitting and bundle cleanup:
   - lazy route entrypoints
   - dependency cleanup
5. Database tool refactor
6. Explorer/game world refactor
7. Shader workshop refactor
8. Game client route cleanup
9. Residual styling/consistency follow-through

## 12. Risks and Watchouts

1. Security work may expose assumptions in local-only operator flows; document temporary constraints explicitly.
2. Route-loader conversion can change hydration and navigation timing; verify SSR/client behavior carefully.
3. Route splitting can hide shared dependencies in unexpected chunks; verify actual build output instead of assuming.
4. Explorer and shader workshop refactors are large enough that they should be split by concern, not done as one rewrite.
5. New shadcn wrappers should be added selectively; do not bulk-import the entire catalog without active usage.

## 13. Success Criteria

This plan is complete when:

1. The dashboard follows `docs/frontend_ui_styling_guide.md` in practice, not just in documentation.
2. Privileged mutation flows are auth-ready and no longer expose sensitive tokens in the browser.
3. Major dashboard tools load through real lazy route boundaries.
4. Initial data loading is route-owned for the primary tools.
5. Zod or equivalent strict schema validation exists at meaningful dashboard boundaries.
6. Destructive and edit interactions use proper shadcn dialogs and feedback patterns.
7. Large feature modules have been decomposed enough that ownership is obvious and future work no longer depends on editing thousand-line files.

## 14. Change Log

- 2026-03-12: Initial plan created from the frontend audit report and new frontend styling guide.
