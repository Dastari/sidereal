# Frontend TanStack Router Audit Prompt

**Status:** Active Prompt
**Date:** 2026-03-12
**Scope Note:** In this repository, the frontend TanStack Router app lives under `dashboard/`.

Perform a complete frontend audit for the `dashboard/` application as a senior TanStack Start / TanStack Router / React 19 / Vite 7 engineer with strong experience in SSR, route architecture, bundle-size reduction, shadcn/ui, validation, API-route hardening, and frontend security.

Your job is not to give generic React advice. Your job is to inspect this specific codebase and produce a concrete audit focused on consistency, maintainability, resilience, performance, and future-proofing.

The audit must generate a detailed written findings report in `docs/reports/`, not just inline output.

## Context

- This repository is Sidereal.
- The frontend app to audit is `dashboard/`.
- The dashboard currently uses:
  - `@tanstack/react-router`
  - `@tanstack/react-start`
  - Vite 7
  - React 19
  - Tailwind CSS 4
  - shadcn/ui with `new-york` style
  - Radix UI primitives
  - `nuqs` for URL/query-state management
- File-based routes live under `dashboard/src/routes/`.
- API route handlers also live under `dashboard/src/routes/` using `api.*.tsx` route files.
- Shared UI wrappers live under `dashboard/src/components/ui/`.
- App/server-specific logic exists under:
  - `dashboard/src/components/`
  - `dashboard/src/features/`
  - `dashboard/src/hooks/`
  - `dashboard/src/lib/`
  - `dashboard/src/server/`
- Relevant local references include:
  - `AGENTS.md`
  - `docs/ui_design_guide.md`
  - `dashboard/package.json`
  - `dashboard/components.json`
  - `dashboard/vite.config.ts`
  - `dashboard/eslint.config.js`
  - `dashboard/prettier.config.js`
  - `dashboard/tsconfig.json`

## Primary Goals

Find and document all meaningful issues, risks, inconsistencies, anti-patterns, and missed opportunities across the dashboard frontend, especially around:

1. Consistent coding style and architectural patterns.
2. Correct and consistent use of route-level and component-level error boundaries.
3. Logical route splitting and code splitting.
4. Bundle-size reduction and dependency discipline.
5. Taking full advantage of modern TanStack Router, TanStack Start, React 19, and Vite features where justified.
6. Better use of shadcn/ui components and patterns where applicable.
7. Introducing or expanding Zod-based validation for forms, search params, route params, and API payloads.
8. API route optimization, request validation, and server/client boundary hygiene.
9. Security issues, including future auth-readiness concerns even if auth is not fully implemented yet.
10. Consistent naming conventions, file organization, and component/module boundaries.

## Audit Scope

Audit all meaningful frontend concerns in `dashboard/`, including but not limited to:

1. Route tree design and route module composition.
2. Layout boundaries, nested routes, and feature ownership.
3. Error handling, not-found handling, pending/loading behavior, and recovery UX.
4. SSR/client boundaries and TanStack Start server-route usage.
5. API route handler design, validation, and performance.
6. Component patterns, hooks, local utilities, and feature-module boundaries.
7. UI library usage, duplicated primitives, and shadcn/ui coverage gaps.
8. URL-state and navigation-state patterns.
9. Bundle composition, lazy loading, route chunking, and heavy dependency usage.
10. Vite configuration, build behavior, and production/runtime optimization opportunities.
11. Naming, import hygiene, code style consistency, and enforceability through tooling.
12. Security, unsafe input handling, unsafe output rendering, and future auth/session concerns.
13. Testing gaps that make frontend regressions or refactors risky.

## Audit For All Of The Following

### 1. Consistency and codebase hygiene

Find:

1. Inconsistent component/file naming patterns.
2. Inconsistent route naming, route ownership, or folder/file layout.
3. Mixed architectural styles within similar features.
4. Repeated custom patterns where a single shared abstraction should exist.
5. Ad hoc state management or utility patterns that differ unnecessarily between features.
6. Inconsistent import style, alias usage, barrel usage, or module boundaries.
7. Places where linting/formatting/type tooling is too weak to enforce the intended conventions.

### 2. TanStack Router / TanStack Start usage quality

Review whether the codebase is making strong use of modern TanStack capabilities where appropriate, including:

1. route-level `errorComponent`,
2. `notFoundComponent`,
3. `pendingComponent`,
4. Suspense-friendly loading boundaries,
5. route-level validation,
6. route-level search-param handling,
7. route-level preloading or prefetch opportunities,
8. route/module lazy loading,
9. clearer loader/server-function boundaries if relevant,
10. route composition that matches feature boundaries rather than growing monolithic route files.

Call out:

1. routes that should be split further,
2. routes that are too fragmented,
3. route modules doing too much UI/data/server work at once,
4. missing route boundary behavior that should exist consistently,
5. places where TanStack Router features are underused or misused,
6. places where current patterns are good and should be kept.

### 3. Error boundaries and resilience

Audit for:

1. missing route-level error boundaries,
2. inconsistent error rendering between routes,
3. missing recovery actions or reset flows,
4. server errors being surfaced in weak or inconsistent ways,
5. missing not-found handling,
6. missing loading/pending states,
7. places where failures are only logged or silently swallowed,
8. places where a localized boundary should exist around heavy/fragile UI.

Be explicit about whether current root-level error handling is sufficient or whether feature/route-level boundaries are missing.

### 4. Bundle size, code splitting, and lazy loading

Focus heavily on reducing bundle size and improving logical chunking.

Find:

1. routes that should be lazy-loaded,
2. feature modules that are always bundled but only used in specialized screens,
3. heavy dependencies that may not need to be in initial client bundles,
4. server-only dependencies or code paths that risk leaking into client bundles,
5. editor/workbench/debug/database tooling that should be split from core dashboard paths,
6. icon/library imports that may inflate bundles,
7. unnecessary global CSS or runtime work,
8. places where route-level chunking does not align with user navigation patterns,
9. opportunities for dynamic imports, route-level splits, or stronger server/client isolation,
10. Vite/build opportunities that would materially reduce JS shipped or improve caching.

Do not recommend manual chunking unless it is justified for this codebase. Prefer structural fixes first.

### 5. Modern React 19 usage

Evaluate whether the app is taking reasonable advantage of modern React features where they improve the codebase, such as:

1. Suspense boundaries,
2. transition-friendly navigation or pending UI,
3. server/client boundary clarity,
4. avoiding unnecessary effect-driven data flow,
5. avoiding stale legacy patterns or defensive ceremony that no longer makes sense,
6. avoiding premature memoization or indirection that harms readability more than it helps.

Do not give vague “use React best practices” advice. Tie every recommendation to specific files and specific payoff.

### 6. shadcn/ui and component-library usage

Audit whether the dashboard is making disciplined use of shadcn/ui and Radix-based primitives.

Find:

1. custom UI code that should probably use an existing shadcn/ui component,
2. duplicated wrapper components,
3. inconsistent styling or interaction behavior across similar controls,
4. places where extending existing shadcn components would be better than inventing bespoke patterns,
5. missing primitives that should likely be added to the local `components/ui/` library,
6. cases where custom components are justified and should remain custom.

Be explicit about whether the current `components/ui/` layer is coherent or drifting.

### 7. Validation strategy, including Zod adoption

Specifically look for opportunities to introduce or standardize Zod-based validation for:

1. form inputs,
2. route params,
3. search params,
4. API request bodies,
5. API query strings,
6. API responses where schema validation would improve safety,
7. normalization/parsing layers currently implemented manually.

Call out:

1. places currently doing manual parsing/validation that should move to Zod,
2. places where TanStack Router validation features are not being used,
3. places where user input can move through the system without clear schema validation,
4. where Zod would improve both correctness and security,
5. where adding Zod would be unnecessary ceremony.

### 8. API routes and server/client boundary optimization

Audit the file-based API routes and surrounding server utilities for:

1. duplicated request parsing,
2. duplicated response shaping,
3. missing validation,
4. inefficient database access patterns,
5. repeated server setup work that could be centralized,
6. serialization overhead,
7. over-fetching or unnecessary round trips,
8. routes doing too much work synchronously,
9. route handlers mixing transport concerns, business logic, and SQL too tightly,
10. client code that should remain client-only versus logic that should move server-side,
11. caching opportunities where safe and meaningful,
12. logging or error behavior that is too weak or too noisy.

Also verify that server-only modules and packages are isolated cleanly enough that the client build is not accidentally paying for them.

### 9. Security and future auth-readiness

Even if full auth is not implemented yet, audit for security weaknesses and missing foundations that will matter later.

Inspect for:

1. unsanitized or weakly validated input,
2. SQL-injection risk,
3. XSS risk,
4. unsafe HTML injection,
5. weak route/API trust boundaries,
6. accidental exposure of internal errors or sensitive metadata,
7. weak handling of destructive/admin actions,
8. assumptions that would break once auth and sessions are added,
9. missing CSRF considerations for mutation endpoints,
10. missing authorization boundaries in route/API design,
11. environment or secret exposure risk,
12. missing rate-limit or abuse-readiness considerations for future public deployment.

Do not simply say “add auth.” Identify where the current structure will make future auth/authorization harder or riskier.

### 10. Naming, file organization, and feature boundaries

Find:

1. components or hooks that live in the wrong layer,
2. feature modules that should be split or merged,
3. routes importing too deeply across unrelated features,
4. utility files that have become dumping grounds,
5. misleading names,
6. vague names like `utils`, `helpers`, or `data` that hide mixed concerns,
7. opportunities to make the project easier to navigate for future contributors.

### 11. Anti-patterns and unnecessary complexity

Call out:

1. local abstractions that add indirection without benefit,
2. premature generalization,
3. route modules with too many responsibilities,
4. effect-heavy flows that should be derived or declarative,
5. duplicated loading/error state patterns,
6. handcrafted UI/state machinery where library support already exists,
7. compatibility or transitional code that should now be removed,
8. dead code, unused components, unused dependencies, or placeholder structures.

## Specific Things To Confirm Or Refute

Be explicit about whether each of the following appears true in this codebase:

1. The route tree is logically split by feature and user workflow.
2. Route-level error handling is consistently applied where it should be.
3. The app is leaving meaningful bundle-size savings on the table.
4. Specialized dashboard screens should be split more aggressively than they currently are.
5. The current `components/ui/` layer is being used consistently.
6. There are custom components that should be replaced by existing shadcn/ui primitives.
7. Search params, route params, and forms need a stronger validation story, likely using Zod.
8. API routes are doing more work than necessary or are shaped inconsistently.
9. The server/client boundary is clean enough to prevent accidental bundle bloat.
10. Current patterns would make future auth, authorization, and security hardening more difficult than necessary.
11. The existing lint/format/type setup is sufficient to enforce the intended conventions.
12. The app is underusing modern TanStack Router / TanStack Start / React capabilities in ways that materially matter.

If evidence is incomplete, say so and identify what should be measured or inspected next.

## Required Output

Produce a formal, detailed frontend audit findings report with findings prioritized by severity:

- Critical
- High
- Medium
- Low

For each finding include:

- title
- severity
- why it matters
- exact file/path references
- concrete recommendation
- code example or implementation sketch where applicable
- category:
  - architecture
  - correctness
  - performance
  - bundle-size
  - security
  - maintainability
  - UX/resilience
- whether it is:
  - must fix
  - should fix
  - optional improvement

## Additional Required Sections

After the findings, include all of the following:

1. A route architecture map of the current dashboard route tree and whether the current split is sensible.
2. A code-splitting and bundle-reduction plan:
   - quick wins
   - medium-effort improvements
   - changes that require architectural refactoring
3. A shadcn/ui adoption review:
   - components that should be added
   - components that should be reused more consistently
   - custom components that should remain custom
4. A validation strategy section:
   - where to introduce Zod first
   - where TanStack Router validation should be added
   - where manual parsing is acceptable
5. A security/auth-readiness section:
   - current risks
   - future auth blockers
   - recommended hardening sequence
6. An API route optimization section:
   - route inventory
   - repeated patterns worth centralizing
   - performance and boundary recommendations
7. A naming/style consistency section:
   - current strengths worth keeping
   - conventions that should be standardized
8. A recommendations list:
   - prioritized actionable changes
   - exact files to update
   - code examples or implementation sketches where applicable
   - clear notes when a recommendation is structural versus a local cleanup

## Output Requirements

- Be specific to this codebase.
- Do not give generic framework advice without file-level justification.
- Call out places where current patterns are already strong and should be preserved.
- Distinguish clearly between proven issues and informed inference.
- Prefer structural fixes over superficial cleanup.
- Treat bundle-size reduction as a primary concern, not an afterthought.
- Treat security concerns seriously even if the app is currently internal/admin-oriented.
- Treat validation as both a correctness concern and a security concern.
- Include concrete recommendations that reference relevant files and code examples wherever that would make the fix clearer.

## Suggested Structure

1. Executive Summary
2. Architecture and Code Organization Findings
3. Routing and Boundary Findings
4. Error Handling and Resilience Findings
5. Bundle Size and Code-Splitting Findings
6. UI Library / shadcn Findings
7. Validation and Forms Findings
8. API Route and Server Boundary Findings
9. Security and Auth-Readiness Findings
10. Naming / Consistency Findings
11. Recommendations List
12. Prioritized Remediation Plan
13. Route Map / API Inventory / Bundle Strategy Appendices

## Deliverable

Write the detailed findings report to:

- `docs/reports/frontend_tanstack_router_audit_report_YYYY-MM-DD.md`

Do not write the completed report anywhere else. The final report belongs in `docs/reports/`.
