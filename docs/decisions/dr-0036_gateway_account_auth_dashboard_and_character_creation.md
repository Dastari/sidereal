# Gateway Account Auth, Dashboard Sessions, and Character Creation

Status: Proposed decision detail (`DR-0036`)  
Date: 2026-04-26  
Owners: Gateway, dashboard/frontend, client runtime, replication

## 0. Status Notes

2026-04-26:

- Account-only registration, explicit gateway character creation, v1 character routes, v1 email login request/verify, SMTP/log/noop email delivery, v1 no-token password reset request, per-email delivery cooldown/hourly caps, TOTP enrollment/QR/verification primitives, login-time TOTP challenges, persisted account roles/scopes, role/scope token claims, and MFA session-context token claims are implemented in the gateway.
- Gateway admin spawn and script-management routes require admin/dev role, verified MFA, and route-specific scope as of 2026-04-26.
- Dashboard login/register, `/mfa-setup` TOTP enrollment for admin/dev dashboard accounts, pathless route `beforeLoad` protection, gateway-backed encrypted HttpOnly dashboard session cookies, and dashboard API guard migration are partially implemented as of 2026-04-26.
- First-administrator OOBE is implemented as of 2026-04-26 through gateway `/auth/v1/bootstrap/status`, gateway `/auth/v1/bootstrap/admin`, `auth_bootstrap_state`, `GATEWAY_BOOTSTRAP_TOKEN`, and dashboard `/setup`.
- Dashboard account character management is implemented as of 2026-04-26 through the `My Account` character-selection surface and gateway-backed list/create/delete/reset routes.
- Persisted session rows/context for refresh-token families, asymmetric JWT/JWKS, per-IP rate limits, verification attempt caps, full public site polish, post-bootstrap MFA recovery policy, and character-scoped world tokens remain open.

## Purpose

Define the target authentication and character lifecycle model for Sidereal's gateway, dashboard, game client, and replication server.

This decision exists because the current implementation has two incompatible auth lanes:

1. game account auth through `sidereal-gateway`,
2. dashboard admin access through a separate password-backed session.

It also exists because registration currently creates a default character, while the intended user experience is explicit character creation and selection after account login.

## Decision

Adopt gateway-backed account auth as the only user/admin authentication authority.

The target model is:

1. Account registration creates only an account.
2. Character creation is explicit and creates the player ECS entity/starter-world graph state.
3. Dashboard login uses gateway account sessions, not a standalone admin password.
4. Admin dashboard routes require:
   - admin/dev role,
   - route-specific scopes,
   - verified MFA.
5. First-administrator setup is a one-time gateway bootstrap ceremony guarded by `GATEWAY_BOOTSTRAP_TOKEN` and durable database state.
6. Email auth supports one-time codes and magic links through SMTP delivery.
7. TOTP app-based MFA is supported with server-generated QR provisioning.
8. Short-lived JWT access/world tokens use asymmetric signing and are verifiable through JWKS.
9. Refresh tokens are opaque, hashed at rest, and rotated on every use.
10. Replication accepts only character-scoped world tokens for session bind.

## Required Auth Capabilities

Gateway auth must support:

- Argon2 password hashing and verification,
- SMTP password reset delivery,
- SMTP one-time login code delivery,
- SMTP magic-link login delivery,
- TOTP secret generation,
- TOTP provisioning URI and QR SVG generation,
- TOTP verification,
- short-lived JWT access tokens,
- character-scoped world tokens,
- rotated opaque refresh tokens,
- string roles,
- string scope claims,
- structured session-context claims,
- one-time first administrator bootstrap guarded by database state,
- database-agnostic storage traits,
- Postgres-backed rate limiting/spam protection,
- JWKS endpoint at `/auth/v1/.well-known/jwks.json`.

## Consequences

Positive:

- One account system controls game login, dashboard login, and admin authorization.
- Dashboard data sources become protected by user-specific roles/scopes rather than a shared password.
- Users can create multiple characters explicitly.
- The game client no longer exposes public account registration.
- Replication token validation becomes character-scoped and does not depend on a shared symmetric secret.

Negative:

- This is a breaking auth schema change during early development.
- Gateway, dashboard, client, and replication must migrate together.
- SMTP introduces operational configuration and abuse-prevention requirements.
- TOTP introduces MFA recovery questions that are intentionally out of v1 scope.
- Character creation now spans SQL auth state and graph persistence; partial-failure cleanup must be handled deliberately.

## Alternatives Considered

Keep dashboard admin password:

- Rejected because it creates a separate auth authority and cannot express account roles, scopes, MFA, or audit context.

Keep account registration creating a default character:

- Rejected because it prevents a proper character creation/selection flow and conflicts with the desired World of Warcraft-style selection surface.

Support password login only:

- Rejected because dashboard admin access needs MFA and password reset/email login require SMTP challenge primitives.

Use in-memory rate limiting:

- Rejected for the target implementation because limits would reset on restart and would not work across multiple gateway instances.

Require SMTP in all environments:

- Rejected because local development needs an explicit log sink mode. Production can require SMTP by deployment configuration.

## Implementation Source

Implementation details live in:

- `docs/plans/gateway_dashboard_auth_character_flow_plan_2026-04-26.md`
- `docs/features/account_character_selection_layout_contract.md`

## Superseded Text

This decision supersedes prior future-direction text that said registration must create a default character and starter corvette.

The corrected target is:

- registration creates account/auth state only,
- character creation creates character ownership and starter graph records,
- world entry remains explicit and character-scoped.

## References

- `docs/decision_register.md` (`DR-0036`)
- `docs/plans/gateway_dashboard_auth_character_flow_plan_2026-04-26.md`
- `docs/decisions/dr-0001_account_character_session_model.md`
- `docs/decisions/dr-0002_explicit_world_entry_flow.md`
- `docs/sidereal_design_document.md`
