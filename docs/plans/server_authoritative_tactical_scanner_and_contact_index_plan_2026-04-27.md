# Server-Authoritative Tactical Scanner and Contact Index Plan - 2026-04-27

Status: Active implementation plan
Last updated: 2026-04-27
Owners: replication + gameplay visibility + tactical UI
Scope: audit follow-up for H3 tactical scanner gating and M1 tactical contact streaming scale from `docs/reports/gateway_replication_client_full_audit_report_2026-04-27.md`

Primary references:
- `docs/reports/gateway_replication_client_full_audit_report_2026-04-27.md`
- `docs/features/tactical_and_owner_lane_protocol_contract.md`
- `docs/features/tactical_sensor_ring_design_contract.md`
- `docs/features/visibility_replication_contract.md`
- `docs/features/visibility_system_v2_signal_detection_contract.md`
- `bins/sidereal-replication/src/replication/tactical.rs`
- `bins/sidereal-replication/src/replication/visibility.rs`
- `bins/sidereal-replication/src/replication/runtime_state.rs`
- `crates/sidereal-game/src/components/scanner_component.rs`

## 0. Implementation Status

2026-04-27 status note:

1. H3 remains open server-side. The client sensor ring gates presentation on an effective scanner profile, but the replication tactical stream still derives fog/contact products from `VisibilityDisclosure.visibility_sources` and does not require a server-resolved scanner on the currently controlled non-player-anchor entity.
2. M1 remains open. `stream_tactical_snapshot_messages` still iterates all replicated entities for each authenticated client at the tactical interval.
3. Existing useful foundations: `ScannerComponent`, `VisibilityRangeM`, `VisibilityRangeBuffM`, `ContactResolutionM`, `SignalSignature`, `VisibilityClientContextCache`, `VisibilityMembershipCache`, and `VisibilitySpatialIndex`.
4. Native impact: changes affect server-authored tactical products consumed by the native tactical map and TAB sensor ring.
5. WASM impact: no protocol split is allowed. Browser clients must consume the same tactical messages and redaction semantics.

2026-04-27 implementation update:

1. Phase 1 is partially implemented in `bins/sidereal-replication/src/replication/tactical.rs`: each tactical stream update resolves an effective scanner from the authenticated player's currently controlled non-player-anchor entity before producing live fog cells or contacts.
2. Implemented source rules: root `ScannerComponent`, direct mounted scanner modules via `MountedOn.parent_entity_id`, deterministic best-scanner selection, `VisibilityRangeM` over `ScannerComponent.base_range_m`, controlled-root `ContactResolutionM`, scanner range gating for full visible contacts, signal-only contacts from the same scanner source, and scanner `max_contacts` budgeting.
3. Still open for H3: scanner-tier redaction helper and full tier disclosure tests.
4. Still open for M1: the first implementation slice still scans replicated entities inside the per-client loop; the authoring cache and spatial contact index remain the next scalability phase.

## 1. Goals

1. Make tactical scanner output server-authoritative.
2. Ensure free roam/player-anchor control produces no scanner-derived tactical contacts or live scanner cells.
3. Ensure controlled entities without an effective scanner produce no scanner-derived tactical contacts or live scanner cells.
4. Preserve owner manifest behavior; owned-asset lists are not scanner products.
5. Replace per-client global entity scans with reusable server-side tactical contact caches and spatial indexes.
6. Preserve the existing tactical snapshot/delta protocol unless a later phase proves a new message type is required.

## 2. Non-Goals

1. Do not widen Lightyear entity replication visibility.
2. Do not let client local-view mode, camera range, or TAB ring state become scanner authority.
3. Do not move private component-derived classification onto the client.
4. Do not make mounted scanner resolution depend on Bevy `Children`; use UUID relationships and existing runtime maps.
5. Do not replace durable fog memory or `PlayerExploredCells`.

## 3. Target Model

The tactical stream should be driven by four explicit server read models:

1. `EffectiveScannerSourceCache`
   - keyed by canonical `player_entity_id`;
   - resolved from `PlayerControlledEntityMap`;
   - contains controlled entity, scanner entity, scanner profile, scanner range, contact resolution, and source world position;
   - absent when the player controls the player anchor, free roam, or a non-scanner entity.

2. `TacticalContactAuthoringCache`
   - keyed by world entity;
   - stores stable source data used to build tactical contacts: GUID, kind, public map icon, faction, world position, heading, velocity, extent, mass, static landmark metadata, signal signature, and player-tag exclusion;
   - refreshed when relevant components change or at the existing tactical cadence for moving entities.

3. `TacticalContactSpatialIndex`
   - keyed by grid cell;
   - indexes contact authoring entries by authoritative f64-derived world position and extent;
   - supports source-radius and signal-radius candidate lookups.

4. `PlayerTacticalStreamState`
   - keeps current sequence, fog/contact memory, signal contact memory, and notification idempotence;
   - remains the per-player diff builder, but consumes candidate entity sets from the scanner source and spatial index instead of scanning every replicated entity.

## 4. Effective Scanner Source Rules

Resolve once per tactical stream update before building contacts.

1. Parse the authenticated player id from `AuthenticatedClientBindings`.
2. Resolve the player ECS entity through `PlayerRuntimeEntityMap`.
3. Resolve the currently controlled entity through `PlayerControlledEntityMap`.
4. If the controlled entity is absent or equals the player anchor entity, no scanner source exists.
5. Resolve the effective scanner from:
   - `ScannerComponent` on the controlled root;
   - mounted child entities whose `MountedOn.parent_entity_id` matches the controlled root `EntityGuid`;
   - later nested mounted scanners only after an explicit recursive mount-chain implementation.
6. Choose the best scanner deterministically:
   - highest `detail_tier`;
   - then largest effective range;
   - then highest `level`;
   - then highest `max_contacts`;
   - then stable GUID order.
7. Scanner range uses the hot visibility range lane:
   - prefer scanner entity `VisibilityRangeM`;
   - otherwise use scanner `base_range_m`;
   - optionally cap to controlled root `VisibilityRangeM` if the feature contract later requires range inheritance.
8. Contact resolution uses controlled root `ContactResolutionM` for first slice; later phases may aggregate mounted scanner resolution buffs.

## 5. Tactical Product Rules

When no effective scanner source exists:

1. send empty live scanner cells;
2. send no scanner-derived tactical contacts;
3. keep already explored fog memory intact;
4. optionally retain stale contact memory only if future stale-intel policy explicitly allows it.

When an effective scanner source exists:

1. live fog cells rasterize from scanner source position and scanner range, not generic visibility sources;
2. full visible contacts may be emitted only if the contact is in the scanner candidate set and authorized for tactical disclosure;
3. signal-only unknown contacts are computed from the scanner source and `SignalSignature`;
4. contact count is capped by scanner `max_contacts`;
5. prioritization should be deterministic:
   - player-owned/self-relevant contacts first when applicable;
   - live visible contacts before signal-only contacts;
   - stronger signal or closer distance before weaker/farther contacts;
   - stable GUID/contact ID tie-breaker.

## 6. Redaction by Scanner Tier

First implementation should enforce the existing tiers:

| Tier | Server disclosure |
| --- | --- |
| No scanner | No scanner contacts or live scanner cells |
| `Basic` | `unknown` contact identity, approximate position, signal strength/quality when signal-derived |
| `Iff` | Basic plus server-authorized relationship/classification where policy allows |
| `Classified` | IFF plus kind/map icon/faction when authorized |
| `Telemetry` | Classified plus velocity/heading and public presentation hints such as size/mass when authorized |

The current exact-contact path in `stream_tactical_snapshot_messages` should be replaced with a `redact_tactical_contact_for_scanner(...)` helper so tests can cover each tier without building a whole Bevy app.

## 7. Contact Index Plan

Phase M1 should remove the `clients * all_replicated_entities` hot loop.

1. Build `TacticalContactAuthoringCache` from replicated world entities once per tactical update.
2. Reuse current `tactical_world_position` precedence: Avian `Position`, then `WorldPosition`, then `GlobalTransform`.
3. Insert non-player contact entities into `TacticalContactSpatialIndex`.
4. For each effective scanner source, query only cells overlapping:
   - scanner range plus max indexed extent for normal contacts;
   - scanner range plus maximum signal detection radius plus max signal extent for signal candidates.
5. Deduplicate candidate entities before redaction.
6. Keep index internals private to `replication/tactical.rs` initially; split to `replication/tactical_index.rs` only once tests or type size justify it.

## 8. Implementation Phases

### Phase 1: Scanner Source Resolution

1. Add `EffectiveScannerSource` and `EffectiveScannerSourceCache` in replication tactical code.
2. Add scanner source resolver that reads `PlayerControlledEntityMap`, player entity map, `EntityGuid`, `ScannerComponent`, `MountedOn`, `VisibilityRangeM`, `ContactResolutionM`, and world position.
3. Gate tactical fog/contact output on scanner source presence.
4. Tests:
   - player anchor/free roam produces no scanner source;
   - controlled entity with no scanner produces no scanner source;
   - root scanner resolves;
   - mounted scanner resolves;
   - deterministic best scanner selection.

### Phase 2: Server Redaction

1. Add `redact_tactical_contact_for_scanner`.
2. Route full visible and signal-only contacts through the redaction helper.
3. Enforce scanner `max_contacts`.
4. Tests:
   - `Basic` redacts faction, velocity, size, mass, and exact classification;
   - `Iff` discloses only allowed classification;
   - `Telemetry` may disclose velocity/heading and public presentation hints;
   - no scanner emits empty contacts even when `ReplicationState` says visible.

### Phase 3: Contact Authoring Cache and Spatial Index

1. Add authoring cache and spatial index resources.
2. Refresh them before streaming tactical messages in `ReplicationVisibilitySet::Streaming`.
3. Replace the per-player `for replicated_entities` loop with scanner candidate lookup.
4. Tests:
   - candidate lookup returns in-range contacts and excludes out-of-range contacts;
   - signal radius includes high-signal contacts outside ordinary scanner range;
   - duplicate cells do not duplicate contacts;
   - index uses f64 authoritative position precedence.

### Phase 4: Metrics and Guardrails

1. Add tactical metrics for:
   - active scanner sources;
   - players without scanner source;
   - contact authoring cache size;
   - spatial cells scanned;
   - candidate count before/after redaction;
   - contact budget truncations;
   - tactical stream update time.
2. Add debug log throttling for clamped/contact-budget cases.
3. Surface key counts in replication health/debug snapshots if they are already tracking visibility/tactical metrics.

### Phase 5: Docs and Validation

1. Update `docs/features/tactical_and_owner_lane_protocol_contract.md` from "open follow-up" to implemented behavior once Phase 1 and Phase 2 land.
2. Update `docs/features/tactical_sensor_ring_design_contract.md` server implementation status.
3. Update `docs/features/visibility_replication_contract.md` with the final scanner-source authority rule.
4. Run native two-client validation:
   - scanner-capable controlled ship sees contacts;
   - non-scanner controlled entity gets no tactical contacts;
   - player-anchor/free roam gets no scanner products;
   - existing `M` tactical map still receives legal scanner products;
   - TAB ring mirrors the same server products.
5. Run required checks:
   - `cargo fmt --all -- --check`;
   - `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings`;
   - `CARGO_INCREMENTAL=0 cargo check --workspace`;
   - client WASM and Windows target checks because shared client protocol expectations are affected.

## 9. Acceptance Criteria

H3 is addressed when:

1. server tactical products are absent without a server-resolved effective scanner source;
2. scanner source resolves from current controlled entity, not camera/free-roam/player anchor;
3. scanner tier redaction is tested;
4. client sensor ring and tactical map consume only server-disclosed tactical data.

M1 is addressed when:

1. tactical streaming no longer loops all replicated entities inside each client/player loop;
2. tactical contacts are built from cached authoring data plus spatial candidate lookup;
3. metrics show candidate counts and index scan cost;
4. tests cover in-range, out-of-range, signal-range, and duplicate candidate behavior.

## 10. Open Decisions

1. Whether the full `M` tactical map should show no scanner products when the player controls no scanner entity, or whether it should retain a separate low-detail "known intel only" mode. First implementation should be strict: no live scanner products without scanner.
2. Whether scanner `base_range_m` should be metadata only or an authoritative fallback when `VisibilityRangeM` is absent. First implementation may use it as fallback to avoid silent no-range scanners, but content should continue authoring `VisibilityRangeM`/`VisibilityRangeBuffM`.
3. Whether stale tactical contact memory should persist when the player loses scanner source. First implementation should fail closed and remove live contacts; stale-intel reintroduction needs a separate policy.
4. Whether mounted scanner resolution should recurse through nested mounts in Phase 1. First implementation should support direct mounted children only, matching current client ring behavior.
