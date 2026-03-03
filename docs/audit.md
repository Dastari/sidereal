# Sidereal Runtime Audit (Client / Replication Server / Gateway)

Date: 2026-03-02  
Scope: `bins/sidereal-client`, `bins/sidereal-replication`, `bins/sidereal-gateway`, and shared crates they depend on at runtime.

This audit documents:
- entities
- components
- systems (including run order constraints)
- resources
- plugins
- where definitions exist vs where they are used
- issues/optimizations and pluginization opportunities
- documentation drift/outdated areas

---

## 1) High-Level Runtime Topology

### Client (`bins/sidereal-client`)
- Bevy runtime with Avian physics + Lightyear client networking.
- Uses `SiderealGameCorePlugin` (shared component registration) but not full authoritative gameplay plugin.
- Spawns local UI/camera/backdrop entities and adopts Lightyear-replicated entities into local presentation/control markers.

### Replication Server (`bins/sidereal-replication`)
- Headless Bevy runtime (`MinimalPlugins + ScheduleRunnerPlugin`) with authoritative simulation.
- Uses full `SiderealGamePlugin` for gameplay systems.
- Hydrates simulation entities from graph persistence and replicates through Lightyear.

### Gateway (`bins/sidereal-gateway`)
- Not Bevy/ECS runtime. Axum + Tokio auth/bootstrap HTTP service.
- Creates starter world as graph records (`sidereal-runtime-sync`) instead of spawning Bevy entities.

### Shared crates used by multiple runtimes
- `crates/sidereal-game`: gameplay components, gameplay systems, archetype defaults/bundles.
- `crates/sidereal-net`: Lightyear protocol registration for messages/channels/components.
- `crates/sidereal-runtime-sync`: graph hydration/persistence sync + starter graph templates.
- `crates/sidereal-persistence`: graph storage APIs.

---

## 2) Plugins Audit

## 2.1 Client Plugins

Registered in `bins/sidereal-client/src/native/mod.rs`.

- **Bevy core plugins**
  - `DefaultPlugins` (non-headless) with custom `WindowPlugin`, `AssetPlugin`, `RenderPlugin`.
  - `MinimalPlugins` (headless mode) + `LogPlugin`, `AssetPlugin`, `ScenePlugin`.
- **Rendering helpers**
  - `FrameTimeDiagnosticsPlugin`.
  - `Material2dPlugin<StarfieldMaterial>`.
  - `Material2dPlugin<SpaceBackgroundMaterial>`.
  - `Material2dPlugin<StreamedSpriteShaderMaterial>`.
- **Physics / network**
  - `PhysicsPlugins` with `PhysicsTransformPlugin` and `PhysicsInterpolationPlugin` disabled.
  - `ClientPlugins` (Lightyear client).
  - `LightyearAvianPlugin` (`AvianReplicationMode::Position`).
- **Gameplay/shared**
  - `SiderealGameCorePlugin` (shared component registration and reflect setup only).
- **Optional remote inspection**
  - `bevy_remote::RemotePlugin` and `RemoteHttpPlugin` via `native/remote.rs`.
- **Protocol registration function (not a Bevy plugin)**
  - `register_lightyear_protocol(&mut app)` from `crates/sidereal-net`.

## 2.2 Replication Server Plugins

Registered in `bins/sidereal-replication/src/main.rs`.

- **Bevy core/headless**
  - `MinimalPlugins` + `ScheduleRunnerPlugin::run_loop(...)`.
  - `AssetPlugin`, `ScenePlugin`, `LogPlugin`.
- **Gameplay/shared**
  - `SiderealGamePlugin` (full gameplay systems + component registration).
- **Physics / network**
  - `PhysicsPlugins` with transform/interpolation disabled.
  - `ServerPlugins` (Lightyear server).
- **Protocol registration**
  - `register_lightyear_protocol(&mut app)` from `crates/sidereal-net` (also adds `NativeInputPlugin<PlayerInput>` internally).
- **Optional remote inspection**
  - `bevy_remote::RemotePlugin` + `RemoteHttpPlugin` via `replication/lifecycle.rs`.

## 2.3 Gateway Plugins

- Gateway adds **no Bevy plugins**.
- Runtime is Axum/Tokio only (`bins/sidereal-gateway/src/main.rs` and `src/api.rs`).

---

## 3) Systems Audit (Order + Task)

## 3.1 Client Systems

Primary registration: `bins/sidereal-client/src/native/mod.rs`.  
Additional systems are defined in `native/*.rs` modules.

### Startup
- `scene::spawn_ui_overlay_camera` (non-headless): spawns dedicated UI overlay camera.
- `transport::start_lightyear_client_transport` (headless): starts Lightyear transport entity/client.
- `auth_net::configure_headless_session_from_env` (headless): seeds session/account switch behavior.

### OnEnter state transitions
- `OnEnter(Auth)`: `auth_ui::setup_auth_screen`.
- `OnEnter(CharacterSelect)`: `scene::setup_character_select_screen`.
- `OnEnter(WorldLoading)`: `(ensure_lightyear_client_system, reset_bootstrap_watchdog_on_enter_in_world).chain()`.
- `OnEnter(InWorld)`: `(ensure_lightyear_client_system, spawn_world_scene, reset_bootstrap_watchdog_on_enter_in_world).chain()`.

### PreUpdate
- `ensure_replicated_entity_spatial_components`: ensures transform/physics components exist on adopted replicated entities.
- `logout_chain.chain()`: logout request handling, disconnect notification, and cleanup sequence.

### FixedPreUpdate
- Input write chain in Lightyear input set:
  1. `input::enforce_single_input_marker_owner` (before send)
  2. `input::send_lightyear_input_messages`
  3. `ApplyDeferred`
- Non-headless path gates this by `in_state(InWorld)`.

### FixedUpdate (before physics simulation)
- Shared gameplay/prediction chain (before `PhysicsSystems::StepSimulation`):
  1. `enforce_motion_ownership_for_world_entities`
  2. `audit_motion_ownership_system` (after 1)
  3. `validate_action_capabilities`
  4. `process_character_movement_actions`
  5. `process_flight_actions`
  6. `recompute_total_mass`
  7. `apply_engine_thrust`
- Additional in-world predicted-control chain (before `process_character_movement_actions`):
  1. `apply_predicted_input_to_action_queue`
  2. `enforce_controlled_planar_motion`

### FixedUpdate (after physics simulation)
- Chained after `PhysicsSystems::StepSimulation`:
  1. `reconcile_controlled_prediction_with_confirmed`
  2. `stabilize_idle_motion`
  3. `clamp_angular_velocity`

### Update (core transport/auth/asset/adoption flow)
- Both headless and non-headless variants run an ordered chain including:
  - prediction manager tuning
  - transport channel checks
  - auth message send/receive
  - asset stream receive + critical asset readiness check (ordered `.after`)
  - replicated entity adoption
  - transform sync + local player view sync (ordered `.after(adopt...)`)
  - control request/response and state logging
  - prediction runtime logging
- Non-headless adds:
  - `scene::handle_character_select_buttons`
  - `transition_world_loading_to_in_world` after adoption.

### Update (InWorld visual/UI/camera passes)
- Visual chain (ordered with `.after`):
  - fullscreen fallback ensure
  - duplicate predicted/interpolated visual suppression
  - streamed visual child cleanup
  - streamed visual asset attachment
  - fullscreen renderable sync
  - backdrop fullscreen sync
- Camera/UI/bootstrap:
  - gameplay camera gating
  - owned-entities panel updates and interactions
  - loading overlay and stream icon updates
  - bootstrap watchdog
  - top-down camera update, then UI-overlay camera sync, then camera motion update
  - UI overlay layer propagation
  - HUD update, nameplate sync, segmented bars update, debug overlay toggle
- Interpolation path:
  - remote/non-controlled motion interpolation is handled by Lightyear interpolation on replicated Avian motion components (`Position`, `Rotation`, `LinearVelocity`, `AngularVelocity`).
  - client-only secondary interpolation systems are not used.
- `audit_active_world_cameras_system` also runs in `InWorld`.

### PostUpdate
- `sync_mounted_hierarchy.before(TransformSystems::Propagate)`: keeps parent-child mount hierarchy consistent.

### Last
- Optional: `platform::enforce_frame_rate_cap_system`.
- In-world chained end-of-frame:
  1. `lock_player_entity_to_controlled_entity_end_of_frame`
  2. `lock_camera_to_player_entity_end_of_frame`
  3. `compute_fullscreen_external_world_system`
  4. `update_starfield_material_system`
  5. `update_space_background_material_system`
  6. `update_ship_nameplate_positions_system`
  7. `update_segmented_bars_system`
  8. `draw_debug_overlay_system`

### Observers
- `log_native_client_connected` on `On<Add, Connected>`.

## 3.2 Replication Server Systems

Registered in `bins/sidereal-replication/src/main.rs`.

### Startup
- Chained startup sequence:
  1. `lifecycle::hydrate_replication_world`
  2. `simulation_entities::hydrate_simulation_entities`
  3. `lifecycle::start_lightyear_server`
  4. `persistence::start_persistence_worker`
  5. `assets::initialize_asset_stream_cache`
- Parallel startup system:
  - `bootstrap_runtime::start_replication_control_listener`

### Update
- Chained runtime flow:
  1. `ApplyDeferred`
  2. `lifecycle::ensure_server_transport_channels`
  3. `auth::receive_client_disconnect_notify`
  4. `auth::cleanup_client_auth_bindings`
  5. `input::receive_latest_realtime_input_messages`
  6. `control::receive_client_control_requests`
  7. `assets::receive_client_asset_requests`
  8. `assets::receive_client_asset_acks`
  9. `input::report_input_drop_metrics`
  10. `persistence::report_persistence_worker_metrics`
  11. `simulation_entities::process_bootstrap_entity_commands`
  12. `runtime_state::log_player_control_state_changes` (`.after(process_bootstrap_entity_commands)`)
  13. `lifecycle::disconnect_idle_clients`

### PostUpdate
- `auth::receive_client_auth_messages`
- `simulation_entities::apply_pending_controlled_by_bindings`
  - runs `.after(lightyear::prelude::ReplicationBufferSystems::AfterBuffer)`.

### FixedUpdate
- Asset stream:
  1. `assets::stream_bootstrap_assets_to_authenticated_clients`
  2. `assets::send_asset_stream_chunks_paced` (`after` 1)
- Visibility/runtime-state chain (after `PhysicsSystems::Writeback`):
  1. `simulation_entities::sync_controlled_entity_transforms`
  2. `runtime_state::sync_player_anchor_to_controlled_entity`
  3. `runtime_state::update_client_observer_anchor_positions`
  4. `runtime_state::compute_controlled_entity_scanner_ranges`
  5. `visibility::update_network_visibility`
- Dirty marking (after `PhysicsSystems::Writeback`):
  - `mark_dirty_persistable_entities`
  - `mark_dirty_persistable_entities_spatial`
  - `mark_dirty_persistable_entities_components`
- Persistence flush:
  - `persistence::flush_simulation_state_persistence.after(visibility::update_network_visibility)`
- Pre-physics constraints:
  - `simulation_entities::enforce_planar_motion.before(PhysicsSystems::Prepare)`
  - `input::drain_native_player_inputs_to_action_queue.before(PhysicsSystems::Prepare)`

### Observers
- `lifecycle::log_replication_client_connected` on connect add.
- `lifecycle::setup_client_replication_sender` on `Add<LinkOf>`.

### Shared gameplay systems active via `SiderealGamePlugin`

From `crates/sidereal-game/src/lib.rs`:
- `PostUpdate`: `bootstrap_ship_mass_components`, `bootstrap_root_dynamic_entity_colliders`, `sync_mounted_hierarchy.before(TransformSystems::Propagate)` with run-if.
- `FixedUpdate` before physics: `validate_action_capabilities -> sync_player_to_controlled_entity -> process_character_movement_actions -> process_flight_actions -> recompute_total_mass -> apply_engine_thrust`.
- `FixedUpdate` after physics: `stabilize_idle_motion -> clamp_angular_velocity`.

## 3.3 Gateway Systems

- No Bevy ECS systems (gateway is HTTP service).
- Operational flow:
  1. load env/config
  2. connect postgres + ensure schema
  3. choose bootstrap dispatcher mode (`udp` or direct)
  4. construct auth service
  5. bind TCP listener
  6. serve Axum router.

---

## 4) Resources Audit

## 4.1 Client resources

Inserted/init in `bins/sidereal-client/src/native/mod.rs` (plus module-level registrars):

- Core time/physics/debug:
  - `Time<Fixed>::from_hz(30.0)`
  - `Gravity(Vec2::ZERO)`
  - optional `FrameRateCap`
  - `LocalSimulationDebugMode`
  - `MotionOwnershipAuditEnabled`
  - `MotionOwnershipAuditState`
- Session/control/network:
  - `ClientSession`
  - `SessionReadyState`
  - `ClientNetworkTick`
  - `ClientInputAckTracker`
  - `ClientInputLogState`
  - `ClientInputSendState`
  - `ClientAuthSyncState`
  - `ClientControlRequestState`
  - `ClientControlDebugState`
  - `HeadlessTransportMode`
- Asset/stream/runtime view:
  - `AssetRootPath`
  - `LocalAssetManager`
  - `RuntimeAssetStreamIndicatorState`
  - `CriticalAssetRequestState`
  - `LocalPlayerViewState`
  - `CharacterSelectionState`
  - `RuntimeEntityHierarchy` (from shared crate)
  - `RemoteEntityRegistry`
- Camera/visual/UI:
  - `FreeCameraState`
  - `OwnedEntitiesPanelState`
  - `FullscreenExternalWorldData`
  - `StarfieldMotionState`
  - `CameraMotionState`
  - `DebugBlueOverlayEnabled`
  - `DebugOverlayEnabled`
- Bootstrap/prediction tuning:
  - `BootstrapWatchdogState`
  - `DeferredPredictedAdoptionState`
  - `PredictionBootstrapTuning`
  - `PredictionCorrectionTuning`
  - `NearbyCollisionProxyTuning`
- Logout/dialog/auth UI support:
  - `PendingDisconnectNotify`
  - `LogoutCleanupRequested`
  - `dialog_ui::DialogQueue` (headless init path)
  - `scene::EmbeddedFonts` (from `scene::insert_embedded_fonts`)
  - `auth_ui::CursorBlink` (from auth UI registrar)
  - optional `BrpAuthToken` from remote config path.

## 4.2 Replication server resources

Initialized by module init functions from `bins/sidereal-replication/src/replication/*`:

- Global:
  - `HierarchyRebuildEnabled(false)`
  - `Gravity(Vec2::ZERO)`
  - `Time<Fixed>::from_hz(30.0)`
- Lifecycle/auth/session:
  - `ClientLastActivity`
  - `PendingIdleUnlink`
  - `IdleDisconnectSeconds`
  - `AuthenticatedClientBindings`
  - optional `BrpAuthToken`
- Input/control:
  - input tracking resources in `input.rs` (tick/drop/rate/latest input maps)
  - `ClientControlRequestOrder`
- Visibility/runtime-state:
  - `ClientVisibilityRegistry`
  - `VisibilityScratch`
  - `ClientObserverAnchorPositionMap`
  - `PlayerControlDebugState`
- Simulation entity bindings:
  - `PlayerControlledEntityMap`
  - `PlayerRuntimeEntityMap`
  - `PendingControlledByBindings`
- Asset streaming:
  - `AssetStreamServerState`
  - `StreamableAssetCache`
  - `AssetDependencyMap`
- Persistence pipeline:
  - `PersistenceWorkerState`
  - `PersistenceDirtyState`
  - `PersistenceSchemaInitState`
  - `SimulationPersistenceTimer`
- Bootstrap bridge:
  - `BootstrapEntityReceiver`
- Shared registry:
  - `GeneratedComponentRegistry` (from `SiderealGameCorePlugin`).

## 4.3 Gateway resources/state

Gateway uses service state (not Bevy resources):

- `AuthConfig` (env-derived)
- `PostgresAuthStore`
- `BootstrapDispatcher` dyn impl:
  - `UdpBootstrapDispatcher` or `DirectBootstrapDispatcher`
- `AuthService` (`Arc`) as Axum state.

---

## 5) Entities Audit

## 5.1 Client entities and archetypes

### Spawned local entities
- UI overlay camera:
  - `Camera2d + UiOverlayCamera` (scene setup).
- Auth and character selection UI trees:
  - auth screen markers in `native/auth_ui.rs`
  - character select markers in `native/scene.rs`.
- In-world local scene:
  - gameplay camera (`GameplayCamera`, `TopDownCamera`)
  - fallback backdrops (`SpaceBackdropFallback`, `DebugBlueBackdrop`)
  - HUD tree (`GameplayHud`, text/bar marker components)
  - owned entities panel and nameplate entities.
- Fullscreen layer local fallback entities:
  - `SpaceBackgroundFullscreenLayerBundle`
  - `StarfieldFullscreenLayerBundle`
  - `FallbackFullscreenLayer` and runtime renderable markers.
- Networking transport entity:
  - `RawClient`, `UdpIo`, `MessageManager`, `ReplicationReceiver`, `LocalAddr`, `PeerAddr`, `Name`.

### Adopted replicated entities
- Lightyear-created network entities become tagged client-side with:
  - `WorldEntity`
  - `ReplicatedAdoptionHandled`
  - one of `ControlledEntity` or `RemoteEntity` / `RemoteVisibleEntity`
  - additional runtime visual markers (`StreamedVisualAttached`, `SuppressedPredictedDuplicateVisual`, etc.) as systems progress.

## 5.2 Replication server entities and archetypes

### Spawned local service entities
- Server transport entity:
  - `RawServer`, `ServerUdpIo`, `LocalAddr`, `Stopped`, `Name`.
- Startup debug hydration markers:
  - `HydratedGraphEntity`.

### Hydrated authoritative simulation entities
- Spawn baseline per graph entity:
  - `Name`, `EntityGuid`, `Transform`, `Visibility`, `Replicate::to_clients(All)`.
- Then insert reflected/persisted components from graph records via shared registry mappings.
- Runtime control markers inserted/managed:
  - `SimulatedControlledEntity`
  - later `ControlledBy`.

## 5.3 Gateway entity creation model

- Gateway does not spawn ECS entities.
- It creates persisted graph entity/component records for starter world with:
  - Player entity
  - Ship hull
  - 5 hardpoint entities
  - 5 module entities (flight computer, engines, fuel tanks)
- Source template:
  - `crates/sidereal-runtime-sync/src/entity_templates.rs`.

---

## 6) Components Audit

## 6.1 Shared gameplay components (`crates/sidereal-game/src/components`)

All macro components use `sidereal_component_macros::sidereal_component(...)` and derive `Reflect + Serialize + Deserialize`.  
For these components:
- **Reflected:** yes (registered via generated inventory path).
- **Replicated:** based on `replicate = ...` metadata (and registered by `register_lightyear_protocol`).
- **Persisted:** based on `persist = ...` metadata (included in generated component registry if true).

### Component catalog

| Type | kind | persist | replicate | predict | visibility | File |
|---|---|---:|---:|---:|---|---|
| `AccountId` | `account_id` | true | true | false | `OwnerOnly` | `crates/sidereal-game/src/components/account_id.rs` |
| `ActionCapabilities` | `action_capabilities` | true | true | true | `Public` | `.../action_capabilities.rs` |
| `ActionQueue` | `action_queue` | true | true | true | `OwnerOnly` | `.../action_queue.rs` |
| `BaseMassKg` | `base_mass_kg` | true | true | false | `Public` | `.../base_mass_kg.rs` |
| `CargoMassKg` | `cargo_mass_kg` | true | true | false | `OwnerOnly` | `.../cargo_mass_kg.rs` |
| `CharacterMovementController` | `character_movement_controller` | true | true | false | `OwnerOnly` | `.../character_movement_controller.rs` |
| `CollisionAabbM` | `collision_aabb_m` | true | true | false | `Public` | `.../collision_aabb_m.rs` |
| `ControlledEntityGuid` | `controlled_entity_guid` | true | true | false | `OwnerOnly` | `.../controlled_entity_guid.rs` |
| `Cost` | `cost` | true | true | false | `OwnerOnly, Public` | `.../cost.rs` |
| `Density` | `density` | false | false | false | `OwnerOnly` | `.../density.rs` |
| `DisplayName` | `display_name` | true | true | false | `Public` | `.../display_name.rs` |
| `Engine` | `engine` | true | true | false | `OwnerOnly` | `.../engine.rs` |
| `EntityGuid` | `entity_guid` | true | true | false | `Public` | `.../entity_guid.rs` |
| `EntityLabels` | `entity_labels` | true | true | false | `Public` | `.../entity_labels.rs` |
| `FactionId` | `faction_id` | true | true | false | `Public` | `.../faction_id.rs` |
| `FactionVisibility` | `faction_visibility` | true | true | false | `Public` | `.../faction_visibility.rs` |
| `FlightComputer` | `flight_computer` | true | true | true | `OwnerOnly` | `.../flight_computer.rs` |
| `FlightTuning` | `flight_tuning` | true | true | true | `OwnerOnly` | `.../flight_tuning.rs` |
| `FocusedEntityGuid` | `focused_entity_guid` | true | true | false | `OwnerOnly` | `.../focused_entity_guid.rs` |
| `FuelTank` | `fuel_tank` | true | true | false | `OwnerOnly` | `.../fuel_tank.rs` |
| `FullscreenLayer` | `fullscreen_layer` | true | true | false | `Public` | `.../fullscreen_layer.rs` |
| `Hardpoint` | `hardpoint` | true | true | false | `Public` | `.../hardpoint.rs` |
| `HealthPool` | `health_pool` | true | true | false | `OwnerOnly` | `.../health_pool.rs` |
| `Inventory` | `inventory` | true | true | false | `OwnerOnly` | `.../inventory.rs` |
| `MassDirty` | `mass_dirty` | true | true | false | `OwnerOnly` | `.../mass_dirty.rs` |
| `MassKg` | `mass_kg` | true | true | false | `Public` | `.../mass_kg.rs` |
| `MaxVelocityMps` | `max_velocity_mps` | true | true | true | `Public` | `.../max_velocity_mps.rs` |
| `ModuleMassKg` | `module_mass_kg` | true | true | false | `Public` | `.../module_mass_kg.rs` |
| `ModuleTag` | `module_tag` | true | true | false | `Public` | `.../module_tag.rs` |
| `MountedOn` | `mounted_on` | true | true | false | `Public` | `.../mounted_on.rs` |
| `OwnerId` | `owner_id` | true | true | false | `OwnerOnly` | `.../owner_id.rs` |
| `ParentGuid` | `parent_guid` | true | true | false | `Public` | `.../parent_guid.rs` |
| `PlayerTag` | `player_tag` | true | true | false | `OwnerOnly` | `.../player_tag.rs` |
| `PublicVisibility` | `public_visibility` | true | true | false | `Public` | `.../public_visibility.rs` |
| `ScannerComponent` | `scanner_component` | true | true | false | `OwnerOnly` | `.../scanner_component.rs` |
| `ScannerRangeBuff` | `scanner_range_buff` | true | true | false | `OwnerOnly` | `.../scanner_range_buff.rs` |
| `ScannerRangeM` | `scanner_range_m` | true | true | false | `Public` | `.../scanner_range_m.rs` |
| `SelectedEntityGuid` | `selected_entity_guid` | true | true | false | `OwnerOnly` | `.../selected_entity_guid.rs` |
| `ShardAssignment` | `shard_assignment` | true | true | false | `OwnerOnly` | `.../shard_assignment.rs` |
| `ShipTag` | `ship_tag` | true | true | false | `Public` | `.../ship_tag.rs` |
| `SizeM` | `size_m` | true | true | true | `Public` | `.../size_m.rs` |
| `SpaceBackgroundShaderSettings` | `space_background_shader_settings` | false | false | false | `OwnerOnly` | `.../space_background_shader_settings.rs` |
| `SpriteShaderAssetId` | `sprite_shader_asset_id` | true | true | false | `Public` | `.../sprite_shader_asset_id.rs` |
| `StarfieldShaderSettings` | `starfield_shader_settings` | false | false | false | `OwnerOnly` | `.../starfield_shader_settings.rs` |
| `TotalMassKg` | `total_mass_kg` | true | true | true | `Public` | `.../total_mass_kg.rs` |
| `VisualAssetId` | `visual_asset_id` | true | true | false | `Public` | `.../visual_asset_id.rs` |

### Avian components used in persistence/replication path

Registered manually in:
- `crates/sidereal-game/src/generated/components.rs` (reflect + persistence registry entries)
- `crates/sidereal-net/src/lightyear_protocol/registration.rs` (replication registration + prediction)

Included kinds:
- `avian_position`
- `avian_rotation`
- `avian_linear_velocity`
- `avian_angular_velocity`
- `avian_rigid_body`
- `avian_mass`
- `avian_angular_inertia`
- `avian_linear_damping`
- `avian_angular_damping`

## 6.2 Client runtime-only components

Defined in `bins/sidereal-client/src/native/components.rs`:
- Marker/UI/camera/runtime presentation components (`WorldEntity`, `GameplayCamera`, `TopDownCamera`, `ShipNameplateRoot`, `StreamedVisualChild`, etc.).
- These are local runtime components and are not in shared persistence/replication registry unless separately mirrored by shared components.

## 6.3 Replication runtime-only components

Defined in replication modules:
- `HydratedGraphEntity` (`replication/lifecycle.rs`)
- `SimulatedControlledEntity` (`replication/simulation_entities.rs`)
- Local-only markers for lifecycle/debug/control management.

---

## 7) Where Definitions Live vs Where Used

## 7.1 Shared definition locations
- Gameplay components: `crates/sidereal-game/src/components/*.rs`
- Gameplay plugin/system wiring: `crates/sidereal-game/src/lib.rs`
- Generated component registry + Avian entries: `crates/sidereal-game/src/generated/components.rs`
- Protocol replication registration: `crates/sidereal-net/src/lightyear_protocol/registration.rs`
- Starter world graph template: `crates/sidereal-runtime-sync/src/entity_templates.rs`

## 7.2 Runtime usage map
- **Client uses shared components** for querying/adoption/UI/visuals:
  - adoption and tag sync in `bins/sidereal-client/src/native/replication.rs`
  - motion and control in `native/motion.rs`
  - UI display in `native/ui.rs`
  - debug overlay in `native/debug_overlay.rs`
- **Replication server uses shared components** for authoritative simulation and persistence:
  - hydration and controlled binding in `replication/simulation_entities.rs`
  - visibility and scanner flow in `replication/runtime_state.rs` and `replication/visibility.rs`
  - persistence dirty/flush in `replication/persistence.rs`
- **Gateway uses shared components** indirectly through graph templates:
  - `new_player_starter_graph_records` in `crates/sidereal-runtime-sync/src/entity_templates.rs`
  - called from `bins/sidereal-gateway/src/auth.rs`.

---

## 8) Issues, Optimizations, and Best-Practice Gaps

Ordered by impact.

1. **Client app orchestration is overly centralized**
- `bins/sidereal-client/src/native/mod.rs` owns too many schedules, conditions, and chains.
- Risk: hard-to-reason ordering regressions and high merge conflict surface.
- Recommendation: split into feature plugins:
  - `ClientTransportPlugin`
  - `ClientAuthPlugin`
  - `ClientReplicationAdoptionPlugin`
  - `ClientVisualsPlugin`
  - `ClientUiPlugin`
  - `ClientPredictionPlugin`
  - `ClientDiagnosticsPlugin`.

2. **Blocking HTTP calls inside frame-driven UI systems**
- `native/auth_ui.rs` and `native/scene.rs` trigger auth/world-enter flows using blocking request patterns.
- Risk: frame stalls, input hitching, poorer UX.
- Recommendation: move to async task/event response model (Bevy task pool / async channel), keep ECS systems non-blocking.

3. **Replication startup performs duplicated graph-load style work**
- Both `hydrate_replication_world` and `hydrate_simulation_entities` touch graph hydration paths.
- Risk: startup overhead and duplicated logic.
- Recommendation: consolidate to one hydration pipeline with optional debug-mode branch.

4. **Visibility algorithm appears O(entities × clients) per fixed tick**
- `replication/visibility.rs` updates per-client visibility by scanning replicated entity sets.
- Risk: scale bottleneck.
- Recommendation: introduce broad-phase spatial partitioning and delta-driven visibility updates.

5. **Gateway auth module is monolithic**
- `bins/sidereal-gateway/src/auth.rs` bundles config/store/hash/tokens/bootstrap dispatch/starter-world persistence.
- Risk: coupling and test friction.
- Recommendation: split into focused modules (`config`, `store`, `token_service`, `bootstrap_dispatch`, `starter_world`).

6. **Potential archetype drift between ECS spawn and graph template**
- Corvette defaults/archetypes exist in both:
  - ECS spawn path (`crates/sidereal-game/src/entities/ship/corvette.rs`)
  - graph template path (`crates/sidereal-runtime-sync/src/entity_templates.rs`)
- Risk: starter world inconsistencies over time.
- Recommendation: generate both ECS and graph outputs from one canonical archetype definition object.

7. **`WorldEntity` marker currently covers multiple semantics on client**
- Used for replicated world state and local helper entities in nearby systems.
- Risk: accidental broad queries and hidden behavior coupling.
- Recommendation: split markers by intent (`ReplicatedWorldEntity`, `ClientWorldUiEntity`, etc.).

8. **Gateway async handler uses sync filesystem existence check**
- In `/assets/stream/{asset_id}` route path handling.
- Risk: avoidable blocking in async context.
- Recommendation: rely on async open and match error rather than sync `exists()` check.

9. **System duplication across schedules on client**
- Some lock/update systems appear both in `Update` and `Last`.
- Risk: unintuitive order dependencies and redundant work.
- Recommendation: centralize end-of-frame ownership locks in one schedule with clear system sets.

---

## 9) Recommended Plugin Separation (Concrete Plan)

## 9.1 Client plugin boundaries
- **`ClientBootstrapPlugin`**: app state transitions, bootstrap watchdog, scene entry/exit.
- **`ClientTransportPlugin`**: Lightyear startup/channel readiness/auth message transport.
- **`ClientReplicationPlugin`**: replicated adoption, controlled/remote tagging, transform sync.
- **`ClientPredictionPlugin`**: fixed-step input send, predicted action queue apply, reconciliation.
- **`ClientVisualsPlugin`**: streamed visuals, fullscreen layers, interpolation smoothing.
- **`ClientUiPlugin`**: auth UI, dialog UI, HUD/panels/nameplates.
- **`ClientDiagnosticsPlugin`**: overlay toggles, camera audits, motion ownership audits.

## 9.2 Replication server plugin boundaries
- **`ReplicationLifecyclePlugin`**: server start/transport idle disconnect and observers.
- **`ReplicationAuthPlugin`**: auth message receive/bindings cleanup.
- **`ReplicationInputPlugin`**: input receive/drain + metrics.
- **`ReplicationControlPlugin`**: control request/order handling.
- **`ReplicationVisibilityPlugin`**: runtime anchor/scanner computations + visibility updates.
- **`ReplicationPersistencePlugin`**: dirty marking, worker lifecycle, periodic flush.
- **`ReplicationAssetsPlugin`**: stream cache init/manifest/chunk pacing/ack handling.
- **`ReplicationBootstrapBridgePlugin`**: bootstrap UDP listener and command ingest.

## 9.3 Gateway module boundaries (non-Bevy)
- **`GatewayAuthModule`**
- **`GatewayBootstrapModule`**
- **`GatewayAssetModule`**
- **`GatewayPersistenceProvisioningModule`**

---

## 10) Documentation Drift / Outdated Content

These are concrete mismatches between docs and current runtime behavior.

1. **Asset serving contract drift**
- Design docs/agent rules emphasize stream-first, no standalone HTTP file serving.
- Current gateway still serves `/assets/stream/{asset_id}` from filesystem in `bins/sidereal-gateway/src/api.rs`.
- Action: update docs to clarify transitional state or remove HTTP asset serving path.

2. **Client cache format drift**
- Design docs describe MMO-style `assets.pak` + index metadata.
- Current client asset cache flow still writes/reads file-tree chunks under `data/cache_stream/**` (`bins/sidereal-client/src/native/assets.rs`).
- Action: either implement pak/index path or update docs to mark current implementation as interim.

3. **WASM parity plan partial staleness**
- `docs/features/wasm_parity_implementation_plan.md` references a monolithic native file baseline.
- Native client now uses modularized `native/*.rs` with `native/mod.rs`.
- Action: refresh plan assumptions and current-state baseline section.

4. **Transport roadmap wording drift**
- Docs mention WebRTC-first direction and historical WebTransport/WebSocket alternatives.
- Current code in active runtime paths is still UDP native (`UdpIo`/`ServerUdpIo`) for client/server.
- Action: explicitly document current production transport and browser-target gap status in one canonical doc.

---

## 11) Quick Completeness Checklist

- Client entities/components/systems/resources/plugins: audited.
- Replication server entities/components/systems/resources/plugins: audited.
- Gateway entities/components/systems/resources/plugins equivalent: audited (non-ECS service model).
- Shared crate definitions and usage points: mapped.
- Component persist/replicate/reflect/predict metadata: cataloged.
- Optimization + pluginization recommendations: included.
- Documentation drift findings: included with file-level evidence.
