# DR-0037: Visibility Signal Detection and Stable Unknown Contacts

Status: Proposed  
Date: 2026-04-27  
Owners: replication + gameplay visibility + tactical UI + planet/content authoring

## Context

Visibility System V1 correctly separates server authorization, delivery narrowing, payload redaction, tactical contacts, and static landmark discovery. It still has two gaps:

1. Large/high-signal bodies such as planets can become discoverable too late, then visually snap in as the local bubble catches up.
2. The tactical lane only emits contacts for entities that are already fully visible through replication visibility, so there is no redacted "something is out there" contact for high-signal unknown targets.

Rapid zoom-out also exposes a related culling issue: objects around the newly visible viewport edge can pop in because server delivery and client local culling were sized too tightly around the previous view.

## Decision

Adopt Visibility System V2 as the next implementation direction:

1. Add generic target-side `SignalSignature` for objects that are detectable at longer range.
2. Add observer-side `ContactResolutionM` so scanner quality can control approximate unknown-contact accuracy over time.
3. Emit signal-only unknown tactical contacts with relative signal strength and stable approximate position.
4. Preserve full world replication authorization: signal-only detection does not grant ordinary entity `ReplicationState` visibility.
5. Let signal detection trigger `StaticLandmark` discovery when the target is a discoverable static landmark.
6. Use buffered and hysteretic server delivery plus client projected-bounds culling to prevent rapid-zoom edge snap-in.

## Rationale

This keeps three concerns separate:

1. detection: the player can perceive that something exists,
2. discovery/knowledge: the player may learn durable landmark identity,
3. full visibility: the player receives full authorized world replication payload.

Planets and stars are naturally high-signal objects, but the runtime should not special-case them as "always visible planets." A generic signal component gives the same mechanism to future beacons, anomalies, emissions, large stations, storms, distress calls, or other content.

Stable approximate contacts preserve player trust. A signal contact that jumps every tactical tick feels like noise or a bug; one fixed approximation that improves with scanner quality feels like incomplete information.

## Alternatives Considered

1. Increase every planet `StaticLandmark.discovery_radius_m`.
   - Rejected as the only solution because it solves planets but does not generalize to other high-signal objects or unknown contacts.
2. Make planets `PublicVisibility`.
   - Rejected because it bypasses player-scoped discovery and weakens existing visibility/redaction semantics.
3. Let high signal grant full entity replication.
   - Rejected because unknown high-signal contacts should not reveal identity, faction, velocity, or component payloads before normal visibility/intel policy allows it.
4. Fix zoom snap only by increasing one fixed delivery radius.
   - Rejected because rapid zoom requires viewport-predictive and hysteretic behavior; one large constant either still fails at edge cases or over-delivers too much data.

## Consequences

Positive:

1. Planets and stars can be discovered before they visually enter the local bubble.
2. Unknown high-signal contacts become a useful tactical affordance without leaking full data.
3. Scanner quality can later improve contact accuracy without changing protocol concepts.
4. Rapid zoom-out can be made visually stable through delivery and local-culling buffers.

Negative:

1. Tactical contact schema must evolve.
2. Replication/tactical runtime needs per-player signal-contact memory.
3. Content authoring needs signal defaults for planets/stars and optional tuning fields.
4. More tests are required to prove redaction, stability, and no full-visibility bypass.

## Implementation Notes

The source-of-truth contract is `docs/features/visibility_system_v2_signal_detection_contract.md`.

Implementation should proceed in this order:

1. Add component schemas and registry tests.
2. Add planet/star Lua authoring defaults.
3. Extend tactical protocol/contact schema.
4. Implement signal-only tactical contact generation with stable approximate memory.
5. Integrate signal-triggered static landmark discovery.
6. Add client unknown-contact icon support.
7. Add server delivery and client projected-culling hysteresis for rapid zoom-out.

Native/WASM impact:

1. Native must validate rapid zoom in/out around parallaxed planets and signal contacts.
2. WASM must share protocol and runtime semantics; no platform-specific signal or discovery authority path is allowed.

## References

1. `docs/features/visibility_system_v2_signal_detection_contract.md`
2. `docs/features/visibility_replication_contract.md`
3. `docs/features/tactical_and_owner_lane_protocol_contract.md`
4. `docs/decisions/dr-0032_discovered_static_landmark_visibility.md`
5. `docs/core_systems_catalog_v1.md`
