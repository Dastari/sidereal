# Client Environment Variable Audit

Date: 2026-03-11
Scope: `bins/sidereal-client`
Status: Audit of all environment-variable inputs accepted by the client code, with relevance and actual usage review

Update note (2026-03-11):
- Native client startup now seeds the core local runtime defaults in-process before app bootstrap when they were not provided explicitly:
  - `GATEWAY_URL=http://127.0.0.1:8080`
  - `SIDEREAL_ASSET_ROOT=.`
  - `REPLICATION_UDP_ADDR=127.0.0.1:7001`
  - `CLIENT_UDP_BIND=127.0.0.1:0`
- CLI precedence remains `CLI > env > built-in default`.

## 1. Executive Summary

The client still accepts a fairly large environment-variable surface. Most of it is real and still meaningful, but it falls into three very different categories:

1. Legitimate runtime configuration
2. Diagnostic/debug controls
3. Transitional kill-switches and partially stale knobs

The highest-value findings are:

1. `SIDEREAL_CLIENT_PHYSICS_MODE=local` is misleading and likely stale. The code prints that it enables "full local simulation, no reconciliation", but the flag does not actually switch the client into a separate local-sim runtime mode.
2. `SIDEREAL_CLIENT_DEBUG_GIZMOS_ON_GAMEPLAY_CAMERA` and `SIDEREAL_CLIENT_DEBUG_ARROW_AS_MESH` are consumed inconsistently. They are parsed into resources, but some startup code re-reads the env directly instead of using those resources.
3. Several `SIDEREAL_CLIENT_DISABLE_*` flags are still meaningful, but they are clearly diagnostic escape hatches rather than stable product-facing configuration. They remain relevant for debugging, but they should be documented as such and reviewed for eventual removal.
4. The env-var surface is not centrally documented. A few variables appear in docs, but many active client knobs only exist in code. Separately, docs still mention at least one old client env var that is no longer accepted by the client.

## 2. Findings

### Finding E1: `SIDEREAL_CLIENT_PHYSICS_MODE=local` is misleading and likely stale
- Severity: High
- Type: correctness, maintainability
- Priority: must fix
- Why it matters:
  The client logs that this flag enables "full local simulation, no reconciliation", which suggests a major runtime-mode switch. The code does not support that claim. Instead, the flag mostly changes diagnostic labeling and suppresses one predicted-adoption path. That makes the variable dangerous because a user could reasonably believe it is a supported local-sim mode when it is not.
- Exact references:
  - `bins/sidereal-client/src/runtime/resources.rs:707`
  - `bins/sidereal-client/src/runtime/resources.rs:713`
  - `bins/sidereal-client/src/runtime/replication.rs:539`
  - `bins/sidereal-client/src/runtime/debug_overlay.rs:1078`
  - `bins/sidereal-client/src/runtime/motion.rs:73`
  - `bins/sidereal-client/src/runtime/motion.rs:539`
- Details:
  `LocalSimulationDebugMode` is created from `SIDEREAL_CLIENT_PHYSICS_MODE`, but the value is only used to:
  - label debug output as `"local"` vs `"predicted"`,
  - change controlled-adoption behavior in one path,
  - relax one audit expectation.

  The system that looks most like a mode switch, `enforce_motion_ownership_for_world_entities`, takes the resource as `_local_mode`, but does not use it.
- Recommendation:
  Choose one of:
  1. implement a real local-simulation mode behind this flag, or
  2. remove/rename the flag and stop claiming it enables full local simulation.

  The second option is more likely correct unless a real local-sim mode is still planned soon.

### Finding E2: debug camera/env handling is inconsistent because startup re-reads env vars directly
- Severity: Medium
- Type: maintainability
- Priority: should fix
- Why it matters:
  The client already has resources for debug-gizmo and debug-arrow behavior, but some startup code bypasses those resources and reads the env again. That produces two configuration paths for the same concept and weakens the resource-driven runtime model.
- Exact references:
  - `bins/sidereal-client/src/runtime/resources.rs:204`
  - `bins/sidereal-client/src/runtime/resources.rs:217`
  - `bins/sidereal-client/src/runtime/mod.rs:185`
  - `bins/sidereal-client/src/runtime/mod.rs:186`
  - `bins/sidereal-client/src/runtime/mod.rs:352`
  - `bins/sidereal-client/src/runtime/scene_world.rs:55`
  - `bins/sidereal-client/src/runtime/scene_world.rs:59`
  - `bins/sidereal-client/src/runtime/camera.rs:304`
- Details:
  `SIDEREAL_CLIENT_DEBUG_GIZMOS_ON_GAMEPLAY_CAMERA` and `SIDEREAL_CLIENT_DEBUG_ARROW_AS_MESH` are each:
  - parsed into resources during client bootstrap,
  - used via those resources in active runtime systems,
  - and then re-read directly from the process environment during scene setup.
- Recommendation:
  Normalize both variables through the existing resources and remove direct env reads from scene/bootstrap code.

### Finding E3: several `SIDEREAL_CLIENT_DISABLE_*` knobs are still meaningful but are clearly transitional escape hatches
- Severity: Medium
- Type: architecture, maintainability
- Priority: should fix
- Why it matters:
  These variables are not dead. They materially alter runtime behavior by removing systems or plugin groups. That makes them useful for debugging, but also risky if they quietly become part of normal launch behavior.
- Exact references:
  - `bins/sidereal-client/src/runtime/mod.rs:210`
  - `bins/sidereal-client/src/runtime/mod.rs:265`
  - `bins/sidereal-client/src/runtime/mod.rs:291`
  - `bins/sidereal-client/src/runtime/plugins.rs:130`
  - `bins/sidereal-client/src/runtime/plugins.rs:131`
- Variables covered:
  - `SIDEREAL_CLIENT_DISABLE_RUNTIME_ASSET_FETCH`
  - `SIDEREAL_CLIENT_DISABLE_REPLICATION_ADOPTION`
  - `SIDEREAL_CLIENT_DISABLE_HIERARCHY_REBUILD`
  - `SIDEREAL_CLIENT_DISABLE_WORLD_VISUALS`
  - `SIDEREAL_CLIENT_DISABLE_MOTION_OWNERSHIP`
- Recommendation:
  Keep them only if they are still needed for active diagnosis. If retained, document them explicitly as diagnostic kill switches, not normal runtime configuration. Otherwise remove them once the underlying subsystem is stable.

### Finding E4: the client env-var surface is real but under-documented and scattered
- Severity: Medium
- Type: maintainability
- Priority: should fix
- Why it matters:
  Active client behavior currently depends on a large number of env vars spread across startup, transport, prediction, rendering, debug overlay, shader loading, BRP, and headless mode. That is manageable only if there is one canonical contributor-facing reference. Right now there is not.
- Exact references:
  - `bins/sidereal-client/src/runtime/mod.rs`
  - `bins/sidereal-client/src/runtime/resources.rs`
  - `bins/sidereal-client/src/runtime/platform.rs`
  - `bins/sidereal-client/src/runtime/transport.rs`
  - `bins/sidereal-client/src/runtime/auth_net.rs`
  - `docs/sidereal_design_document.md:472`
  - `docs/features/prediction_runtime_tuning_and_validation.md:99`
- Details:
  Only a subset of active client env vars appears in docs today. Backend-selection and some prediction tuning are documented; much of the rest is code-only.
- Recommendation:
  Add a dedicated client env-var reference section under `docs/features/` or extend the design doc with a complete categorized client env-var table.

### Finding E5: docs still mention at least one stale client env var that the client no longer accepts
- Severity: Medium
- Type: maintainability, docs divergence
- Priority: should fix
- Why it matters:
  Stale env guidance leads users to set variables that do nothing and confuses runtime diagnosis.
- Exact references:
  - `docs/features/lightyear_integration_analysis.md:349`
  - `docs/features/prediction_runtime_tuning_and_validation.md:57`
- Details:
  Both docs reference `SIDEREAL_CLIENT_REMOTE_ROOT_ANCHOR_FALLBACK`, but there is no active client code accepting that env var in `bins/sidereal-client`.
- Recommendation:
  Remove or update those docs unless the feature is being restored intentionally.

### Finding E6: transport/bootstrap env vars are still relevant and meaningfully used
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  These values directly affect client bootstrap and transport behavior and are not dead config.
- Exact references:
  - `bins/sidereal-client/src/runtime/app_state.rs:100`
  - `bins/sidereal-client/src/runtime/mod.rs:397`
  - `bins/sidereal-client/src/runtime/mod.rs:409`
  - `bins/sidereal-client/src/runtime/transport.rs:97`
  - `bins/sidereal-client/src/runtime/transport.rs:103`
  - `bins/sidereal-client/src/runtime/transport.rs:115`
  - `bins/sidereal-client/src/runtime/transport.rs:124`
  - `bins/sidereal-client/src/runtime/transport.rs:163`
- Variables covered:
  - `GATEWAY_URL`
  - `SIDEREAL_CLIENT_HEADLESS`
  - `SIDEREAL_ASSET_ROOT`
  - `REPLICATION_UDP_ADDR`
  - `CLIENT_UDP_BIND`
  - `REPLICATION_WEBTRANSPORT_ADDR` (WASM)
  - `REPLICATION_WEBTRANSPORT_CERT_SHA256` (WASM)
- Recommendation:
  Keep these. They are still part of the real runtime contract.

### Finding E7: BRP env vars are meaningful, validated, and worth keeping
- Severity: Low
- Type: architecture
- Priority: keep
- Why it matters:
  BRP configuration is handled in one shared place, uses service-specific and generic fallback names, and validates loopback-only plus token requirements.
- Exact references:
  - `bins/sidereal-client/src/runtime/mod.rs:400`
  - `bins/sidereal-client/src/platform/native/remote.rs:10`
  - `crates/sidereal-core/src/remote_inspect.rs:13`
  - `crates/sidereal-core/src/remote_inspect.rs:16`
  - `crates/sidereal-core/src/remote_inspect.rs:22`
  - `crates/sidereal-core/src/remote_inspect.rs:28`
  - `crates/sidereal-core/src/remote_inspect.rs:34`
- Variables covered:
  - `SIDEREAL_CLIENT_BRP_ENABLED`
  - `SIDEREAL_CLIENT_BRP_BIND_ADDR`
  - `SIDEREAL_CLIENT_BRP_PORT`
  - `SIDEREAL_CLIENT_BRP_AUTH_TOKEN`
  - plus generic fallbacks:
    - `SIDEREAL_BRP_ENABLED`
    - `SIDEREAL_BRP_BIND_ADDR`
    - `SIDEREAL_BRP_PORT`
    - `SIDEREAL_BRP_AUTH_TOKEN`
- Recommendation:
  Keep as-is. This is one of the cleaner env-driven subsystems.

## 3. Full Client Env-Var Catalog

Status legend:

- `Active`: real runtime behavior change
- `Debug`: diagnostics/logging/visual debugging only
- `Transitional`: active but looks like a temporary kill switch or migration aid
- `Weak`: accepted but not meaningfully delivering what the name implies
- `Scoped`: only used on one platform/path

### 3.1 Bootstrap, transport, and service endpoints

| Variable | Default | Usage | Status | Notes |
|---|---:|---|---|---|
| `GATEWAY_URL` | `http://127.0.0.1:8080` | Sets default gateway URL in `ClientSession` | Active | Generic name, not client-prefixed |
| `SIDEREAL_CLIENT_HEADLESS` | `false` | Chooses headless app path | Active, Scoped | Native runtime only |
| `SIDEREAL_ASSET_ROOT` | `"."` | Sets asset/cache root passed into client runtime | Active | Used for native and WASM builder inputs |
| `REPLICATION_UDP_ADDR` | `127.0.0.1:7001` | Native Lightyear UDP remote peer if gateway did not provide one | Active, Scoped | Native only |
| `CLIENT_UDP_BIND` | `127.0.0.1:0` | Native local UDP bind address | Active, Scoped | Native only |
| `REPLICATION_WEBTRANSPORT_ADDR` | none | WASM remote WebTransport addr fallback | Active, Scoped | WASM only |
| `REPLICATION_WEBTRANSPORT_CERT_SHA256` | none | WASM certificate digest fallback | Active, Scoped | WASM only |

### 3.2 Headless auth/bootstrap controls

| Variable | Default | Usage | Status | Notes |
|---|---:|---|---|---|
| `SIDEREAL_CLIENT_HEADLESS_PLAYER_ENTITY_ID` | none | Seeds headless session player id | Active, Scoped | Headless-only |
| `SIDEREAL_CLIENT_HEADLESS_ACCESS_TOKEN` | none | Seeds headless session access token | Active, Scoped | Headless-only |
| `SIDEREAL_CLIENT_HEADLESS_SWITCH_PLAYER_ENTITY_ID` | none | Enables scripted account switch | Active, Scoped | Headless-only |
| `SIDEREAL_CLIENT_HEADLESS_SWITCH_ACCESS_TOKEN` | none | Enables scripted account switch | Active, Scoped | Headless-only |
| `SIDEREAL_CLIENT_HEADLESS_SWITCH_AFTER_S` | `1.0` | Delay before headless switch | Active, Scoped | Headless-only |

### 3.3 BRP / remote inspection

| Variable | Default | Usage | Status | Notes |
|---|---:|---|---|---|
| `SIDEREAL_CLIENT_BRP_ENABLED` | `false` | Enables client BRP | Active | Validated loopback-only |
| `SIDEREAL_CLIENT_BRP_BIND_ADDR` | `127.0.0.1` | BRP bind IP | Active | Must remain loopback |
| `SIDEREAL_CLIENT_BRP_PORT` | `15714` | BRP port | Active | Service-specific override |
| `SIDEREAL_CLIENT_BRP_AUTH_TOKEN` | none | Required when BRP enabled | Active | Minimum length 16 |
| `SIDEREAL_BRP_ENABLED` | `false` | Generic fallback for BRP enablement | Active | Shared fallback |
| `SIDEREAL_BRP_BIND_ADDR` | `127.0.0.1` | Generic fallback bind IP | Active | Shared fallback |
| `SIDEREAL_BRP_PORT` | service default | Generic fallback port | Active | Shared fallback |
| `SIDEREAL_BRP_AUTH_TOKEN` | none | Generic fallback token | Active | Shared fallback |

### 3.4 Rendering backend and adapter controls

| Variable | Default | Usage | Status | Notes |
|---|---:|---|---|---|
| `SIDEREAL_CLIENT_WGPU_BACKENDS` | none | First-priority backend selection | Active | Native only |
| `WGPU_BACKEND` | none | Fallback backend selection via `Backends::from_env()` | Active | Native and WASM backend helper paths |
| `SIDEREAL_CLIENT_FORCE_SOFTWARE_ADAPTER` | `false` | Forces WGPU fallback adapter | Active | Native only |

### 3.5 Shader and material controls

| Variable | Default | Usage | Status | Notes |
|---|---:|---|---|---|
| `SIDEREAL_ENABLE_SHADER_MATERIALS` | native=`true`, wasm=`false` | Enables shader-material rendering paths | Active | Broad shared name, not client-specific |
| `SIDEREAL_CLIENT_ENABLE_STREAMED_SHADER_OVERRIDES` | `true` | Enables streamed shader cache overrides | Active | When false, installs built-in fallback shaders only |

### 3.6 Session, prediction, and motion tuning

| Variable | Default | Usage | Status | Notes |
|---|---:|---|---|---|
| `SIDEREAL_CLIENT_SESSION_READY_TIMEOUT_S` | `6.0` | Session-ready watchdog timeout | Active | Used by auth watchdog |
| `SIDEREAL_CLIENT_DEFER_WARN_AFTER_S` | `1.0` | Predicted adoption warn delay | Active | Meaningful |
| `SIDEREAL_CLIENT_DEFER_WARN_INTERVAL_S` | `1.0` | Predicted adoption warn repeat interval | Active | Meaningful |
| `SIDEREAL_CLIENT_DEFER_DIALOG_AFTER_S` | `4.0` | Predicted adoption dialog delay | Active | Meaningful |
| `SIDEREAL_CLIENT_DEFER_SUMMARY_INTERVAL_S` | `30.0` | Predicted adoption/runtime summary interval | Active | Meaningful |
| `SIDEREAL_CLIENT_ROLLBACK_STATE` | `check` | Prediction manager rollback mode | Active | `always`, `disabled`, else `check` |
| `SIDEREAL_CLIENT_MAX_ROLLBACK_TICKS` | `100` | Prediction rollback cap | Active | Applied on manager insertion |
| `SIDEREAL_CLIENT_INSTANT_CORRECTION` | `false` | Instant vs smooth correction policy | Active | Applied on manager insertion |
| `SIDEREAL_CLIENT_NEARBY_COLLISION_PROXY_RADIUS_M` | `200.0` | Nearby collision proxy scan radius | Active | Meaningful |
| `SIDEREAL_CLIENT_NEARBY_COLLISION_PROXY_MAX` | `8` | Max local collision proxies | Active | Meaningful |
| `SIDEREAL_CLIENT_MOTION_OWNERSHIP_RECONCILE_INTERVAL_S` | `0.1` | Motion ownership reconcile interval | Active | Meaningful |
| `SIDEREAL_CLIENT_PHYSICS_MODE` | none | Parses `"local"` into `LocalSimulationDebugMode` | Weak | Name/behavior mismatch |
| `SIDEREAL_CLIENT_MOTION_AUDIT` | `false` | Enables motion ownership audit logging | Debug | Real, but diagnostic |

### 3.7 Debugging and developer visibility

| Variable | Default | Usage | Status | Notes |
|---|---:|---|---|---|
| `SIDEREAL_DEBUG_BLUE_FULLSCREEN` | `false` | Spawns blue fullscreen debug backdrop | Debug | Visual diagnostic only |
| `SIDEREAL_CLIENT_DEBUG_GIZMOS_ON_GAMEPLAY_CAMERA` | `false` | Changes gizmo render-layer/camera behavior | Debug | Parsed twice; should be normalized |
| `SIDEREAL_CLIENT_DEBUG_ARROW_AS_MESH` | `false` | Changes debug velocity-arrow rendering mode | Debug | Parsed twice; should be normalized |
| `SIDEREAL_DEBUG_CONTROL_LOGS` | `false` | Enables control debug logs | Debug | Meaningful |
| `SIDEREAL_DEBUG_INPUT_LOGS` | `false` | Enables input debug logs | Debug | Meaningful |
| `SIDEREAL_CLIENT_LOG_FILE` | generated log path | Overrides panic/startup/dev-console log file path | Debug | Native only; meaningful |

### 3.8 Transitional / kill-switch style variables

| Variable | Default | Usage | Status | Notes |
|---|---:|---|---|---|
| `SIDEREAL_CLIENT_DISABLE_RUNTIME_ASSET_FETCH` | `false` | Skips runtime asset-fetch queue/poll systems | Transitional | Diagnostic escape hatch |
| `SIDEREAL_CLIENT_DISABLE_REPLICATION_ADOPTION` | `false` | Skips replicated-entity adoption | Transitional | Very disruptive if used casually |
| `SIDEREAL_CLIENT_DISABLE_HIERARCHY_REBUILD` | `false` | Disables mounted hierarchy rebuild system | Transitional | Diagnostic only |
| `SIDEREAL_CLIENT_DISABLE_WORLD_VISUALS` | `false` | Skips visuals plugin | Transitional | Useful for isolation, not normal runtime |
| `SIDEREAL_CLIENT_DISABLE_MOTION_OWNERSHIP` | `false` | Removes motion-ownership enforcement/audit path | Transitional | Diagnostic and risky |

## 4. Variables That Are Relevant and Should Be Kept

The following groups are clearly still relevant:

1. transport/bootstrap vars
2. asset-root/cache vars
3. BRP vars
4. WGPU backend selection vars
5. shader/material controls
6. prediction tuning vars
7. headless-mode vars
8. debug logging vars

These all have visible runtime effects and should not be classified as dead.

## 5. Variables That Are Relevant but Should Be Reclassified

These are still meaningful, but they should be explicitly framed as diagnostic or transitional rather than stable user-facing runtime config:

1. `SIDEREAL_CLIENT_DISABLE_RUNTIME_ASSET_FETCH`
2. `SIDEREAL_CLIENT_DISABLE_REPLICATION_ADOPTION`
3. `SIDEREAL_CLIENT_DISABLE_HIERARCHY_REBUILD`
4. `SIDEREAL_CLIENT_DISABLE_WORLD_VISUALS`
5. `SIDEREAL_CLIENT_DISABLE_MOTION_OWNERSHIP`
6. `SIDEREAL_DEBUG_BLUE_FULLSCREEN`
7. `SIDEREAL_CLIENT_MOTION_AUDIT`

## 6. Variables That Need Cleanup

### 6.1 Misleading / weak

1. `SIDEREAL_CLIENT_PHYSICS_MODE`
   - currently weak and misleading
   - should either become real or be removed/renamed

### 6.2 Inconsistently consumed

1. `SIDEREAL_CLIENT_DEBUG_GIZMOS_ON_GAMEPLAY_CAMERA`
2. `SIDEREAL_CLIENT_DEBUG_ARROW_AS_MESH`

These should be routed through their resources everywhere instead of mixing resource use with repeated `std::env::var(...)` calls.

### 6.3 Stale docs references

1. `SIDEREAL_CLIENT_REMOTE_ROOT_ANCHOR_FALLBACK`
   - referenced in docs
   - not accepted by the client anymore

## 7. Recommended Remediation

### Immediate

1. Remove or rename `SIDEREAL_CLIENT_PHYSICS_MODE` unless a real local-sim mode is being implemented now.
2. Normalize debug-gizmo/debug-arrow handling through resources only.
3. Add a canonical client env-var reference doc so launch/runtime debugging is not code archaeology.
4. Remove or update stale docs references to `SIDEREAL_CLIENT_REMOTE_ROOT_ANCHOR_FALLBACK`.

### Short-term

1. Mark all `SIDEREAL_CLIENT_DISABLE_*` variables as diagnostic-only in docs and launch scripts.
2. Decide which transitional flags still earn their keep and which should be deleted.
3. Standardize naming where appropriate:
   - `GATEWAY_URL` vs client-prefixed naming
   - `SIDEREAL_ENABLE_SHADER_MATERIALS` vs client-specific naming if it is meant to be a client-only concern

### Longer-term

1. Centralize env-var parsing into one client config module rather than scattering `std::env::var(...)` across startup and feature modules.
2. Distinguish clearly between:
   - supported runtime configuration,
   - developer diagnostics,
   - temporary subsystem kill switches.

## 8. 2026-03-11 Status Note

At the time of this audit:

1. The client accepted a large but mostly real env-var surface.
2. The most questionable knob was `SIDEREAL_CLIENT_PHYSICS_MODE`, because its behavior no longer matched its own startup message.
3. The cleanest env-var subsystem was BRP configuration, because it is centrally parsed and validated.
4. The least clean area was debug/transitional client startup flags, where behavior is real but the surface is inconsistent and under-documented.
