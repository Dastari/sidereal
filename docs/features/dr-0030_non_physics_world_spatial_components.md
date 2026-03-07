# DR-0030: Non-Physics World Spatial Components

Status: Accepted  
Date: 2026-03-07  
Owners: client runtime + replication + persistence + scripting

Primary references:
- `docs/sidereal_design_document.md`
- `docs/features/procedural_planets.md`
- `AGENTS.md`

## 1. Context

Static celestial and decorative world entities were being authored with Avian motion components
only to participate in world-space placement and rendering.

That created the wrong coupling:

1. non-physics entities flowed through the physics spatial lane,
2. static bodies could silently fall back to origin when Avian-only consumers missed them,
3. planets and stars were treated as if they were physics entities even though they should only be
   persistent, spatially placed world renderables.

## 2. Decision

Adopt a dedicated non-physics spatial lane:

1. `WorldPosition`
   - persistent replicated world-space position for non-physics entities
2. `WorldRotation`
   - persistent replicated planar heading/radians for non-physics entities

Normative rules:

1. Static celestial/decorative world entities must use `WorldPosition` / `WorldRotation`, not
   `avian_position` / `avian_rotation`, unless they are genuinely simulated by Avian.
2. Avian transform components remain the authoritative lane for physics entities only.
3. Client and replication spatial consumers must resolve world-space from:
   1. Avian `Position` / `Rotation` when present
   2. otherwise `WorldPosition` / `WorldRotation`
4. Render culling and visibility may still treat these entities as ordinary world-space objects.
   The decision is specifically about avoiding unnecessary participation in the physics loop, not
   about making them fullscreen overlays.

## 3. Rationale

This preserves the correct architecture:

1. static world entities remain persistent world objects,
2. they still participate in world-space rendering, tactical display, and visibility where
   applicable,
3. they do not pay the conceptual/runtime cost of being treated as physics bodies.

## 4. Immediate Scope

Current migration scope:

1. `planet.body` Lua bundle now emits `world_position` / `world_rotation`,
2. replication syncs non-physics transforms from `WorldPosition` / `WorldRotation`,
3. client adoption/transform/lighting/planet visual paths resolve either Avian or world-space
   components.

## 5. Explicitly Rejected

1. Keeping planets/stars on Avian transform components permanently
   - rejected because they are not physics entities
2. Making celestial bodies fullscreen/background-only composition entities
   - rejected because they are still spatial world entities that should be render-culled normally
3. Removing Avian components from static bodies without adding a generic world-space lane
   - rejected because it would break hydration/placement/visibility/rendering consumers
