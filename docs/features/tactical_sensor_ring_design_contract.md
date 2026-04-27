# Tactical Sensor Ring Design Contract

Status: Proposed feature contract
Last updated: 2026-04-27
Owners: client runtime + replication + gameplay
Scope: TAB tactical sensor ring HUD, scanner capability gating, server-authored contact disclosure, and client-side directional presentation

Primary references:

- `docs/features/tactical_and_owner_lane_protocol_contract.md`
- `docs/features/visibility_replication_contract.md`
- `docs/component_authoring_guide.md`
- `docs/ui_design_guide.md`
- `bins/sidereal-client/src/runtime/tactical.rs`
- `bins/sidereal-client/src/runtime/ui.rs`
- `bins/sidereal-replication/src/replication/tactical.rs`
- `crates/sidereal-game/src/components/scanner_component.rs`
- `crates/sidereal-game/src/components/visibility_range_m.rs`
- `crates/sidereal-game/src/components/visibility_range_buff_m.rs`

## 0. Implementation Status

2026-04-27 status note:

1. Implemented first client slice: `ScannerComponent` now carries contact detail tier, density/directional support, and contact cap fields; starter ship bundles author root scanners; the native client has a TAB-toggle tactical sensor ring that draws broken radial ticks, a forward cue, optional density bars, and contact blips from `TacticalContactsCache`.
2. Implemented client availability gating: the ring opens only for a non-player-anchor controlled entity with an active scanner profile and closes when the tactical map is active, control changes, or scanner capability disappears.
3. Open server work: tactical contact disclosure is not yet fully gated/redacted by the controlled entity scanner tier. Existing tactical lane data is consumed as-is by the first client ring slice.
4. Open visual polish: the first slice uses simple `Mesh2d` primitives for ticks and blips; SVG contact icon reuse, scanner pulse effects, and target selection remain future work.
5. Native impact: this feature adds native Bevy HUD presentation and TAB input handling.
6. WASM impact: implementation remains in shared client code. Live browser validation can remain deferred while native stabilization is the priority, but shared client code must compile for WASM.

2026-04-27 visual target note:

1. The desired look follows the attached references: a non-modal combat HUD instrument rather than a full map, with the ship at the center, a broken cyan/blue tick ring, bright forward-sector awareness marks, and colored hostile/unknown blips around the circumference.
2. The ring should feel diegetic and lightweight: no filled panel, no large labels, no full-screen takeover, and no replacement of the existing `M` tactical map.
3. The visual language should stay close to Sidereal's UI guide: restrained cyan/blue instrument markings, red hostile contacts only when server-disclosed, low alpha stale/unknown contacts, and no one-off palette.

## 1. Feature Summary

The tactical sensor ring is a TAB-toggled HUD overlay centered around the currently controlled entity. It gives the pilot fast directional awareness without entering the full tactical map.

The ring shows:

1. nearby disclosed contacts,
2. nearby disclosed landmarks,
3. hostile/friendly/unknown contact relationship when the scanner supports it,
4. density by direction when the scanner supports it,
5. coarse distance and stale/live state,
6. optional directional awareness cues such as bearing ticks or forward-relative sectors.

The ring is presentation only. It does not create gameplay authority, does not widen replication visibility, and does not infer hidden data client-side.

## 2. Non-Negotiable Rules

1. The ring appears only for an active controlled entity that has an effective scanner profile.
2. Free-roam/player-anchor camera mode must not allow the scanner ring.
3. Controlling an entity without sensors must not allow the scanner ring.
4. If the player changes control from a scanner-capable entity to free-roam or a non-scanner entity while the ring is open, the client closes the ring immediately.
5. Scanner availability is not just a client UX check. Server tactical contact disclosure must also be gated by the scanner capability of the currently controlled entity.
6. The client must never derive faction, threat, hidden density, cargo, or detailed classification from raw replicated ECS data unless the server tactical lane disclosed it.
7. Tactical lane behavior must preserve the project visibility order: authorization, then delivery narrowing, then payload redaction.
8. The ring must not replace the existing full tactical map. It is a normal-flight HUD mode.

## 3. User Interaction

### 3.1 Activation

Default behavior:

1. Press `TAB` to toggle the tactical sensor ring.
2. Press `TAB` again to close it.
3. Suppress TAB behavior while the dev console or text-entry UI has focus.
4. If the full tactical map is active, the first implementation should hide or ignore the sensor ring to avoid competing overlays.

Suggested input flow:

```text
TAB pressed
  if dev console open:
    ignore
  else if no active controlled entity:
    keep ring hidden
  else if controlled entity has no effective scanner:
    keep ring hidden
  else if full tactical map is active:
    keep ring hidden
  else:
    toggle ring
```

Optional feedback:

1. A short non-blocking local status message may say "No scanner available" when TAB is pressed without sensors.
2. Do not use a blocking dialog for this. It is not a critical failure.

### 3.2 Control Changes

Whenever `LocalPlayerViewState.controlled_entity_id` changes:

1. Re-resolve the active scanner profile.
2. If no scanner profile exists, set `TacticalSensorRingUiState.enabled = false`.
3. Clear marker smoothing state for contacts tied to the previous control context.
4. Recompute bearings from the new controlled entity orientation.

## 4. Rendering Design

### 4.1 First Implementation: UI Overlay Render Entities

The first implementation should not be a shader.

It should render as Bevy 2D presentation entities on the existing UI overlay path:

1. `UiOverlayCamera`
2. `RenderLayers::layer(UI_OVERLAY_RENDER_LAYER)`
3. `Svg2d` for contact/landmark icons
4. simple `Mesh2d` or sprite-like quads for ring ticks, density sectors, and directional bars

This is "UI" in the sense that it is HUD/presentation, but it should not primarily be Bevy UI nodes. Polar geometry, rotation, icon orientation, and per-contact marker movement are easier and cleaner as screen-space 2D render entities than as layout nodes.

Recommended entity structure:

```text
UiOverlayCamera
└── TacticalSensorRingRoot
    ├── Ring tick segment entities
    ├── Range/density sector entities
    ├── Directional awareness cue entities
    └── Contact/landmark marker entities
```

### 4.2 Why Not a Shader First

A shader is not the right core implementation for the first slice because:

1. Contact markers need per-entity icon selection and tinting.
2. Ring geometry must follow a controlled entity that may not be exactly screen centered.
3. Scanner detail tiers are gameplay disclosure decisions, not shader decisions.
4. Future target selection and hover/pick behavior are easier with ECS marker entities.
5. The existing tactical map already has dynamic marker logic that can be reused or extracted.

### 4.3 Future Shader Option

A shader can be added later for ring polish, not for the core contact logic.

Good shader candidates:

1. subtle scanline/noise over the ring,
2. glow falloff on the circular ring,
3. sweep/pulse effect when the scanner refreshes,
4. dense sector heat haze or bloom-like edge.

If a shader is added later:

1. Author it through the normal asset registry path.
2. Keep source shader and streamed cache shader paths in schema parity.
3. Do not hardcode shader asset IDs in Rust runtime code.
4. Keep marker icons and contact semantics outside the shader.

## 5. Visual Design

### 5.1 Layout

The ring should be centered around the controlled entity's screen position.

Sizing:

1. Radius: `min(viewport_width, viewport_height) * 0.22`.
2. Clamp radius to approximately `140px..260px`.
3. Marker band sits just outside the ring radius.
4. If the controlled entity is near screen edge, clamp the ring center enough to keep most of the ring visible.

Orientation:

1. Ship forward is the top of the ring.
2. Bearings are relative to the controlled entity rotation.
3. This makes the ring a pilot awareness instrument rather than a world-north map.

### 5.2 Ring Anatomy

Recommended first slice:

1. Outer ring: broken tactical tick marks, low alpha. Implemented first as `Mesh2d` rectangles.
2. Cardinal/quarter ticks: slightly stronger. Implemented.
3. Forward cone: subtle brighter cue at the top. Implemented as a short fan of brighter tick bars.
4. Contact band: icons just outside the ring. Implemented first as simple colored blips from tactical contacts.
5. Density band: faint sector arcs/ticks inside or outside the tick ring. Implemented first as radial sector bars from disclosed live contacts when scanner density support is true.
6. Optional stale trail: dimmer marker ghost for stale contacts if already in the tactical cache. Implemented first as lower-alpha stale contact blips.

### 5.3 Contact Icon Rules

Marker placement:

1. Compute vector from controlled entity position to contact position.
2. Convert vector into bearing relative to controlled entity forward.
3. Place marker at `ring_center + unit_bearing * marker_radius`.
4. Use distance bucket to vary icon scale/alpha, not radial placement in the first slice.

Distance buckets:

1. near: full opacity, normal icon size,
2. mid: slightly reduced opacity,
3. far edge of scanner range: small/dim marker,
4. stale: dim marker, no density contribution unless explicitly desired.

Icon orientation:

1. Ship/contact icons may rotate to the contact heading if telemetry is disclosed.
2. If heading is not disclosed, rotate the marker to point inward or keep it upright.
3. Landmark icons should remain upright or use a stable orientation.

### 5.4 Color and Disclosure

Use the existing UI design palette. Do not introduce a new one-off color system.

Suggested roles:

1. self/owned controlled entity: blue-white or existing info blue,
2. unknown/basic contact: grey-blue,
3. friendly: info blue,
4. hostile: error red,
5. neutral: warning yellow or subdued white,
6. landmark: subdued white/yellow depending on kind,
7. stale: same role color with lower alpha.

The client color role must come from server-disclosed tactical fields. The client must not inspect private `FactionId` or other ECS data to decide relationship.

## 6. Scanner Availability and Capability Model

### 6.1 Scanner Installed

A scanner is considered installed when the controlled entity has an effective scanner profile resolved from:

1. `ScannerComponent` on the controlled root entity, or
2. `ScannerComponent` on a mounted child/module whose `MountedOn.parent_entity_id` points at the controlled root.

No scanner profile means:

1. TAB cannot open the ring,
2. client closes any open ring,
3. server sends no scanner-derived contact detail for that controlled entity.

### 6.2 Scanner Component Shape

Recommended target shape:

```rust
#[derive(Debug, Clone, Copy, Reflect, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScannerContactDetailTier {
    Basic,
    Iff,
    Classified,
    Telemetry,
}

#[sidereal_component(kind = "scanner_component", persist = true, replicate = true, visibility = [OwnerOnly])]
#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct ScannerComponent {
    pub base_range_m: f32,
    pub level: u8,
    pub detail_tier: ScannerContactDetailTier,
    pub supports_density: bool,
    pub supports_directional_awareness: bool,
    pub max_contacts: u16,
}
```

Important:

1. Effective visibility range remains the generic `VisibilityRangeM` / `VisibilityRangeBuffM` lane.
2. Scanner authoring should pair `ScannerComponent` with `VisibilityRangeBuffM`.
3. `base_range_m` is scanner metadata and content authoring context; the hot visibility range remains `VisibilityRangeM`.
4. If the schema changes in early development, update all producers/consumers and reset local/dev databases rather than adding compatibility aliases.

### 6.3 Multiple Scanners

If multiple scanner components are installed, resolve one effective scanner profile:

1. highest `detail_tier`,
2. then highest `level`,
3. then highest effective range,
4. then highest `max_contacts`.

Later work can merge capabilities from multiple scanners, but first implementation should choose the best single effective profile for deterministic behavior.

## 7. Contact Disclosure Matrix

Use the existing tactical contact lane as much as possible. The first slice should avoid a new message type unless the latest checkout already introduced one.

Suggested interpretation of existing `TacticalContact` fields:

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
}
```

Disclosure by scanner tier:

| Scanner tier | Contact output                                                                | Ring presentation                       |
| ------------ | ----------------------------------------------------------------------------- | --------------------------------------- |
| No scanner   | no scanner contacts                                                           | ring unavailable                        |
| Basic        | `kind = "contact"`, no faction, no velocity, `contact_quality = "basic"`      | grey unknown blips                      |
| IFF          | relation in `classification = "friendly" / "hostile" / "neutral" / "unknown"` | blue/red/yellow/grey                    |
| Classified   | actual `kind` and `map_icon_asset_id` where policy allows                     | ship/landmark/contact-specific icons    |
| Telemetry    | velocity and heading disclosed                                                | heading-aware icons, better threat cues |

Threat presentation:

1. Hostile classification is enough for a red marker.
2. Telemetry scanners can add stronger threat styling when a hostile is closing or pointing toward the player.
3. The first slice should avoid inventing client-side threat from hidden private data.

## 8. Detection Density

First implementation:

1. Density is computed client-side from already-disclosed tactical contacts.
2. Divide the ring into 24 sectors.
3. Count live contacts per sector.
4. If scanner supports density, draw faint sector arcs/ticks with alpha based on count.
5. Do not count undisclosed contacts.
6. Do not create aggregate hidden-contact leakage.

Future implementation:

1. Add server-authored density sectors if the design wants "something is out there" without exact contact disclosure.
2. Such sectors must be redacted server-side and should be a separate tactical sensor product.
3. Aggregate density must still obey authorization and scanner capability.

## 9. Server Runtime Plan

Primary file likely to update:

1. `bins/sidereal-replication/src/replication/tactical.rs`

Server tasks:

1. Resolve the selected player's currently controlled entity through the control map.
2. Resolve scanner profile from controlled root or mounted children.
3. If no scanner profile exists, produce no scanner contacts and no live scanner cells for that player.
4. Redact each `TacticalContact` according to scanner tier before sending snapshot/delta.
5. Enforce `max_contacts`.
6. Sort contacts deterministically by distance from observer anchor, then entity ID.
7. Preserve sequence/snapshot/delta behavior already used by the tactical lane.
8. Add tests for redaction and no-scanner behavior.

Server must not:

1. use the player anchor as a scanner when the player is in free roam,
2. disclose contacts from a non-scanner controlled entity,
3. widen world replication visibility because the ring is open,
4. put private components into the tactical lane.

## 10. Client Runtime Plan

Recommended new file:

1. `bins/sidereal-client/src/runtime/sensor_ring.rs` (first slice implemented)

Recommended supporting refactor:

1. Extract tactical SVG icon loading/tinting helpers from the current tactical map UI code into a shared module such as `runtime/tactical_markers.rs`.
2. Use that helper from both the existing tactical map and the new ring.

Client resources:

```rust
pub(crate) struct TacticalSensorRingUiState {
    pub enabled: bool,
    pub alpha: f32,
    pub last_controlled_entity_id: Option<String>,
    pub last_unavailable_notice_at_s: f64,
}

pub(crate) struct ActiveScannerProfileCache {
    pub controlled_entity_id: Option<String>,
    pub profile: Option<ResolvedScannerProfile>,
}
```

Client marker components:

```rust
pub(crate) struct TacticalSensorRingRoot;
pub(crate) struct TacticalSensorRingMarkerDynamic {
    pub key: String,
}
pub(crate) struct TacticalSensorRingDensitySegment {
    pub sector_index: u8,
}
```

Client systems:

1. `update_active_scanner_profile_cache_system`
   - Runs after local control state and replicated entity adoption are current.
   - Finds scanner on controlled root or mounted children.
2. `toggle_tactical_sensor_ring_system`
   - Handles TAB.
   - Suppresses when console/text input is active.
   - Requires active controlled entity and scanner profile.
3. `close_sensor_ring_when_unavailable_system`
   - Closes ring when free roam, no controlled entity, or no scanner.
4. `update_tactical_sensor_ring_overlay_system`
   - Runs after tactical snapshot/delta receive.
   - Builds marker list from `TacticalContactsCache`.
   - Projects ring center from controlled entity screen position.
   - Updates/spawns/despawns marker and density entities.

Scheduling guidance:

1. The ring should update after `tactical::receive_tactical_snapshot_messages`.
2. Projection should use post-transform-propagation data if the latest visual transform is needed.
3. Keep the update cheap: no whole-world scans beyond scanner-profile resolution and current tactical cache iteration.

## 11. Asset and Icon Strategy

First slice:

1. Reuse existing `MapIcon` asset IDs from tactical contacts.
2. If `map_icon_asset_id` is not disclosed, use a generic contact marker.
3. If no generic marker asset exists, reuse the default ship marker tinted grey until a dedicated `map_icon_contact_unknown_svg` asset is authored.

Future asset option:

1. Add `map_icon_contact_unknown_svg`.
2. Add `map_icon_contact_hostile_svg` only if shape, not just tint, should communicate threat.
3. Author any new asset IDs in Lua asset registry scripts, not hardcoded Rust paths.

## 12. Documentation Updates When Moved Into Repo

Current documentation maintenance:

1. Keep this contract at `docs/features/tactical_sensor_ring_design_contract.md` unless the feature is renamed across the docs index.
2. Add dated status/update notes as client/server implementation slices land.
3. Update `docs/features/tactical_and_owner_lane_protocol_contract.md` with the scanner-tier meanings of `contact_quality` and `classification`.
4. Update `docs/features/visibility_replication_contract.md` to state that scanner capability must resolve from the currently controlled entity and that free roam has no scanner source.
5. If the design becomes project-wide policy beyond DR-0037, add a decision detail under `docs/decisions/` and link it from `docs/decision_register.md`.

## 13. Tests

Server tests:

1. No controlled entity means no scanner contact disclosure.
2. Free-roam/player anchor means no scanner contact disclosure.
3. Controlled entity without scanner means no scanner contact disclosure.
4. Controlled entity with root scanner allows scanner contact disclosure.
5. Controlled entity with mounted scanner allows scanner contact disclosure.
6. Basic tier redacts faction, specific kind, map icon, and velocity.
7. IFF tier exposes relationship classification only.
8. Classified tier exposes kind/map icon when policy allows.
9. Telemetry tier exposes velocity.
10. `max_contacts` truncates nearest contacts deterministically.

Client tests:

1. TAB toggles ring only when scanner profile exists.
2. TAB does not open ring during free roam.
3. TAB does not open ring for controlled entity without scanner.
4. Ring closes when active scanner profile disappears.
5. Bearing-to-ring-position math is ship-relative.
6. Density sector bucketing is deterministic.
7. Contact color role maps from disclosed `classification` and `contact_quality`.
8. Stale contacts render dimmer than live contacts.

Manual/native acceptance:

1. Start in a scanner-capable ship and press TAB: ring appears around controlled ship.
2. Press TAB again: ring closes.
3. Switch to free roam and press TAB: ring does not appear.
4. Control an entity without scanner and press TAB: ring does not appear.
5. Nearby basic contacts show as grey directional blips.
6. IFF-capable scanner shows red/blue/neutral relationship colors.
7. Density sectors appear only when `supports_density = true`.
8. Existing `M` tactical map behavior remains intact.

## 14. Quality Gates

Minimum gates after implementation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
```

Because client code is touched:

```bash
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

Run targeted tests for:

1. `sidereal-game`
2. `sidereal-net` if protocol/component version handling changes
3. `sidereal-replication`
4. `sidereal-client`

## 15. Implementation Phases

Phase 1: Documentation and scanner contract

1. Keep this document active under `docs/features/`.
2. Update visibility/tactical protocol docs. Started.
3. Extend `ScannerComponent`. Implemented.
4. Update Lua ship bundle authoring. Implemented for starter ship bundles.

Phase 2: Server disclosure enforcement

1. Resolve active controlled scanner profile.
2. Gate tactical contacts and live cells on active scanner.
3. Apply tiered redaction.
4. Add server tests.

Phase 3: Client ring HUD

1. Add sensor ring state/resources/components. Implemented.
2. Add TAB toggle and availability checks. Implemented.
3. Add ring marker/density rendering on UI overlay layer. Implemented first as `Mesh2d` primitives.
4. Reuse/extract tactical marker icon tinting.
5. Add client tests.

Phase 4: Native validation and polish

1. Validate normal flight with scanner ship.
2. Validate free roam and no-scanner controlled entity.
3. Tune ring size, alpha, density, marker scale, and stale/live visibility.
4. Confirm tactical map still works independently.

## 16. Open Future Options

These are intentionally out of the first slice:

1. server-authored hidden aggregate density sectors,
2. active scan ping/pulse,
3. scanner warmup/cooldown,
4. scanner damage/degradation,
5. jamming/noise/false positives,
6. target lock handoff from ring marker,
7. hover/click selection from the ring,
8. faction-shared sensor data,
9. party/fleet sensor fusion,
10. shader polish pass for ring glow and sweep effects.
