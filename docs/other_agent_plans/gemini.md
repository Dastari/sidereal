This is an excellent, well-scoped architectural blueprint. You’ve identified the exact friction points that cause multiplayer physics games to fail at scale: identity desync, control state ambiguity, and O(N\*M) visibility cascades.Here is the vertical-slice architecture to implement this safely and deterministically using Bevy 0.18, Lightyear 0.26.4, and Avian3D 0.5.1. Executive SummaryThis architecture enforces strict server authority with deterministic client prediction for owned entities and smooth interpolation for remote entities. We decouple Session Identity (the network connection), Player Identity (the logical presence/camera anchor), and Controlled Identity (the physical avatar/ship). Physics simulation is strictly driven by component-based force accumulators running in a fixed timestep, enabling Lightyear to flawlessly roll back and replay the Avian3D simulation upon receiving delayed inputs or server corrections. Spatial interest management is staged to prevent main-thread bottlenecks, and state persistence relies entirely on stable UUIDs.2. Architecture DiagramCode snippet[CLIENT] [SERVER] [PERSISTENCE]
+-------------------+ +-------------------+ +--------------------+
| Input System |--[PlayerInput]------>| Input Buffer | | DB (Postgres/Sled) |
| (Keyboard/Mouse) | | (Lightyear) | | |
+-------------------+ +-------------------+ +--------------------+
| | ^
v v (FixedUpdate) | (Async DB Task)
+-------------------+ +-------------------+ +--------------------+
| Action Queue | | Action Queue | | Hydration Pipeline |
+-------------------+ +-------------------+ | - GraphEntityRecord|
| | | - Component Blobs |
v v +--------------------+
+-------------------+ +-------------------+ ^
| Flight Computer | | Flight Computer | |
+-------------------+ +-------------------+ |
| | |
v v |
+-------------------+ +-------------------+ +--------------------+
| Engine/Forces | | Engine/Forces | | Snapshot/Serialize |
| (Add ExternalForce| | (Add ExternalForce| | System (Tick-based)|
+-------------------+ +-------------------+ +--------------------+
| | ^
v (FixedUpdate) v (FixedUpdate) |
+-------------------+ +-------------------+ +--------------------+
| Avian3D Physics |<-----(Rollback)------| Avian3D Physics |--------------------->| Interest Management|
| (Prediction Only) | | (Authoritative) | | - Spatial Grid |
+-------------------+ +-------------------+ | - Visibility Masks |
| | +--------------------+
v (Update) | (Replication) |
+-------------------+ | |
| Camera Anchor |<-----(Snapshots)---------------+-------------------------------------------+
+-------------------+ (Interpolated) 3. Component ModelIdentity & LinkageGlobalId(uuid::Uuid): The single source of truth for cross-boundary identity.SessionState: Server-side only. Maps a ClientId to a GlobalId (the Player).ControlLink: Contains target: GlobalId. If target == self, it's free-roam.CameraTarget: Client-side tag. Indicates the entity the camera should track.Gameplay Pipeline (The "Flight Computer")ActionQueue: Vec<Action> (e.g., Thrust, Pitch, Yaw, Fire). Cleared every fixed tick.ShipSpecs: Mass, max thrust, turning rate limits.Engine: Current fuel, engine health, active throttle state.Avian Components: Position, Rotation, LinearVelocity, AngularVelocity, ExternalForce, ExternalTorque.Networking Markers (Lightyear)Replicate: Server-side. Dynamically updated. For the controlling client: PredictionTarget::Only(client_id). For everyone else: InterpolationTarget::All.4. Networking / Protocol ModelInputs (Client -> Server)Rust#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct PlayerInput {
pub actions: Vec<PlayerAction>, // E.g., Move(Vec3), Fire(bool)
pub look_dir: Quat, // Intent-based aiming
}
Control Handoff Messages (Reliable Channel)Rust#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ControlRequest {
pub target_uuid: Uuid,
pub seq_num: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ControlAck {
pub target_uuid: Uuid,
pub seq_num: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ControlReject {
pub seq_num: u32,
pub reason: RejectReason,
} 5. Simulation & Scheduling PipelineTo ensure Avian and Lightyear play nicely, ordering is critical.PreUpdate (Network Receive):Lightyear receives packets.VisibilityManager filters what the client receives.FixedPreUpdate (Input & Rollback Prep):Lightyear handles Rollback triggers.Route PlayerInput into the controlled entity's ActionQueue.FixedUpdate (Physics & Gameplay):sys_flight_computer: Reads ActionQueue -> Computes desired vectors.sys_engine_burn: Reads desired vectors -> Applies ExternalForce / ExternalTorque.PhysicsSet::Sync / PhysicsSet::Step: Avian integrates velocities and moves transforms.sys_clear_actions: Flush the queue for the next tick.Update (Render & Interpolation):Lightyear interpolates Position/Rotation for remote entities.sys_camera_anchor: Updates camera transform to match the PlayerEntity transform (which may be following a ControlLink).PostUpdate (Network Send):Server syncs authoritative Position/Rotation/Velocity to clients via Lightyear.6. Persistence PipelineModel:Rustpub struct GraphEntityRecord {
pub entity_uuid: Uuid,
pub parent_uuid: Option<Uuid>,
pub template_id: String, // E.g., "fighter_class_a"
pub component_data: Vec<u8>, // Bincode/MessagePack serialized Map of components
}
Hydration Flow:Read all records for a zone/sector.Spawn Bevy entities with GlobalId. Keep a HashMap<Uuid, Entity>.Deserialize components into the ECS.Run a resolution pass: Resolve parent_uuid into Bevy hierarchy (BuildChildren::add_child) and resolve ControlLink UUIDs to Bevy entities.7. Interest Management & Visibility StrategyTo avoid $O(\text{clients} \times \text{entities})$ scans, implement a 2D Spatial Grid (Top-Down XY) on the server.Index Update (FixedUpdate tail): Update the spatial grid with current entity bounds.Authorization: Check faction/stealth rules. (e.g., Entity A is cloaked, requires Scanner Level 3).Delivery/Culling: For each client, query the spatial grid using a bounding box centered on their PlayerEntity (Screen size + 20% edge buffer).Lightyear Integration: Compare the result set with the client's current visibility state. Use Lightyear's VisibilityManager::gain_visibility and VisibilityManager::lose_visibility.8. Common Anti-Patterns (The "Do Not Do This" List)Writing to Transform instead of Position. Avian uses Position and Rotation as the source of truth. Modifying Bevy's Transform during FixedUpdate causes physics desync.Stateful components not included in rollback. If an entity has an Overheating component that disables the engine, it must be registered for prediction in Lightyear. If the server rolls back and Overheating isn't rolled back, the client prediction will diverge massively.Applying Input in Update. Inputs must be applied in FixedPreUpdate or FixedUpdate. Using frame-rate dependent input application guarantees divergent prediction.Raw Entity ID in network messages. Bevy Entity IDs are generational indices local to the specific World. Sending Entity(4v1) over the network will map to random memory garbage on the client. Always use GlobalId(Uuid).Camera driving physics. The camera is a visual observer. It should follow the visual representation (interpolated or predicted transform) in Update or PostUpdate.9. Validation & Test PlanScenario 1: Control Swap ReliabilityAction: Client sends ControlRequest(ShipB). Simulates 50% packet loss.Expected Outcome: Client retains control of ShipA until ControlAck(ShipB) arrives. No duplicate prediction states occur.Scenario 2: Single-Writer EnforcementAction: Malicious client sends PlayerInput mapped to a remote entity they don't own.Expected Outcome: Server drops input; routing strictly binds to the authenticated session's ControlLink.Scenario 3: Interpolation vs Prediction BoundariesAction: Two clients fly next to each other. Network latency is injected (150ms).Expected Outcome: Client A's ship responds instantly to Client A. Client B's ship moves smoothly on Client A's screen (delayed by ~150ms + interpolation buffer). Both converge deterministically upon server authoritative state arrival.10. Starter Code Skeleton (Focus: Control Routing)Here is a concise view of how to securely route inputs to forces using the pipeline.Rust// 1. The Input Router System (FixedUpdate)
fn route_player_inputs(
mut client_inputs: EventReader<InputEvent<PlayerInput>>, // Lightyear event
session_map: Res<SessionMap>, // ClientId -> Player UUID
entity_map: Res<UuidMap>, // UUID -> Bevy Entity
mut q_action_queues: Query<&mut ActionQueue>,
q_control_links: Query<&ControlLink>,
) {
for event in client_inputs.read() {
let Some(player_uuid) = session_map.get(event.client_id()) else { continue };
let Some(player_entity) = entity_map.get(player_uuid) else { continue };

        // Follow the explicit control chain
        if let Ok(control_link) = q_control_links.get(*player_entity) {
            let Some(target_entity) = entity_map.get(&control_link.target_uuid) else { continue };

            // Push intent to the authoritative target
            if let Ok(mut queue) = q_action_queues.get_mut(*target_entity) {
                queue.0.extend(event.input().actions.clone());
            }
        }
    }

}

// 2. The Flight Computer & Engine System (FixedUpdate)
fn sys_flight_computer_and_engine(
mut query: Query<(
&mut ActionQueue,
&ShipSpecs,
&mut ExternalForce,
&mut ExternalTorque,
&Rotation, // Avian component
)>,
) {
for (mut queue, specs, mut force, mut torque, rot) in query.iter_mut() {
let mut linear_thrust = Vec3::ZERO;

        for action in queue.0.drain(..) {
            match action {
                PlayerAction::Thrust(fwd) => linear_thrust += Vec3::Z * fwd,
                // Handle Pitch/Yaw/Roll -> apply to torque
                _ => {}
            }
        }

        // Clamp to engine limits and apply
        let clamped_thrust = linear_thrust.clamp_length_max(specs.max_thrust);
        let world_force = rot.0 * clamped_thrust;

        force.apply_force(world_force);
    }

}
Next Step: Would you like to dive deeper into the precise setup of the Lightyear Protocol configuration (to correctly annotate which components are strictly interpolated vs predicted), or should we detail the UUID-to-Entity resolution pass in the Hydration Pipeline?
