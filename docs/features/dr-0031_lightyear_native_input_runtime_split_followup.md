# DR-0031: Lightyear Native Input Runtime Split Follow-Up

- Status: Accepted
- Date: 2026-03-08
- Owners: networking / replication runtime

## Context

- The replication server accumulates long-running Bevy warnings such as:
  - `lightyear_inputs::client::* has not run for 3258167296 ticks`
  - `lightyear_replication::host::HostServerPlugin::add_prediction_interpolation_components has not run ...`
- We also hit a direct upstream Lightyear input failure tracked in `docs/features/lightyear_upstream_issue_snapshot.md` as issue `#1200`, `Panic: subtract with overflow`, in `lightyear_inputs::server::receive_input_message`.
- Sidereal already has an explicit authenticated authoritative input lane on the replication server via `ClientRealtimeInputMessage`, player/session binding, and `LatestRealtimeInputsByPlayer`.
- The client still needs Lightyear native input locally for predicted `ActionState<PlayerInput>` / `InputMarker<PlayerInput>` behavior.

## Decision

Sidereal will temporarily split runtime usage this way:

- client runtime keeps Lightyear native input enabled for local prediction/runtime `ActionState<PlayerInput>` behavior,
- replication server keeps only the Lightyear native input protocol registration needed for wire compatibility with native clients, but does not run the upstream native server receive/update systems,
- authoritative server-side input continues through Sidereal's authenticated `ClientRealtimeInputMessage` path only.

We still want the upstream Lightyear runtime split/fix, but Sidereal will not keep the replication server on the crashing upstream native-input path while `#1200` remains unresolved.

## Required upstream change

The preferred Lightyear fix is one of:

1. Expose the native input sequence type cleanly and allow explicit runtime-specific registration.
2. Add distinct native plugins:
   - client-only native input plugin
   - server-only native input plugin
3. Or make `lightyear_inputs_native::plugin::InputPlugin<A>` role-configurable so it does not unconditionally install both sides.

Sidereal should then reevaluate whether the replication server needs Lightyear native server input at all, or whether the authenticated Sidereal realtime input lane remains the cleaner authoritative path.

## Why keep this local mitigation?

- It does not duplicate Lightyear native-input internals; it only avoids the crashing upstream server receive/update path while keeping protocol registration intact.
- It preserves the existing authoritative Sidereal input contract instead of routing server authority through two parallel input systems.
- It avoids a known upstream overflow panic on the replication server while keeping the client-side prediction path intact.

## Consequences

### Positive

- Replication no longer depends on the upstream native server receive/update path that is currently panicking.
- Sidereal keeps one authoritative server input source: authenticated realtime intent messages.
- Client prediction still keeps the existing native Lightyear `ActionState<PlayerInput>` path.

### Negative

- Sidereal still carries Lightyear native input protocol registration on replication for compatibility, even though authoritative server input does not use that runtime path.
- The longer-term Lightyear runtime split follow-up is still useful, but it is no longer a blocker for Sidereal replication stability.

## Follow-up

1. Track upstream Lightyear issue `#1200` and retest when a fix lands.
2. Implement the native input runtime split in `/home/toby/dev/lightyear` only if Sidereal still needs the server-native path after the current stabilized input architecture is reassessed.
3. Remove the remaining overnight warning sources we own locally:
   - dormant hierarchy rebuild system registration
   - unnecessary replication-side asset/scene runtime plugins if still present

## References

- `/home/toby/dev/lightyear/lightyear_inputs_native/src/plugin.rs`
- `/home/toby/.cargo/git/checkouts/lightyear-cdfa8a04895fe5e3/2986703/lightyear_inputs/src/input_buffer.rs`
- `crates/sidereal-net/src/lightyear_protocol/registration.rs`
- `bins/sidereal-replication/src/main.rs`
- `bins/sidereal-replication/src/replication/input.rs`
