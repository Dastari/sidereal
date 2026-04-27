# Gateway Dashboard Auth and Character Flow Plan

Status: Proposed implementation plan  
Last updated: 2026-04-26  
Owners: Gateway, dashboard/frontend, client runtime, replication  
Scope: Gateway account auth, dashboard sessions, public registration/login, SMTP email challenges, TOTP MFA, character creation/selection, and replication token validation  
Primary references:
- `docs/sidereal_design_document.md`
- `docs/decision_register.md` (`DR-0036`)
- `docs/decisions/dr-0036_gateway_account_auth_dashboard_and_character_creation.md`
- `docs/decisions/dr-0001_account_character_session_model.md`
- `docs/decisions/dr-0002_explicit_world_entry_flow.md`
- `docs/frontend_ui_styling_guide.md`
- `docs/ui_design_guide.md`

## 0. Implementation Status

2026-04-26:

- This is the source implementation plan for replacing the dashboard admin-password flow with gateway-backed game-account authentication.
- The implementation is not yet complete. The dashboard session route is gateway-backed, but the final public site polish, persisted auth-session records, JWKS/asymmetric signing, and game-client character-selection UI remain open.
- First gateway lifecycle tranche implemented on 2026-04-26:
  - gateway registration creates account/auth state without a default character,
  - `/auth/v1/*` compatibility routes exist for register/login/refresh/me/characters/world-entry,
  - `POST /auth/v1/characters` creates an explicit account-owned character,
  - gateway tests cover empty character lists after registration and explicit character creation.
- Email/auth challenge tranche implemented on 2026-04-26:
  - `/auth/v1/login/email/request` issues enumeration-safe email login challenges for known accounts,
  - `/auth/v1/login/email/verify` consumes a one-time code or opaque magic-link token and issues auth tokens,
  - `/auth/v1/password-reset/request` sends reset delivery without returning the raw token in the HTTP response,
  - gateway email delivery supports `noop`, explicit local `log`, and SMTP modes through `lettre`,
  - email delivery events are stored through the gateway auth store and enforce per-email resend cooldown/hourly caps for password reset and email login,
  - gateway tests cover email login code/token consumption, replay rejection, public password reset token suppression, and resend cooldown.
- TOTP/MFA foundation tranche implemented on 2026-04-26:
  - `/auth/v1/mfa/totp/enroll` requires an account bearer token and returns a provisioning URI, QR SVG, and manual Base32 secret,
  - `/auth/v1/mfa/totp/verify` requires the same account bearer token, verifies the current RFC 6238 code with configured drift, activates TOTP for the account, and returns fresh MFA-verified gateway tokens for immediate dashboard continuation,
  - pending TOTP enrollment secrets and active TOTP secrets are encrypted at rest with ChaCha20-Poly1305 using `GATEWAY_AUTH_SECRET_KEY_B64` when configured,
  - gateway tests cover service-level TOTP activation and the v1 enroll/verify route flow.
- Login-time MFA tranche implemented on 2026-04-26:
  - `/auth/v1/login/password` now returns `status=authenticated` with tokens for non-MFA accounts and `status=mfa_required` with a TOTP challenge for accounts with verified TOTP,
  - `/auth/v1/login/challenge/totp` verifies and consumes the TOTP login challenge before issuing account tokens,
  - access tokens now include defaulted `scope` and `session_context` claims; TOTP challenge completion issues `auth_method=password_totp`, `mfa_verified=true`, and `mfa_methods=["totp"]`,
  - native/WASM game-client login uses the same v1 password/TOTP challenge flow, and legacy `/auth/login` rejects MFA-enabled accounts instead of issuing bypass tokens,
  - gateway tests cover service-level and HTTP route challenge flow plus replay rejection.
- Password reset surface update implemented on 2026-04-26:
  - game-client auth UI is login-only and no longer contains register, forgot-request, or forgot-confirm flows,
  - game-client `Forgot Password?` opens dashboard `/forgot-password` externally; native clients use `SIDEREAL_DASHBOARD_URL` as the dashboard base URL,
  - legacy gateway `/auth/password-reset/request` and `/auth/password-reset/confirm` routes were removed; dashboard and admin reset callers use `/auth/v1/password-reset/*`.
- Roles/scopes/admin authorization tranche implemented on 2026-04-26:
  - `auth_account_roles` and `auth_account_scopes` are created by the gateway schema ensure path and mirrored in the in-memory test store,
  - access-token issuance loads persisted account roles and scopes into `roles`, space-delimited `scope`, and `session_context.active_scope`,
  - gateway admin spawn and script-management routes require `admin`/`dev_tool`/`developer`, `session_context.mfa_verified=true`, and the route-specific scope (`admin:spawn`, `scripts:read`, or `scripts:write`),
  - gateway tests cover role/scope token issuance plus admin rejection for non-admin, missing MFA, and missing route scope.
- Dashboard auth migration tranche implemented on 2026-04-26:
  - dashboard `/login` is a public gateway-backed login/register surface,
  - dashboard `/mfa-setup` is a public account route for admin/dev accounts that have `dashboard:access` but lack verified TOTP; it enrolls TOTP through the gateway, renders the returned QR SVG/manual secret through the themed reusable `TotpSetupPanel`, verifies the code with the shared six-cell `TotpCodeInput`, and rewrites the encrypted dashboard cookie with the fresh MFA-verified tokens,
  - the pathless `_dashboard` route uses TanStack Router `beforeLoad` to require an authenticated dashboard session before all current dashboard tool routes,
  - `/api/dashboard-session` proxies gateway password login, TOTP challenge completion, registration, status refresh, and logout while storing gateway tokens in an encrypted HttpOnly `SameSite=Strict` cookie,
  - dashboard privileged API handlers now use the gateway-backed session guard with admin/dev role, verified MFA, and route-specific scopes, and dashboard admin spawn forwards the logged-in gateway access token instead of `SIDEREAL_DASHBOARD_ADMIN_BEARER_TOKEN`.
- First-admin OOBE tranche implemented on 2026-04-26:
  - gateway exposes `/auth/v1/bootstrap/status` and `/auth/v1/bootstrap/admin`,
  - `auth_bootstrap_state` permanently records the first completed administrator bootstrap for the current database,
  - bootstrap is eligible only while no bootstrap row exists and no account has `admin`, `dev_tool`, or `developer` role,
  - `POST /auth/v1/bootstrap/admin` requires `GATEWAY_BOOTSTRAP_TOKEN`, creates the first administrator atomically, assigns the initial dashboard/admin scopes, and returns a gateway session with `auth_method=bootstrap_token` and `mfa_verified=true`,
  - dashboard `/setup` is shown before `/login` when first-admin bootstrap is required and the dashboard session secret plus gateway bootstrap token are configured.
- Account character-selection tranche implemented on 2026-04-26:
  - authenticated dashboard root `/` is now the `My Account` character-selection surface,
  - regular authenticated accounts may access `/` for account-owned character management but are redirected away from every other dashboard tool route,
  - the dashboard shell hides admin tool navigation unless the session has admin/dev/developer role, verified MFA, and `dashboard:access`,
  - dashboard account character routes proxy the logged-in gateway access token for list/create/delete/reset,
  - native client character select consumes gateway character summaries, displays character names instead of raw IDs, and swaps to the gateway world-entry character token before replication auth,
  - gateway v1 character routes now support account-owned delete and reset,
  - reset preserves `player_entity_id`, removes graph records owned by that character, and re-seeds starter-world graph records,
  - reusable layout contract lives in `docs/features/account_character_selection_layout_contract.md`.
- This plan defines the intended contract for the implementation work and should be used by future agents to split tasks safely.
- This plan supersedes the old future direction where registration creates a default character. The new target is account-only registration followed by explicit character creation.
- Native client impact: the game client becomes login-only and character creation moves to the character-select flow.
- WASM impact: shared auth/character gateway DTOs and client runtime state must compile for WASM; browser transport remains WebTransport-first for runtime replication and HTTP-based for gateway auth/assets.

## 1. Goals

1. Make the gateway the only authority for account auth, dashboard auth, email auth, MFA, token issuance, character ownership, and world-entry token minting.
2. Replace the dashboard-only admin password with game-account login.
3. Allow game accounts to be administrators through roles/scopes stored on the backend.
4. Add a public themed web surface before the dashboard for registration, login, email code/magic link completion, MFA setup, and character management.
5. Move game account registration out of the game client.
6. Move character creation from registration to explicit character creation in:
   - game client character select,
   - web account character management,
   - dashboard character management where appropriate.
7. Protect all existing dashboard tool routes and their server data sources with gateway-issued session tokens, roles, scopes, and MFA where required.
8. Add modern auth foundations:
   - Argon2 password hashing and verification,
   - short-lived JWT access tokens,
   - rotated opaque refresh tokens,
   - string scope claims alongside roles,
   - structured session-context claims,
   - database-agnostic storage traits,
   - password-reset token issuance and verification,
   - email one-time login code and magic-link challenges,
   - TOTP secret generation, provisioning, QR generation, and verification,
   - SMTP delivery,
   - Postgres-backed rate limiting and spam protection,
   - JWKS at `/auth/v1/.well-known/jwks.json`.

## 2. Non-Goals

- Do not introduce account-scoped gameplay progression or runtime state. Character-local gameplay state remains on the persisted player ECS entity.
- Do not add separate per-player SQL side tables for authoritative runtime state.
- Do not add character deletion in v1 unless a separate destructive lifecycle contract is written.
- Do not make browser/WASM runtime replication default to WebSocket.
- Do not make dashboard routes public or rely on client-side route hiding for authorization.
- Do not store gateway access/refresh tokens in browser local storage.
- Do not stream asset payloads through replication.

## 3. Target Lifecycle

The target lifecycle is:

1. Public visitor opens the dashboard web frontend public route.
2. Visitor registers or logs into a gateway account.
3. Gateway creates or resumes an account session.
4. If account policy requires MFA, gateway returns a challenge or limited session until MFA completes.
5. Authenticated user enters account character management.
6. User creates a character if none exists.
7. User selects a character and enters world.
8. Gateway validates account-to-character ownership and mints a character-scoped world token.
9. Client sends the world token in the replication auth message.
10. Replication validates the world token, character binding, and ownership, then binds the transport session.

Game client target state:

`Auth -> CharacterSelect -> WorldLoading -> AssetLoading -> InWorld`

Dashboard target state:

`Public -> Authenticated Account -> Account Character Management`

Admin dashboard target state:

`Public -> Authenticated Admin/Dev Account -> MFA if required -> Admin Dashboard Tools`

## 4. Identity Model

Terms:

- `Account`: credential and auth-session container.
- `Character`: durable gameplay identity represented by a persisted player ECS entity (`player_entity_id`).
- `Session`: authenticated gateway session for an account.
- `World session`: character-scoped runtime binding between a connected client and one selected character.

Invariants:

- Account registration creates only the account.
- Character creation creates the `auth_characters` ownership row and persisted starter-world graph records.
- Runtime world entry is explicit and character-scoped.
- Replication never trusts a client-selected `player_entity_id` unless the gateway world token matches it.
- No raw Bevy `Entity` IDs cross service boundaries.

## 5. Gateway API Contract

Add versioned routes and migrate clients to them. Avoid adding new unversioned `/auth/*` endpoints.

### 5.0 First Administrator Bootstrap

`GET /auth/v1/bootstrap/status`

Response:

```json
{
  "required": true,
  "configured": true
}
```

Rules:

- `required=true` only when the database has no `auth_bootstrap_state` row and no account role named `admin`, `dev_tool`, or `developer`.
- `configured=true` only when the gateway has a non-empty `GATEWAY_BOOTSTRAP_TOKEN`.
- This route is public but reveals only whether setup is required/configured; it must not reveal account details.

`POST /auth/v1/bootstrap/admin`

Request:

```json
{
  "email": "admin@example.com",
  "password": "very-strong-password",
  "setup_token": "operator-provided-bootstrap-token"
}
```

Response: same token shape as gateway login.

Rules:

- Requires `GATEWAY_BOOTSTRAP_TOKEN` and rejects an invalid setup token.
- Creates exactly one first administrator for the current database lifecycle.
- Uses an atomic store operation. The Postgres implementation takes an advisory transaction lock, verifies that bootstrap is still eligible, inserts the account, inserts initial roles/scopes, and inserts `auth_bootstrap_state` in one statement.
- Once `auth_bootstrap_state` exists or any admin/dev account exists, the endpoint must fail with conflict semantics.
- The initial admin receives role `admin` and the scopes currently needed by dashboard/admin bootstrap work: `dashboard:access`, `admin:spawn`, `scripts:read`, `scripts:write`, `dashboard:database:read`, `dashboard:database:write`, `dashboard:brp:proxy`, `admin:accounts:read`, `admin:accounts:write`, `characters:read`, and `characters:write`.
- Bootstrap-issued sessions set `session_context.auth_method=bootstrap_token`, `session_context.mfa_verified=true`, and `session_context.mfa_methods=["bootstrap_token"]` so the first admin can enter the dashboard and configure real MFA after initial setup.
- Operators should remove `GATEWAY_BOOTSTRAP_TOKEN` after successful setup. The database guard is the hard lock; the env var is the possession factor for the one-time setup ceremony.

### 5.1 Public Account Routes

`POST /auth/v1/register`

Request:

```json
{
  "email": "pilot@example.com",
  "password": "very-strong-password"
}
```

Response:

```json
{
  "account_id": "uuid",
  "email": "pilot@example.com",
  "requires_mfa": false
}
```

Rules:

- Creates an account only.
- Does not create a character.
- Does not persist starter-world graph records.
- Does not enter world.
- Does not reveal whether an email exists outside normal registration conflict semantics.

`POST /auth/v1/login/password`

Request:

```json
{
  "email": "pilot@example.com",
  "password": "very-strong-password"
}
```

Response when MFA is not required:

```json
{
  "access_token": "...",
  "refresh_token": "...",
  "token_type": "bearer",
  "expires_in_s": 900,
  "session": {
    "session_id": "uuid",
    "mfa_verified": false,
    "required_mfa_methods": []
  }
}
```

Response when TOTP is required:

```json
{
  "challenge_id": "uuid",
  "challenge_type": "totp",
  "expires_in_s": 300
}
```

`POST /auth/v1/refresh`

- Accepts an opaque refresh token.
- Consumes the presented refresh token.
- Issues a new refresh token and access token.
- Rejects reuse of the consumed token.

`POST /auth/v1/logout`

- Revokes the current session and outstanding refresh token family for that session.
- Dashboard server should clear HttpOnly cookies after success or local failure.

`GET /auth/v1/me`

Response:

```json
{
  "account_id": "uuid",
  "email": "pilot@example.com",
  "roles": ["admin"],
  "scopes": ["dashboard:access", "characters:write"],
  "session_context": {
    "auth_method": "password_totp",
    "mfa_verified": true,
    "mfa_methods": ["totp"],
    "active_scope": ["dashboard:access"],
    "active_character_id": null
  }
}
```

### 5.2 Email Login Routes

`POST /auth/v1/login/email/request`

Request:

```json
{
  "email": "pilot@example.com"
}
```

Response:

```json
{
  "accepted": true
}
```

Rules:

- Always returns accepted for enumeration resistance.
- If the account exists and rate limits allow, sends an email containing:
  - a short one-time code,
  - a magic-link URL.
- The code and magic link consume the same challenge.

`POST /auth/v1/login/email/verify`

Request for code:

```json
{
  "challenge_id": "uuid",
  "code": "123456"
}
```

Request for magic token:

```json
{
  "challenge_id": "uuid",
  "token": "opaque-token"
}
```

Rules:

- Challenge is single-use.
- Challenge has an expiry.
- Verification attempts are capped.
- Successful verification issues account-session tokens.

`GET /auth/v1/login/email/complete?challenge_id=...&token=...`

Rules:

- Browser convenience route for magic links.
- Consumes the challenge and redirects to the dashboard frontend callback route with a short-lived handoff result.
- Does not expose tokens in URLs where avoidable. Prefer setting gateway/domain cookies server-side when same-origin deployment is available; otherwise issue a short handoff code consumed by the dashboard server.

### 5.3 Password Reset Routes

`POST /auth/v1/password-reset/request`

Request:

```json
{
  "email": "pilot@example.com"
}
```

Response:

```json
{
  "accepted": true
}
```

Rules:

- Always accepted for enumeration resistance.
- Raw reset token is never returned to browser clients.
- SMTP/log delivery sends the reset link.
- Delivery failures are logged but the public response remains generic.

`POST /auth/v1/password-reset/confirm`

Request:

```json
{
  "reset_token": "opaque-token",
  "new_password": "new-strong-password"
}
```

Rules:

- Reset token is single-use.
- Reset token has an expiry.
- Successful reset revokes active sessions and refresh tokens for the account.

### 5.4 TOTP MFA Routes

`POST /auth/v1/mfa/totp/enroll`

Rules:

- Requires authenticated account session.
- Generates a pending TOTP secret.
- Stores the secret encrypted.
- Returns provisioning URI and QR SVG.
- Does not mark MFA active until verification.

Response:

```json
{
  "enrollment_id": "uuid",
  "issuer": "Sidereal",
  "account_label": "Sidereal:pilot@example.com",
  "provisioning_uri": "otpauth://totp/Sidereal:pilot@example.com?...",
  "qr_svg": "<svg ...>",
  "manual_secret": "BASE32SECRET"
}
```

`POST /auth/v1/mfa/totp/verify`

Request:

```json
{
  "enrollment_id": "uuid",
  "code": "123456"
}
```

Rules:

- Verifies current TOTP code with a narrow allowed time-step window.
- Marks TOTP verified for the account.
- Returns fresh access/refresh tokens whose `session_context.auth_method` is `totp_enrollment`, `mfa_verified=true`, and `mfa_methods=["totp"]` so an admin who just enrolled can enter the dashboard without logging in again.
- Future password logins for the account require `/auth/v1/login/challenge/totp`.

`POST /auth/v1/login/challenge/totp`

Request:

```json
{
  "challenge_id": "uuid",
  "code": "123456"
}
```

Rules:

- Used after password login reports `challenge_type=totp`.
- Verifies and consumes the challenge.
- Implemented 2026-04-26: issues tokens with `auth_method=password_totp`, `mfa_verified=true`, and `mfa_methods=["totp"]`.

`DELETE /auth/v1/mfa/totp`

Rules:

- Requires current MFA-authenticated session.
- Admin accounts cannot disable the final MFA factor unless another verified MFA method exists or a future recovery policy is implemented.

### 5.5 Character Routes

`GET /auth/v1/characters`

Response:

```json
{
  "characters": [
    {
      "player_entity_id": "uuid",
      "display_name": "Talanah",
      "created_at_epoch_s": 1714000000,
      "status": "active"
    }
  ]
}
```

`POST /auth/v1/characters`

Request:

```json
{
  "display_name": "Talanah"
}
```

Response:

```json
{
  "player_entity_id": "uuid",
  "display_name": "Talanah",
  "created_at_epoch_s": 1714000000,
  "status": "active"
}
```

Rules:

- Display-name-only v1.
- Gateway creates the `auth_characters` row and graph starter-world records.
- Gateway uses existing Lua starter-world bundle logic.
- Failure must not leave a listable active character.
- Character creation is account-authenticated but not admin-only.

`POST /auth/v1/world/enter`

Request:

```json
{
  "player_entity_id": "uuid"
}
```

Response:

```json
{
  "accepted": true,
  "world_access_token": "...",
  "replication_transport": {
    "udp_addr": "host:port",
    "webtransport_addr": "host:port",
    "webtransport_certificate_sha256": "..."
  }
}
```

Rules:

- Validates active character ownership.
- Dispatches bootstrap to replication.
- Mints a character-scoped world token containing `world:session` and `player_entity_id`.
- Does not use account-only tokens for replication bind.

### 5.6 JWKS

`GET /auth/v1/.well-known/jwks.json`

Response follows RFC 7517 and contains public keys for access/world token verification. Include at least:

- `kid`
- `kty`
- `alg`
- `use`
- key material fields for the selected algorithm

## 6. JWT and Token Model

### 6.1 Signing

Move access/world JWT signing from shared HS256 secrets to asymmetric signing.

Recommended implementation:

- Use EdDSA if library support is straightforward in the chosen Rust JWT stack.
- Otherwise use RS256 with PEM-backed RSA keys.
- Always publish the public key through JWKS.
- Use `kid` on every signed token.

### 6.2 Account Access Token Claims

Account access token claims:

```json
{
  "iss": "sidereal-gateway",
  "aud": "sidereal",
  "sub": "account-uuid",
  "sid": "session-uuid",
  "jti": "token-uuid",
  "iat": 1714000000,
  "nbf": 1714000000,
  "exp": 1714000900,
  "roles": ["admin"],
  "scope": "dashboard:access characters:read characters:write",
  "session_context": {
    "auth_method": "password_totp",
    "mfa_verified": true,
    "mfa_methods": ["totp"],
    "active_scope": ["dashboard:access", "characters:read"],
    "active_character_id": null
  }
}
```

### 6.3 World Token Claims

World token claims include account claims plus:

```json
{
  "player_entity_id": "character-uuid",
  "scope": "world:session assets:read",
  "session_context": {
    "active_character_id": "character-uuid"
  }
}
```

Rules:

- Replication accepts only world tokens.
- Asset payload routes accept account tokens with `assets:read` or world tokens with `assets:read`.
- Admin APIs accept only account tokens with required admin scopes and MFA state.

### 6.4 Refresh Tokens

Refresh tokens are opaque random values.

Rules:

- Store only hashes.
- Rotate on every refresh.
- Mark consumed token rows.
- Treat refresh-token reuse as suspicious and revoke the session/token family.
- Tie refresh tokens to `session_id` and account.

## 7. Roles and Scopes

Roles are coarse labels:

- `user`
- `admin`
- `dev_tool`

Scopes are first-class authorization strings. Initial scopes:

- `account:read`
- `account:write`
- `characters:read`
- `characters:write`
- `world:enter`
- `world:session`
- `assets:read`
- `dashboard:access`
- `dashboard:database:read`
- `dashboard:database:write`
- `dashboard:brp:proxy`
- `dashboard:scripts:read`
- `dashboard:scripts:write`
- `scripts:read` (implemented for gateway script-management endpoints)
- `scripts:write` (implemented for gateway script-management endpoints)
- `admin:spawn`
- `admin:accounts:read`
- `admin:accounts:write`

Admin dashboard access requires:

- role `admin` or `dev_tool`,
- `dashboard:access`,
- `session_context.mfa_verified=true`,
- route-specific scopes for individual tools/mutations.

## 8. Gateway Storage

Refactor `AuthStore` into database-agnostic storage methods. Postgres remains the production implementation; in-memory store remains test-only.

### 8.1 Tables

`auth_accounts`

- `account_id UUID PRIMARY KEY`
- `email TEXT NOT NULL UNIQUE`
- `password_hash TEXT`
- `created_at_epoch_s BIGINT NOT NULL`
- `disabled_at_epoch_s BIGINT NULL`

`auth_account_roles`

- `account_id UUID NOT NULL`
- `role TEXT NOT NULL`
- primary key `(account_id, role)`

`auth_account_scopes`

- `account_id UUID NOT NULL`
- `scope TEXT NOT NULL`
- primary key `(account_id, scope)`

`auth_bootstrap_state`

- `id SMALLINT PRIMARY KEY CHECK (id = 1)`
- `completed_by_account_id UUID NOT NULL`
- `completed_at_epoch_s BIGINT NOT NULL`
- Stores completion of first-administrator setup for the current database lifecycle.
- Deleting/resetting the database is the supported way to re-enable first-administrator OOBE in development.

`auth_sessions`

- `session_id UUID PRIMARY KEY`
- `account_id UUID NOT NULL`
- `created_at_epoch_s BIGINT NOT NULL`
- `expires_at_epoch_s BIGINT NOT NULL`
- `revoked_at_epoch_s BIGINT NULL`
- `auth_method TEXT NOT NULL`
- `mfa_verified BOOLEAN NOT NULL`
- `mfa_methods TEXT[] NOT NULL`

`auth_refresh_tokens`

- `token_hash TEXT PRIMARY KEY`
- `session_id UUID NOT NULL`
- `account_id UUID NOT NULL`
- `expires_at_epoch_s BIGINT NOT NULL`
- `created_at_epoch_s BIGINT NOT NULL`
- `consumed_at_epoch_s BIGINT NULL`
- `revoked_at_epoch_s BIGINT NULL`

`auth_login_challenges`

- `challenge_id UUID PRIMARY KEY`
- `account_id UUID NULL`
- `normalized_email TEXT NOT NULL`
- `challenge_kind TEXT NOT NULL`
- `code_hash TEXT NULL`
- `token_hash TEXT NULL`
- `expires_at_epoch_s BIGINT NOT NULL`
- `created_at_epoch_s BIGINT NOT NULL`
- `consumed_at_epoch_s BIGINT NULL`
- `attempt_count INT NOT NULL DEFAULT 0`
- `max_attempts INT NOT NULL`
- `remote_addr_hash TEXT NULL`

`auth_password_reset_tokens`

- `token_hash TEXT PRIMARY KEY`
- `account_id UUID NOT NULL`
- `expires_at_epoch_s BIGINT NOT NULL`
- `created_at_epoch_s BIGINT NOT NULL`
- `consumed_at_epoch_s BIGINT NULL`

`auth_totp_secrets`

- `account_id UUID PRIMARY KEY`
- `encrypted_secret TEXT NOT NULL`
- `created_at_epoch_s BIGINT NOT NULL`
- `verified_at_epoch_s BIGINT NULL`
- `disabled_at_epoch_s BIGINT NULL`

`auth_characters`

- `account_id UUID NOT NULL`
- `player_entity_id TEXT PRIMARY KEY`
- `display_name TEXT NOT NULL`
- `status TEXT NOT NULL`
- `created_at_epoch_s BIGINT NOT NULL`
- `updated_at_epoch_s BIGINT NOT NULL`

`auth_rate_limit_events`

- `event_id UUID PRIMARY KEY`
- `bucket_key TEXT NOT NULL`
- `action_kind TEXT NOT NULL`
- `occurred_at_epoch_s BIGINT NOT NULL`

`auth_challenge_delivery_attempts`

- `attempt_id UUID PRIMARY KEY`
- `challenge_id UUID NULL`
- `normalized_email_hash TEXT NOT NULL`
- `remote_addr_hash TEXT NULL`
- `delivery_kind TEXT NOT NULL`
- `result TEXT NOT NULL`
- `occurred_at_epoch_s BIGINT NOT NULL`
- `error_kind TEXT NULL`

`auth_challenge_verify_attempts`

- `attempt_id UUID PRIMARY KEY`
- `challenge_id UUID NOT NULL`
- `result TEXT NOT NULL`
- `occurred_at_epoch_s BIGINT NOT NULL`
- `remote_addr_hash TEXT NULL`

### 8.2 Schema Discipline

- This is a breaking early-development schema change.
- Do not add long-lived compatibility aliases for account-level `player_entity_id`.
- Update all producers/consumers in the same change.
- Reset local/dev databases after migration.

## 9. SMTP Email Delivery

### 9.1 Delivery Abstraction

Add a gateway email delivery trait:

```rust
trait EmailDelivery: Send + Sync {
    async fn send(&self, message: EmailMessage) -> Result<(), EmailDeliveryError>;
}
```

Implementations:

- `SmtpEmailDelivery` for production/local SMTP.
- `LogEmailDelivery` for explicit local development.
- `NoopEmailDelivery` for tests.

Use `lettre` for SMTP unless implementation discovery finds a stronger maintained option already in the dependency graph.

### 9.2 Email Types

Email templates:

- `password_reset`
- `email_login_code`
- `email_login_magic_link`
- `mfa_changed`
- `session_security_notice` optional future template

Templates should include:

- product name,
- target email,
- expiration time,
- code or link,
- note that no action is needed if the user did not request it.

Do not log raw email challenge tokens, reset tokens, or TOTP secrets in production or normal operational logs. The explicit local `log` delivery mode is an unsafe development aid and may print email bodies so local developers can exercise flows without SMTP.

### 9.3 SMTP Environment

Gateway environment variables:

- `GATEWAY_EMAIL_DELIVERY=noop|log|smtp`
- `GATEWAY_PUBLIC_BASE_URL` default `http://localhost:3000`
- `GATEWAY_SMTP_RELAY`
- `GATEWAY_SMTP_USERNAME`
- `GATEWAY_SMTP_PASSWORD`
- `GATEWAY_SMTP_FROM`
- `GATEWAY_EMAIL_CHALLENGE_TTL_S`
- `GATEWAY_EMAIL_RESEND_COOLDOWN_S`
- `GATEWAY_EMAIL_MAX_PER_EMAIL_PER_HOUR`
- `GATEWAY_SMTP_PORT`, `GATEWAY_SMTP_SECURITY`, `GATEWAY_SMTP_TIMEOUT_MS`, and per-IP SMTP caps remain planned follow-up knobs.
- `GATEWAY_EMAIL_CHALLENGE_MAX_VERIFY_ATTEMPTS`

Startup rules:

- If `GATEWAY_EMAIL_DELIVERY=smtp`, required SMTP env vars must be valid or gateway startup fails.
- If `GATEWAY_EMAIL_DELIVERY=log`, email bodies may be logged only in local/dev and must be clearly marked unsafe for production.
- If `GATEWAY_EMAIL_DELIVERY=noop`, password reset and email login request routes return accepted and store challenge/reset primitives, but no outbound message is sent; this mode is only acceptable for isolated tests/dev.

## 10. Rate Limiting and Spam Protection

Persist rate limiting state in Postgres so limits survive restart and work across gateway instances.

Limit dimensions:

- normalized email hash,
- remote IP hash,
- account ID where known,
- challenge ID,
- action kind.

Initial action kinds:

- `register`
- `password_login`
- `email_login_request`
- `email_login_verify`
- `password_reset_request`
- `password_reset_confirm`
- `totp_verify`
- `refresh`

Rules:

- Per-email resend cooldown for email login and password reset is implemented for known-account deliveries.
- Per-email hourly cap for SMTP/log delivery is implemented for known-account deliveries.
- Per-IP hourly cap for challenge generation.
- Challenge verification attempt cap.
- Expired/consumed challenges are rejected.
- Public responses remain enumeration-safe.
- Server logs and audit rows record rate-limited attempts.

Suggested defaults:

- email login challenge TTL: 10 minutes,
- password reset TTL: 60 minutes,
- email resend cooldown: 60 seconds,
- email deliveries per email per hour: 5,
- email deliveries per IP per hour: 20,
- challenge verify attempts: 5.

These defaults should be env-overridable.

## 11. TOTP MFA and QR Generation

Use RFC 6238-compatible TOTP.

Recommended crates:

- `totp-rs` for TOTP generation/verification and `otpauth://` provisioning URI support.
- `qrcode` for QR SVG generation.

Rules:

- TOTP secrets are encrypted at rest using a gateway auth secret.
- QR SVG is generated server-side and returned only during enrollment.
- The manual secret and provisioning URI are not logged.
- Admin accounts require verified TOTP for dashboard admin access.
- MFA state is represented in `session_context`, not inferred only from roles.

Environment:

- `GATEWAY_AUTH_SECRET_KEY_B64`
- `GATEWAY_TOTP_ISSUER=Sidereal`
- `GATEWAY_TOTP_STEP_S=30`
- `GATEWAY_TOTP_DIGITS=6`
- `GATEWAY_TOTP_ALLOWED_DRIFT_STEPS=1`
- `GATEWAY_TOTP_ENROLLMENT_TTL_S=600`
- `GATEWAY_TOTP_LOGIN_CHALLENGE_TTL_S=300`

2026-04-26 implementation note:

- TOTP uses in-gateway RFC 6238/HOTP code generation and verification with HMAC-SHA1.
- QR SVG generation uses the `qrcode` crate.
- TOTP secret encryption uses ChaCha20-Poly1305 and stores nonce+ciphertext as URL-safe base64.
- If `GATEWAY_AUTH_SECRET_KEY_B64` is not set, the current implementation derives a compatibility encryption key from `GATEWAY_JWT_SECRET`; production deployments should set `GATEWAY_AUTH_SECRET_KEY_B64` to a 32-byte base64 value before enabling TOTP.
- Login-time MFA challenges are persisted in `auth_totp_login_challenges` and are consumed on successful verification. Verification-attempt caps remain open.
- Dashboard TOTP UI uses `dashboard/src/components/auth/TotpSetupPanel.tsx` for enrollment QR/manual-secret layout and `dashboard/src/components/auth/TotpCodeInput.tsx` for reusable six-cell code entry. The setup panel recolors the gateway-provided QR SVG with semantic theme tokens while preserving a high-contrast QR surface for scanner compatibility.

## 12. Dashboard Web App

### 12.1 Route Structure

Target routes:

- `/` public site
- `/register`
- `/login`
- `/login/email/complete`
- `/account/characters`
- `/account/mfa`
- `/dashboard/*`

Current dashboard tool routes should move under `/dashboard/*` or otherwise be protected by the dashboard route guard.

### 12.2 Session Storage

Dashboard server owns gateway tokens through HttpOnly cookies:

- account access token cookie,
- refresh token cookie,
- optional CSRF token cookie/header for mutations if needed by the framework shape.

Rules:

- `HttpOnly`
- `SameSite=Strict`
- `Secure` on HTTPS
- no local storage token storage
- no server-side admin bearer env token for normal dashboard operation

Replace `dashboard/src/server/dashboard-auth.ts` password/HMAC-cookie logic with gateway-backed session verification.

2026-04-26 implementation note:

- The dashboard currently stores access/refresh tokens together in one encrypted HttpOnly cookie named `sidereal_dashboard_auth`.
- Cookie encryption uses `SIDEREAL_DASHBOARD_SESSION_SECRET`; this secret is required for dashboard login.
- `/api/dashboard-session` refreshes the access token when it is close to expiry and rewrites the cookie.
- `/mfa-setup` uses the account session to create and verify TOTP enrollment, then replaces the cookie with the MFA-verified tokens returned by `/auth/v1/mfa/totp/verify`.
- Client-side auth state contains only status fields; tokens are not exposed to browser JavaScript.
- Interim note: until persisted auth-session rows land, gateway refresh currently preserves verified MFA for accounts with active TOTP by reissuing refresh tokens with a `refresh_totp` session context. Persisting the original session context on refresh-token families remains open and should replace that account-level inference.

### 12.3 Shared Guard

Add a shared server guard:

```ts
requireDashboardAuth(request, {
  scopes: ['dashboard:access'],
  requireAdmin: true,
  requireMfa: true,
})
```

Rules:

- Refreshes access token when possible.
- Fails closed on missing/invalid/expired session.
- Applies route-specific scopes.
- Verifies admin and MFA claims for admin dashboard routes.
- Keeps same-origin mutation checks.

### 12.4 Public Site

The public site should use the same theme tokens and component wrappers as the dashboard.

Required surfaces:

- public landing/home with login/register actions,
- registration form,
- password login form,
- email login request form,
- email code entry form,
- magic-link completion route,
- password reset request/confirm forms,
- account MFA setup route,
- account character management route.

Do not eagerly load heavy admin/dashboard tools on public routes.

## 13. Dashboard Character Management

Add account-owned character management in a layout similar in structure to the game client character select:

- full-viewport account surface,
- selected character preview/name,
- right-side character list,
- primary actions:
  - `Create Character`,
  - `Enter World` where applicable,
  - `Logout`.

Character creation:

- display name only in v1,
- uses `POST /auth/v1/characters`,
- refreshes list and selects the new character.

Admin database views can continue to show all accounts/characters, but all privileged actions require admin/MFA/scopes.

## 14. Game Client

### 14.1 Remove Registration

Remove game-client registration:

- remove `AuthAction::Register`,
- remove Register UI button,
- remove `GatewayHttpAdapter.register`,
- update native and WASM gateway adapters,
- update tests.

Account registration belongs to the public web site.

### 14.2 Login Flows

Game client auth UI supports:

- password login,
- email one-time code request/verify,
- TOTP challenge,
- password reset request/confirm.

Magic links are primarily web/dashboard flow; game client supports manual code entry.

### 14.3 Character Select

Replace the current simple panel with a complete character-select surface:

- full-screen Sidereal/thegridcn-styled scene,
- right-side character list,
- central selected-character name/preview,
- empty state that opens character creation,
- actions:
  - `Logout`,
  - `Create Character`,
  - `Enter World`.

Rules:

- `Enter World` uses the selected active character.
- Client stores account token for gateway calls and world token for replication/asset session.
- Logout clears account token, refresh token, world token, selected character, replication transport state, and pending request state.
- Critical auth/gateway failures use persistent dialog UI.

## 15. Replication

Replication auth must change from shared secret validation to public-key/JWKS validation.

Rules:

- Accept only world tokens with `world:session`.
- Require `player_entity_id` claim.
- Require `session_context.active_character_id` to match `player_entity_id`.
- Require `ClientAuthMessage.player_entity_id` to match token `player_entity_id`.
- Keep hydrated `AccountId` ownership validation.
- Reject account-only dashboard tokens.
- Emit `ServerSessionDeniedMessage` for invalid/expired/mismatched tokens.

Replication config:

- Replace `GATEWAY_JWT_SECRET` with one of:
  - `GATEWAY_JWT_PUBLIC_KEY_PEM_PATH`,
  - `GATEWAY_JWT_JWKS_PATH`,
  - later `GATEWAY_JWKS_URL` if online key fetch is added.

## 16. Gateway Configuration

Auth/token env:

- `GATEWAY_JWT_ISSUER`
- `GATEWAY_JWT_AUDIENCE`
- `GATEWAY_JWT_PRIVATE_KEY_PEM_PATH`
- `GATEWAY_JWT_ACTIVE_KID`
- `GATEWAY_ACCESS_TOKEN_TTL_S`
- `GATEWAY_REFRESH_TOKEN_TTL_S`
- `GATEWAY_RESET_TOKEN_TTL_S`
- `GATEWAY_AUTH_SECRET_KEY_B64`
- `GATEWAY_BOOTSTRAP_ADMIN_EMAILS`

Email env:

- see section 9.3.

TOTP env:

- see section 11.

Dashboard env:

- `GATEWAY_API_URL`
- `SIDEREAL_DASHBOARD_SESSION_SECRET`
- `SIDEREAL_DASHBOARD_URL` for native game clients opening dashboard account flows externally.
- Gateway first-admin setup also requires `GATEWAY_BOOTSTRAP_TOKEN` on the gateway while OOBE is active.
- Remove normal-use dependency on `SIDEREAL_DASHBOARD_ADMIN_PASSWORD`.
- Remove normal-use dependency on `SIDEREAL_DASHBOARD_ADMIN_BEARER_TOKEN`.

## 17. Implementation Phases

### Phase 1: Auth Model and Storage

- Add new auth DTOs in `crates/sidereal-core/src/gateway_dtos.rs`.
- Extend `crates/sidereal-core/src/auth.rs` claims.
- Refactor gateway `AuthStore`.
- Add Postgres schema ensure logic.
- Add in-memory store support for tests.
- Add token/session model with roles/scopes/session context. `Partially implemented 2026-04-26 with persisted roles/scopes, token role/scope claims, and active scopes. Persisted session rows remain open.`
- Add first-administrator bootstrap state and atomic first-admin creation. `Implemented 2026-04-26 with auth_bootstrap_state, GATEWAY_BOOTSTRAP_TOKEN verification, and dashboard /setup integration.`
- Add asymmetric signing and JWKS.

### Phase 2: Email Delivery, Challenges, MFA

- Add SMTP/log/noop email delivery. `Implemented 2026-04-26 for SMTP relay, log, and noop modes.`
- Add challenge issuance/verification. `Implemented 2026-04-26 for email one-time code and magic-link token login.`
- Add Postgres-backed rate limiting. `Partially implemented 2026-04-26 for per-email resend cooldown and hourly delivery caps; per-IP and verification-attempt caps remain open.`
- Add password reset delivery through email. `Implemented 2026-04-26 for v1 no-token HTTP responses.`
- Add TOTP enrollment, QR SVG generation, and verification. `Implemented 2026-04-26 for authenticated enrollment, activation, login-time challenge verification, and MFA session-context claims.`
- Add admin MFA policy. `Implemented 2026-04-26 for gateway admin spawn/script routes and dashboard route/API guards; persisted session-context refresh hardening remains open.`

### Phase 3: Character Creation and World Entry

- Move starter-world persistence to `create_character`.
- Remove automatic character creation from register.
- Add character display name to gateway DTOs/store.
- Add world-token minting on world entry. `Implemented 2026-04-26: /world/enter returns tokens scoped to the selected player_entity_id and sets session_context.active_character_id.`
- Update gateway tests.

### Phase 4: Dashboard Public/Auth/Admin Routes

- Add public routes.
- Add first-admin setup route. `Implemented 2026-04-26 as /setup backed by /api/bootstrap and gateway /auth/v1/bootstrap/* endpoints.`
- Add account auth cookies. `Implemented 2026-04-26 for encrypted HttpOnly dashboard session cookie.`
- Replace dashboard password guard. `Implemented 2026-04-26 for the shared dashboard API guard and route beforeLoad guard.`
- Move/protect admin routes. `Partially implemented 2026-04-26 by protecting current pathless dashboard routes and dashboard API handlers in place; final /dashboard/* route namespace remains open.`
- Forward logged-in user token to gateway admin APIs. `Implemented 2026-04-26 for dashboard admin spawn and Genesis script gateway calls; the legacy env bearer remains only as a direct helper/test fallback.`
- Add dashboard TOTP enrollment screen. `Implemented 2026-04-26 as /mfa-setup for admin/dev accounts with dashboard:access but no verified MFA.`
- Add account character management UI.
- Replace placeholder dashboard overview with reusable account character-selection layout. `Implemented 2026-04-26; see docs/features/account_character_selection_layout_contract.md.`

### Phase 5: Game Client Flow

- Remove registration.
- Add email code and TOTP challenge screens/states. `TOTP challenge implemented 2026-04-26 for native/WASM password login. Email code remains open for the client.`
- Add character creation from character select.
- Redesign character selection surface.
- Update native and WASM adapters.

### Phase 6: Replication Verification

- Add public-key/JWKS token verification.
- Require world token scope and character binding.
- Update replication config/tests.
- Update transport e2e token generation helpers.

### Phase 7: Cleanup and Docs

- Remove obsolete dashboard password docs/code.
- Update AGENTS.md if implementation makes new contributor rules enforceable.
- Update all auth references from unversioned routes to `/auth/v1`.
- Run full quality gates.

## 18. Test Plan

Gateway tests:

- register creates account only and no character,
- duplicate register conflicts safely,
- password login succeeds,
- password login with TOTP returns challenge,
- TOTP challenge consumes once and rejects replay,
- TOTP enrollment returns valid provisioning URI and QR SVG,
- refresh token rotation invalidates old token,
- refresh token replay revokes session,
- logout revokes session,
- JWKS endpoint returns active public key,
- admin API rejects non-admin,
- admin API rejects admin without MFA,
- admin API accepts admin with MFA and scope,
- email login request is enumeration-safe,
- email code verifies and consumes challenge,
- magic link verifies and consumes challenge,
- email challenge resend cooldown works,
- per-email and per-IP rate limits work,
- password reset sends email/log delivery and never returns raw token,
- password reset consumes token once,
- character creation persists auth row and graph starter records,
- character creation failure leaves no active character,
- world entry returns character-scoped token.

Dashboard tests:

- public register/login validation uses Zod,
- email magic callback handles success/failure,
- account character management can create/select character,
- `/dashboard/*` rejects unauthenticated sessions,
- admin routes reject non-MFA admin sessions,
- admin server loaders require scopes,
- mutation APIs reject cross-origin requests,
- gateway admin calls forward logged-in user token.

Client tests:

- register action no longer exists,
- password login transitions to character select,
- email code login transitions to character select,
- TOTP challenge flow transitions to character select,
- empty character list opens creation path,
- create character refreshes list and selects new character,
- enter world stores world token and enters world loading,
- logout clears auth/world/session state.

Replication tests:

- accepts valid world token for matching character,
- rejects account-only token,
- rejects missing `world:session`,
- rejects mismatched `player_entity_id`,
- rejects expired token,
- rejects invalid signature/key.

Quality gates:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo test -p sidereal-gateway
cargo test -p sidereal-replication
cargo test -p sidereal-client
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
cd dashboard && pnpm test
cd dashboard && pnpm lint
cd dashboard && pnpm build
```

## 19. Open Implementation Risks

- Asymmetric JWT support may require switching from `jsonwebtoken` if EdDSA/JWKS ergonomics are insufficient. RS256 with PEM keys is an acceptable fallback.
- SMTP failure behavior must avoid account enumeration while still leaving useful operator logs.
- Character creation spans auth SQL and graph persistence; implementation must avoid listable partial characters on graph persistence failure.
- Dashboard cookie/token handling depends on deployment origin. If gateway and dashboard are cross-origin, the dashboard server should proxy auth session calls and keep tokens server-side.
- Magic-link completion must avoid putting long-lived tokens in URLs.
- MFA recovery is not defined in v1. Admin accounts should not be allowed to disable their final MFA method without a separate recovery policy.

## 20. External References

- OWASP Authentication Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
- OWASP JSON Web Token Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/JSON_Web_Token_for_Java_Cheat_Sheet.html
- OWASP Forgot Password Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/Forgot_Password_Cheat_Sheet.html
- OWASP Multifactor Authentication Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/Multifactor_Authentication_Cheat_Sheet.html
- NIST SP 800-63B: https://pages.nist.gov/800-63-4/sp800-63b.html
- RFC 7517 JSON Web Key: https://www.rfc-editor.org/rfc/rfc7517.html
- RFC 6238 TOTP: https://datatracker.ietf.org/doc/html/rfc6238
- Google Authenticator Key URI Format: https://github.com/google/google-authenticator/wiki/Key-Uri-Format
