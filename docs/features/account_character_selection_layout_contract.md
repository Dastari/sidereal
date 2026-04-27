# Account Character Selection Layout Contract

Status: Active partial implementation spec  
Last updated: 2026-04-26  
Owners: Dashboard/frontend, native client runtime, gateway  
Scope: Account-facing character selection, creation, reset, deletion, and reusable visual layout for dashboard and in-game client  
Primary references:
- `docs/frontend_ui_styling_guide.md`
- `docs/ui_design_guide.md`
- `docs/plans/gateway_dashboard_auth_character_flow_plan_2026-04-26.md`
- `docs/decisions/dr-0036_gateway_account_auth_dashboard_and_character_creation.md`

## 0. Implementation Status

2026-04-26:

- Dashboard root `/` inside the authenticated shell is now the `My Account` character-selection surface instead of placeholder overview panels.
- The implemented dashboard layout uses a large selected-character presentation lane, a right-side character roster, create/reset/delete controls, and account status.
- Dashboard character operations flow through account-scoped dashboard API routes, which forward the logged-in gateway access token to gateway `/auth/v1/characters` routes.
- Regular authenticated dashboard users are allowed into `/` only. They receive the `My Account` character-management surface and do not see or access admin tool navigation.
- Dashboard tool routes other than `/` remain admin-only and require gateway admin/dev/developer role, `dashboard:access` scope, route-specific scopes where applicable, and verified MFA.
- Gateway supports account-owned character creation, deletion, and reset. Reset removes persisted graph records owned by the player entity and re-seeds starter-world records for the same `player_entity_id`.
- Native client impact: the Bevy character select screen now consumes gateway character summaries, shows display names as the primary roster label, keeps `player_entity_id` as secondary diagnostic text, and follows the shared selected-character plus roster layout. `Enter World` replaces the account token with the character-scoped token returned by the gateway before replication bind.
- Native client login uses the same gateway v1 password/TOTP challenge flow as the dashboard before character selection is loaded.
- Native client auth UI is login-only. Account registration and password reset are web/dashboard flows; the client exposes only a text-style `Forgot Password?` link that opens dashboard `/forgot-password` in the external browser. Native builds use `SIDEREAL_DASHBOARD_URL` for that dashboard base URL, defaulting to `http://127.0.0.1:3000`.
- WASM impact: the browser dashboard implementation is React/TanStack only. Shared gateway DTOs and API semantics are expected to remain usable by the WASM client account flow.

## 1. Shared Layout Model

The character selection screen is a game-facing account surface, not an admin dashboard.

The canonical layout is:

- Full-screen shell using Sidereal dark space theme and semantic tokens.
- Primary presentation lane:
  - selected character visual centered and large,
  - character display name below the visual,
  - stable `player_entity_id` shown as secondary mono diagnostic text where appropriate,
  - primary `Enter World` action,
  - secondary `Reset` action,
  - destructive `Delete` action.
- Roster lane:
  - account summary at top,
  - vertical list of account-owned characters,
  - selected row highlighted,
  - `Create New` action,
  - refresh or reload affordance where the platform has an external source of truth.
- Empty state:
  - no placeholder dashboard statistics,
  - clear create-character call to action,
  - no implicit default character creation.

The dashboard implementation may show `Enter World` disabled until the dashboard-hosted game-client handoff is wired. The native client should enable `Enter World` once gateway world-entry token minting and replication session bind are available.

## 2. Visual Styling Rules

- Use the same semantic theme tokens as the dashboard and native UI guides: `background`, `card`, `foreground`, `muted-foreground`, `primary`, `border`, `destructive`.
- Keep the roster dense and readable; avoid marketing cards or explanatory feature panels.
- Use sharp HUD/GridCN framing for bounded panels and controls.
- Character rows must have stable dimensions so selection, status text, icons, or loading state do not shift the roster.
- The selected character visual should be the first-viewport signal. Dashboard currently uses the ship icon asset as a stand-in until character/ship renders are available.
- Do not use browser-native `alert`, `confirm`, or `prompt` for reset/delete. Use the shared confirmation dialog pattern.

## 3. Character Operations

All character lifecycle operations are account-scoped and gateway-owned.

Dashboard API routes:

- `GET /api/account/characters`
- `POST /api/account/characters`
- `DELETE /api/account/characters/:playerEntityId`
- `POST /api/account/characters/:playerEntityId/reset`

Gateway API routes:

- `GET /auth/v1/characters`
- `POST /auth/v1/characters`
- `DELETE /auth/v1/characters/:playerEntityId`
- `POST /auth/v1/characters/:playerEntityId/reset`

Rules:

- Dashboard routes require a valid encrypted dashboard session cookie.
- Dashboard routes forward the logged-in account access token to the gateway.
- The account character dashboard routes require account authentication only; they must not require dashboard admin role/scope.
- Non-account dashboard tool routes require the gateway admin authorization boundary and must redirect regular users back to `/`.
- Gateway routes validate token subject and account ownership before mutating a character.
- Character creation creates a new `player_entity_id`, account ownership row, and starter-world graph records.
- Character reset preserves the same `player_entity_id`, removes existing graph records owned by that player entity, and re-seeds starter-world graph records.
- Character deletion removes graph records owned by that player entity, then removes the account ownership row.

## 4. Native Client Reuse Notes

Native client implementation should reuse this interaction model:

- `Auth -> CharacterSelect -> WorldLoading -> AssetLoading -> InWorld`
- Roster lane maps to Bevy UI list controls.
- Primary selected-character lane maps to in-world preview or rendered ship/character preview.
- `Create New` opens a modal/screen with display-name entry and future archetype options.
- `Reset` and `Delete` require persistent confirmation dialogs.
- `Enter World` calls gateway world-entry for the selected `player_entity_id`, stores the returned character-scoped access token, and proceeds only after replication session-ready bind.

The native client must not reintroduce account registration or in-client password reset request/confirm forms. Registration and password reset remain public web/account surfaces.
