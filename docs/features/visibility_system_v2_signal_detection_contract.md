# Visibility System V2: Signal Detection, Unknown Contacts, and Zoom-Safe Culling

Status: Active partial implementation spec
Last updated: 2026-04-27
Owners: replication + gameplay visibility + tactical UI + planet/content authoring
Scope: the V2 visibility direction for signal-based detection, redacted unknown tactical contacts, static landmark discovery, and local/server culling behavior for rapidly changing view scale

Primary references:
- `docs/core_systems_catalog_v1.md`
- `docs/features/visibility_replication_contract.md`
- `docs/features/tactical_and_owner_lane_protocol_contract.md`
- `docs/features/procedural_planets.md`
- `docs/decisions/dr-0032_discovered_static_landmark_visibility.md`
- `docs/decisions/dr-0037_visibility_signal_detection_and_stable_unknown_contacts.md`

## 0. Implementation Status

2026-04-27 status note:

1. Implemented partial V2 baseline: `SignalSignature` and `ContactResolutionM` are registered gameplay components; planet/star bundles emit `signal_signature`; starter players receive `contact_resolution_m`.
2. Implemented partial V2 baseline: signal-only targets can produce redacted unknown tactical contacts with stable per-player contact IDs, `map_icon_unknown_contact_svg`, `signal_strength`, `contact_quality`, and `position_accuracy_m`.
3. Implemented partial V2 baseline: signal-bearing static landmarks can be discovered before full local-bubble visibility, and planet/star/black-hole signal contacts enqueue a server-authored `long_range_gravity_well_detected` notification once per target per player stream.
4. Implemented partial V2 baseline: signal-only discovery updates durable `DiscoveredStaticLandmarks` but does not emit the identity-bearing `LandmarkDiscovery` notification payload; player-facing signal messaging stays redacted until normal visibility/delivery discloses the body.
5. Implemented partial V2 baseline: native client local-view delivery requests use projected viewport overscan, and planet visual children use buffered projected-bounds culling with rapid zoom-out expansion plus short retention hysteresis.
6. Open: richer unknown-contact UI presentation and broader runtime tuning still need follow-up.
7. Native impact: V2 changes server tactical/contact generation, server delivery-range requests, and client tactical/viewport culling. Native validation must include rapid zoom in/out around parallaxed planets and high-signal non-landmarks.
8. WASM impact: no platform-specific visibility model is allowed. Browser clients must consume the same tactical/contact protocol and use equivalent projected-culling rules once live WASM parity resumes.

## 1. System Label

Core catalog label:

- Human title: `Visibility System V2`
- Stable label: `system.visibility.v2`
- Short slug: `visibility_v2`

V1 remains the current implementation label for the existing server-owned authorization, delivery, and redaction pipeline. V2 is now partially implemented as a signal/intel layer over that baseline.

## 2. Goals

Visibility System V2 must solve four player-visible problems:

1. Large/high-signal bodies such as planets and stars should become discoverable before they visually enter the tight local-bubble delivery edge.
2. High-signal but unidentified targets should produce an unknown tactical contact with relative strength, not full entity disclosure.
3. Unknown contacts should not jitter every tactical update.
4. Rapid zoom-out should not reveal empty edges that fill in a moment later because server delivery or client culling was sized only to the previous viewport.

## 3. Non-Goals

V2 must not:

1. make planets globally public by default,
2. let signal detection grant full `ReplicationState` visibility for ordinary entities,
3. leak raw Bevy `Entity` IDs or unauthorized component payloads,
4. let the client author discovery state,
5. replace `DiscoveredStaticLandmarks` as the durable player-scoped static landmark memory,
6. introduce native-only architecture that makes later WASM parity harder.

## 4. New Gameplay Components

### 4.1 `SignalSignature`

Persistable/replicated gameplay component in `crates/sidereal-game/src/components/signal_signature.rs`:

```rust
#[sidereal_component(kind = "signal_signature", persist = true, replicate = true, visibility = [Public])]
pub struct SignalSignature {
    pub strength: f32,
    pub detection_radius_m: f32,
    pub use_extent_for_detection: bool,
}
```

Semantics:

1. `strength` is relative emitted signal strength for tactical/intel presentation.
2. `detection_radius_m` is target-side bonus range added to observer visibility range for signal-only detection.
3. `use_extent_for_detection=true` includes target extent so large bodies can be detected from their visible edge.
4. Absence means no signal-only detection outside normal visibility.
5. This component is generic. Content may call a thing a scanner, beacon, planet, star, anomaly, heat source, or distress signal, but the engine/runtime reads target signal terms generically.

Default content authoring:

1. planets: `strength = 1.0`, `detection_radius_m = 4_000.0`, `use_extent_for_detection = true`,
2. stars: `strength = 2.0`, `detection_radius_m = 12_000.0`, `use_extent_for_detection = true`,
3. black holes/anomalies: authored explicitly, suggested first default `strength = 1.5`, `detection_radius_m = 6_000.0`, `use_extent_for_detection = true`,
4. ships/asteroids: no default signal signature unless content explicitly authors one.

### 4.2 `ContactResolutionM`

Add an effective observer-side component in `crates/sidereal-game/src/components/contact_resolution_m.rs`:

```rust
#[sidereal_component(kind = "contact_resolution_m", persist = true, replicate = true, visibility = [OwnerOnly])]
pub struct ContactResolutionM(pub f32);
```

Semantics:

1. Lower values mean better approximate-position accuracy.
2. First default is `100.0m`, matching tactical fog cell size.
3. The tactical signal contact path reads only effective root `ContactResolutionM`.
4. Future scanner modules may contribute to this value through a `ContactResolutionBuffM` aggregation path, but V2 should not require child/module scans in the tactical hot path.

## 5. Signal Detection Semantics

For each player visibility source and target carrying `SignalSignature`, compute:

```text
effective_detection_range =
    visibility_source.range_m
  + signal_signature.detection_radius_m
  + optional_target_extent_m
```

The target is signal-detected when:

```text
distance(observer_source, target_position) <= effective_detection_range
```

Relative strength:

```text
relative_strength =
    signal_signature.strength
  * clamp01(1.0 - distance / effective_detection_range)
```

Initial quality bands:

| Strength | Contact quality |
| --- | --- |
| `< 0.15` | ignored |
| `0.15..0.35` | `weak` |
| `0.35..0.65` | `moderate` |
| `0.65..1.0` | `strong` |
| `>= 1.0` | `overwhelming` |

These values are tunable constants, but the thresholds must stay server-owned and shared by all clients through payload output, not client-side interpretation of hidden world state.

## 6. Unknown Tactical Contacts

Signal-only contacts are tactical-lane products, not full local-bubble replication.

When a signal target is not otherwise visible to the client, the server may emit a redacted `TacticalContact`:

```rust
pub struct TacticalContact {
    pub entity_id: String,
    pub kind: String,
    pub map_icon_asset_id: Option<String>,
    pub faction_id: Option<String>,
    pub position_xy: [f64; 2],
    pub heading_rad: f64,
    pub velocity_xy: Option<[f64; 2]>,
    pub is_live_now: bool,
    pub last_seen_tick: u64,
    pub classification: Option<String>,
    pub contact_quality: Option<String>,
    pub signal_strength: Option<f32>,
    pub position_accuracy_m: Option<f32>,
}
```

Signal-only values:

1. `entity_id`: server-stable contact ID. Prefer an opaque per-player contact ID if raw target GUID disclosure is considered identity leakage.
2. `kind`: `"unknown"`.
3. `map_icon_asset_id`: `"map_icon_unknown_contact_svg"`.
4. `faction_id`: `None`.
5. `velocity_xy`: `None`.
6. `classification`: `Some("unknown")`.
7. `contact_quality`: one of the quality bands above.
8. `signal_strength`: normalized relative strength.
9. `position_accuracy_m`: effective resolution used to generate the approximate point.

Full visible contact wins over signal-only contact when both are available in the same tactical update.

## 7. Stable Approximate Position

Unknown signal contact positions must be approximate and stable.

Server per-player tactical stream state must retain signal-contact memory:

```rust
struct SignalContactMemory {
    contact_id: String,
    approximate_position_xy: [f64; 2],
    position_accuracy_m: f32,
    strongest_signal_strength: f32,
    last_detected_tick: u64,
}
```

Rules:

1. First detection fixes the approximate position.
2. The approximate position does not change every update.
3. The position may improve only when the observer has a meaningfully better `ContactResolutionM`.
4. When the target becomes fully visible, replace the unknown contact with the normal full tactical contact.
5. When the target drops from full visibility back to signal-only, reuse the best known approximate contact memory unless explicit stale-intel policy says otherwise.

Initial approximation:

```text
cell_size = max(contact_resolution_m, 1.0)
approx_x = floor(target_x / cell_size) * cell_size + cell_size / 2
approx_y = floor(target_y / cell_size) * cell_size + cell_size / 2
```

If grid-centered approximation feels too artificial, V2 may add deterministic per-player/target offset. It must not add frame-varying randomness.

## 8. Static Landmark Integration

Static landmarks remain durable player knowledge via `DiscoveredStaticLandmarks`.

For entities carrying both `StaticLandmark` and `SignalSignature`:

1. Signal detection may trigger server-authored landmark discovery.
2. Discovery inserts the landmark GUID into the player entity's `DiscoveredStaticLandmarks`.
3. Direct normal discovery may emit the existing identity-bearing landmark notification.
4. Signal-only discovery must not emit identity-bearing landmark notification payloads. The player-facing signal notification remains the redacted `long_range_gravity_well_detected` generic event.
5. After discovery, existing discovered-landmark authorization applies.
6. Full world replication still requires delivery narrowing.

`StaticLandmark.discovery_radius_m` remains supported for direct content tuning. New planet/star authoring should prefer `SignalSignature` for "this object is easy to notice from far away" behavior.

## 9. Server Delivery and Zoom-Safe Prefetch

The current snap-in issue during rapid zoom-out is a delivery/culling hysteresis problem, not only a discovery problem.

V2 server delivery must size local-bubble delivery from a buffered projected viewport, not only the exact current screen edge.

Required behavior:

1. Client view messages should send a delivery range based on the larger of:
   - current viewport world radius,
   - target/anticipated viewport world radius during zoom animation,
   - minimum tactical/local delivery floor.
2. Add an outer prefetch margin so edge objects are already delivered before they enter the visible frame.
3. Add hysteresis so objects are not dropped immediately when they leave the exact visible bounds.
4. For parallaxed/layered landmarks, delivery bounds must account for:
   - render-layer parallax factor,
   - `RuntimeWorldVisualStack` max scale multiplier,
   - screen-scale factor,
   - entity extent.

Recommended initial formula:

```text
requested_delivery_range_m =
    max(current_view_radius_m, predicted_zoom_out_view_radius_m)
  + max(300.0, current_view_radius_m * 0.35)
```

For parallaxed landmarks, use the existing discovered-landmark delivery scale concept:

```text
projected_delivery_range_m =
    requested_delivery_range_m / layer_parallax_factor
```

The exact constants should be tuned in playtests, but the architecture must be viewport-predictive and hysteretic rather than a single tight current-FOV radius.

## 10. Client Local Culling Contract

Client render culling must also use buffered projected bounds.

For planets and other large/parallaxed world visuals:

1. Project visual bounds using authoritative world position plus layer parallax.
2. Cull only when the projected visual bounds are outside an expanded viewport.
3. Use a larger expansion while zoom velocity is outward.
4. Retain recently visible large/parallaxed visuals for a short grace period to avoid one-frame disappear/reappear churn.
5. The current native planet-body visual path implements this at the generated planet visual child level, not through Bevy frustum culling, because the planet shaders are already spawned with `NoFrustumCulling`.

Recommended initial values:

1. static viewport margin: `25%` of the larger viewport dimension,
2. rapid zoom-out margin: `50%` of the larger viewport dimension,
3. retention grace: `0.35s`,
4. minimum large-body margin: `max(projected_radius_px, 96px)`.

This does not mean rendering the whole world. It means local culling uses a stable outer shell so rapid zoom changes cannot outrun delivery/render preparation.

## 11. Tactical Map Presentation

Unknown contacts must render as a question-mark marker.

Required asset:

1. Register `map_icon_unknown_contact_svg`.
2. Source path should be `data/icons/unknown_contact.svg`.
3. The client tactical map should prefer `contact.map_icon_asset_id` and fall back to `TacticalPresentationDefaults` only if the contact does not specify one.

Unknown contacts should show relative strength through existing or future UI affordances:

1. marker color/intensity,
2. subtle pulse rate,
3. tooltip or selection readout text such as `Weak signal`, `Strong signal`.

The first implementation only needs the icon and strength-bearing payload; richer UI can follow.

## 12. Tests and Acceptance Criteria

Minimum tests:

1. `SignalSignature` and `ContactResolutionM` register in component metadata/registry tests.
2. `planet.body` emits `signal_signature` for planets and stars.
3. A high-signal planet can be detected before normal visibility range would authorize full local-bubble replication.
4. Signal-only contacts are redacted as `kind = "unknown"`.
5. Signal-only contacts use `map_icon_unknown_contact_svg`.
6. Signal-only contacts include `signal_strength` and `position_accuracy_m`.
7. Signal-only contacts do not include faction, velocity, exact identity metadata, or unauthorized classification.
8. Approximate signal contact position remains stable across repeated tactical updates.
9. Better `ContactResolutionM` can improve approximate position without frame-to-frame jitter.
10. Full visible tactical contact replaces signal-only contact once normal visibility applies.
11. Signal detection of a `StaticLandmark` writes `DiscoveredStaticLandmarks`.
12. Signal detection does not grant full `ReplicationState` visibility for ordinary non-landmark entities.
13. Rapid zoom-out around a parallaxed discovered planet does not produce edge snap-in in native client validation.

Required validation for implementation work:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

Run targeted tests for touched crates, especially `sidereal-game`, `sidereal-scripting`, `sidereal-replication`, and `sidereal-client` tactical/visibility tests.
