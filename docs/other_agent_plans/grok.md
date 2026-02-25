Executive Summary
A strict server-authoritative vertical slice using Bevy 0.18, Lightyear 0.26.4 + lightyear_avian3d, and Avian3D 0.5. Clients send only intent (actions). Server owns the single source of truth for every Position/Rotation/LinearVelocity/AngularVelocity. Clients run full prediction + rollback only on their currently-controlled entity (via Predicted marker). All other entities receive server snapshots and are rendered via interpolation only. Stable NetworkId(Uuid) is the only cross-boundary identity. Control handoff is an explicit request/ack protocol with sequence numbers. Persistence is a flat graph of NetworkId-keyed records using bevy_reflect + serde. Interest management is server-side spatial (grid hash) with camera-centered XY culling + ownership exceptions. The design satisfies every hard requirement and is ready to scale to hundreds of clients / thousands of entities without O(n·m) scans in the hot path.2. Architecture Diagram (ASCII)

[Client A] ──(Input + ControlReq)──┐
Predicted entity only │ UDP/WebTransport (Lightyear)
Interpolated remotes │
Camera ← Player ← Controlled │
│
[Shared] ──(common::components, protocol, constants)──
│
[Server] ──(Authoritative Sim)────┘
• Player + Control mapping
• Fixed-tick sim (one writer)
• SpatialIndex (grid 100 m cells)
• Visibility pipeline (auth → cull → redact)
• Persistence (GraphEntityRecord on timer/disconnect)
• lightyear_avian3d ordering

Crate boundaries (workspace) space_game_common – NetworkId, protocol messages, shared components, constants, reflect registrations.  
space_game_core – gameplay systems, force pipeline, Avian setup, control routing (no net).  
space_game_client – Bevy client app, input collection, Predicted insertion, camera, visual systems.  
space_game_server – headless server app, auth, interest, persistence, replication config.  
space_game_persistence (optional) – GraphEntityRecord, hydrate/serialize helpers.

3. Component Model (minimum vertical slice)rust

// common/src/components.rs #[derive(Component, Reflect, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)] #[reflect(Component)]
pub struct NetworkId(pub Uuid); // stable identity – inserted at spawn, never changes

#[derive(Component, Replicate)] // Lightyear
pub struct Replicated; // marker for replication (or use Replicate bundle)

#[derive(Component)] // Lightyear (client only)
pub struct Predicted;

#[derive(Component)] // Lightyear (client only on remotes)
pub struct Interpolated;

// Ownership / control chain – explicit, persisted #[derive(Component, Reflect, Serialize, Deserialize)]
pub struct Player {
pub account_id: Uuid,
pub client_id: Option<ClientId>, // Lightyear ClientId when connected
}

#[derive(Component, Reflect, Serialize, Deserialize)]
pub struct CurrentControl {
pub target: NetworkId, // free-roam = self (player's own NetworkId)
}

#[derive(Component, Reflect, Serialize, Deserialize, Default)]
pub struct ActionQueue(pub Vec<ShipAction>); // intent only

// Avian motion components (authoritative, replicated via lightyear_avian3d)
use avian3d::prelude::\*;
// Position, Rotation, LinearVelocity, AngularVelocity, ExternalForce, ExternalTorque, RigidBody, etc.

// Gameplay force pipeline (component-driven) #[derive(Component, Reflect, Serialize, Deserialize)]
pub struct FlightComputer {
pub thrust_max: f32,
pub torque_max: f32,
}

#[derive(Component, Reflect, Serialize, Deserialize)]
pub struct Fuel { pub current: f32, pub capacity: f32 }

All durable gameplay state derives Reflect + Serialize + Deserialize. Runtime-only (e.g. pending control seq) lives in resources or non-reflect components.4. Networking / Protocol Modelrust

// common/src/protocol.rs
use lightyear::prelude::\*;

#[derive(Message, Serialize, Deserialize, Clone, Debug)]
pub struct ClientInput {
pub tick: Tick,
pub actions: Vec<ShipAction>, // thrust, strafe, yaw, etc.
}

#[derive(Message, Serialize, Deserialize, Clone, Debug)]
pub struct ControlRequest {
pub target: Uuid, // NetworkId of desired entity
pub seq: u32,
}

#[derive(Message, Serialize, Deserialize, Clone, Debug)]
pub struct ControlResponse {
pub seq: u32,
pub accepted: bool,
pub target: Uuid,
pub reason: Option<String>,
}

// Registration (in both client & server apps)
app.add_message::<ClientInput>(Channel::UnreliableOrdered);
app.add_message::<ControlRequest>(Channel::ReliableOrdered);
app.add_message::<ControlResponse>(Channel::ReliableOrdered);

Input routing (server): Session -> Player -> CurrentControl.target -> find entity by NetworkId -> write ActionQueue.
Client clears pending ControlRequest only on matching seq ack/reject.
Server rejects if target not owned by the requesting ClientId or does not exist.5. Simulation / Prediction / Interpolation PipelineServer & Client (predicted only) – FixedUpdate (or FixedPreUpdate) Lightyear receive + rollback (built-in, runs before user systems).  
input_apply (server: all clients; client: only With<Predicted> entities) – write to ActionQueue.  
force_pipeline: ActionQueue -> FlightComputer -> (thrust \* forward, torque) -> ExternalForce / ExternalTorque (gated by With<Predicted> || IsServer).  
Avian physics (FixedPostUpdate via lightyear_avian3d::Avian3dPlugin – it sets correct ordering and replication mode for Position/Rotation/velocities).

Client only Predicted entity: full sim + Lightyear rollback/replay on misprediction.  
All other replicated entities: Interpolated marker + Lightyear visual interpolation (lerp in PostUpdate between last two confirmed ticks using overstep).  
lightyear_avian3d writes Confirmed<Position> etc. to the single entity; visual systems read the interpolated values.

Camera / Render
camera_follow runs in PostUpdate.after(avian::SyncPlugin), reads CurrentControl.target → Position (never writes back).Single-writer enforcement All force/torque systems: .run_if(in_predicted_or_server)  
No system ever writes Position/Velocity on Interpolated entities.  
Camera/render systems have no write access to Avian motion components.

6. Persistence / Hydration Pipelinerust

// persistence/src/lib.rs #[derive(Serialize, Deserialize)]
pub struct GraphComponentRecord {
type_name: String,
data: Vec<u8>, // bincode of reflected value
}

#[derive(Serialize, Deserialize)]
pub struct GraphEntityRecord {
id: Uuid,
components: Vec<GraphComponentRecord>,
relations: Vec<(String, Uuid)>, // relation name -> target NetworkId
}

Save (server, timer or on player disconnect): query all entities with NetworkId, reflect each component, walk hierarchy via ChildOf or custom relations, write graph.
Load (deterministic): spawn in NetworkId-sorted order, insert reflected components, build temp NetworkId -> Entity map, then resolve all relations and insert ChildOf etc.
Account/auth identity stored separately; gameplay Player + CurrentControl are persisted on the player entity.7. Interest Management + Visibility StrategyServer resource: SpatialIndex (simple grid hashmap: cell size 100 m, HashMap<IVec2, Vec<Entity>>).
Update: on Position change (or every 5 ticks) → remove/add to cells (O(1) per moving entity). Per-client visibility (runs in FixedUpdate.after(physics), or PostUpdate): Get camera pos = controlled entity Position (XY only).  
Query cells in radius 300 m + buffer (e.g. ±2 cells).  
Always include: owned entities, public/faction exceptions.  
Result → set Lightyear NetworkVisibility or custom per-client replication target (remove Replicate or use visibility filter).

Avoids full scans; scales linearly with moving entities + visible count. Edge buffer prevents pop-in. Server never sends redacted payloads.8. Test Plan + InstrumentationUnit / integration tests (headless Bevy + Lightyear test harness) Two clients connect → each sees own entity (Predicted) and other (Interpolated).  
Client A moves → instant response on A, smooth on B; after 2 s both positions match within 0.01 m.  
Control swap (A requests B’s ship) → ack, prediction switches instantly on A, camera follows new target, no jump.  
Entity moves out of 300 m cull → stops replicating to distant client, re-appears on re-enter.  
Forced desync (inject bad position on client) → rollback restores convergence.  
Reconnect → same NetworkId re-binds to same player entity, no duplicate.

Instrumentation tracing spans on rollback count, cull time, replication bandwidth per client.  
Assert no Position writes on !Predicted entities in CI.  
Determinism test: run server 1000 ticks with fixed seed, save/load, replay inputs → identical final state.

9. Risks and MitigationsPhysics non-determinism (float, WASM vs native): Avian fixed-timestep + identical code + same FixedUpdate rate → mitigated by shared space_game_core. Test cross-platform nightly.  
   Control race / duplicate prediction: sequence numbers + server authoritative ack → client pending cleared only on match.  
   High rollback cost on complex ships: limit prediction window (Lightyear default 10 ticks), cap forces.  
   Interest hot-path: grid ensures O(1) cell lookup; profile with 500 entities / 100 clients.  
   Transform divergence: never write Transform directly; always go through Avian Position + sync plugin.  
   Persistence non-determinism: spawn order by sorted NetworkId; relations resolved after full graph load.

Anti-patterns (DO NOT DO) Setting Transform or Avian Position on client except via prediction rollback.  
Replicating raw Bevy Entity IDs.  
Running Avian physics on Interpolated entities.  
Storing control in ad-hoc resources instead of CurrentControl component on persisted Player.  
Full O(clients·entities) visibility scan.  
Using null for free-roam (always self-control).  
Camera systems writing any simulation component.

10. Optional Starter Code Skeletonrust

// space_game_client/src/lib.rs
pub fn client_app() -> App {
let mut app = App::new();
app.add_plugins((DefaultPlugins, LightyearClientPlugin::new(...), lightyear_avian3d::plugin()));
app.add_plugins(CommonPlugin); // registers reflect, messages
app.add_systems(PreUpdate, (collect_input, send_input).after(Receive));
app.add_systems(FixedUpdate, mark_predicted_controlled.after(Rollback));
app.add_systems(PostUpdate, camera_follow);
app
}

// space_game_core/src/gameplay.rs #[derive(SystemSet, Hash, Debug, PartialEq, Eq, Clone)]
pub enum GameplaySet { InputApply, ForcePipeline, Physics }

pub fn force_pipeline(
mut query: Query<(&ActionQueue, &FlightComputer, &mut ExternalForce, &mut ExternalTorque), With<PredictedOrServer>>,
) { ... }

// Server similar, plus:
app.add_systems(FixedUpdate, update_spatial_index.in_set(GameplaySet::Physics));
app.add_systems(PostUpdate, compute_visibility);

Runbook for 2-client demo (success criteria): cargo run --bin server  
cargo run --bin client -- --id 1 (spawn, move with WASD → instant local ship)  
cargo run --bin client -- --id 2 (see other ship smooth)  
Both clients move independently → own ship instant, remote smooth, positions converge < 0.01 m after 3 s.  
Client 1 sends control request to a second owned ship (if slice extended) → ack, camera jumps to new target, prediction now on new entity, no desync.  
Move one ship 400 m away → distant client stops receiving updates (no pop, re-appears on approach).  
Kill/restart client 1 → reconnects, re-binds to same NetworkId, no duplicate, camera follows last controlled entity.

This slice is production-correct, minimal, and directly extensible to full game scope. Correctness is enforced at the type/system level; scale is baked into the interest pipeline from day one.
