# Server and Gateway Environment Variable Audit

Date: 2026-03-11
Scope: `bins/sidereal-replication`, `bins/sidereal-gateway`
Status: Active audit and startup-config reconciliation

## 1. Executive Summary

Replication and gateway had accumulated a mixed environment-variable surface:

1. real startup/runtime configuration,
2. service tuning,
3. diagnostics/debug controls.

The highest-value issue was that several core runtime values still depended on Makefile-exported env vars even though they are the normal local-dev defaults. That made running the binaries directly less correct than running them through `make`.

2026-03-11 update:
- replication and gateway now accept explicit CLI arguments for the real startup/runtime configuration surface,
- environment variables remain supported as overrides,
- binary defaults now align with the non-debug defaults already encoded in `Makefile`,
- diagnostic/tuning env vars that are still useful remain env-driven for now instead of being promoted to first-class CLI flags.

Precedence for the new startup configuration contract is:

1. CLI arguments
2. environment variables
3. built-in defaults

## 2. Findings

### Finding S1: core startup config was still Makefile-coupled
- Severity: High
- Type: correctness, operability
- Priority: fixed
- Why it mattered:
  Running `sidereal-replication` or `sidereal-gateway` directly did not reliably produce the same bind addresses, data roots, or auth/bootstrap wiring that `make run-*` produced.
- 2026-03-11 fix:
  Replication and gateway now resolve startup config through explicit CLI/config modules and export the resolved values back into the process environment for deeper modules that still read env directly.

### Finding S2: runtime config and debug/tuning knobs were intermixed
- Severity: Medium
- Type: maintainability
- Priority: partially fixed
- Why it matters:
  Not every env var should become a public startup flag. Some are clearly operational configuration; others are temporary tuning or diagnostics.
- 2026-03-11 direction:
  - promoted to CLI: binds, database URLs, asset/script roots, JWT secret, advertised transport addresses, token TTLs, BRP startup config, health bind, headless/TUI startup
  - retained as env-only for now: visibility tuning, persistence cadence/queue tuning, snapshot cadence, idle disconnect tuning, script sandbox/query limits, summary/debug logs

### Finding S3: gateway and replication shared some config names but resolved them inconsistently
- Severity: Medium
- Type: maintainability, docs divergence
- Priority: fixed
- Why it mattered:
  `GATEWAY_JWT_SECRET`, data roots, replication public addresses, and control-channel addresses were being read in multiple places with ad hoc defaults.
- 2026-03-11 fix:
  Startup config now resolves those values once, then applies canonical env values before service/bootstrap initialization.

## 3. Startup Config Promoted to CLI

### 3.1 Replication

| Runtime concern | CLI flag | Env fallback | Built-in default |
|---|---|---|---|
| Headless/TUI | `--headless` | `SIDEREAL_REPLICATION_HEADLESS` | auto-TUI on interactive terminal |
| Postgres URL | `--database-url` | `REPLICATION_DATABASE_URL` | `postgres://sidereal:sidereal@127.0.0.1:5432/sidereal` |
| UDP bind | `--udp-bind` | `REPLICATION_UDP_BIND` | `0.0.0.0:7001` |
| WebTransport bind | `--webtransport-bind` | `REPLICATION_WEBTRANSPORT_BIND` | `0.0.0.0:7003` |
| WebTransport cert | `--webtransport-cert-pem` | `REPLICATION_WEBTRANSPORT_CERT_PEM` | `./data/dev_certs/replication-webtransport-cert.pem` |
| WebTransport key | `--webtransport-key-pem` | `REPLICATION_WEBTRANSPORT_KEY_PEM` | `./data/dev_certs/replication-webtransport-key.pem` |
| Control UDP bind | `--control-udp-bind` | `REPLICATION_CONTROL_UDP_BIND` | `127.0.0.1:9004` |
| Health bind | `--health-bind` | `REPLICATION_HEALTH_BIND` | `127.0.0.1:15716` |
| Asset root | `--asset-root` | `ASSET_ROOT` | `./data` |
| Scripts root | `--scripts-root` | `SIDEREAL_SCRIPTS_ROOT` | `./data/scripts` |
| Shared JWT secret | `--jwt-secret` | `GATEWAY_JWT_SECRET` | `0123456789abcdef0123456789abcdef` |
| BRP enabled | `--replication-brp-enabled` / `--replication-brp-disabled` | `SIDEREAL_REPLICATION_BRP_ENABLED`, `SIDEREAL_BRP_ENABLED` | `false` |
| BRP bind | `--replication-brp-bind-addr` | `SIDEREAL_REPLICATION_BRP_BIND_ADDR`, `SIDEREAL_BRP_BIND_ADDR` | `127.0.0.1` |
| BRP port | `--replication-brp-port` | `SIDEREAL_REPLICATION_BRP_PORT`, `SIDEREAL_BRP_PORT` | `15713` |
| BRP token | `--replication-brp-auth-token` | `SIDEREAL_REPLICATION_BRP_AUTH_TOKEN`, `SIDEREAL_BRP_AUTH_TOKEN` | `0123456789abcdef` |

### 3.2 Gateway

| Runtime concern | CLI flag | Env fallback | Built-in default |
|---|---|---|---|
| HTTP bind | `--bind` | `GATEWAY_BIND` | `0.0.0.0:8080` |
| Postgres URL | `--database-url` | `GATEWAY_DATABASE_URL` | `postgres://sidereal:sidereal@127.0.0.1:5432/sidereal` |
| Bootstrap mode | `--bootstrap-mode` | `GATEWAY_BOOTSTRAP_MODE` | `udp` |
| JWT secret | `--jwt-secret` | `GATEWAY_JWT_SECRET` | `0123456789abcdef0123456789abcdef` |
| Access token TTL | `--access-token-ttl-s` | `GATEWAY_ACCESS_TOKEN_TTL_S` | `900` |
| Refresh token TTL | `--refresh-token-ttl-s` | `GATEWAY_REFRESH_TOKEN_TTL_S` | `2592000` |
| Reset token TTL | `--reset-token-ttl-s` | `GATEWAY_RESET_TOKEN_TTL_S` | `3600` |
| Allowed origins | `--allowed-origins` | `GATEWAY_ALLOWED_ORIGINS` | `http://localhost:3000,http://127.0.0.1:3000` |
| Asset root | `--asset-root` | `ASSET_ROOT` | `./data` |
| Scripts root | `--scripts-root` | `SIDEREAL_SCRIPTS_ROOT` | `./data/scripts` |
| Gateway control UDP bind | `--replication-control-udp-bind` | `GATEWAY_REPLICATION_CONTROL_UDP_BIND` | `0.0.0.0:0` |
| Replication control target | `--replication-control-udp-addr` | `REPLICATION_CONTROL_UDP_ADDR` | `127.0.0.1:9004` |
| Advertised replication UDP | `--replication-udp-public-addr` | `REPLICATION_UDP_PUBLIC_ADDR`, `REPLICATION_UDP_ADDR` | `127.0.0.1:7001` |
| Advertised replication WebTransport | `--replication-webtransport-public-addr` | `REPLICATION_WEBTRANSPORT_PUBLIC_ADDR` | `127.0.0.1:7003` |
| Advertised WebTransport digest | `--replication-webtransport-cert-sha256` | `REPLICATION_WEBTRANSPORT_CERT_SHA256` | none |

## 4. Env Vars Retained as Env-Only

These remain active, but were intentionally not promoted to public CLI in this pass.

### 4.1 Replication tuning and diagnostics

- `REPLICATION_UPDATE_CAP_HZ`
- `REPLICATION_IDLE_DISCONNECT_SECONDS`
- `SIDEREAL_PERSIST_INTERVAL_S`
- `SIDEREAL_PERSIST_QUEUE_CAPACITY`
- `SIDEREAL_REPLICATION_SUMMARY_LOGS`
- `SIDEREAL_DEBUG_INPUT_LOGS`
- `SIDEREAL_DEBUG_CONTROL_LOGS`
- `REPLICATION_HEALTH_SNAPSHOT_HZ`
- `REPLICATION_WORLD_SNAPSHOT_HZ`
- `SIDEREAL_VISIBILITY_DELIVERY_RANGE_M`
- `SIDEREAL_VISIBILITY_CELL_SIZE_M`
- `SIDEREAL_VISIBILITY_CANDIDATE_MODE`
- `SIDEREAL_VISIBILITY_BYPASS_ALL`

### 4.2 Shared scripting policy and limits still affecting server/gateway

- `SIDEREAL_SCRIPT_MEMORY_LIMIT_BYTES`
- `SIDEREAL_SCRIPT_INSTRUCTION_LIMIT`
- `SIDEREAL_SCRIPT_HOOK_INTERVAL`
- `SIDEREAL_SCRIPT_MAX_QUERY_RADIUS_M`
- `SIDEREAL_SCRIPT_MAX_QUERIES_PER_HANDLER`
- `SIDEREAL_SCRIPT_MAX_RESULTS_PER_HANDLER`

These are still real, but they behave more like runtime policy/tuning than service bootstrap wiring.

## 5. Recommendations

1. Keep the new startup CLI surface stable and contributor-facing.
2. Do not promote every tuning knob to CLI by default; review them subsystem-by-subsystem instead.
3. Follow up by moving more replication/gateway internals from direct env reads to explicit resources/config structs where that reduces duplication.
4. Revisit whether the dev JWT secret should remain a built-in default once the local-dev bootstrap flow becomes less Makefile-centric.
