# DR-0032: Discovered Static Landmark Visibility

Status: Accepted  
Date: 2026-03-09  
Owners: gameplay / replication / world-authoring

Update notes:
- 2026-03-10: Clarified that Lua-authored quest/guidance landmarks are allowed through a separate owner-only player landmark flow. They do not directly inject world-entity visibility to clients and must follow the scripting authority contract in `docs/features/scripting_support.md`.

## Context

Planets, stars, black holes, nebulae, and similar static celestial entities are durable world landmarks, but the runtime visibility path treated them too much like ordinary scanner-range contacts.

That created two problems:

1. A player could lose in-world presentation of already-known landmarks just by leaving active scanner range.
2. Free-roam camera and delivery range risked becoming an accidental proxy for discovery/knowledge state.

We need a distinct visibility rule for static landmarks that preserves server authority and player-scoped persistent knowledge.

## Decision

1. Introduce an explicit `StaticLandmark` gameplay component for qualifying static world entities.
2. Persist player-scoped discovery state on the player ECS entity using `DiscoveredStaticLandmarks`.
3. Landmark discovery is server-authored from legitimate live visibility/discovery overlap, not from detached camera position alone.
4. After discovery, qualifying static landmarks remain authorization-eligible independent of current scanner range.
5. Delivery narrowing still applies after authorization using the current local observer/view scope.
6. Discovery-based authorization applies only to static landmark presence. It does not make landmarks generic `PublicVisibility` entities and does not widen payload disclosure for unrelated entities.
7. Static world configuration entities such as `EnvironmentLighting` are not part of the discovered-landmark lane and must use their own replication policy.
8. Lua may add player-scoped quest/guidance landmarks only through validated server intents that write an owner-only landmark product on the player/session state.
9. Lua-authored landmarks are annotations for client HUD/map use; they do not directly grant world-entity visibility or bypass replication redaction.

## Rationale

This keeps three concepts separate:

1. knowledge state: whether the player has discovered the landmark
2. authorization: whether the player is allowed to know about it now
3. delivery: whether it should be sent/rendered in the current local view

That separation matches the existing visibility contract and avoids tying durable landmark knowledge to scanner range or camera exploits.

It also preserves the scripting contract: Lua can request player landmark state through validated intents, but cannot directly mutate client visibility or inject hidden ECS payloads.

## Consequences

Positive:

1. Static landmarks can remain visible after discovery without abusing `PublicVisibility`.
2. Discovery becomes explicit, debuggable, and player-scoped in persisted ECS data.
3. The rule generalizes beyond planets to other immobile celestial/anomaly content.
4. Quest and mission scripts can guide players with server-authored owner-only landmarks without opening a client-authority backdoor.

Negative:

1. Landmark discovery state must be authored, persisted, and tested as a separate data path.
2. Static landmarks and static world-config entities now need different replication treatment.
3. We now need a second reduced replication product for scripted player landmarks, plus validation for hidden-target cases.

## Implementation Notes

2026-03-09 initial implementation:

1. Added `StaticLandmark` and `DiscoveredStaticLandmarks` components in `crates/sidereal-game`.
2. Planet bundle authoring now marks planets as `StaticLandmark` instead of relying on `PublicVisibility`.
3. Replication visibility records landmark discovery on the player entity and authorizes discovered landmarks through a dedicated authorization branch before delivery narrowing.

Native/WASM impact:

1. No protocol shape change for the browser transport boundary in this first pass.
2. The behavior change is server-authoritative and transport-agnostic.
3. If scripted player landmarks are added, both native and WASM clients should consume the same replicated owner-only landmark product; no platform-specific landmark authority path is allowed.

## References

1. `docs/features/visibility_replication_contract.md`
2. `docs/plans/discovered_static_landmark_visibility_plan_2026-03-09.md`
3. `docs/decisions/dr-0030_non_physics_world_spatial_components.md`
4. `docs/features/scripting_support.md`
