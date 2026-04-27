# Prediction Parity Layer and Input Auth Plan

- Date: 2026-04-26
- Status: Active implementation plan
- Scope: native prediction parity, Avian client/server simulation, authenticated server input, Lightyear input-auth follow-up

## 0. Status Notes

2026-04-26 implementation start:

1. Added a shared fixed-step simulation plugin in `sidereal-game` with explicit runtime roles:
   - `ServerAuthority`
   - `ClientPrediction`
2. Server and client prediction now share the same schedule boundary for intent application and gameplay simulation.
3. Sidereal realtime input remains the production server-authoritative input path.
4. Client prediction still uses Lightyear native `ActionState<PlayerInput>` locally.
5. Lightyear server-native input remains protocol-only on replication until upstream has both:
   - a fix for input-buffer panic issue `#1200`,
   - generic server input target authorization for the issue tracked as `#1283`.
6. Added an initial Avian-backed parity test that runs server-authority and client-prediction apps with identical flight input and compares motion state.
7. Expanded the parity harness to run Avian's default `FixedPostUpdate` physics schedule and verify real movement/turning for forward thrust, combined thrust/turn, sustained left turn, and sustained right turn.
8. Moved replication-side post-physics visibility, persistence, combat follow-up, and fixed-step diagnostics into `FixedPostUpdate` after Avian writeback so outgoing observer state samples authoritative post-step motion.

## 1. Target Architecture

Sidereal prediction must run the same fixed-step gameplay and Avian physics logic on the server and on the predicting client. The server remains authoritative; the client only predicts the active controlled dynamic root and reconciles from Lightyear-confirmed state.

The canonical input flow is:

1. client samples local input,
2. client writes local Lightyear `ActionState<PlayerInput>` for prediction,
3. client sends authenticated `ClientRealtimeInputMessage`,
4. server authenticates player binding, control generation, and target entity,
5. both sides convert the same input snapshot into `ActionQueue`,
6. shared simulation systems consume `ActionQueue` in fixed tick before Avian physics.

## 2. Lightyear Input Decision

Do not re-enable Lightyear's native server input receive path as Sidereal's authoritative input source yet.

Acceptable Lightyear work must be generic upstream-quality work:

1. split native input plugin role registration into protocol-only, client runtime, and server runtime modes;
2. add a generic server-side input authorization hook before a message can write a target entity's input buffer;
3. preserve upstream default behavior for non-Sidereal users;
4. add Lightyear tests for unauthorized target rejection and the `#1200` panic path.

Sidereal-specific player auth, account binding, and control-generation checks stay in Sidereal.

## 3. Validation Direction

The parity harness must expand until it covers:

1. neutral input,
2. forward thrust,
3. turn left/right,
4. combined thrust and turn,
5. brake,
6. afterburner with and without fuel,
7. mass/inertia changes before prediction activation,
8. control handoff,
9. two-client observer motion.

Only after these scenarios pass should broad repair systems be removed or narrowed.
