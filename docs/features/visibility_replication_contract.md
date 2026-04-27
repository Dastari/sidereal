# Visibility and Replication Contract

Status: Active implementation contract
Last updated: 2026-04-27
Owners: replication + gameplay + client runtime
Scope: server-authoritative visibility, delivery narrowing, payload disclosure, and tactical/owner lane interaction

Primary references:
- `docs/sidereal_design_document.md`
- `AGENTS.md`
- `docs/decisions/dr-0017_dual_lane_replication_and_owner_asset_manifest.md`
- `docs/plans/scan_intel_minimap_spatial_plan.md`
- `docs/features/visibility_system_v2_signal_detection_contract.md`

## 0. Implementation Status

2026-04-24 status note:

1. Implemented: server-driven per-client visibility updates, spatial-grid candidate preselection, fail-closed policy checks, generic `VisibilityRangeM`/`VisibilityRangeBuffM`, and player-scoped debug/inspection components.
2. Implemented: discoverable static landmarks, parallax-aware delivery behavior, extent-aware large-body culling, and lower-cadence landmark discovery maintenance are current runtime behavior.
3. Implemented: tactical fog/contact and owner manifest lanes are separate read models that do not widen local-bubble authorization.
4. Partial/open: richer faction/public redaction details, large-scale tuning, and future background-simulation promotion interactions still need follow-up as those systems mature.

2026-04-27 status note:

1. Implemented partial V2 baseline: generic `SignalSignature` detection now feeds redacted unknown tactical contacts, scanner-controlled approximate contact accuracy, signal-triggered static-landmark discovery, and buffered native local-view delivery requests.
2. V2 does not replace the V1 authorization order. Signal-only detection may create a tactical/intel product or trigger `StaticLandmark` discovery, but it must not grant ordinary full entity replication by itself.
3. Signal-only static-landmark discovery must not emit identity-bearing landmark notification payloads; player-facing signal messaging remains redacted until normal visibility/delivery discloses the body.
4. Rapid zoom-out culling must use buffered/hysteretic projected bounds on both server delivery requests and client local render culling so large/parallaxed bodies do not snap in at viewport edges. Current native client requests apply viewport overscan; additional render-local culling tuning remains open.

2026-04-27 status note:

1. Tactical sensor ring client presentation now requires an effective scanner profile on the actively controlled non-player-anchor entity before TAB can open the ring.
2. The target server contract is stricter than the first client slice: tactical contact disclosure and any scanner-derived contact detail must resolve scanner capability from the currently controlled entity, not from free-roam/player-anchor camera state.
3. Free roam/player-anchor control has no scanner source for scanner-derived tactical products. Existing full tactical-map/fog behavior remains separate until server-side scanner-tier gating is implemented.

2026-04-24 update:
- Visibility spatial indexing and static-landmark discovery now resolve static non-physics world entities from canonical `WorldPosition` before falling back to Bevy `GlobalTransform`. This prevents static planets/celestial bodies from being indexed at stale/default transform positions before transform propagation catches up.
- Native impact: discoverable `WorldPosition` landmarks such as planets can be discovered/delivered when the player is actually near their authoritative world location. WASM impact: no protocol split; the server-side visibility fix benefits all clients.

2026-04-24 update:
- Static landmark discovery now emits a server-authored player notification after `DiscoveredStaticLandmarks` is updated. This is a presentation/history side effect only; it does not widen authorization, delivery, or payload disclosure for the discovered landmark or any related entity.
- Native impact: the selected player sees a non-blocking toast for newly discovered landmarks. WASM impact: shared notification protocol and queue path; no platform-specific discovery authority path.

2026-04-24 update:
- DR-0035 makes f64 authoritative world coordinates the accepted target for visibility and replication. Visibility candidate generation, observer anchors, static-landmark discovery, delivery checks, tactical contacts, and owner manifest positions must prefer f64 Avian `Position` / f64 `WorldPosition` before falling back to f32 Bevy transforms.
- Native impact: visibility and delivery checks remain stable at galaxy-scale coordinates. WASM impact: no platform-specific visibility model; browser clients consume the same f64 protocol payloads and project to f32 only at render/UI boundaries.

## 1. Goal

Keep visibility and replication server-authoritative, scalable, and disclosure-safe:

1. Server decides what each client can know.
2. Delivery culling narrows authorized data only.
3. Component/field disclosure is policy-driven.
4. Tactical/fog/intel memory behavior is explicit and lane-based.

## 2. Canonical Stage Order (Mandatory)

All visibility-sensitive changes must preserve:

1. Authorization scope (security entitlement):
   1. ownership/public/faction policy,
   2. scanner/fog/intel grant policy.
2. Delivery scope (performance narrowing):
   1. local bubble/tactical lane range and mode.
3. Payload scope (redaction):
   1. component/field masking before serialization.

Delivery must never widen authorization.

Spatial candidate generation is optimization input only, not authorization.

## 3. Runtime Baseline (Implemented)

Current implementation baseline:

1. Per-client visibility updates are server-driven in replication fixed tick.
2. Candidate preselection uses spatial grid by default (`SIDEREAL_VISIBILITY_CANDIDATE_MODE=spatial_grid`).
3. Full policy checks run after candidates; policy exceptions (owner/public/faction/scanner) are fail-closed safe.
4. World position checks use authoritative Avian `Position` or `WorldPosition` when present; `GlobalTransform` is fallback-only for entities without an authoritative spatial component.
5. Observer anchor identity is player entity (`camera <- player <- controlled(optional)`).
6. Current runtime uses generic `VisibilityRangeM` / `VisibilityRangeBuffM` with no implicit `ShipTag` baseline.
7. `VisibilitySpatialGrid` and `VisibilityDisclosure` are mirrored onto player entity for owner debug/inspection.
8. Delivery range is dynamic per client view and reflected in runtime visibility telemetry.
9. Fullscreen authored config entities are treated as non-spatial overlays: legacy `FullscreenLayer` entities and fullscreen-phase `RuntimeRenderLayerDefinition` entities bypass delivery-range/visibility-range candidate culling and remain replicated while connected.
10. Background authoring settings such as `SpaceBackgroundShaderSettings` and `StarfieldShaderSettings` are durable world configuration and remain persistable so hydration recreates the full authored config entity rather than only the layer-definition shell.
11. Discoverable static landmarks use explicit landmark classification plus player-scoped durable discovery state; they are not modeled as generic public-visibility entities.
12. Discovery notifications are derived from authoritative discovery updates and are not client-side discovery authority.

2026-03-09 update:
- The native client renders fullscreen background passes directly from those authored fullscreen entities again. Client-local fullscreen renderable copies were removed because they could diverge from the authored source during zoom/hydration transitions and expose the black fallback layer.
- Delivery-scope and visibility-range distance checks must account for entity extent, not only entity center position. Large bodies such as stars and planets remain delivered/authorized while any visible portion overlaps the active delivery or visibility radius; center-point-only culling is not runtime-correct.

2026-03-09 update:
- Static discoverable landmarks now have a distinct post-discovery authorization lane. Once a player legitimately discovers a qualifying landmark, replication may authorize that landmark without requiring current scanner/visibility-range overlap, but local delivery narrowing still applies.
- Landmark discovery state is persisted on the player ECS entity, not inferred ad hoc from free-roam camera position and not stored in account-side tables.
- Discovery-based authorization grants only landmark presence for qualifying static entities; it does not widen payload disclosure for unrelated entities or turn landmarks into generic public visibility.

2026-03-09 update:
- For parallaxed discovered landmarks, delivery narrowing must account for the authored world-layer parallax factor rather than using the authoritative center alone. Otherwise the server can cull a landmark before its projected render center leaves the buffered viewport.
- Candidate prefiltering must not reject already-discovered static landmarks before the landmark-specific delivery check runs.

2026-03-11 update:
- Static-landmark discovery maintenance now runs on a lower-cadence server lane separate from the hottest per-tick visibility membership update. The current runtime default cadence is 0.25 seconds.
- Dynamic per-tick visibility still consumes the authoritative discovered-landmark state every visibility tick; the cadence split changes maintenance timing only and does not change the authorization ordering (`Authorization -> Delivery -> Payload`).

## 3.1 Static Landmark Discovery Contract

Current backend behavior:

1. Discoverable landmarks are authored with `StaticLandmark` on ordinary persisted world entities.
2. Static landmark discovery state is persisted on the player ECS entity as `DiscoveredStaticLandmarks`; it is not account-scoped and must not move to a side table.
3. Discovery checks are driven by player visibility sources resolved from the generic `VisibilityRangeM` / `VisibilityRangeBuffM` path.
4. Discovery overlap uses the visibility source range plus optional `StaticLandmark.discovery_radius_m`; when `use_extent_for_discovery=true`, the entity extent is included so large bodies can be discovered by their visible edge, not only by their center point.
5. `always_known=true` landmarks are considered discoverable/authorized without a range overlap. `discoverable=false` landmarks do not enter normal discovery unless another policy authorizes them.
6. Discovery maintenance runs at a lower cadence than the hot visibility membership tick; current runtime default is 0.25 seconds.
7. Discovery position resolution must prefer authoritative physics `Position` when present, then static `WorldPosition`, then Bevy `GlobalTransform` as a fallback only.
8. A discovered landmark may be authorized outside current scanner coverage, but it must still pass delivery narrowing before world replication is sent to the client.
9. Discovery authorization grants landmark presence only. It does not widen unrelated entity visibility, bypass payload redaction, or convert the landmark into generic public visibility.
10. Parallaxed/layered landmarks must use projected delivery bounds that account for render-layer parallax and visual-stack scale so a discovered planet is not dropped while its rendered disc remains inside the buffered view.

Current limitations:

1. The backend does not yet emit a dedicated "landmark discovered" gameplay/UI notification message.
2. The client does not yet have a sonar/toast presentation path for first discovery.
3. Tactical-map landmark reveal is currently implied through ordinary replicated/map-icon visibility rather than a dedicated discovery delta stream.
4. Discovery is player-local only; faction, party, or organization-shared discovery policies are not implemented.
5. Landmark intel has one binary state: undiscovered/discovered. Rich scan quality, discovery history, codex text, and partial classification tiers are future work.
6. Test coverage exists for authorization/delivery behavior and static `WorldPosition` regression, but a broader lifecycle suite is still needed for persistence/hydration, no-rediscovery-spam, notification emission, and map projection.

Planned direction:

1. Add a server-authored discovery event, for example `ServerLandmarkDiscoveredMessage`, keyed by authenticated `player_entity_id` and landmark entity UUID.
2. Route the event into a generic client notification/sonar queue; the UI component must not infer discoveries from local-only rendering side effects.
3. Add tactical/minimap projection support so newly discovered landmarks can reveal/update markers without requiring the client to scrape raw ECS internals.
4. Extend discovery policy later for faction/shared discovery as an explicit server rule, not a client-side cache merge.
5. Add optional scan-intel tiers for richer landmark metadata while preserving `Authorization -> Delivery -> Payload` redaction order.
6. Add lifecycle tests covering discovery persistence, hydration, notification idempotence, extent-aware overlap, `WorldPosition` indexing, and discovered-landmark delivery bounds.

## 4. Multi-Lane Contract (Current + Approved Direction)

Lane model:

1. `LocalBubbleLane`:
   1. high-rate nearby simulation state,
   2. authoritative world entities.
2. `TacticalLane`:
   1. lower-rate reduced contact/fog payload for zoomed-out map.
3. `OwnerAssetManifestLane`:
   1. owner-only asset list/state,
   2. independent of local bubble relevance.

Normative rules:

1. Owned-asset UI must not depend on local-bubble world-entity presence.
2. Owner manifest is server-authored and client-cached as read model.
3. Tactical lane never upgrades authorization; it is a reduced delivery/product lane.

## 5. Fog of War and Intel Memory Contract

Fog/intel behavior:

1. Players start with unexplored space (`0` explored coverage).
2. Exploration permanently grows discovered map coverage (`ExploredCells`).
3. Live intel is only from current visibility/live visibility.
4. Outside live visibility, only server-stored last-known intel may be shown (stale memory).
5. Tactical explored-memory persistence uses chunked binary component payloads (adaptive dense/sparse chunk encoding), not flat JSON coordinate lists.
6. Tactical fog memory cell size is 100m and independent from visibility relevance spatial grid cell size.

Authoritative placement:

1. Intel memory is server-authoritative and persisted on player-scoped data (player entity components).
2. Raw intel-memory components are not required to be standard replicated world components.
3. Client receives tactical projection products (snapshot/delta lane payloads), not unrestricted raw memory.

## 6. Disclosure Policy (Pending Scope Preserved)

Still required and explicitly preserved:

1. Faction-based visibility scopes.
2. Component/field-level visibility/redaction policy (for example inventory detail requiring scan-intel grant).
3. Snapshot-vs-stream grant semantics for scan intel.

Current status:

1. These are active contract requirements.
2. Some parts remain implementation-in-progress and must not be removed from docs.

## 7. Edit Checklist (Mandatory)

For any PR touching visibility, tactical delivery, fog/intel memory, or redaction:

1. Verify stage order remains `Authorization -> Delivery -> Payload`.
2. Verify tactical/owner lanes do not bypass authorization.
3. Verify unauthorized component/field data is never serialized.
4. Verify player observer-anchor identity rules remain consistent.
5. Verify fog/intel memory semantics:
   1. unexplored vs explored,
   2. live vs stale intel.
6. Add/update tests for changed behavior.
7. Update:
   1. this contract,
   2. any related decision detail under `docs/decisions/`,
   3. `docs/decision_register.md` links when decisions change.

## 8. Visibility Range Naming Direction (Accepted)

Canonical generic visibility-range terminology is now:

1. `VisibilityRangeM`
   - effective resolved visibility/disclosure range read by the hot visibility path
2. `VisibilityRangeBuffM`
   - generic contributing modifier to visibility range

Normative direction:

1. Visibility systems should converge on `VisibilityRangeM` / `VisibilityRangeBuffM`.
2. Genre-specific names such as `scanner` may remain in Lua/content authoring, but not as the engine-owned built-in runtime concept.
3. `ShipTag` must not imply hidden baseline visibility range.
4. Root entities may carry both effective `VisibilityRangeM` and local `VisibilityRangeBuffM`.
5. Aggregation should compute root effective range from generic contributors before hot visibility checks run.

Implementation note:

1. Runtime code now uses `VisibilityRangeM` / `VisibilityRangeBuffM`.
2. `VisibilityDisclosure` now carries `visibility_sources`, not `scanner_sources`.
3. Genre-specific `scanner_*` wording may still exist in content/action names, but not as the engine-owned runtime component names.
4. See:
   - `docs/decisions/dr-0028_generic_visibility_range_components.md`
