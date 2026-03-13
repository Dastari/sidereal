# Discovered Static Landmark Visibility Plan

Status: Implementation plan  
Date: 2026-03-09  
Scope: server-authoritative discovery and post-discovery visibility for static celestial landmarks

Status notes:
- 2026-03-10: Extended the plan with a separate Lua-authored player landmark flow for quest/navigation markers. This is additive to discovered static landmark visibility and follows the scripting authority contract in `docs/features/scripting_support.md`.

Primary references:
- `docs/features/visibility_replication_contract.md`
- `docs/sidereal_design_document.md`
- `docs/decisions/dr-0030_non_physics_world_spatial_components.md`
- `docs/features/galaxy_world_structure.md`
- `AGENTS.md`

## 1. Problem Statement

Current visibility behavior is scanner/range-centric and treats planets, stars, black holes, environment-lighting anchors, and similar static celestial content too much like ordinary live-relevance entities.

That creates two design/runtime mismatches:

1. Static landmarks that the player has already discovered can disappear from in-world presentation when they leave active scanner range, even though the content is durable and should remain known.
2. Free-roam camera behavior and delivery culling are doing too much work as a proxy for knowledge state, which risks both spoiler leakage and inconsistent landmark persistence.

We need a distinct visibility lane for static discovered landmarks:

1. Discovery remains server-authoritative.
2. Discovery is persisted on the player entity, not an account row or side table.
3. Once discovered, qualifying static landmarks remain visible to that player independent of current scanner range.
4. This applies only to static landmark classes, not dynamic live-sim entities.
5. The lane must remain compatible with existing authorization-first visibility rules.

## 2. Desired Gameplay Rule

Target rule:

1. A player starts with no discovered static landmarks except any explicitly authored/tutorial/bootstrap exceptions.
2. When a qualifying static landmark enters discovery criteria, the server records it in that player's durable discovery state.
3. After discovery, the player may continue to see that landmark in the game world whenever it is in local world-view scope, even if it is outside current scanner range.
4. Discovery does not imply live tactical/contact detail for unrelated dynamic entities around that landmark.
5. Discovery does not imply universal/global galaxy knowledge; it is player-scoped durable knowledge.
6. Static landmarks remain static-authoritative content: no client authority, no free-roam self-upgrade, no unrestricted world dump.

Examples of likely qualifying content:

1. planets
2. moons
3. stars
4. black holes
5. nebulae / large static gas features
6. other immobile celestial or landmark-grade world entities

Examples of likely non-qualifying content:

1. ships
2. projectiles
3. moving stations if they ever become dynamic
4. cargo drops
5. live tactical contacts that should remain scanner/intel-driven

## 3. Core Design Principles

This feature must preserve:

1. `Authorization -> Delivery -> Payload` ordering from the visibility contract.
2. Player-scoped persistent runtime state on the player ECS entity.
3. Generic entity terminology for generic systems and APIs.
4. Static non-physics landmark placement via `WorldPosition` / `WorldRotation`.
5. One-way authority: client input never authors discovery or landmark visibility directly.

Additional design principles for this feature:

1. Discovery and post-discovery presence are not the same as public visibility.
2. Static landmark visibility should be modeled as a separate authorization lane, not a hack on scanner range.
3. Discovery persistence should store stable entity UUIDs or stable landmark IDs only, never Bevy runtime `Entity`.
4. The system should generalize beyond planets from day one.
5. The implementation should prefer static landmark products over full live-entity semantics where possible.

## 4. Proposed Runtime Model

Introduce a new server-owned concept:

1. `Discovered Static Landmark Visibility`

High-level model:

1. Static landmark entities opt into a landmark/discovery classification.
2. Replication visibility evaluates a discovered-landmark authorization path before delivery narrowing.
3. The player entity persists durable discovery state identifying which landmarks are known.
4. If a landmark is discovered and the client's current local view is near enough to need it rendered, the server continues to deliver the landmark even outside scanner range.
5. Delivery remains camera/player scoped as a narrowing filter; it just no longer depends on current scanner range for already discovered static landmarks.

This creates two distinct questions:

1. "Does the player know this landmark exists?"  
   Answered by durable discovery state.
2. "Should the server send/render it right now?"  
   Answered by local delivery scope using current camera/player view.

## 5. Proposed Data Model

### 5.1 Landmark classification on world entities

Add a world-entity classification component for static discoverable landmarks.

Candidate component family:

1. `StaticLandmark`
2. `StaticLandmarkKind`
3. optional `LandmarkDiscoveryPolicy`

Minimum requirements for the landmark component:

1. persistable
2. replicated/publicly safe as metadata
3. stable enough for server-side visibility filtering and debugging

Suggested fields:

1. landmark kind enum/string:
   1. `Planet`
   2. `Moon`
   3. `Star`
   4. `BlackHole`
   5. `Nebula`
   6. future static anomaly kinds
2. `discoverable: bool`
3. optional `discovery_radius_m`
4. optional `always_known: bool`
5. optional `use_extent_for_discovery: bool`

Important:

1. This should not be hardcoded to planet labels alone.
2. Existing celestial-body data such as `CelestialBodyKind` can contribute to classification, but the durable visibility lane should still have an explicit landmark/discovery contract rather than implicit label-only branching.

### 5.2 Player-scoped persistent discovery state

Persist discovery state on the player entity.

Candidate component:

1. `DiscoveredStaticLandmarks`

Suggested payload shape:

1. stable list/set of landmark UUIDs
2. optional per-entry metadata:
   1. first discovered tick/time
   2. discovery method
   3. last confirmed visibility time

Preferred minimal first-pass:

1. `Vec<Uuid>` or deterministic set-like serialized shape of landmark entity GUIDs

Requirements:

1. `Reflect`
2. `Serialize` / `Deserialize`
3. persistable through graph records
4. covered by hydration roundtrip tests

Out-of-scope for first pass:

1. large annotation payloads
2. per-landmark journal text
3. account-wide shared discovery

### 5.3 Optional client-facing reduced product

Depending on replication pressure, consider a reduced landmark product lane later:

1. landmark GUID
2. world position
3. size/extents
4. landmark type
5. static visual identity

First implementation can likely reuse current world-entity replication for static landmark entities, as long as authorization/delivery are corrected.

### 5.4 Lua-authored player landmark lane for quests and guidance

Quest and scripted guidance landmarks should not directly "push landmarks to clients" as an ad-hoc client API. They need a server-authoritative player-scoped data lane that Lua can drive through validated intents.

Recommended first-pass shape:

1. Add an owner-only persisted or session-scoped player component such as `ScriptedPlayerLandmarks`.
2. Each entry represents a client-facing landmark marker rather than a world-entity authorization grant.
3. Lua writes this state through validated script intents, not raw ECS access and not direct replication bypass.

Suggested entry fields:

1. `landmark_id`: stable logical ID for add/update/remove
2. `target_entity_id: Option<Uuid>` for markers anchored to an entity already known to the server
3. `world_position: Option<Vec2/Vec3-like payload>` for markers that point at a coordinate instead of an entity
4. `label` / `short_description`
5. `icon` or `marker_kind`
6. `objective_state` or `quest_stage_id`
7. `expires_at` or `ttl_s` if the marker is temporary
8. `discoverability_policy` or equivalent validation flag when the target references an undiscovered static landmark

Rules:

1. This lane is owner-scoped replicated UI/navigation state, not a new client authority path.
2. A scripted landmark may reference an existing discovered static landmark, a dynamic quest target, or a pure coordinate.
3. When a scripted landmark references an undiscovered hidden world entity, replication must not leak hidden entity payload through the marker. The marker must degrade to a validated coarse pointer, obfuscated coordinate, or be rejected by policy.
4. The client consumes these entries as HUD/map landmarks only. They do not imply full world-entity replication authorization.
5. If persistence is not needed for a marker class, session-scoped player landmark state is acceptable; durable quest landmarks should still live on the player ECS entity when they must survive reconnect/restart.

## 6. Discovery Rules

Discovery should be server-authored and deterministic.

Recommended first-pass discovery criteria:

1. Player discovers a static landmark when any part of the landmark overlaps current live visibility/discovery radius.
2. Discovery checks use landmark extent, not center-only distance.
3. Discovery uses player observer anchor / controlled-entity visibility sources, not detached free-roam camera alone.
4. Free-roam camera must not discover new landmarks by panning into unexplored space with no legitimate discovery source.

Recommended implementation rule:

1. Landmark discovery is granted when the landmark would have been legitimately visible under current live visibility/scanner rules from the player-authorized observer state.

This keeps the exploit boundary explicit:

1. A player can continue seeing a discovered planet while roaming nearby.
2. A detached camera cannot reveal undiscovered planets in unexplored space.

## 7. Authorization and Delivery Changes

Update the visibility pipeline to add a discovered-landmark authorization path.

Proposed authorization structure:

1. Owner/faction/public checks
2. live scanner/disclosure checks
3. discovered static landmark checks

Discovered landmark authorization rule:

1. Authorized if:
   1. target entity is a qualifying static landmark,
   2. player entity discovery state contains that landmark,
   3. landmark payload is within the allowed public/static disclosure scope

Delivery rule after authorization:

1. Use local view distance / delivery-scope narrowing to decide whether to send the landmark now.
2. Do not require current scanner range once discovery is recorded.
3. Keep extent-aware delivery checks for large bodies.

Important:

1. Discovery-based authorization must not widen payload disclosure for private or dynamic components.
2. Discovery lane should authorize only the static landmark presence/product, not unrelated hidden entities or hidden internals.
3. Scripted player landmarks are a separate owner-only replication product and must not reuse discovered-landmark authorization as a backdoor for hidden world state.

### 7.1 Scripted landmark replication rule

For Lua-authored quest/guidance markers:

1. Replicate `ScriptedPlayerLandmarks` owner-only to the bound authenticated player session.
2. The product contains only client marker metadata needed for UI/navigation.
3. If the marker targets a world entity, the replication payload uses stable UUID/logical metadata only and does not imply full component/product visibility for that entity.
4. Validation must reject script requests that would reveal undiscovered/private world state more precisely than policy allows.

## 8. Culling and Rendering Implications

This plan does not replace the separate projected-render-bounds work already identified for parallaxed planets.

However, the visibility design must align with it:

1. Server delivery is based on authoritative landmark world position/extent and discovery state.
2. Client render placement may use client-only projected offsets for parallax.
3. Client frustum/render culling will likely need projected landmark bounds as a separate follow-up.

Normative rule:

1. Discovery-based authorization uses authoritative landmark world placement, not projected/parallaxed visual position.

## 9. Environment Lighting and Related Static World Config

Environment-lighting config needs explicit treatment.

Current issue:

1. `EnvironmentLighting` appears alongside static celestial content but is not a landmark the player "discovers" in the same way.

Recommended rule:

1. Do not put environment-lighting config into the discovered-landmark lane.
2. Treat it as durable world/system configuration with its own replication policy.
3. If clients need it whenever they are in a system or shard, that should be its own always-authorized static config rule, not landmark discovery.

This plan therefore distinguishes:

1. discovered landmark entities: player-knowledge gated
2. static world config entities: system/runtime-config gated

## 10. Implementation Phases

### Phase 0: Decision and contract updates

Before code:

1. Add a DR for discovered static landmark visibility policy.
2. Update `docs/features/visibility_replication_contract.md` with the new authorization lane and discovery semantics.
3. Update any celestial/world-structure docs that currently imply all such bodies are simply `PublicVisibility`.

Deliverables:

1. DR accepted
2. visibility contract updated
3. this plan linked from decision register if promoted to source-of-truth

### Phase 1: Schema and classification

Add components:

1. landmark classification component(s)
2. player discovery state component

Update:

1. component registry generation
2. persistence/hydration mappings
3. Lua bundle allowlists and world-authoring paths
4. script intent and replication schema for owner-scoped `ScriptedPlayerLandmarks`

Tests:

1. component metadata tests
2. persistence/hydration roundtrip tests

### Phase 2: Discovery recording

Server runtime work:

1. evaluate static landmark discovery against current legitimate live visibility sources
2. write discovered landmark GUIDs onto the player entity component
3. avoid duplicate writes/churn for already-discovered landmarks

Operational rule:

1. discovery writes should be idempotent and bounded

Tests:

1. discover-once behavior
2. no discovery from detached free camera alone
3. extent-aware discovery for large bodies

### Phase 3: Visibility authorization lane

Replication visibility work:

1. add discovered-landmark authorization check
2. ensure it runs before delivery narrowing
3. preserve payload redaction constraints

Tests:

1. discovered landmark remains authorized outside scanner range
2. undiscovered landmark remains hidden outside live visibility
3. dynamic entities do not inherit landmark persistence behavior
4. scripted landmark replication does not widen hidden-entity payload disclosure

### Phase 3A: Lua landmark flow

Scripting/runtime work:

1. add validated script intents such as `upsert_player_landmark` and `remove_player_landmark`
2. store landmark entries on the player entity or session-scoped player runtime state according to durability requirements
3. replicate owner-only landmark marker products to the authenticated client
4. enforce policy checks for entity-backed markers targeting undiscovered/private content

Tests:

1. Lua can add, update, and remove a quest landmark for the owning player
2. reconnect preserves durable quest landmarks when configured as persisted state
3. scripted landmark for an undiscovered static landmark does not leak full hidden entity payload
4. one player's quest landmarks never replicate to another player

### Phase 4: Client/runtime consumption

Client behavior:

1. continue rendering delivered landmarks through existing world visual path
2. preserve parallax/projection behavior for planets and similar bodies
3. keep discovery authorization independent from projected render position

Tests:

1. landmark does not disappear when scanner range is lost but delivery/local view still includes it
2. detached camera cannot reveal undiscovered landmarks

### Phase 5: Optimization and product split follow-up

If needed later:

1. split static landmark delivery into a reduced replication product
2. add projected visual bounds for client frustum culling
3. add system-scale landmark preselection acceleration

This phase is optional and should come after correctness.

## 11. Concrete Code Areas Likely Touched

Server / shared:

1. `crates/sidereal-game/src/components/`
2. `crates/sidereal-game/src/components/mod.rs`
3. `crates/sidereal-game/src/generated/` regeneration path as needed
4. `bins/sidereal-replication/src/replication/visibility.rs`
5. player persistence/hydration mapping paths

Content authoring:

1. `data/scripts/bundles/starter/planet_body.lua`
2. future star/black-hole/nebula bundle authoring
3. bundle allowlists / registry
4. quest/mission Lua handlers that emit landmark intents

Docs:

1. `docs/features/visibility_replication_contract.md`
2. `docs/features/galaxy_world_structure.md`
3. new DR under `docs/decisions/`
4. `docs/decision_register.md`

Client:

1. likely minimal for first pass unless a reduced landmark product lane is introduced
2. projected planet render-bounds follow-up likely lives in `bins/sidereal-client/src/runtime/visuals.rs`

## 12. Testing Plan

### Unit tests

1. landmark classification validation
2. discovery state serialization and hydration
3. discovered-landmark authorization helper behavior
4. extent-aware overlap checks

### Integration tests

1. undiscovered planet outside live visibility is not delivered
2. entering legitimate discovery range records the landmark on the player entity
3. discovered planet remains delivered after scanner range is lost
4. free-roam camera alone does not discover hidden landmark
5. reconnect/hydration preserves discovered landmarks
6. multiple player entities on one account keep separate discovery state unless explicitly shared
7. Lua-authored owner landmark appears only for the targeted player client
8. Lua-authored landmark update/remove roundtrips through owner-only replication

### Regression checks

1. no payload widening for dynamic entities
2. owner/public/faction rules remain unchanged
3. tactical/fog unexplored semantics remain fail-closed

## 13. Risks and Mitigations

### Risk: discovery state grows unbounded

Mitigation:

1. first-pass scope only static landmark classes
2. store only stable UUIDs plus minimal metadata
3. consider chunking/compression only if counts become large

### Risk: free-roam camera leaks unexplored landmarks

Mitigation:

1. discovery must use legitimate observer/scanner sources, not detached camera alone
2. keep camera delivery as narrowing only after authorization

### Risk: quest landmark flow leaks hidden targets

Mitigation:

1. keep scripted landmarks as a reduced owner-only marker product
2. require validation when a marker references an undiscovered or private entity
3. allow coarse/fuzzy markers or logical labels where exact hidden positions are not authorized

### Risk: overloading `PublicVisibility`

Mitigation:

1. discovered landmark visibility must be a distinct policy path
2. do not reuse `PublicVisibility` as shorthand for "permanently known after discovery"

### Risk: static config entities get lumped in with landmarks

Mitigation:

1. keep environment lighting and similar system config on a separate always-authorized/static-config rule

### Risk: client render culling still looks wrong

Mitigation:

1. treat projected render-bounds as a separate client correctness task
2. do not distort server authorization with projected visual placement

## 14. Open Questions

These should be resolved in the DR or implementation kickoff:

1. Which entities qualify for first-pass landmark discovery:
   1. only planets/stars/black holes
   2. or also nebulae/anomalies/stations
2. Is discovery per player entity, per account, or optionally faction-shared
   1. recommended first pass: per player entity only
3. What exact radius/discovery criterion should be used:
   1. current live visibility overlap
   2. explicit landmark discovery radius
4. Should discovered landmarks remain visible globally once known, or only within a coarse local/system view scope
   1. recommended first pass: local/system view scope only
5. Should some landmarks be authored `always_known`
   1. tutorial homeworld
   2. system primary star
6. Do we want a reduced static-landmark replication product in phase 1, or only after correctness lands
   1. recommended first pass: reuse ordinary world replication where possible
7. Should scripted quest landmarks always persist, or can some remain session-only
   1. recommended first pass: support both, defaulting quests that survive reconnect to persisted player-entity state

## 15. Recommended First-Pass Decisions

To keep implementation tractable:

1. Introduce a generic `StaticLandmark` classification lane.
2. Persist discovered landmark GUIDs on the player entity.
3. Start with:
   1. planets
   2. stars
   3. black holes
4. Discovery occurs from legitimate live visibility overlap using extent-aware checks.
5. Detached free-roam camera does not grant discovery.
6. Discovered landmarks remain deliverable independent of current scanner range, but only within local/system delivery scope.
7. Environment-lighting config is explicitly excluded from the landmark-discovery lane.
8. Lua-authored quest/guidance landmarks use a separate owner-only player landmark lane driven by validated script intents, not direct client injection.

## 16. Success Criteria

This plan is complete when:

1. static landmark discovery is server-authoritative and persisted on the player entity
2. discovered planets/stars/black holes remain visible outside scanner range without leaking undiscovered landmarks
3. free-roam camera cannot reveal new landmarks by panning into unexplored space
4. visibility contract docs and DRs clearly distinguish:
   1. live scanner visibility
   2. explored-memory
   3. discovered static landmark visibility
5. client world rendering continues to work with authoritative world positions plus client-only projected visual offsets
6. Lua can add/remove owner-scoped quest landmarks without bypassing visibility/redaction rules
