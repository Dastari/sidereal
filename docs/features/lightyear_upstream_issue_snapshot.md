# Lightyear Upstream Issue Snapshot

Status: Active feature reference
Last updated: 2026-04-24
Owners: replication + client runtime
Scope: Open GitHub issues for `cBournhonesque/lightyear`  
Source: <https://github.com/cBournhonesque/lightyear/issues> and the GitHub API endpoint `https://api.github.com/repos/cBournhonesque/lightyear/issues?state=open&per_page=100&page=1`  
Filter: Open issues only, excluding open pull requests

## 0. Implementation Status

2026-04-24 status note:

1. This document is an upstream triage reference, not an implementation contract.
2. The full open-issue inventory remains the 2026-03-08 snapshot; the 2026-04-23 note below is a targeted PR verification update only.
3. Current Sidereal guidance remains conservative: check this snapshot before assuming an unexplained Lightyear behavior is local-only, and update it when a new upstream search changes the local risk assessment.

Update 2026-04-23: PR [#1421](https://github.com/cBournhonesque/lightyear/pull/1421) was verified as merged into `cBournhonesque:main` via commit [`af25682`](https://github.com/cBournhonesque/lightyear/commit/af25682) on 2026-04-22. The associated GitHub Actions run [`24797547143`](https://github.com/cBournhonesque/lightyear/actions/runs/24797547143) was not clean: `Lint` failed in `Format`, and `Test` failed in `lightyear_tests` with exit code 1. Public unauthenticated metadata did not expose detailed `lightyear_tests` logs. Local reproduction against `af25682` and its parent `eedb9ed` found the visible formatting failure and the targeted `lightyear_interpolation` unit failure were already present before #1421; the new #1421 confirmed-history tests passed locally. This update does not refresh the full open-issue inventory below.

## Purpose

Use this file as the first triage reference when Lightyear behaves in a way that looks unexplained locally.

Working rule:
- Check this file before assuming a Lightyear bug is unique to Sidereal.
- If a Sidereal problem clearly matches an upstream issue, link that upstream issue in the local doc/PR/commit context.
- If a new Lightyear problem is not covered here, search upstream and then update this file with the new issue or with a note that no upstream issue was found as of the search date.

## Snapshot Summary

Open issues in snapshot: 90

Largest labeled buckets:
- `A-Replication`: 17
- `C-Bug`: 14
- `A-Prediction`: 12
- `A-Input`: 11
- `A-Interpolation`: 9
- `C-Performance`: 9
- Unlabeled: 24

High-level read:
- Prediction, interpolation, replication ordering, and input ownership are still active upstream problem areas.
- Host-client mode still has multiple unresolved bugs and should not be treated as a clean proxy for dedicated-server behaviour.
- Transport and netcode edges remain active, especially around disconnect handling, multi-transport setups, and token validation.
- Avian integration still has open hierarchy and collider-sync issues that matter for Sidereal's modular entity model.

## Sidereal Watchlist

These are the upstream issues that currently matter most to Sidereal's architecture and implementation rules.

| Issue | Why it matters to Sidereal | Local guidance |
|---|---|---|
| [#1034](https://github.com/cBournhonesque/lightyear/issues/1034) `Add PredictionSwitching` | This is the upstream version of our predicted-to-interpolated control-swap problem. | Keep treating control transfer as a custom Sidereal responsibility until upstream switching is real and verified. |
| [#1380](https://github.com/cBournhonesque/lightyear/issues/1380) `Required components do not get assigned by interpolation` | Risk for any setup where interpolated entities rely on Bevy/Avian required components. | Do not assume interpolated entities will hydrate required physics state correctly without local validation. |
| [#1287](https://github.com/cBournhonesque/lightyear/issues/1287) `Improve relationship replication` | Parent-child and relationship ordering are core to our hierarchy and mount model. | Keep hierarchy persistence/hydration authoritative on our side; do not trust Lightyear to preserve full relationship semantics yet. |
| [#1195](https://github.com/cBournhonesque/lightyear/issues/1195) `Replicating relationships loses the order of the Relationship Target` | Ordered hardpoints/modules matter to deterministic hydration and gameplay. | Treat relationship order as unstable across replication unless explicitly rebuilt by Sidereal. |
| [#651](https://github.com/cBournhonesque/lightyear/issues/651) `Add option in Visibility to not despawn entities when they stop being visible` | Sidereal wants data-driven visibility and intel memory, not simple despawn/respawn churn. | Keep our own visibility/redaction contract; do not assume upstream can suspend replication without despawn. |
| [#1283](https://github.com/cBournhonesque/lightyear/issues/1283) `Input should not be accepted from any client for any entity` | This directly overlaps our session-bound input routing rule. | Continue binding transport/session identity to authoritative `player_entity_id` and reject mismatched claims server-side. |
| [#692](https://github.com/cBournhonesque/lightyear/issues/692) `Server doesn't check ConnectionToken's server address` | Upstream netcode token validation has a security gap. | Do not rely on Lightyear token checks alone for trust boundaries; keep gateway/auth/session validation explicit. |
| [#1402](https://github.com/cBournhonesque/lightyear/issues/1402) `Default no_input_delay config...` | Default localhost input timing is unreliable even in a simple setup. | Treat localhost and host-mode input results as suspect; validate with explicit latency settings and dedicated-server paths. |
| [#1434](https://github.com/cBournhonesque/lightyear/issues/1434) `Bevy Enhanced Input and host-client does not work` | Fresh upstream report of client input / control regression around missing `Controlled`, reported on `0.26.4` and `main`. | Treat this as adjacent evidence that control/input assignment remains fragile upstream. Not a direct match for Sidereal's dedicated-server rollback/bootstrap repro, but relevant context when evaluating `0.26.4` behavior. |
| [#1417](https://github.com/cBournhonesque/lightyear/issues/1417) `ServerMultiMessageSender` HostClient receive bug | Host-client message delivery remains inconsistent. | Do not use host-client correctness as a sign that our dedicated transport path is sound, or vice versa. |
| [#1394](https://github.com/cBournhonesque/lightyear/issues/1394) and [#1348](https://github.com/cBournhonesque/lightyear/issues/1348) | Multiple recent host-client input regressions are still open. | Prefer dedicated client/server validation for gameplay input work. |
| [#1235](https://github.com/cBournhonesque/lightyear/issues/1235), [#1251](https://github.com/cBournhonesque/lightyear/issues/1251), [#942](https://github.com/cBournhonesque/lightyear/issues/942), [#888](https://github.com/cBournhonesque/lightyear/issues/888) | Prediction and rollback behaviour still has unresolved correctness and ergonomics gaps. | Keep Sidereal's prediction/reconciliation decisions conservative and test-heavy. |
| [#1328](https://github.com/cBournhonesque/lightyear/issues/1328) and [#957](https://github.com/cBournhonesque/lightyear/issues/957) | Pre-spawn and future-message handling still have known correctness bugs. | Avoid assuming pre-spawn timelines are robust enough for critical gameplay flows without local coverage. |
| [#847](https://github.com/cBournhonesque/lightyear/issues/847), [#1228](https://github.com/cBournhonesque/lightyear/issues/1228), [#967](https://github.com/cBournhonesque/lightyear/issues/967), [#963](https://github.com/cBournhonesque/lightyear/issues/963), [#890](https://github.com/cBournhonesque/lightyear/issues/890), [#829](https://github.com/cBournhonesque/lightyear/issues/829) | Interpolation timeline behaviour and event timing are still evolving. | Keep our interpolation adoption incremental and validate event/VFX timing separately from transform smoothing. |
| [#740](https://github.com/cBournhonesque/lightyear/issues/740), [#1332](https://github.com/cBournhonesque/lightyear/issues/1332), [#1045](https://github.com/cBournhonesque/lightyear/issues/1045) | Replication correctness and efficiency tradeoffs remain open. | Be careful with replication mode assumptions and benchmark any bandwidth/perf decisions locally. |
| [#1266](https://github.com/cBournhonesque/lightyear/issues/1266), [#1128](https://github.com/cBournhonesque/lightyear/issues/1128), [#1253](https://github.com/cBournhonesque/lightyear/issues/1253) | Avian integration still has interpolation/collider edge cases. | Keep Avian + hierarchy + child-collider behaviour under explicit Sidereal tests. |
| [#1351](https://github.com/cBournhonesque/lightyear/issues/1351) | Multi-transport server topology is still unsettled upstream. | Treat our WebTransport-first plus native transport split as an integration boundary we own. |
| [#1303](https://github.com/cBournhonesque/lightyear/issues/1303), [#1278](https://github.com/cBournhonesque/lightyear/issues/1278), [#1174](https://github.com/cBournhonesque/lightyear/issues/1174), [#949](https://github.com/cBournhonesque/lightyear/issues/949), [#905](https://github.com/cBournhonesque/lightyear/issues/905) | Transport lifecycle and connection UX still have unresolved edges. | Keep disconnect handling, fallback behaviour, and server startup/shutdown semantics explicit in Sidereal. |
| [#643](https://github.com/cBournhonesque/lightyear/issues/643) and [#1363](https://github.com/cBournhonesque/lightyear/issues/1363) | Compile-time and dependency-health issues affect upgrade cost. | Keep Lightyear upgrade work scoped and verify build/perf impact before committing to new protocol surface area. |

## Related Merged Pull Request

Not part of the issue count above, but directly relevant:

- [#1421](https://github.com/cBournhonesque/lightyear/pull/1421) `interpolation: initialize confirmed history when Interpolated is added`
  - Merged into upstream `main` via commit [`af25682`](https://github.com/cBournhonesque/lightyear/commit/af25682) on 2026-04-22.
  - The merge commit's public Actions run [`24797547143`](https://github.com/cBournhonesque/lightyear/actions/runs/24797547143) showed failures in `Format` and `lightyear_tests`; unauthenticated public metadata did not expose full test logs.
  - Local checks against `af25682` and parent `eedb9ed` indicate the visible failures were pre-existing: `cargo fmt --all -- --check` failed on unrelated `lightyear_avian` / `lightyear_replication` files in both commits, and `cargo test -p lightyear_interpolation --lib` failed in both commits on `plugin::tests::test_interpolation_delay` due to exact float comparison (`0.6000061` vs `0.6`). The new #1421 confirmed-history tests passed locally.
  - This appears to address one specific interpolation-history gap that also shows up in Sidereal's control-transfer analysis.
  - Treat it as landed on upstream `main` with no reproduced evidence that it caused the visible CI failures, but still not as a released dependency until a tagged Lightyear release includes it and Sidereal validates it against the predicted/interpolated handoff flow.

## Full Inventory

### Prediction / Interpolation

| Issue | Labels | Title |
|---|---|---|
| [#1328](https://github.com/cBournhonesque/lightyear/issues/1328) | C-Bug, A-Prediction | Applying messages from the future causes prespawned entity match errors |
| [#1251](https://github.com/cBournhonesque/lightyear/issues/1251) | A-Prediction | Prediction issues |
| [#1244](https://github.com/cBournhonesque/lightyear/issues/1244) | A-Prediction, A-Replication, A-Interpolation | Add replicate_if_predicted/interpolated |
| [#1235](https://github.com/cBournhonesque/lightyear/issues/1235) | A-Prediction | Rollbacking to a tick earlier than the previous rollback causes issues |
| [#1232](https://github.com/cBournhonesque/lightyear/issues/1232) | A-Prediction, A-Replication, C-Performance | Relax prediction assumptions |
| [#1228](https://github.com/cBournhonesque/lightyear/issues/1228) | A-Interpolation | Spawn Interpolated entity only when the interpolation timeline reaches the spawn tick |
| [#1097](https://github.com/cBournhonesque/lightyear/issues/1097) | A-Prediction, A-Input | Add unit test for remote input behaviour in lockstep mode |
| [#1074](https://github.com/cBournhonesque/lightyear/issues/1074) | A-Prediction | Fix some prediction inefficiencies |
| [#1044](https://github.com/cBournhonesque/lightyear/issues/1044) | A-Prediction | Predicted DefaultFilter during rollbacks |
| [#1034](https://github.com/cBournhonesque/lightyear/issues/1034) | A-Prediction, A-Interpolation | Add PredictionSwitching |
| [#967](https://github.com/cBournhonesque/lightyear/issues/967) | A-Interpolation | Provide an interpolation buffer to apply events/components at the interpolation_tick |
| [#963](https://github.com/cBournhonesque/lightyear/issues/963) | A-Replication, A-Interpolation | Provide a way to send messages/component for a given Timeline? |
| [#957](https://github.com/cBournhonesque/lightyear/issues/957) | C-Bug, A-Prediction, A-Replication | The Confirmed.tick can be incorrect when spawning PreSpawned entities |
| [#890](https://github.com/cBournhonesque/lightyear/issues/890) | A-Interpolation | Improve interpolation default config |
| [#888](https://github.com/cBournhonesque/lightyear/issues/888) | A-Prediction | Improve prediction |
| [#886](https://github.com/cBournhonesque/lightyear/issues/886) | A-Prediction, A-Interpolation, C-Example | Add Extrapolation + an entity with replicated vehicles |
| [#847](https://github.com/cBournhonesque/lightyear/issues/847) | A-Interpolation | Enable sending interpolation updates with a history. |
| [#829](https://github.com/cBournhonesque/lightyear/issues/829) | A-Interpolation | Interpolation API is confusing |

### Replication / Visibility / Hierarchy

| Issue | Labels | Title |
|---|---|---|
| [#1332](https://github.com/cBournhonesque/lightyear/issues/1332) | A-Replication, C-Performance | Optimize replication cache efficiency |
| [#1287](https://github.com/cBournhonesque/lightyear/issues/1287) | A-Replication, A-Hierarchy, A-Avian | Improve relationship replication |
| [#1264](https://github.com/cBournhonesque/lightyear/issues/1264) | A-Replication | Improve Lifetime behaviour |
| [#1262](https://github.com/cBournhonesque/lightyear/issues/1262) | A-Replication | Make Replicated a Relationship |
| [#1233](https://github.com/cBournhonesque/lightyear/issues/1233) | A-Input, A-Replication | Add unit test for prespawned input |
| [#1195](https://github.com/cBournhonesque/lightyear/issues/1195) | A-Replication | Replicating relationships loses the order of the Relationship Target |
| [#1178](https://github.com/cBournhonesque/lightyear/issues/1178) | C-Bug, A-Replication | segfault in projectiles demo |
| [#1122](https://github.com/cBournhonesque/lightyear/issues/1122) | A-Replication | Maybe do not include the ReplicationGroup in replication updates |
| [#1045](https://github.com/cBournhonesque/lightyear/issues/1045) | A-Replication | Improve DeltaCompression performance |
| [#996](https://github.com/cBournhonesque/lightyear/issues/996) | A-Replication | Register Res<State<S>> and Res<NextState<S>> for synchronization |
| [#960](https://github.com/cBournhonesque/lightyear/issues/960) | C-Bug, P-Critical, A-Replication | Add unit test for replication bug |
| [#740](https://github.com/cBournhonesque/lightyear/issues/740) | A-Replication | ReplicationMode::SinceLastSend doesn't work in all situations |
| [#651](https://github.com/cBournhonesque/lightyear/issues/651) | A-Visibility | Add option in Visibility to not despawn entities when they stop being visible |
| [#630](https://github.com/cBournhonesque/lightyear/issues/630) | A-Replication | feat: Multiple Protocol Support |

### Input / Sync / Host Mode

| Issue | Labels | Title |
|---|---|---|
| [#1417](https://github.com/cBournhonesque/lightyear/issues/1417) | - | Messages sent via `ServerMultiMessageSender` can't be received in HostClient mode |
| [#1434](https://github.com/cBournhonesque/lightyear/issues/1434) | - | Bevy Enhanced Input and host-client does not work |
| [#1402](https://github.com/cBournhonesque/lightyear/issues/1402) | C-Bug, A-Input, A-Sync | Default `no_input_delay` config doesn't reliably deliver inputs on localhost due to sync error margin tolerance |
| [#1394](https://github.com/cBournhonesque/lightyear/issues/1394) | C-Bug, A-Input, C-Example | Input Broken on host-client Mode examples |
| [#1348](https://github.com/cBournhonesque/lightyear/issues/1348) | C-Bug, C-Example | Projectiles example — Host-client server cannot move; input mapped to PLACEHOLDER entity |
| [#1336](https://github.com/cBournhonesque/lightyear/issues/1336) | A-Input | TickDelta not rounded correctly? |
| [#1283](https://github.com/cBournhonesque/lightyear/issues/1283) | A-Input | Input should not be accepted from any client for any entity |
| [#1280](https://github.com/cBournhonesque/lightyear/issues/1280) | - | Make RawConnection work in HostServer mode |
| [#1238](https://github.com/cBournhonesque/lightyear/issues/1238) | C-Bug, A-Input, C-Example | Remaining issues for release |
| [#1148](https://github.com/cBournhonesque/lightyear/issues/1148) | - | Don't serialize messages from Server to HostClient |
| [#1111](https://github.com/cBournhonesque/lightyear/issues/1111) | A-Input | Extra features for deterministic lockstep |
| [#1086](https://github.com/cBournhonesque/lightyear/issues/1086) | A-Sync | Potential timeline sync issues |
| [#1080](https://github.com/cBournhonesque/lightyear/issues/1080) | A-Input | Support 'global' inputs by attaching InputMarker to the Client entity |
| [#1077](https://github.com/cBournhonesque/lightyear/issues/1077) | C-Bug | HostClient is probably not feasible because frames seem frozen when alt-tabbing? |
| [#1041](https://github.com/cBournhonesque/lightyear/issues/1041) | A-Input, C-Performance | Issues with inputs |
| [#961](https://github.com/cBournhonesque/lightyear/issues/961) | C-Bug, A-Input, C-Example | Avian3d example broken in host-server mode with predict_all = False |
| [#927](https://github.com/cBournhonesque/lightyear/issues/927) | A-Sync | Update sync in PreUpdate |

### Transport / Netcode

| Issue | Labels | Title |
|---|---|---|
| [#1376](https://github.com/cBournhonesque/lightyear/issues/1376) | A-Transport | Disconnect reason should be an enum instead of a String |
| [#1351](https://github.com/cBournhonesque/lightyear/issues/1351) | - | Handle multiple ServerIOs in the same app (Steam + UDP + WebTransport, ecc.) |
| [#1303](https://github.com/cBournhonesque/lightyear/issues/1303) | A-Transport | Unable to access OS assigned port for ServerUdpIo |
| [#1278](https://github.com/cBournhonesque/lightyear/issues/1278) | - | Unable to connect to hostname / domain directly |
| [#1174](https://github.com/cBournhonesque/lightyear/issues/1174) | C-Bug, A-Transport | Unlink doesn't shut down the underlying IO |
| [#1156](https://github.com/cBournhonesque/lightyear/issues/1156) | - | Steam does not work in headless mode |
| [#1088](https://github.com/cBournhonesque/lightyear/issues/1088) | A-Transport | Reset netcode state on Stopped |
| [#1081](https://github.com/cBournhonesque/lightyear/issues/1081) | A-Transport | Make it easier to use a LinkConditioner on the server |
| [#949](https://github.com/cBournhonesque/lightyear/issues/949) | - | Closing tab on webtransport client disconnects but then spams the server logs |
| [#905](https://github.com/cBournhonesque/lightyear/issues/905) | - | Client don't disconnect and spams error when the server stops while using LocalChannel transport |
| [#692](https://github.com/cBournhonesque/lightyear/issues/692) | A-Netcode | Server doesn't check ConnectionToken's server address |

### Avian / Physics

| Issue | Labels | Title |
|---|---|---|
| [#1266](https://github.com/cBournhonesque/lightyear/issues/1266) | A-Avian | Integrate with bevy_transform_interpolation |
| [#1253](https://github.com/cBournhonesque/lightyear/issues/1253) | C-Bug, C-Example, A-Avian | Collision issue with avian |
| [#1128](https://github.com/cBournhonesque/lightyear/issues/1128) | A-Avian | lightyear_avian interferes with child collider sync |

### Performance / Build / Maintenance

| Issue | Labels | Title |
|---|---|---|
| [#1363](https://github.com/cBournhonesque/lightyear/issues/1363) | - | Cargo audit found a few unmaintained dependencies in lightyear |
| [#1342](https://github.com/cBournhonesque/lightyear/issues/1342) | C-Performance | Avoid live memory allocations |
| [#1294](https://github.com/cBournhonesque/lightyear/issues/1294) | C-Performance | Avoid nested hashmaps |
| [#1290](https://github.com/cBournhonesque/lightyear/issues/1290) | C-Performance | Memory Leak? |
| [#1063](https://github.com/cBournhonesque/lightyear/issues/1063) | C-Performance | Check for extra allocations/leaks |
| [#893](https://github.com/cBournhonesque/lightyear/issues/893) | C-Performance | Make NetworkTarget operations cheaper |
| [#763](https://github.com/cBournhonesque/lightyear/issues/763) | - | Limit code-gen if component replication is unidirectional |
| [#643](https://github.com/cBournhonesque/lightyear/issues/643) | C-Performance | `register_component` / `register_resource` calls cause noticeable growth in compile time |

### Docs / Examples / Tooling / Other

| Issue | Labels | Title |
|---|---|---|
| [#1413](https://github.com/cBournhonesque/lightyear/issues/1413) | - | jenkinssoftware.com is down and that broke documentation within lightyear |
| [#1380](https://github.com/cBournhonesque/lightyear/issues/1380) | - | Required components do not get assigned by interpolation. |
| [#1350](https://github.com/cBournhonesque/lightyear/issues/1350) | - | Replicon integration |
| [#1347](https://github.com/cBournhonesque/lightyear/issues/1347) | - | Add visibility unit test |
| [#1301](https://github.com/cBournhonesque/lightyear/issues/1301) | - | Publish `lightyear-test` utilities for external game projects |
| [#1200](https://github.com/cBournhonesque/lightyear/issues/1200) | - | Panic: subtract with overflow |
| [#1185](https://github.com/cBournhonesque/lightyear/issues/1185) | C-Usability | Remove ClientState in favor of a custom QueryData |
| [#1184](https://github.com/cBournhonesque/lightyear/issues/1184) | C-Bug, C-Example | Examples fail under certain feature combinations |
| [#1182](https://github.com/cBournhonesque/lightyear/issues/1182) | - | Issues after 0.21 |
| [#1176](https://github.com/cBournhonesque/lightyear/issues/1176) | - | `LinkState::Linked` Typo |
| [#1132](https://github.com/cBournhonesque/lightyear/issues/1132) | - | Add example for bandwidth test |
| [#1131](https://github.com/cBournhonesque/lightyear/issues/1131) | - | Update avian example to optionally have child colliders |
| [#1123](https://github.com/cBournhonesque/lightyear/issues/1123) | C-Book | Mention PredictionManager in the docs on client prediction |
| [#1108](https://github.com/cBournhonesque/lightyear/issues/1108) | C-Bug | Implement TickCleanUp and TickSync for every component/resource that holds ticks |
| [#1046](https://github.com/cBournhonesque/lightyear/issues/1046) | - | Use crossfig for easier features management |
| [#942](https://github.com/cBournhonesque/lightyear/issues/942) | - | TimeManager + rollback issues |
| [#882](https://github.com/cBournhonesque/lightyear/issues/882) | C-Usability | Use type restrictions in component registry |
| [#836](https://github.com/cBournhonesque/lightyear/issues/836) | - | Add tick-buffered channel? |
| [#834](https://github.com/cBournhonesque/lightyear/issues/834) | - | Horizontally scaling `lightyear` for high-availability |
| [#800](https://github.com/cBournhonesque/lightyear/issues/800) | C-Usability | Flesh out the visualizer |
