# Prediction Runtime Tuning and Validation

Status: Active feature reference
Last updated: 2026-04-28
Owners: client runtime + replication
Scope: Lightyear-native prediction/interpolation behavior verification and production default tuning

## 0. Implementation Status

2026-04-24 status note:

1. Active native-stabilization work is in progress after the Lightyear-native migration.
2. Implemented: client runtime now tracks native focus transitions, sends forced-neutral input on focus loss/regain, suppresses immediate active-input sends for a short recovery window, and exposes recovery state in the debug overlay.
3. Known risk: native full-client tests still include pre-existing visual/transform failures unrelated to the first focus-loss recovery slice; keep this tracker current as those are resolved.
4. WASM impact: no browser-specific runtime behavior changed in the native focus-loss slice; shared prediction code must remain target-compatible.

2026-04-26 status note:

1. Implemented: native/client runtime disables the Lightyear/Avian pre-physics `Transform -> Position` sync while using `PositionButInterpolateTransform`.
2. Reason: Sidereal's simulation state is Avian `Position`/`Rotation` owned; render `Transform` may be visually corrected/interpolated and must not feed back into predicted physics state.
3. Native impact: local predicted control should no longer be reset by stale visual transforms before fixed-step physics runs.
4. WASM impact: shared client runtime behavior only; no target-specific branch was introduced.

2026-04-26 status note:

1. Implemented: client prediction disables authoritative `FuelTank` consumption while continuing to read replicated fuel availability for local thrust prediction.
2. Reason: `FuelTank` is server-owned replicated gameplay state, not a predicted rollback component. Consuming it inside client prediction or rollback can starve local thrust over time and create progressively larger confirmed-vs-predicted corrections.
3. Native impact: controlled ship prediction no longer mutates non-rollback fuel state during input resimulation.
4. WASM impact: shared client runtime behavior only; no target-specific branch was introduced.

2026-04-26 status note:

1. Implemented: unfocused native clients now default `SIDEREAL_CLIENT_UNFOCUSED_MAX_PREDICTED_TICKS` to the focused `SIDEREAL_CLIENT_MAX_PREDICTED_TICKS` value instead of zero.
2. Reason: two local native clients cannot both hold OS window focus; dropping Lightyear prediction lead to zero on each focus change caused artificial stalls/recovery snapbacks during multiplayer diagnostics even though realtime input delivery remained healthy.
3. Native impact: focus changes still force neutral input boundaries and recovery diagnostics, but they no longer reconfigure the input timeline into a no-prediction mode unless explicitly requested with `SIDEREAL_CLIENT_UNFOCUSED_MAX_PREDICTED_TICKS=0` or `--input-unfocused-max-predicted-ticks 0`.
4. WASM impact: shared client runtime default only; browser focus/background throttling remains platform-managed.

2026-04-26 status note:

1. Implemented: shared flight force calculation now derives thrust heading from Avian `Rotation`, not render `Transform`, and client prediction repairs predicted controlled `Mass`/`AngularInertia` whenever replicated size/mass/collision setup changes.
2. Reason: authoritative and predicted physics must use the same simulation-owned state. A predicted clone with missing or stale inertia can rotate faster or slower than the server even when input delivery is healthy.
3. Native impact: controlled ship turning diagnostics now log rotation, angular velocity, mass, and inertia on both client and server via `SIDEREAL_DEBUG_MOTION_REPLICATION=1`.
4. WASM impact: shared simulation/client runtime behavior only; no target-specific branch was introduced.

2026-04-26 status note:

1. Implemented: `sidereal-game` now exposes `SiderealSharedSimulationPlugin`, `SimulationRuntimeRole`, and `SiderealSimulationSet`.
2. Reason: server-authoritative simulation and client prediction need one fixed-step gameplay/Avian pipeline instead of duplicated client/server system lists that can drift.
3. Native impact: replication uses the `ServerAuthority` role; native client prediction uses the `ClientPrediction` role and still relies on client-side marker gating so remote entities do not gain local motion authority. Shared post-physics systems now run in `FixedPostUpdate` after Avian's default physics schedule.
4. WASM impact: shared client runtime behavior only; no target-specific branch was introduced.
5. Validation: `crates/sidereal-game/tests/shared_simulation_plugin.rs` now includes Avian-backed parity tests comparing server-authority and client-prediction motion for forward thrust, combined thrust/turn, sustained left turn, and sustained right turn.

2026-04-26 status note:

1. Implemented: replication post-physics control, visibility, persistence, and fixed-step diagnostics now run in `FixedPostUpdate` after Avian writeback instead of being scheduled in `FixedUpdate` with ineffective cross-schedule `after(PhysicsSystems::Writeback)` ordering.
2. Reason: observer anchors, visibility membership, persistence dirty marking, and streamed tactical/owner state must sample authoritative post-physics motion, not pre-step state.
3. Native impact: remote clients should receive movement based on the server's latest authoritative Avian state for that fixed tick.
4. WASM impact: server-side schedule correction only; no browser-specific branch was introduced.

2026-04-27 status note:

1. Implemented: replication initializes a hydrated player's existing authoritative control lease at generation 1 before the first handoff request, so the first target change after server startup acks generation 2 instead of regressing the client from its local pending generation.
2. Implemented: controlled predicted motion bootstrap ignores older/equal prediction seed generations on the same predicted entity, preventing stale handoff state from clearing prediction history and snapping the entity back to its handoff seed.
3. Implemented: control handoff marks the player entity dirty and forces the next persistence pass so `controlled_entity_guid` survives quick server restarts instead of waiting for the normal persistence interval.
4. Native impact: selecting a ship after login should no longer produce the generation 2 -> 1 reseed pattern seen in native logs, and quick restart/relogin should be less likely to return to free-roam after a successful handoff.
5. WASM impact: shared client runtime and replication-server behavior only; no browser-specific branch was introduced.

2026-04-27 status note:

1. Implemented: native client prediction now disables Lightyear input-based rollback while keeping Lightyear state rollback/correction active.
2. Reason: replication no longer runs Lightyear's native server input receiver, so Lightyear input rollback can only compare against local/native input tracker state that the server did not authoritatively confirm. Sidereal's authoritative reconciliation source is replicated server state generated from authenticated `ClientRealtimeInputMessage`.
3. Implemented: control handoff also writes a targeted player-entity persistence snapshot, and graph persistence ignores older entity/component writes when a newer `last_tick` has already reached the database. This prevents queued broad snapshots from restoring stale `controlled_entity_guid` after a quick restart.
4. Native impact: controlled ships should no longer snap back from Lightyear `Rollback::FromInputs` history after the server moves them, and successful ship selection should persist without waiting for large world snapshots.
5. WASM impact: shared client prediction and replication persistence behavior only; no browser-specific branch was introduced.

2026-04-27 status note:

1. Implemented: the default native prediction rollback state is now Lightyear state rollback `Always` while input rollback remains disabled.
2. Reason: dynamic control handoff can reseed or replace the predicted lane. Relying on Lightyear's state rollback `Check` mode allowed a handoff to miss correction when local prediction history was stale, leaving `Confirmed<Position>` moving while the local predicted `Position` stayed behind. `Always` keeps reconciliation owned by Lightyear but forces every confirmed state update to drive rollback/resimulation.
3. Native impact: ship and free-roam handoffs should converge to server state through Lightyear rollback instead of leaving the predicted entity stuck at the handoff origin.
4. WASM impact: shared client runtime behavior only; no target-specific code path was introduced.

2026-04-27 status note:

1. Implemented: shared character/free-roam movement now distinguishes physics-backed entities from non-physics player anchors.
2. Reason: Avian `RigidBody` entities should set `LinearVelocity` and let physics integrate motion, but a self-controlled player anchor can have `LinearVelocity` without `RigidBody`; in that lane, shared fixed-step movement must also advance `Position`/`Transform` or client prediction stays at the handoff origin while the server confirms movement.
3. Native impact: switching from a ship to free-roam/self-control should no longer produce local-prediction snapbacks caused by the player anchor receiving velocity but not moving locally.
4. WASM impact: shared simulation behavior only; no target-specific branch was introduced.

2026-04-27 status note:

1. Implemented: `ServerSessionReadyMessage` now carries the server's current `control_generation` and authoritative `controlled_entity_id`.
2. Reason: reconnecting clients could enter world with local `controlled_entity_generation = 0` while the replication server retained a later lease generation for the same player, causing every realtime input packet to be rejected as `stale_control_generation` until a separate control request happened.
3. Native impact: reconnect/session bootstrap starts input from the current server lease instead of requiring a control handoff ACK before movement can be accepted.
4. WASM impact: shared replication protocol change; browser clients must be rebuilt against the same protocol version.

2026-04-27 status note:

1. Implemented: replication now removes timed-out latest realtime input snapshots, stale-generation snapshots, and fresh-generation snapshots that target a no-longer-controlled entity as soon as the fixed-step drain observes them.
2. Implemented: explicit client disconnect notify also clears that player's realtime input tick tracker, rate-limit windows, latest input snapshot, and input activity timestamp.
3. Reason: latest realtime input is short-lived intent, not durable session state. Leaving an unusable snapshot resident after disconnect or handoff caused repeated fixed-tick "no actions after realtime selection" diagnostics and allowed stale state to keep contributing to drop counters.
4. Native impact: crashed or closed native clients should stop producing repeated server input-route logs once their input snapshot times out or disconnect notify is processed.
5. WASM impact: replication-server behavior only; no browser-specific branch was introduced.

2026-04-27 status note:

1. Implemented: replication disconnect cleanup now neutralizes the disconnected player's current authoritative control target by clearing its `ActionQueue`, setting `FlightComputer` throttle/yaw to neutral, clearing `brake_active`, and disabling afterburner state.
2. Reason: realtime input snapshots are only one source of intent. A previously processed action can leave durable `FlightComputer` state non-neutral; if a client crashes while thrusting or turning, the server must not keep applying that stale player intent and burning fuel after the session is gone.
3. Native impact: crashed or disconnected native clients should leave ships coasting with neutral controls instead of continuing thrust/turn input in the background.
4. WASM impact: replication-server behavior only; no browser-specific branch was introduced.

2026-04-27 status note:

Superseded later on 2026-04-27 by the Lightyear-only controlled reconciliation note below.

1. Implemented: client prediction treats disabled fuel consumption as a prediction-only mode and no longer requires local replicated `FuelTank` availability before applying thrust or torque.
2. Superseded implementation: controlled predicted entities applied each new `ConfirmedTick` as a Sidereal-owned correction delta against `PredictionHistory<Position>`, `PredictionHistory<Rotation>`, `PredictionHistory<LinearVelocity>`, and `PredictionHistory<AngularVelocity>`.
3. Reason: the server remains authoritative for fuel and motion, but a selected ship can temporarily lack complete owner-only module/fuel state on the client. Confirmed server state must still reconcile the predicted lane instead of leaving the local body stuck at its handoff pose while the server ghost moves away.
4. Native impact: selecting a ship after login or control handoff should converge toward the authoritative server pose even if prediction parity is temporarily incomplete.
5. WASM impact: shared client/runtime and shared simulation behavior only; no target-specific branch was introduced.

2026-04-27 status note:

1. Implemented: Sidereal now patches Lightyear's UDP transport send path locally so `WouldBlock`/`EAGAIN` socket backpressure preserves the unsent packet and remaining per-link queue for the next frame instead of draining and dropping them.
2. Reason: reconnect/login replication bursts can temporarily fill the UDP socket send buffer; treating that as packet loss caused server-side `lightyear_udp::server` error spam and could starve confirmed-state delivery immediately after client connection.
3. Native impact: native UDP transport should degrade to extra latency under short send-buffer backpressure instead of losing queued replication packets during session bootstrap.
4. WASM impact: no browser transport behavior changed; the patch is in the UDP transport crate used by native runtime.

2026-04-27 status note:

1. Implemented: Sidereal's custom realtime input bridge now suppresses active local input when the controlled predicted entity's `ConfirmedTick` falls beyond the configured prediction budget.
2. Reason: Lightyear still owns rollback/correction, but Sidereal injects `ActionState<PlayerInput>` from its authenticated realtime input lane. That bridge must not keep advancing prediction indefinitely when confirmed server state has stopped arriving or is far outside the allowed lead.
3. Native impact: a second unfocused/local client or a client connected to a stale/disconnected server should enter a neutral recovery window instead of building hundreds of unconfirmed predicted ticks and then rubberbanding violently.
4. WASM impact: shared client runtime behavior only; no target-specific branch was introduced.

2026-04-27 status note:

1. Implemented: free-roam/player-anchor control now follows the same predicted-readiness rule as ship/entity control. The client no longer binds `ControlledEntity` or `SimulationMotionWriter` to a confirmed/interpolated player-anchor fallback while waiting for Lightyear to spawn the predicted clone.
2. Implemented: client motion-ownership reconciliation now refuses confirmed/interpolated fallback targets for all control lanes, including player-anchor free roam.
3. Implemented: replication observer-anchor position resolution now prefers authoritative Avian f64 `Position` before f32 `GlobalTransform`/`Transform` fallback.
4. Reason: control handoff must preserve single-writer motion ownership. Letting free roam write to a confirmed/interpolated anchor allowed local prediction and replicated correction to fight during ship -> free-roam switches.
5. Native impact: switching between a ship and free roam should wait briefly for the predicted anchor instead of producing local snap/jump behavior.
6. WASM impact: shared client/runtime and replication-server behavior only; no target-specific branch was introduced.

2026-04-27 status note:

1. Implemented: the Sidereal client no longer schedules a custom controlled-motion reconciliation writer. `seed_controlled_predicted_motion_from_confirmed` remains only for control-generation bootstrap; ongoing correction is owned by Lightyear rollback/visual correction.
2. Implemented: removed the per-entity `ControlledPredictionReconciliationState` component and the emergency fallback correction path.
3. Implemented: client motion diagnostics still log confirmed tick/history state and prediction-history length/newest tick for controlled entities, so rubber-band reports can distinguish confirmed-state latency from missing rollback history without Sidereal writing corrections itself.
4. Reason: controlled prediction must have one correction owner. Applying any Sidereal-side correction directly to Avian `Position`, `Rotation`, `LinearVelocity`, or `AngularVelocity` can double-correct with Lightyear and progressively worsen rotation during back-and-forth input.
5. Native impact: slow controlled flight and on-the-spot rotation should no longer receive any Sidereal-side reconciliation corrections after the initial control-generation seed.
6. WASM impact: shared client/runtime behavior only; no browser-specific branch was introduced.

2026-04-28 status note:

1. Implemented: client realtime input and local `ActionState<PlayerInput>` insertion now require `ControlBootstrapState::ActivePredicted` for the exact active control lease. Confirmed/interpolated GUID fallbacks are no longer eligible for input authority.
2. Implemented: pending control requests and pending predicted bootstrap remove local input markers, local action state, and `SimulationMotionWriter` instead of allowing old or fallback entities to keep receiving input.
3. Implemented: control ACK/reject lease changes reset client input send/ack state so ticks and last-sent actions from one target/generation cannot leak into the next target/generation.
4. Implemented: replication accepted rebinds explicitly clear short-lived realtime input state for the authenticated player and neutralize old and new targets before fresh input is accepted.
5. Reason: control handoff must be a lease boundary. If the predicted clone for the authoritative target/generation is missing, the correct behavior is to suppress active input and surface diagnostics, not to write fallback simulation state locally.
6. Native impact: ship/free-roam/ship switching should no longer drive confirmed or interpolated entities locally while Lightyear is still establishing the predicted lane.
7. WASM impact: shared client/runtime and replication-server behavior only; no target-specific branch was introduced.

2026-04-28 status note:

1. Implemented: server control-role rearm is now staged across visibility membership passes instead of calling `lose_visibility()` and `gain_visibility()` in the same tick.
2. Implemented: the client-side conflicting `Predicted`/`Interpolated` marker sanitizer was removed. A runtime entity must receive exactly one Lightyear role from the server topology rather than being repaired locally after a mixed-role spawn.
3. Reason: the latest native logs showed the server accepting generation-2 ship input and moving the authoritative ship, while the client promoted the already-interpolated ship into prediction, sanitized both markers locally, and then left stale Lightyear prediction/interpolation sidecar state to fight rollback/visual correction.
4. Native impact: switching from free roam to an already-visible owned ship should now wait for a clean Lightyear role transition instead of mutating one local clone from observer to prediction in place.
5. WASM impact: shared client/runtime and replication-server behavior only; no target-specific branch was introduced.

2026-04-28 status note:

1. Implemented: removed the active client systems that fabricated missing `Confirmed<T>` motion mirrors for interpolated entities, inserted a missing Lightyear `PredictionManager`, continuously synced interpolated transforms without Lightyear history, and repaired predicted/interpolated visual transforms in PostUpdate.
2. Implemented: presentation bootstrap now uses the existing hidden-until-ready reveal gate only; it does not keep repairing dynamic interpolated transforms while waiting for Lightyear interpolation history.
3. Reason: these systems were local repair shims for invalid or incomplete Lightyear role/bootstrap states. After server-side staged role rearm, mixed-role handoff is not a valid recoverable state; it should be prevented by topology and caught by diagnostics/tests, not patched on the client.
4. Native impact: control handoff presentation now relies on clean Lightyear role lifecycle plus the hidden-until-ready visual gate. It no longer seeds Lightyear confirmation sidecars, creates missing Lightyear prediction managers, or repairs predicted transforms locally after rollback/visual correction.
5. WASM impact: shared client/runtime behavior only; no target-specific branch was introduced.

## 1. Purpose

Track remaining non-structural work after Lightyear-native migration completion:
- prediction/interpolation behavior tuning under load,
- rollback/correction default validation,
- deferred adoption telemetry baselining.

## 2. Current Baseline

- Lightyear-native replication/prediction/interpolation is active.
- Legacy world-delta runtime paths are removed.
- Legacy mirror-motion components are removed from runtime simulation/replication flow.
- Fixed-step simulation remains authoritative at 60 Hz.

2026-03-11 update:
- Shared core `SIM_TICK_HZ` now matches the active 60 Hz client and replication runtime.

## 2.1 Current Native Runtime Status (2026-03-08)

- Native client now reaches in-world state and can render replicated ships after world entry.
- In-world controls are not yet functioning reliably for the controlled entity.
- Motion/correction behavior still shows intermittent jumping/snapping and needs focused native debugging before feel tuning can be considered complete.
- Native runtime stabilization is the immediate priority; resumed browser/WASM parity validation should wait until these native control and motion issues are under control.

## 2.2 Runtime Safeguards (2026-02-26)

- Client realtime input sending is change-driven with heartbeat:
  - send immediately when action set changes,
  - send immediately when routed controlled entity changes,
  - otherwise send heartbeat at 10 Hz (`0.1s`) to preserve liveness.
  - when the primary window is unfocused, client sends neutral intent snapshots (prevents stuck held-key intent across focus changes).
- Transport channel QoS separation:
  - realtime input uses `InputChannel` (`SequencedUnreliable`, latest-wins),
  - control/session uses `ControlChannel` (reliable),
  - asset stream/request/ack uses `AssetChannel` (reliable, isolated from input path).
- Server ingress keeps latest-intent semantics:
  - validates tick ordering per authenticated client/player/controlled-entity/control-generation input stream and rate limits per authenticated player,
  - validates `control_generation` against the server-issued control lease before applying tick-order rejection or accepting a realtime input snapshot,
  - clears per-player realtime input tick/latest-intent state on a fresh authenticated bind and on accepted control rebind so a restarted native client or newly active target/generation is not rejected against stale ticks from a prior lease,
  - stores latest input snapshot by player/tick,
  - expires realtime input snapshots after `REPLICATION_REALTIME_INPUT_TIMEOUT_SECONDS` (default `0.35s`) so authoritative motion cannot stay latched if the client loses focus, background-throttles, or misses the neutral heartbeat,
  - removes expired, stale-generation, or wrong-controlled-target latest snapshots during drain so disconnected clients and completed control handoffs do not leave reusable input residue,
  - drains into `ActionQueue` by replace/overwrite (`queue.clear()` then push latest actions), never backlog append.
- Remote/non-controlled visual smoothing path:
  - replicated Avian motion components (`Position`, `Rotation`, `LinearVelocity`, `AngularVelocity`) are registered with Lightyear interpolation functions in protocol registration.
  - non-controlled root entities are now receive-only without `Predicted`/`Interpolated` markers, so replication applies motion directly to Avian components and transform sync follows authoritative values.
  - client render transform sync applies frame-rate-independent visual smoothing for non-controlled, non-proxy entities and snaps on large corrections, while controlled/proxy entities continue snap-sync behavior.
  - this avoids `Predicted -> Interpolated` marker-transition gaps for remote roots while preserving local controlled prediction/reconciliation behavior.
- Visibility/anchor consistency guard:
  - replication observer-anchor position updates and visibility lookups canonicalize `player_entity_id` (UUID wire form) before spatial-delivery evaluation to avoid asymmetric visibility when mixed ID formats are present.
  - server visibility and observer-anchor sampling in `FixedUpdate` now read authoritative Avian `Position` first (with transform fallback) instead of relying solely on `GlobalTransform`, preventing stale/zero spatial samples during low-FPS transform-propagation lag.
  - replication delivery scope range is runtime-tunable via `SIDEREAL_VISIBILITY_DELIVERY_RANGE_M` (default `300` meters), enabling controlled validation of range-culling behavior without code changes.
- Auth/visibility handshake stability guard:
  - client auth resend retries are gated by `ServerSessionReady` for the selected player (not by first replicated-world observation), preventing repeated auth rebinding loops while visibility is still warming up.
  - replication auth handling is idempotent for repeated same-client/same-player auth messages and no longer replays global `lose_visibility` reset for that case.
- Remote-root anchor consistency fallback:
  - replication server mirrors controlled ship motion (`Position`, `Rotation`, `LinearVelocity`, `AngularVelocity`) onto the corresponding player anchor in fixed tick.
  - client fallback that aligns remote controlled ship-root motion to remote player-anchor motion is disabled by default; enable only for diagnostics with `SIDEREAL_CLIENT_REMOTE_ROOT_ANCHOR_FALLBACK=1`.
- Client single-writer hardening for remote entities:
  - client fixed-tick writer path excludes generic global mutation systems (`validate_action_capabilities` and global `recompute_total_mass`); mass/inertia sync is controlled-entity-scoped.
  - client-side idle-stabilization and angular clamp post-physics systems are scoped to `ControlledEntity` only, preventing local mutation of replicated remote interpolated `LinearVelocity`/`AngularVelocity` when `FlightComputer` is present on remote roots.
  - flight writer systems are gated by runtime `FlightControlAuthority` marker:
    - server assigns marker to authoritative `FlightComputer` roots by default,
    - client assigns marker only to locally controlled root and removes it from receive-only roots,
    - replicated entities retain `FlightComputer` data (no destructive client stripping), preserving component parity for effects/inspection while enforcing single-writer motion ownership.
  - client prediction does not consume replicated `FuelTank` state; only the authoritative server path burns fuel.
- WASM impact:
  - no target-specific branching introduced,
  - interpolation registration and scheduling behavior are shared between native and WASM builds.

2026-03-22 update:

- Dynamic predicted/interpolated handoff is still a live Lightyear edge in Sidereal's current fork/runtime shape.
- Native client input timeline no longer defaults to zero input delay.
  - Sidereal now defaults `SIDEREAL_CLIENT_INPUT_DELAY_TICKS` to `2` for native timeline setup, with `--input-delay-ticks` as the equivalent CLI override.
  - Reason: the project is currently reproducing multi-second confirmed-vs-predicted drift and aborted rollbacks under `fixed_input_delay(0)`, and Lightyear upstream already treats zero-delay localhost timing as fragile.
- Superseded 2026-04-28: the client runtime no longer seeds missing `Confirmed<T>` mirrors for Avian motion components on interpolated entities. Clean role rearm must provide a valid Lightyear lane; incomplete lanes remain hidden or diagnosed instead of locally fabricating confirmation state.
- World-entry auth/bootstrap sequencing is tighter now.
  - Replication now sends `ServerSessionDeniedMessage` for every terminal auth rejection after the peer is identified, including invalid token, token/player mismatch, account ownership mismatch, and missing runtime player cases.
  - Native client bootstrap-required asset download now waits for `ServerSessionReady` for the selected player instead of starting immediately after `/world/enter` acceptance.
  - Goal: let the observer lane become immediately presentable and interpolation-ready without waiting for a later delta to populate `Confirmed<T>`.
- Superseded 2026-04-28: conflicting `Predicted` + `Interpolated` marker cleanup was removed from the client. Mixed-role entities are invalid topology; server visibility role rearm now stages loss before re-gain so the client receives a clean role lifecycle.
- Native impact:
  - reduces the chance that the local prediction timeline outruns confirmed state badly enough to exceed the rollback budget after focus churn or localhost sync jitter.
  - reduces long observer-transition stalls where the authoritative/server lane is advancing but the local observer presentation is still waiting for confirmed bootstrap.
- WASM impact:
  - shared runtime behavior only; no target-specific branching added.

## 2.4 Input Liveness Guard (2026-03-09)

- Sidereal now treats realtime input snapshots as short-lived intent, not durable movement state.
- The replication server clears authoritative input for a player when no fresh realtime snapshot has arrived within `REPLICATION_REALTIME_INPUT_TIMEOUT_SECONDS` (default `0.35s`).
- This guard is intentionally longer than the client heartbeat interval (`0.1s`) so ordinary jitter does not zero live controls, but short enough to stop stale held-key motion when a native client is alt-tabbed or OS-throttled before it can deliver an unfocused neutral snapshot.
- Native impact:
  - losing window focus should no longer leave the server simulating old movement/fire intent indefinitely if the client stops running its fixed input send path in the background.
- WASM impact:
  - no WASM-specific branching; the same authoritative stale-input expiry applies to browser clients.

2026-04-24 update:

- Native focus-loss recovery now has an explicit client-side recovery resource and diagnostics.
  - On focus loss/regain, the native client marks a forced neutral realtime input send so the transition is an intentional input boundary rather than waiting for the ordinary heartbeat.
  - After a meaningful unfocused interval (`SIDEREAL_CLIENT_FOCUS_RECOVERY_MIN_UNFOCUSED_S`, default `0.5`), focus regain enters a short active-input suppression window (`SIDEREAL_CLIENT_FOCUS_RECOVERY_SUPPRESS_INPUT_S`, default `0.15`) while replication continues.
  - The debug overlay exposes the prediction recovery phase, suppression state, last unfocused duration, transition count, and forced-neutral send count.
- Direct predicted Avian state realignment from confirmed authoritative state is not enabled in this slice.
  - The planned threshold controls are parsed and logged (`SIDEREAL_CLIENT_FOCUS_RECOVERY_RESYNC_AFTER_S`, default `1.0`; `SIDEREAL_CLIENT_FOCUS_RECOVERY_MAX_TICK_GAP`, default `60`) for the next measured implementation phase.
  - Any future realignment must remain scoped to the active local predicted root and must not mutate observer/interpolated clones.
- Native impact:
  - refocus no longer immediately sends/apply held active keyboard state as the first post-focus input packet;
  - long focus stalls have observable recovery state before adding direct prediction resync.
- WASM impact:
  - no target-specific WASM behavior changes in this slice;
  - the resource/system lives in shared client runtime, but browser visibility/focus semantics are not mapped into this recovery policy yet.

2026-04-24 update:

- Realtime input packets now carry the authoritative `control_generation` associated with the client's current controlled target.
- The replication server rejects stale-generation realtime input before it updates latest-intent state or drains into an authoritative `ActionQueue`.
- Native impact:
  - delayed pre-handoff packets can no longer apply held movement/fire intent to the newly controlled entity after a control lease advances.
- WASM impact:
  - no platform branch; browser clients use the same input payload and lease validation.

2026-04-26 update:

- Realtime input ordering state is now scoped to the authenticated client input stream instead of only the player UUID.
- A fresh authenticated bind clears that player's previous latest realtime input and sequence/rate windows, matching the existing control-request sequence reset.
- Native impact:
  - restarting or reconnecting a native client no longer causes the replication server to drop the new lower tick stream as duplicate/out-of-order against a prior session.
- WASM impact:
  - no platform branch; browser clients use the same authenticated session reset and stream-scoped ordering.

2026-04-27 update:

- Sidereal patches Lightyear's `lightyear_inputs` crate locally so client input buffers retain 512 ticks instead of upstream 0.26.4's hardcoded 20 ticks.
- Reason: Sidereal's native rollback budget is currently 160 ticks. When a correction replay crossed Lightyear's 20-tick input-retention floor, the client no longer had historical `ActionState<PlayerInput>` samples and could replay from the control-handoff seed with missing/neutral input, producing repeated snapbacks while the authoritative server continued moving.
- Native impact:
  - predicted controlled motion now has enough local input history to replay across the configured rollback window after server restart, control handoff, focus jitter, or localhost stalls.
- WASM impact:
  - shared client/runtime dependency behavior only; no target-specific branch was introduced.

2026-04-24 update, superseded 2026-04-28:

- Client conflicting `Predicted` + `Interpolated` marker cleanup now resolves the intended local control lane from any pending control request first, then desired control state, then the last acknowledged authoritative target.
- This prevents a dynamic handoff race where Lightyear can deliver the target entity's `Predicted` marker before the reliable control ack updates `LocalPlayerViewState`; the sanitizer must keep `Predicted` for that pending target instead of stripping it as an observer marker.
- Superseded reason: the client-side sanitizer is removed. Correctness now comes from server-side staged visibility role rearm plus client refusal to bind input until `ControlBootstrapState::ActivePredicted`.
- Native impact:
  - owner-controlled ship handoff should no longer get stuck in `PendingPredicted` after a successful server ack when the prediction marker arrives slightly before the ack.
- WASM impact:
  - shared client runtime behavior only; no target-specific branch.

2026-04-28 update:

- Client local control view state is now lease-owned by `ServerSessionReadyMessage` and `ServerControlAckMessage` / `ServerControlRejectMessage` only.
- The legacy client replication sync that copied replicated `ControlledEntityGuid` from the local player entity back into `LocalPlayerViewState` was removed. Replicated control components are confirmation/diagnostic data during handoff, not a writer for pending or acknowledged local intent.
- Client adoption now ignores `EntityGuid` entities that do not have a Lightyear replication lane marker (`Replicated`, `Predicted`, or `Interpolated`). This prevents orphan/non-network-role entities from being promoted into rendered `WorldEntity` state and flickering as visibility changes.
- Native impact:
  - ship fuel HUD, scanner availability, selected-control UI, and `ControlledEntity` tagging should continue to follow the latest session-ready/control-ACK lease even if stale replicated player-anchor components arrive afterward.
  - free-roam/frustum visuals should no longer adopt unclassified `EntityGuid` entities as replicated world visuals.
- WASM impact:
  - shared client runtime behavior only; no target-specific branch was introduced.

## 2.3 Dynamic Handoff Lightyear Exception (2026-03-09)

- Lightyear applies `Predicted` / `Interpolated` classification from the spawn action delivered to a receiver.
- Sidereal's dynamic control handoff can promote an already-visible entity into the owner-predicted lane after initial replication.
- For that Sidereal-specific case, the replication server intentionally forces a sender-local respawn transition on handoff by cycling visibility for the affected receiver after updating `PredictionTarget` / `InterpolationTarget`.
- For owner-specific control lanes, Sidereal prefers `manual(vec![client_sender_entity])` targets once the concrete `ClientOf` sender entity is known.
  - This is narrower than the generic peer-id `NetworkTarget` form used in many Lightyear examples.
  - The reason is Sidereal's runtime can retarget ownership after connect/auth/hydration, and sender-entity targeting avoids depending on a second remote-id-to-sender resolution step during those handoff transitions.
- The persisted player-anchor replication sync is intentionally idempotent.
  - Sidereal keeps reevaluating anchor-vs-ship control mode every fixed tick, but it must not blindly reinsert Lightyear target components each frame.
  - Replacing `PredictionTarget` / `InterpolationTarget` unnecessarily can fight the hook-driven per-sender replication state that Lightyear maintains.
- This is an intentional exception to the simpler "predict on first spawn and never retarget" model used by many Lightyear examples.

## 3. Remaining Work

1. Validate prediction/interpolation behavior under gameplay load:
   - confirmed/predicted/interpolated entity behavior remains stable under connect/disconnect churn.
   - controlled entity input path actually produces authoritative in-world motion.
   - intermittent correction/jump behavior is removed or reduced to intentional correction cases only.
2. Validate and lock correction/rollback defaults:
   - `SIDEREAL_CLIENT_MAX_ROLLBACK_TICKS`
   - `SIDEREAL_CLIENT_INSTANT_CORRECTION`
3. Run controlled multi-client load sessions and capture deferred-adoption telemetry:
   - `avg_wait_s`
   - `max_wait_s`
4. Lock recommended defaults for defer/adoption diagnostics:
   - `SIDEREAL_CLIENT_DEFER_WARN_AFTER_S`
   - `SIDEREAL_CLIENT_DEFER_WARN_INTERVAL_S`
   - `SIDEREAL_CLIENT_DEFER_DIALOG_AFTER_S`
   - `SIDEREAL_CLIENT_DEFER_SUMMARY_INTERVAL_S`

2026-03-11 update:

1. Native client prediction/runtime tuning values now have command-line equivalents in addition to env vars; use `sidereal-client --help` for the current native option surface.
2. The old env-driven debug startup toggles were removed from the native client startup path. This playbook should only reference active prediction/runtime tuning inputs, not debug-only launch flags.

## 4. Runtime Tuning Playbook

1. Start with defaults:
   - `SIDEREAL_CLIENT_DEFER_WARN_AFTER_S=1.0`
   - `SIDEREAL_CLIENT_DEFER_WARN_INTERVAL_S=1.0`
   - `SIDEREAL_CLIENT_DEFER_DIALOG_AFTER_S=4.0`
   - `SIDEREAL_CLIENT_DEFER_SUMMARY_INTERVAL_S=30.0`
2. Run at least 2 concurrent clients with repeated reconnect + immediate input bursts.
3. Watch logs for:
   - `predicted adoption delay summary` (`samples`, `avg_wait_s`, `max_wait_s`)
   - controlled-adoption delay/stall warnings
   - correction/rollback configuration logs from prediction-manager setup
4. Tune thresholds:
   - raise warn thresholds if harmless startup delays spam warnings,
   - lower dialog threshold if real control gaps are being hidden.

## 5. Acceptance Criteria

- Controlled entity appears consistently within acceptable join latency under expected load.
- Prediction anomaly warnings are rare or absent during nominal operation.
- Locked defaults are documented in this file and reflected in runtime env documentation.

## 6. References

- `docs/sidereal_implementation_checklist.md`
- `docs/sidereal_design_document.md`
- `bins/sidereal-client/src/native.rs`
- `bins/sidereal-replication/src/replication/runtime_state.rs`

2026-03-22 update:

1. Shared authoritative flight now uses fixed-step time for thrust application.
   - `crates/sidereal-game/src/flight.rs` now reads `Res<Time<Fixed>>` in `apply_engine_thrust`.
   - This aligns the client prediction path and replication authoritative path with the repo rule that simulation math must be fixed-step only.
2. Client control binding now uses an explicit bootstrap state rather than clone preference alone.
   - `bins/sidereal-client/src/runtime/resources.rs` now defines `ControlBootstrapState` / `ControlBootstrapPhase`.
   - `bins/sidereal-client/src/runtime/replication.rs` now keeps non-anchor ship control in `PendingPredicted` until a real `Predicted` root exists, instead of falling back to confirmed/interpolated ship control.
3. Motion ownership now consumes that bootstrap state as an input contract.
   - `bins/sidereal-client/src/runtime/motion.rs` prefers the explicit active predicted bootstrap state instead of rediscovering control solely from clone scoring.
4. Debug overlay diagnostics now expose the control bootstrap phase directly.
   - This makes the two-client repros easier to classify as `Pending`, `Anchor`, or `Predicted` control states instead of relying only on `Control Lane`.
5. Replication role rearm was narrowed.
   - `bins/sidereal-replication/src/replication/control.rs` now rearms visible clients only when the replication topology itself changes (`Replicate`, `PredictionTarget`, `InterpolationTarget`), not on every control-bookkeeping mutation.
6. Control bootstrap now follows a server-issued lease generation.
   - `ServerControlAckMessage` / `ServerControlRejectMessage` now carry `control_generation`.
   - `bins/sidereal-client/src/runtime/replication.rs` now prefers that authoritative generation when transitioning `ControlBootstrapState`, instead of relying only on local target-string change detection.
7. Transform repair was narrowed after the Lightyear fork gained late-lane Avian transform bootstrap.
   - `bins/sidereal-client/src/runtime/transforms.rs` now seeds only uninitialized `FrameInterpolate<Transform>` state for predicted/interpolated lanes rather than using broad drift snapback as a normal runtime path.
8. Client timeline tuning is now less aggressive under native focus churn.
   - `bins/sidereal-client/src/runtime/transport.rs` now configures a bounded focused prediction window and, by default, preserves that same bounded window while unfocused. A zero unfocused prediction window remains available only as an explicit diagnostic override.
   - The same file now inserts a tuned `InterpolationConfig` so remote observer entities keep a slightly deeper interpolation buffer instead of riding the default low-delay path.

2026-04-26 update:

1. Controlled predicted motion now seeds from confirmed server state when a control generation becomes active.
   - `bins/sidereal-client/src/runtime/motion.rs` copies `Confirmed<Position>`, `Confirmed<Rotation>`, `Confirmed<LinearVelocity>`, and `Confirmed<AngularVelocity>` into the active local predicted entity once per `ControlBootstrapState` generation.
   - The same handoff step seeds Lightyear `PredictionHistory` and `FrameInterpolate<Transform>` at the confirmed/current timeline ticks so the first rollback after a ship-control switch cannot replay from a stale origin baseline.
2. This is a bootstrap invariant, not a continuous drift repair path.
   - Normal movement remains shared fixed-step prediction plus server reconciliation.
   - Native impact: ship-control handoff starts prediction from the latest authoritative pose.
   - WASM impact: same shared client runtime code path; no platform-specific behavior was added.
