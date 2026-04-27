# Gateway, Replication, and Client Full Audit Report - 2026-04-27

Status note 2026-04-27: Static audit from the current working tree. This report intentionally reviews the gateway, replication server, shared game core, scripting runtime, and native/WASM client-facing runtime code together because the highest-risk behavior crosses those boundaries. No code was changed as part of this audit.

Status note 2026-04-27: The repository was already heavily modified before this report was created. Findings therefore describe the current local tree, not a clean `main` baseline.

## Scope

Reviewed areas:

1. Gateway auth, admin, asset delivery, character creation, and starter-world scripting.
2. Replication server auth/session binding, realtime input, control handoff, Lightyear roles, visibility membership, tactical lane, owner lane, persistence scheduling, combat events, and runtime scripting.
3. Client bootstrap, transport, realtime input, control state, local view updates, prediction, interpolation/adoption, motion ownership, rendering layers, tactical UI, sensor ring, and asset bootstrap.
4. Shared gameplay core, fixed-step simulation, Avian physics integration, mass/inertia, component registry, action routing, flight, combat, and visibility-related components.
5. Source-of-truth docs under `docs/features/`, especially visibility, tactical/owner lanes, prediction runtime, and asset delivery.

External benchmark references used:

- [Valve Source Multiplayer Networking](https://developer.valvesoftware.com/wiki/Source_Multiplayer_Networking) for server authority, client prediction, entity interpolation, and lag-compensation framing.
- [Unity Netcode package guidance](https://docs.unity.com/en-us/multiplayer/netcode/netcode), [Unity prediction](https://docs.unity.cn/Packages/com.unity.netcode%401.5/manual/intro-to-prediction.html), and [Unity interpolation](https://docs.unity.cn/Packages/com.unity.netcode%401.5/manual/interpolation.html) for server-authoritative predicted/interpolated timelines and load distribution.
- [Unreal Replication Graph](https://dev.epicgames.com/documentation/en-us/unreal-engine/replication-graph?application_version=4.27) for large-world replication-list caching and persistent relevance nodes.
- [Lightyear interest management](https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/interest_management.html), [prediction/interpolation](https://cbournhonesque.github.io/lightyear/book/tutorial/advanced_systems.html), [visual interpolation](https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/visual_interpolation.html), and [pre-spawned predicted entities](https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/client_replication.html) for the Rust/Bevy networking model this project uses.

## Executive Assessment

The architecture is pointed in the right direction for a server-authoritative top-down MMO space game. The strongest areas are authenticated session binding, generation-scoped control handoff, latest-wins realtime input, shared client/server fixed-step simulation, Avian f64 authoritative motion, server-side visibility membership, redacted signal contacts, and HTTP-based asset delivery.

The project is not yet at the bar implied by "galaxy-wide MMO" scale. The remaining risks are mostly not conceptual; they are concrete implementation gaps:

1. Client-provided local view range is trusted too directly by the server.
2. One visibility anchor path still samples f32 render transforms before f64 authoritative positions.
3. Tactical/scanner products are not yet gated by the currently controlled non-player-anchor scanner on the server.
4. Runtime scripting is safe but too narrow, too flight-specific, and currently downcasts script movement targets to f32.
5. Gateway starter-world setup still encodes "ship" as a backend invariant.
6. Several large modules are now doing too much, especially client visuals/UI/runtime and replication visibility/tactical/control.

There were no obvious findings where the current code lets a client authoritatively set world transforms or directly control another player's entity. The main exposure risks are read-side disclosure, tactical intel over-disclosure, and performance-amplified visibility queries.

## Positive Findings

- Replication auth binds transport client to a gateway-issued JWT player claim and rejects token/message player mismatches (`bins/sidereal-replication/src/replication/auth.rs:478`, `bins/sidereal-replication/src/replication/auth.rs:493`, `bins/sidereal-replication/src/replication/auth.rs:624`).
- `ServerSessionReadyMessage` carries protocol version, control generation, and current controlled entity (`crates/sidereal-net/src/lightyear_protocol/messages.rs:13`), which closes a common reconnect/input-stale failure mode.
- Control requests reject player mismatches, stale request sequence numbers, and unowned controlled targets (`bins/sidereal-replication/src/replication/control.rs:227`, `bins/sidereal-replication/src/replication/control.rs:255`, `bins/sidereal-replication/src/replication/control.rs:383`).
- Realtime input is latest-wins, generation-scoped, authenticated-player scoped, rate limited, bounded by packet action count, and drained only into the current authoritative target (`bins/sidereal-replication/src/replication/input.rs:215`, `bins/sidereal-replication/src/replication/input.rs:380`, `bins/sidereal-replication/src/replication/input.rs:548`).
- Shared simulation is now centralized through `SiderealSharedSimulationPlugin` for server and client prediction (`crates/sidereal-game/src/lib.rs:155`), matching the modern prediction standard of shared deterministic-ish gameplay code.
- Client disables transform-to-position feedback in Lightyear/Avian prediction mode (`bins/sidereal-client/src/runtime/app_setup.rs:142`), which aligns with the single-writer motion contract.
- Visibility uses an explicit authorization, delivery, and disclosure model in the docs (`docs/features/visibility_replication_contract.md:58`) and the implementation has a spatial-grid candidate pass plus full policy checks.
- Signal-only tactical contacts are redacted: unknown contact id, approximate position, no faction, no velocity, no size, no mass (`bins/sidereal-replication/src/replication/tactical.rs:905`).
- Gateway asset bootstrap and payload fetches keep asset bytes on HTTP, not replication transport (`bins/sidereal-gateway/src/api.rs:578`, `bins/sidereal-gateway/src/api.rs:682`), matching the asset delivery contract.
- Gateway admin methods require admin/dev role, MFA, and route-specific scopes (`bins/sidereal-gateway/src/auth/service.rs:1001`).
- Lua runtime disables IO/OS/package libraries and enforces memory and instruction budgets (`crates/sidereal-scripting/src/lib.rs:1526`, `crates/sidereal-scripting/src/lib.rs:1540`, `crates/sidereal-scripting/src/lib.rs:1552`).

## High-Severity Findings

### H1. Server accepts unbounded client local-view delivery radius

Evidence:

- Client clamps requested delivery radius to `50_000.0` meters (`bins/sidereal-client/src/runtime/control.rs:178`).
- Server stores the received radius as `message.delivery_range_m.max(1.0)` without a server-side upper bound (`bins/sidereal-replication/src/replication/visibility.rs:1008`, `bins/sidereal-replication/src/replication/visibility.rs:1041`).
- That value is then used to build the per-client context and spatial candidate set (`bins/sidereal-replication/src/replication/visibility.rs:2138`, `bins/sidereal-replication/src/replication/visibility.rs:2197`).

Impact:

A modified client can request an arbitrarily large delivery radius. This does not directly bypass authorization, but it can widen delivery for any entity that is already authorized by owner/public/faction/range/discovery policy and can force large candidate-cell scans. This is both a scaling problem and a disclosure-budget problem. Delivery scope is supposed to narrow authorization, but the narrowing budget must be server-owned.

Recommendation:

Add a server-side clamp and budget policy. Suggested initial contract:

1. `REPLICATION_VISIBILITY_DELIVERY_RANGE_MAX_M`, defaulting to the native client max or lower.
2. Per-view-mode caps, for example tactical, map, and admin/debug lanes.
3. Reject or clamp non-finite values.
4. Telemetry counter for clamped requests and per-client max candidate count.
5. Regression test: malicious client sends `f32::MAX`, `NaN`, `Infinity`, and large finite values; server clamps and does not produce an abnormal candidate set.

### H2. Observer anchor visibility path violates f64 world-coordinate contract

Evidence:

- Visibility contract says candidate generation and observer anchors must prefer f64 Avian `Position` or f64 `WorldPosition` before f32 Bevy transforms (`docs/features/visibility_replication_contract.md:45`).
- `update_client_observer_anchor_positions` queries `Position`, `GlobalTransform`, and `Transform`, but resolves `GlobalTransform` first, then `Transform`, then `Position` (`bins/sidereal-replication/src/replication/runtime_state.rs:141`, `bins/sidereal-replication/src/replication/runtime_state.rs:178`).
- Other visibility paths correctly prefer `Position`, then `WorldPosition`, then `GlobalTransform` (`bins/sidereal-replication/src/replication/visibility.rs:1061`).

Impact:

At galaxy-scale coordinates, f32 render transforms lose precision. This can make observer anchors, delivery range, tactical candidate sets, and visibility membership disagree with authoritative f64 world state. It is also a direct rule violation.

Recommendation:

Change observer anchor resolution to prefer `Position` first. Add a regression test with a large f64 coordinate where `Transform` is rounded/stale and `Position` is authoritative. The test should assert that `ClientObserverAnchorPositionMap` stores the f64-derived position.

### H3. Tactical scanner gating is still client-presentational, not server-authoritative

Evidence:

- The tactical contract says target server behavior must gate tactical output by the currently controlled non-player-anchor entity's effective `ScannerComponent`, and free roam/player-anchor control must not be a scanner source (`docs/features/tactical_and_owner_lane_protocol_contract.md:39`).
- The native client sensor ring already requires an active scanner profile on the controlled entity (`bins/sidereal-client/src/runtime/sensor_ring.rs:79`, `bins/sidereal-client/src/runtime/sensor_ring.rs:128`).
- The replication tactical stream derives live fog cells and signal detection from `VisibilityDisclosure.visibility_sources`, not from a server-resolved active scanner profile for the controlled entity (`bins/sidereal-replication/src/replication/tactical.rs:760`, `bins/sidereal-replication/src/replication/tactical.rs:870`).

Impact:

The server still has a broader tactical/intel model than the UI implies. A client may be unable to open the sensor ring, while the server still emits tactical fog/contact products from generic visibility sources. That is an over-disclosure risk for scanner-derived intel and a blocker for scanner tier redaction.

Recommendation:

Implement a server-side `EffectiveScannerSource` or equivalent read model:

1. Resolve current controlled entity from `PlayerControlledEntityMap`.
2. Reject player-anchor/free-roam as a scanner source.
3. Find root/mounted `ScannerComponent` plus effective `VisibilityRangeM`.
4. Use that profile for tactical fog/contact production, contact detail tier, max contacts, and signal detection.
5. Keep owner manifest separate and unaffected.
6. Add tests for free roam, controlled entity with no scanner, mounted scanner, scanner tier changes, and spoofed local-view mode.

### H4. Runtime scripting is safe but too narrow, f32-based in movement intent, and flight-specific

Evidence:

- Script snapshots expose only `guid`, position, and `script_state`; `has/get` only supports `script_state` (`bins/sidereal-replication/src/replication/runtime_scripting.rs:27`, `bins/sidereal-replication/src/replication/runtime_scripting.rs:43`).
- Script intents are limited to `fly_towards`, `stop`, `set_script_state`, and `notify_player` (`bins/sidereal-replication/src/replication/runtime_scripting.rs:111`, `bins/sidereal-replication/src/replication/runtime_scripting.rs:689`).
- `fly_towards` parses f64 JSON numbers but immediately casts them to f32 and stores a `Vec2` (`bins/sidereal-replication/src/replication/runtime_scripting.rs:701`).
- `apply_script_intents` requires `FlightComputer` and `Transform`, then directly mutates `FlightComputer` throttle/yaw/brake (`bins/sidereal-replication/src/replication/runtime_scripting.rs:525`, `bins/sidereal-replication/src/replication/runtime_scripting.rs:548`).
- `is_script_controllable` allows only non-player `OwnerId` values and requires `ScriptState` (`bins/sidereal-replication/src/replication/runtime_scripting.rs:621`).

Impact:

The sandboxing is a strength, but the gameplay interface is not yet an MMO scripting interface. It hardcodes a flight-computer intent path, uses render `Transform` rather than authoritative f64 position for script steering, and cannot express generic entity capabilities, AI behaviors, mission events, scan rules, faction logic, or data-driven module effects without more Rust-side special cases.

Recommendation:

Promote scripting to a generic, intent-only ECS capability API:

1. Script snapshots should expose a whitelisted component view by generated component kind, with per-component redaction and no raw Bevy `Entity`.
2. Movement intents should use f64 target coordinates and resolve authoritative `Position`/`WorldPosition`.
3. Replace `fly_towards` with generic intents such as `set_navigation_target`, `activate_capability`, `clear_capability`, and `emit_game_event`.
4. Flight should consume the generic intent and translate it through Rust authority systems, not script runtime directly writing `FlightComputer`.
5. Add per-script CPU/time telemetry and event fanout limits.

### H5. Gateway starter world hardcodes "ship" as a backend invariant

Evidence:

- `player_init.lua` is expected to select `ship_bundle_id` (`bins/sidereal-gateway/src/auth/starter_world.rs:139`).
- The selected bundle must have `bundle_class == "ship"` (`bins/sidereal-gateway/src/auth/starter_world.rs:149`).
- The controlled entity is resolved by a `Ship` label before falling back to the first record (`bins/sidereal-gateway/src/auth/starter_world.rs:272`).

Impact:

This conflicts with the repo rule that generic runtime structures should use entity terminology unless behavior is truly ship-only. It also makes non-ship starters, stations, drones, probes, characters, and future trust/organization-controlled entities harder than they need to be.

Recommendation:

Rename the starter contract around generic controlled entities:

1. `controlled_bundle_id` instead of `ship_bundle_id`.
2. Bundle class such as `controllable`, `starter_controlled_entity`, or explicit capability tags.
3. Controlled entity selection by `ControlledStartTarget` component or bundle metadata, not label text.
4. Keep "ship" as content taxonomy in Lua/assets/UI only.

## Medium-Severity Findings

### M1. Tactical contact streaming still scans all replicated entities per client interval

Evidence:

- `stream_tactical_snapshot_messages` loops over each authenticated player/client and then iterates all `replicated_entities` (`bins/sidereal-replication/src/replication/tactical.rs:671`, `bins/sidereal-replication/src/replication/tactical.rs:718`, `bins/sidereal-replication/src/replication/tactical.rs:816`).

Impact:

At MMO scale, this becomes `clients * replicated_entities` every tactical interval. Visibility membership has a spatial index; tactical contacts should reuse spatial/visibility products instead of rebuilding a global scan per player. Unreal's Replication Graph guidance is relevant here: large games avoid per-actor/per-connection checks by building persistent shared replication lists.

Recommendation:

Use visibility membership, spatial-grid cells, and per-sector contact buckets as inputs. Split tactical into:

1. authoring cache by entity,
2. per-cell contact index,
3. per-player scanner source query,
4. stream diff builder with max contact budget and priority.

### M2. Protocol message and channel directions are broader than their semantics

Evidence:

- All messages and channels are registered `NetworkDirection::Bidirectional`, including server-only tactical snapshots, server-only control responses, and client-only input messages (`crates/sidereal-net/src/lightyear_protocol/registration.rs:73`).

Impact:

Receivers currently enforce semantics, so this is not an immediate exploit by itself. It does increase the fuzzing surface and makes it easier for future code to accidentally start consuming a message from the wrong side.

Recommendation:

If Lightyear supports directional registration for the current version, narrow directions. If it does not, add explicit receiver-side tests and comments naming the expected direction for every message.

### M3. Control role rearm is still a Lightyear workaround with churn risk

Evidence:

- Role changes call `ReplicationState::lose_visibility` and `gain_visibility` for currently visible clients (`bins/sidereal-replication/src/replication/control.rs:638`, `bins/sidereal-replication/src/replication/control.rs:651`).
- The workaround is triggered for player anchors and controlled entities when replicate/prediction/interpolation topology changes (`bins/sidereal-replication/src/replication/control.rs:812`, `bins/sidereal-replication/src/replication/control.rs:872`).

Impact:

This is scoped better than older broad rearm behavior, but it can still cause spawn/despawn churn, duplicate presentation, interpolation history loss, and bandwidth spikes during control changes.

Recommendation:

Keep it only while pinned Lightyear behavior requires it. Add metrics for rearmed client count per control change and tests for repeated control handoff without duplicate client entities.

### M4. Action capability validation is a no-op

Evidence:

- `ActionCapabilities::can_handle` exists (`crates/sidereal-game/src/actions.rs:127`).
- `validate_action_capabilities` is explicitly a legacy no-op (`crates/sidereal-game/src/actions.rs:133`).

Impact:

Unsupported actions are left to downstream systems. That works while action sets are small, but it becomes brittle once scanners, mining, docking, modules, scripts, NPC AI, and player abilities expand. It also weakens the reusable generic action system described in the file header.

Recommendation:

Reinstate capability filtering in one shared system, but keep it non-authoritative for UI convenience. Server should drop or mark unsupported actions before component systems run; client prediction should do the same for parity.

### M5. Client prediction currently includes combat/projectile side effects that need a stricter authority audit

Evidence:

- Client prediction runs weapon cooldown bootstrap/tick, weapon fire processing, projectile update, and engine thrust in shared simulation (`crates/sidereal-game/src/lib.rs:224`).
- Client app marks new ballistic projectiles as pre-spawned after local weapon fire processing (`bins/sidereal-client/src/runtime/app_setup.rs:220`).

Impact:

Pre-spawned predicted projectiles are a valid modern approach, and Lightyear documents this pattern. The risk is that cooldowns, ammo, projectile spawn, projectile movement, impact, and authoritative hit/damage side effects must be partitioned exactly. The server path includes impacts/damage/destruction; the client path does not include damage, but the full side-effect boundary should be tested.

Recommendation:

Document and test which combat components are predicted, rollback-owned, server-owned, or cosmetic-only. Add tests for ammo/cooldown divergence, local projectile reconciliation, rejected fire input, and server hit correction.

### M6. Gateway character creation can orphan graph records on auth-store insert failure

Evidence:

- `create_character` persists starter world records before inserting the character row (`bins/sidereal-gateway/src/auth/service.rs:494`, `bins/sidereal-gateway/src/auth/service.rs:507`).
- The cleanup in the error branch only runs when starter-world persistence fails, before an auth character row exists (`bins/sidereal-gateway/src/auth/service.rs:495`).

Impact:

If graph persistence succeeds but `insert_account_character` fails, graph records for the new player id can remain without a character row. This is a cross-store consistency bug.

Recommendation:

Use a reservation/transaction pattern:

1. Insert pending character row.
2. Persist graph records.
3. Mark character active.
4. On graph failure, delete pending row.
5. On row activation failure, remove graph records.

### M7. `auth_accounts.player_entity_id` is legacy single-character residue

Evidence:

- `auth_accounts` still has `player_entity_id TEXT NOT NULL` (`bins/sidereal-gateway/src/auth/store.rs:211`).
- Multi-character state exists in `auth_characters` (`bins/sidereal-gateway/src/auth/store.rs:219`).

Impact:

This does not appear to drive current selected-character runtime state directly, but it is easy for older client or dashboard code to treat an account as having one player entity. That conflicts with the account/character rule.

Recommendation:

Deprecate account-level `player_entity_id` in API types and docs. During early-development schema discipline, prefer a clean schema reset over adding compatibility layers.

### M8. Client runtime code splitting is improved but still has large mixed-concern modules

Evidence from line counts:

- `bins/sidereal-client/src/runtime/visuals.rs`: 3752 lines.
- `bins/sidereal-client/src/runtime/ui.rs`: 3411 lines.
- `bins/sidereal-client/src/runtime/backdrop.rs`: 2262 lines.
- `bins/sidereal-client/src/runtime/debug_overlay.rs`: 2054 lines.
- `bins/sidereal-client/src/runtime/replication.rs`: 1844 lines.
- `bins/sidereal-client/src/runtime/auth_ui.rs`: 2023 lines.
- `bins/sidereal-replication/src/replication/visibility.rs`: 4036 lines.
- `bins/sidereal-replication/src/replication/scripting.rs`: 1936 lines.

Impact:

Large modules make ownership, test targeting, and regression review harder. The client visual/rendering path is especially likely to accumulate duplicate repair and presentation logic.

Recommendation:

Split by domain without changing behavior:

1. `runtime/visuals/planets.rs`, `projectiles.rs`, `thrusters.rs`, `materials.rs`, `visibility.rs`, `streamed.rs`.
2. `runtime/ui/tactical_map.rs`, `owner_panel.rs`, `notifications.rs`, `auth.rs`.
3. `replication/visibility/{cache,index,policy,landmarks,metrics}.rs`.
4. `replication/scripting/{catalog,drafts,runtime_api}.rs`.

### M9. Runtime scripting event dispatch has no spatial or handler index

Evidence:

- Untargeted script events iterate every entity in the script snapshot (`bins/sidereal-replication/src/replication/runtime_scripting.rs:430`).
- Tick scheduling scans all snapshot entities and parses handler config from JSON each run (`bins/sidereal-replication/src/replication/runtime_scripting.rs:297`, `bins/sidereal-replication/src/replication/runtime_scripting.rs:299`).

Impact:

This is acceptable for a small AI prototype but not for MMO-scale world scripting. It also encourages hardcoding more backend Rust systems because the scripting runtime is not cheap or expressive enough for many gameplay features.

Recommendation:

Maintain a handler index keyed by event name, component kind, and spatial sector. Only target relevant scripted entities. Cache parsed script hook config until `ScriptState` changes.

### M10. Compatibility bootstrap comments conflict with early schema discipline

Evidence:

- Mass bootstrap comments explicitly handle older graph records that predate derived mass components (`crates/sidereal-game/src/mass.rs:228`).

Impact:

The repo currently has a strict early-development rule against legacy compatibility shims. This may be a practical local-dev helper, but the intent should be explicit. Otherwise more compatibility backfills will accumulate.

Recommendation:

Either document this as an intentional dev-only hydration repair, or remove it once component generation/persistence always produces the canonical mass schema.

## Low-Severity Findings

### L1. Generic terminology is only partially achieved

`ShipTag` still exists (`crates/sidereal-game/src/components/ship_tag.rs`), gateway starter world requires ship bundles, several client comments and UI fallbacks refer to ships, and flight docs are engine-specific. Some of this is content/domain language and fine at the UI/content layer. Backend runtime contracts should prefer `entity`, `controlled entity`, `propulsion`, `thrust provider`, `module`, or capability terms.

### L2. Asset manifest exposes full catalog metadata after auth

Authenticated `/assets/bootstrap-manifest` returns the full catalog plus required assets (`bins/sidereal-gateway/src/api.rs:596`, `bins/sidereal-gateway/src/api.rs:617`). This matches the current asset contract, but for production MMO secrecy you may eventually want public/bootstrap/runtime subsets, entitlement-gated asset groups, or CDN token scoping.

### L3. Visibility bypass is test-only, which is correct

`SIDEREAL_VISIBILITY_BYPASS_ALL` returns false outside tests (`bins/sidereal-replication/src/replication/visibility.rs:71`). Keep it that way.

## System-by-System Notes

### Gateway

The gateway is mostly aligned with the security rules. Admin bootstrap goes through gateway flow, admin operations require role/MFA/scope, and asset payloads are separated from replication. The two main issues are starter-world specificity and cross-store character creation consistency.

Recommended next gateway work:

1. Convert starter-world scripts from ship-specific selection to generic controlled entity selection.
2. Fix character creation ordering/cleanup.
3. Remove or quarantine account-level `player_entity_id`.
4. Add integration tests for failed graph persistence, failed character insert, and selected-character token issuance.

### Replication Auth, Control, and Input

This area is stronger than the previous audit baseline. Auth binds the selected player entity to transport identity; control changes validate ownership and use generations; realtime input validates generation and authoritative target before applying.

Remaining work:

1. Keep control handoff tests growing around stale messages and repeated target switches.
2. Narrow protocol directions or test receiver-side enforcement.
3. Measure role rearm churn.
4. Ensure control/input tests cover player-anchor self-control, ship/entity control, disconnect, reconnect, stale generation, stale target, and spoofed player id.

### Visibility and Tactical Lanes

The core visibility model is sound: fail-closed authorization, delivery narrowing, spatial candidate preselection, and redacted signal contacts. The biggest current problem is that delivery budget is client-provided without server clamp. The second is scanner source authority: tactical output should be driven by a server-resolved active scanner source, not only generic visibility sources.

Recommended next visibility work:

1. Server clamp local-view delivery range.
2. Fix observer anchor f64 preference.
3. Implement active scanner server source.
4. Reuse spatial/contact indexes for tactical stream generation.
5. Add component/field redaction policy for future tactical detail tiers.

### Client Prediction, Interpolation, and Motion Ownership

The client is close to the modern pattern:

1. Local controlled entity predicts in a fixed-step shared simulation.
2. Remote/non-controlled entities interpolate or receive authoritative state.
3. Render transforms do not feed back into Avian simulation.
4. Handoff waits for predicted clone readiness before local writes.

Remaining risks:

1. Client combat prediction needs a precise authority/rollback contract.
2. Visual/adoption/recovery systems should be measured until steady-state interventions trend toward zero.
3. WASM builds must continue compiling shared runtime code because the client is one crate with native bin and WASM lib targets.

### Rendering Layers and Engines

Render-layer and asset systems are data-driven enough for the current stage. Runtime render graph validation exists in scripting (`crates/sidereal-scripting/src/lib.rs:683`), and asset catalogs are Lua-authored.

The main issue is module size and ownership. `visuals.rs`, `backdrop.rs`, `ui.rs`, `render_layers.rs`, and `shaders.rs` have accumulated too many subdomains. This is becoming an engineering velocity risk more than a runtime correctness risk.

Recommended split order:

1. Move visual effects with independent state first: projectiles, thrusters, explosions.
2. Move data-driven material/shader binding next.
3. Move tactical/owner UI out of broad `ui.rs`.
4. Keep `app_setup.rs` and plugin files as wiring only.

### Shared Game Core and Physics

Shared fixed-step simulation is the right foundation. Mass/inertia synchronization is present, and the client avoids fuel consumption in prediction per the prediction docs. The flight system is stable for current ship gameplay, but it is still module-family specific.

Recommended evolution:

1. Keep physics math in shared Rust.
2. Rename or layer `Engine` into a generic propulsion/thrust provider if non-engine motion modules are planned.
3. Reinstate capability filtering.
4. Keep scripts emitting intents, not transforms or physics writes.

## MMO Scaling Assessment

Modern authoritative MMO networking patterns converge on the same ideas:

1. Dedicated server authority over game state.
2. Client prediction only for locally controlled entities.
3. Snapshot/interpolation timelines for remote entities.
4. Interest management/AOI so clients only receive entities they can interact with or know about.
5. Persistent replication-list caches or spatial rooms/cells to avoid per-entity/per-client checks.
6. Server-owned bandwidth and CPU budgets.
7. Separate read models for strategic/intel UI, not raw full world replication.

Sidereal currently meets items 1 through 4 in architecture and partially meets 5 through 7. For "galaxy-wide" play, the next scaling step is not just bigger coordinates. It needs world partitioning:

1. Sector/cell ownership and shard routing.
2. A cross-shard interest service or gateway fanout policy.
3. Background simulation tiers for distant systems.
4. Contact/intel summaries that can cross sector boundaries without raw entity replication.
5. Server-side AOI budgets per player and per sector.
6. Deterministic persistence write ownership for entities crossing sectors.

The current `VisibilitySpatialIndex` and tactical lanes are good local-shard building blocks. They should evolve into a replication graph/room-like model with persistent per-sector lists, not repeated global scans.

## Data Exposure Review

No direct transform-authority exploit was found. The client does not appear able to authoritatively set world position/velocity. The strongest protections are authenticated player binding and authoritative controlled-target validation.

Potential data exposure vectors to close:

1. Client-supplied delivery radius can widen authorized delivery and server work.
2. Tactical scanner products can be emitted without server-side active scanner gating.
3. Full visible tactical contacts include faction, size, mass, heading, and velocity. That is acceptable for full visibility, but future scanner tiers need field-level redaction.
4. Authenticated asset bootstrap currently returns full catalog metadata. This is acceptable under current contract but may reveal future unreleased/hidden asset IDs in production.
5. Admin/script endpoints are well guarded, but script draft/publish routes are high impact and should have audit logs if not already covered.

## Duplication and Consolidation Opportunities

1. Visibility position resolution exists in multiple forms. Consolidate f64-first world-position resolution into one shared helper used by visibility, tactical, observer anchors, scripting, owner manifest, and notifications.
2. Client scanner profile calculation should move into shared or server-reusable code so tactical server gating and client presentation cannot drift.
3. Action capability filtering should be one shared gameplay system, not per-subsystem implicit ignore logic.
4. Client visual systems should be split by effect type to avoid repeated entity classification and label parsing.
5. Gateway starter-world bundle resolution should be generic bundle metadata, reused by admin spawn and character creation.

## Prioritized Remediation Plan

### P0 - Security and contract correctness

1. Server-clamp `ClientLocalViewModeMessage.delivery_range_m`.
2. Fix observer anchor position resolution to prefer f64 `Position`.
3. Implement server-side active scanner source for tactical contacts/fog.
4. Add tests for malicious local-view range, scannerless free roam, and large-coordinate visibility anchors.

### P1 - Prediction and networking robustness

1. Add control handoff integration tests across self/entity/entity switches and disconnect/reconnect.
2. Audit predicted combat component ownership and add rollback/reconciliation tests.
3. Add role-rearm metrics and duplicate-clone regression tests.
4. Narrow protocol directions or add explicit receiver-side direction tests.

### P2 - Scaling and maintainability

1. Replace tactical all-entity scans with spatial/contact indexes.
2. Split largest client and replication modules by domain.
3. Build a sector/room replication model that can become the shard-level AOI layer.
4. Rework scripting runtime around generic entity/capability intents and f64 positions.
5. Genericize starter controlled entity setup away from ship-only backend contracts.

## Validation Gaps

This was a static audit. I did not run:

```bash
cargo fmt --all -- --check
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings
CARGO_INCREMENTAL=0 cargo check --workspace
CARGO_INCREMENTAL=0 cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
CARGO_INCREMENTAL=0 cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

Recommended targeted tests to add or run after fixes:

1. `sidereal-replication` visibility tests for delivery clamp and f64 anchor.
2. `sidereal-replication` tactical tests for scannerless/free-roam redaction.
3. `sidereal-game` shared simulation parity tests for any new propulsion/capability changes.
4. Native two-client smoke test for control handoff, prediction, remote interpolation, and tactical map.
5. WASM check whenever shared client runtime, transport, asset loading, or prediction code changes.

## Appendix: Evidence Index

Key internal references:

- Visibility contract: `docs/features/visibility_replication_contract.md:58`.
- Tactical scanner target behavior: `docs/features/tactical_and_owner_lane_protocol_contract.md:39`.
- Prediction runtime baseline: `docs/features/prediction_runtime_tuning_and_validation.md:45`.
- Asset delivery contract: `docs/features/asset_delivery_contract.md:57`.
- Protocol version and messages: `crates/sidereal-net/src/lightyear_protocol/messages.rs:4`.
- Protocol bidirectional registration: `crates/sidereal-net/src/lightyear_protocol/registration.rs:73`.
- Replication server plugin setup: `bins/sidereal-replication/src/main.rs:123`.
- Replication schedule ordering: `bins/sidereal-replication/src/plugins.rs:120`.
- Auth binding: `bins/sidereal-replication/src/replication/auth.rs:478`.
- Realtime input validation/drain: `bins/sidereal-replication/src/replication/input.rs:215`, `bins/sidereal-replication/src/replication/input.rs:516`.
- Control handoff: `bins/sidereal-replication/src/replication/control.rs:220`, `bins/sidereal-replication/src/replication/control.rs:434`, `bins/sidereal-replication/src/replication/control.rs:638`.
- Visibility local-view receive: `bins/sidereal-replication/src/replication/visibility.rs:1008`.
- Observer anchor position: `bins/sidereal-replication/src/replication/runtime_state.rs:141`.
- Tactical contact stream: `bins/sidereal-replication/src/replication/tactical.rs:671`.
- Runtime scripting API: `bins/sidereal-replication/src/replication/runtime_scripting.rs:111`, `bins/sidereal-replication/src/replication/runtime_scripting.rs:525`.
- Gateway starter world: `bins/sidereal-gateway/src/auth/starter_world.rs:139`.
- Gateway character creation: `bins/sidereal-gateway/src/auth/service.rs:486`.
- Gateway admin claims: `bins/sidereal-gateway/src/auth/service.rs:1001`.
- Gateway assets: `bins/sidereal-gateway/src/api.rs:578`, `bins/sidereal-gateway/src/api.rs:682`.
- Shared simulation plugin: `crates/sidereal-game/src/lib.rs:155`.
- Flight/engine implementation: `crates/sidereal-game/src/flight.rs:1`.
- Action capability no-op: `crates/sidereal-game/src/actions.rs:133`.
- Client runtime prediction setup: `bins/sidereal-client/src/runtime/app_setup.rs:117`.
- Client input send: `bins/sidereal-client/src/runtime/input.rs:238`.
- Client local-view update: `bins/sidereal-client/src/runtime/control.rs:132`.
- Client scanner profile cache: `bins/sidereal-client/src/runtime/sensor_ring.rs:79`.
