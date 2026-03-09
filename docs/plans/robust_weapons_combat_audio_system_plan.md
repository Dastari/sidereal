# Robust Weapons, Combat Effects, and Audio System Plan

Status: Proposed implementation plan  
Date: 2026-03-03  
Owners: gameplay runtime + replication + client audio/visuals

Primary architecture references:
- `docs/sidereal_design_document.md`
- `docs/features/visibility_replication_contract.md`
- `docs/decisions/dr-0013_action_acceptor_control_routing.md`
- `docs/component_authoring_guide.md`

## 1. Goal

Build a scalable, server-authoritative combat pipeline that supports:

1. Many weapons per ship (for example 10+ hardpoints with independent fire rates).
2. Multiple weapon families (ballistic, beam/laser, missiles, point-defense/turrets).
3. Visibility/fog-aware network delivery (no unconditional broadcast).
4. Decoupled client VFX/SFX playback (tracers, shield impacts, explosions, weapon audio).
5. Extensible audio categories/mixing (`effects`, `music`, `dialog`, etc.).

This plan replaces ad-hoc tracer/event coupling with a generic combat event model.

## 2. Non-Goals (Initial Iteration)

1. Full lag-compensated rewind hit validation.
2. Final VFX polish and audio mastering.
3. Complete shield gameplay model (only impact/effect signaling baseline).

## 3. Core Architecture

Separate concerns into four layers:

1. Authoritative simulation (server, fixed tick):
- Validate fire intent, cooldown/ammo/energy constraints, mount state.
- Resolve hit logic (ray/shape/projectile) and damage/shield interactions.

2. Combat event generation (server, fixed tick):
- Emit canonical events from authoritative outcomes (`WeaponFired`, `Impact`, `BeamState`, `MissileSpawned`, `MissileDestroyed`).

3. Visibility-aware routing (server, fixed tick):
- Route each event to eligible recipients using existing visibility contract and delivery narrowing.
- Keep policy generic and reusable; weapon systems must not hardcode visibility checks.

4. Client presentation (render/audio frame loops):
- Consume routed events and render local effects.
- No client-authoritative gameplay writes.

## 4. Authoritative Weapon Model

## 4.1 Component/Data model (proposed)

Add or evolve persistable/replicated gameplay components under `crates/sidereal-game/src/components/`:

1. `WeaponTag`
2. `WeaponMountProfile`
- `mount_type` (`Fixed`, `Turret`)
- traverse limits, slew rate, stabilization flags

3. `WeaponFireProfile`
- `weapon_family` (`Ballistic`, `Beam`, `Missile`, `PD`, ...)
- rate/cooldown params
- ammo/energy requirements

4. `WeaponBallisticProfile` (ballistics only)
- muzzle velocity, spread, range, damage profile

5. `WeaponBeamProfile` (beam only)
- warmup, sustain cost, max duration, contact logic

6. `WeaponMissileProfile` (missiles only)
- spawn entity template, launch impulse, guidance type

7. `WeaponRuntimeState`
- cooldown timers, spin state, heat, trigger-latched state

8. `ShieldProfile` and `ShieldRuntimeState` (for impact routing/effects)

Use `#[sidereal_component(...)]` for persistable/replicated gameplay schema.

## 4.2 Action model

Keep intent actions generic:

1. `FirePrimary` / `FireSecondary` / future `FireGroup(n)` and `CeaseFire`.
2. Server maps intent to mounted weapons/groups on controlled entity.
3. No direct transform/velocity mutation from client intent.

## 5. Combat Event Contract

Create a shared network event/message set in `sidereal-net`:

1. `ServerWeaponFiredMessage`
- shooter entity id
- weapon entity id / hardpoint id
- origin/direction or origin/velocity
- presentation hints (`weapon_family`, tracer style id)

2. `ServerImpactMessage`
- attacker id, target id
- impact position/normal
- `hit_layer` (`Shield`, `Hull`, `Armor`, `Environment`)
- damage type/magnitude bands

3. `ServerBeamStateMessage`
- `BeamStart` / `BeamUpdate` / `BeamStop`

4. `ServerMissileLifecycleMessage`
- spawn/retarget/destroy milestones

These are presentation events only; gameplay truth remains replicated components and authoritative state.

## 6. Visibility-Aware Event Routing

Integrate event routing with `visibility_replication_contract.md` stages:

1. Authorization scope:
- owner/public/faction/scanner policy decides whether event is eligible.

2. Delivery scope:
- per-client distance/camera narrowing may reduce delivery set.

3. Payload scope:
- redact fields when required (for example hidden attacker metadata).

Routing requirements:

1. No `NetworkTarget::All` for combat effects in final model.
2. Routing logic must be reusable service-level infra, not weapon-family specific.
3. Event recipients should be computed from visibility maps/caches produced in the same fixed tick.

## 7. Weapon Family Presentation Strategy

Use event-to-effect mapping on client:

1. Ballistic:
- pooled tracer entities/instances (shader/sprite/mesh based)
- optional impact sparks/decal events

2. Beam/Laser:
- persistent beam render state keyed by `(shooter, weapon)` with start/stop/update events

3. Missile:
- authoritative missile entity replicated as world entity (not just effect)
- additional local trail/exhaust VFX

4. Shield impacts:
- impact event drives shield surface flash/ripple on target ship
- should be visible to all recipients of the impact event

## 8. Audio System Plan

## 8.1 Category-based audio mixing

Add client audio buses/categories:

1. `music`
2. `effects`
3. `dialog`
4. `ui`
5. optional `ambient`

Add a persistent config resource:

1. per-category volume scalar
2. mute toggles
3. future ducking rules (for example dialog ducks effects/music)

## 8.2 Weapon audio profile model

Support simple and advanced weapon audio:

1. Simple one-shot profile:
- `fire_clip_asset_id`
- optional pitch/volume variance

2. Segmented loop profile (for gatlings/spool weapons):
- one clip (or equivalent source) with timeline markers:
  - `spin_up_start_s`
  - `loop_start_s`
  - `loop_end_s`
  - `spin_down_start_s`
  - `clip_end_s`
- runtime controls:
  - `ramp_up_rate`
  - `ramp_down_rate`
  - `min_spin_for_loop`

Required behavior:

1. Trigger pressed:
- play from spin-up start.
- transition into `loop_start_s..loop_end_s` region while held.

2. Trigger released:
- jump/play from `spin_down_start_s` and stop at end.

## 8.3 Bevy-compatible implementation approach

Baseline:

1. Use Bevy asset pipeline for source loading (`AudioSource` / logical asset ids).
2. Keep audio playback as client presentation state, driven by authoritative combat events.

Advanced loop-marker control:

1. Bevy default audio path is suitable for standard `ONCE/LOOP`, but loop-region markers and jump-to-offset control require a custom playback controller.
2. Implement a thin `WeaponAudioPlaybackController` backend over the active Bevy audio stack (rodio/symphonia path), exposing:
- play at offset
- loop region
- jump to offset on state transition

Alternative content path:

1. Optionally author separate `spin_up`, `loop`, `spin_down` clips and stitch at runtime when marker-based single-file control is unavailable on target platform.

WASM parity requirement:

1. Any advanced control path must compile/run on both native and `wasm32`.
2. If platform differences exist, keep semantic behavior identical via backend adapters.

## 9. Performance and Scalability

Server:

1. Keep hit simulation query-based for high-RPM weapons (avoid bullet entity flood).
2. Batch weapon processing by controlled root/hardpoint group.
3. Emit compact events; avoid per-frame heavyweight payloads.

Client:

1. Use pooled VFX entities or instancing for tracers/impacts.
2. Reuse beam state objects.
3. Limit max active presentation effects with graceful LOD/degradation.

Network:

1. Introduce dedicated combat-effects channel(s) with QoS by event type:
- unreliable/sequenced for high-frequency fire visuals
- reliable for lifecycle-critical events (`BeamStart/Stop`, missile spawn/despawn)

## 10. Implementation Phases

## Phase A: Event Foundations

1. Add canonical combat event structs/resources in `sidereal-game`.
2. Add corresponding network messages in `sidereal-net`.
3. Add visibility-aware event router in replication service.
4. Keep existing local tracer path as fallback during migration.

## Phase B: Weapon Family Unification

1. Refactor ballistic firing to emit explicit `WeaponFired` + `Impact` events (not ammo-delta inference).
2. Add beam lifecycle events and placeholder beam renderer.
3. Add missile lifecycle event contract and authoritative missile entity baseline.

## Phase C: Audio Architecture

1. Add category mixer resources + settings UI hooks.
2. Implement weapon audio profiles and event-driven playback.
3. Add segmented loop controller (single clip marker model) with fallback strategy.

## Phase D: Shield Impact Effects

1. Add shield impact event surface data.
2. Render shield flash/ripple effect on recipients.
3. Verify fog-aware visibility and no leaked effect metadata.

## 11. Testing Plan

Unit tests (`sidereal-game`):

1. Weapon state transitions and cooldown determinism.
2. Spin/ramp state machine correctness.
3. Event emission for ballistic/beam/missile families.

Integration tests (`sidereal-replication`):

1. Visibility-aware event routing (owner/public/faction/scanner cases).
2. No event delivery when entity is out of visibility scope.
3. Payload redaction behavior where required.

Client tests/checks:

1. Native + WASM compile parity for combat/audio systems.
2. Multi-client session: remote weapon effects visible only when visibility policy allows.
3. Audio segmented loop transitions (press/hold/release) at varying network conditions.

## 12. Open Decisions

1. Message schema versioning strategy for combat effects.
2. Exact audio backend approach for sample-accurate marker loops on WASM.
3. Beam prediction/interpolation policy for very high-latency links.
4. Shield gameplay layering detail (shield-only HP vs layered mitigation stack).
