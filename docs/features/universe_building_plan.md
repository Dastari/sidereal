# Universe/World Building Plan for Sidereal

**Status**: Design Proposal  
**Date**: 2026-02-19  
**Audience**: Developers and content creators building the game universe

## Overview

This document outlines the architecture and workflow for building the Sidereal universe - from creating solar systems with planets, moons, and asteroid belts to streaming them to clients.

## Current Architecture Analysis

### What We Have

**Services**:
- `sidereal-gateway`: Auth, account management
- `sidereal-replication`: Read-model, persistence, client state distribution
- `sidereal-shard`: Authoritative simulation (physics, entities)
- `sidereal-orchestrator`: **Empty scaffold** - perfect for universe management!

**Data Flow**:
```
Graph DB (PostgreSQL + AGE)
    ↓
[Replication] - Hydrates world state on startup
    ↓
[Shard] - Simulates entities (ships, physics)
    ↓
[Replication] - Filters visibility, distributes to clients
    ↓
[Client] - Renders with procedural shaders
```

**Key Capabilities**:
- ✅ Graph persistence for entities/components
- ✅ Component reflection + serialization
- ✅ Deterministic procedural shaders (planets, asteroids, space backgrounds)
- ✅ Asset streaming (models, shaders)
- ❌ **No universe authoring tools yet**
- ❌ **No solar system/celestial body spawning system**

## The Problem

Currently:
1. **No way to author solar systems** - need to define planets, orbits, asteroid belts
2. **No celestial body entities** - planets/moons don't exist in ECS yet
3. **Shaders exist but no integration** - procedural shaders ready but not wired to entities
4. **No procedural mesh generation** - asteroids need irregular meshes
5. **No streaming strategy** - how do clients get planet meshes/materials?

## Proposed Solution: Three-Tier Universe Architecture

### Tier 1: Universe Definition (Orchestrator)

**Purpose**: Author and manage universe structure  
**Tool**: `sidereal-orchestrator` becomes the universe management service

**Responsibilities**:
1. Define solar systems (star, planets, moons, asteroid belts)
2. Generate deterministic seeds for all celestial bodies
3. Create entity definitions with components
4. Persist to graph database
5. Provide universe editor/admin API

### Tier 2: Runtime Simulation (Shard + Replication)

**Purpose**: Simulate and distribute universe state  

**Responsibilities**:
1. Load celestial body entities from graph
2. Simulate orbital mechanics (simplified or Kepler)
3. Handle client visibility of celestial objects
4. Stream entity metadata to clients

### Tier 3: Client Rendering (Client)

**Purpose**: Procedurally render celestial bodies  

**Responsibilities**:
1. Receive entity metadata (type, seed, position, size)
2. Generate meshes procedurally (asteroids)
3. Apply procedural materials (planet/asteroid shaders)
4. Render with LOD based on distance

## Detailed Design

### Phase 1: Define Core Components

New ECS components needed in `crates/sidereal-game`:

```rust
// Celestial body identification
#[derive(Component, Reflect, Serialize, Deserialize)]
pub struct CelestialBody {
    pub body_type: CelestialBodyType,
    pub seed: u32,
    pub system_id: String,
}

#[derive(Reflect, Serialize, Deserialize)]
pub enum CelestialBodyType {
    Star { temperature: f32, radius: f32 },
    Planet { planet_type: PlanetType, radius: f32 },
    Moon { radius: f32 },
    AsteroidBelt { inner_radius: f32, outer_radius: f32, density: f32 },
    Asteroid { asteroid_type: AsteroidType, size: f32 },
}

// Orbital mechanics (simplified Kepler elements)
#[derive(Component, Reflect, Serialize, Deserialize)]
pub struct OrbitalElements {
    pub semi_major_axis: f32,        // Orbit size (meters)
    pub eccentricity: f32,            // Orbit shape (0=circle, <1=ellipse)
    pub inclination: f32,             // Orbit tilt (radians)
    pub longitude_of_ascending_node: f32,
    pub argument_of_periapsis: f32,
    pub mean_anomaly_at_epoch: f32,   // Starting position
    pub orbital_period: f32,          // Seconds for one orbit
    pub parent_body_id: Option<Uuid>, // What we orbit around
}

// Visual/shader parameters
#[derive(Component, Reflect, Serialize, Deserialize)]
pub struct ProceduralMaterial {
    pub shader_id: String,            // "planet_core", "asteroid", etc.
    pub parameters: HashMap<String, ShaderParamValue>,
}

#[derive(Reflect, Serialize, Deserialize)]
pub enum ShaderParamValue {
    Float(f32),
    Vec3(Vec3),
    Vec4(Vec4),
    UInt(u32),
}

// Render metadata
#[derive(Component, Reflect, Serialize, Deserialize)]
pub struct RenderMetadata {
    pub mesh_type: MeshType,
    pub lod_distances: Vec<f32>,      // When to switch LOD levels
}

#[derive(Reflect, Serialize, Deserialize)]
pub enum MeshType {
    Sphere { radius: f32, subdivisions: u32 },
    ProceduralAsteroid { seed: u32, base_radius: f32, detail: u32 },
    AssetReference { asset_id: String },
}

// Background environment per system
#[derive(Component, Reflect, Serialize, Deserialize)]
pub struct SystemBackground {
    pub space_shader_params: SpaceBackgroundParams,
}
```

### Phase 2: Orchestrator - Universe Editor

Transform `sidereal-orchestrator` into a universe authoring tool:

```rust
// bins/sidereal-orchestrator/src/main.rs

use sidereal_game::*;
use sidereal_persistence::*;

struct UniverseOrchestrator {
    db: DbPool,
    systems: HashMap<String, SolarSystem>,
}

struct SolarSystem {
    id: String,
    name: String,
    star: StarDefinition,
    planets: Vec<PlanetDefinition>,
    asteroid_belts: Vec<AsteroidBeltDefinition>,
    background: SpaceBackgroundParams,
}

impl UniverseOrchestrator {
    /// Create a new solar system
    pub async fn create_solar_system(
        &mut self,
        definition: SolarSystemDefinition,
    ) -> Result<String> {
        // 1. Generate deterministic seeds
        let system_seed = hash_string(&definition.name);
        
        // 2. Create star entity
        let star_entity = self.create_star_entity(
            &definition.star,
            system_seed,
        );
        
        // 3. Create planets with orbits
        let mut planet_entities = Vec::new();
        for (i, planet_def) in definition.planets.iter().enumerate() {
            let planet_seed = system_seed.wrapping_add(i as u32 * 1000);
            let planet = self.create_planet_entity(
                planet_def,
                planet_seed,
                star_entity.guid,
            );
            planet_entities.push(planet);
            
            // 3a. Create moons
            for (j, moon_def) in planet_def.moons.iter().enumerate() {
                let moon_seed = planet_seed.wrapping_add(j as u32 * 100);
                self.create_moon_entity(
                    moon_def,
                    moon_seed,
                    planet.guid,
                );
            }
        }
        
        // 4. Create asteroid belts
        for (i, belt_def) in definition.asteroid_belts.iter().enumerate() {
            let belt_seed = system_seed.wrapping_add(10000 + i as u32 * 1000);
            self.create_asteroid_belt(
                belt_def,
                belt_seed,
                star_entity.guid,
            );
        }
        
        // 5. Create system background environment
        self.create_system_background(
            definition.background,
            system_seed,
        );
        
        // 6. Persist all to graph
        self.persist_system_to_graph(&definition.id).await?;
        
        Ok(definition.id)
    }
    
    /// Generate asteroid belt entities
    fn create_asteroid_belt(
        &mut self,
        belt_def: &AsteroidBeltDefinition,
        seed: u32,
        star_id: Uuid,
    ) {
        // Generate N asteroids in belt
        let count = (belt_def.density * 1000.0) as usize;
        
        for i in 0..count {
            let asteroid_seed = seed.wrapping_add(i as u32);
            let rng = SeededRng::new(asteroid_seed);
            
            // Random orbit within belt bounds
            let orbit_radius = rng.range(
                belt_def.inner_radius,
                belt_def.outer_radius
            );
            
            let asteroid = EntityDefinition {
                guid: Uuid::new_v4(),
                components: vec![
                    // Core identity
                    CelestialBody {
                        body_type: CelestialBodyType::Asteroid {
                            asteroid_type: belt_def.predominant_type,
                            size: rng.range(10.0, 100.0), // meters
                        },
                        seed: asteroid_seed,
                        system_id: belt_def.system_id.clone(),
                    },
                    
                    // Orbital mechanics
                    OrbitalElements {
                        semi_major_axis: orbit_radius,
                        eccentricity: rng.range(0.0, 0.1),
                        inclination: rng.range(-0.1, 0.1),
                        mean_anomaly_at_epoch: rng.range(0.0, TAU),
                        orbital_period: calculate_period(orbit_radius, star_mass),
                        parent_body_id: Some(star_id),
                        ..Default::default()
                    },
                    
                    // Position (initial)
                    PositionM(calculate_orbital_position(/* ... */)),
                    
                    // Physical properties
                    MassKg(calculate_asteroid_mass(size)),
                    SizeM { length: size, width: size, height: size },
                    
                    // Collision
                    CollisionAabbM { half_extents: Vec3::splat(size * 0.5) },
                    
                    // Rendering
                    ProceduralMaterial {
                        shader_id: "asteroid".to_string(),
                        parameters: generate_asteroid_params(
                            asteroid_seed,
                            belt_def.mineral_richness,
                        ),
                    },
                    RenderMetadata {
                        mesh_type: MeshType::ProceduralAsteroid {
                            seed: asteroid_seed,
                            base_radius: size * 0.5,
                            detail: 3, // ico subdivision level
                        },
                        lod_distances: vec![500.0, 2000.0, 10000.0],
                    },
                    
                    // Authority
                    ShardAssignment(1), // Assign to shard
                    OwnerKind::World,
                ],
            };
            
            self.entities.push(asteroid);
        }
    }
}
```

#### Orchestrator API Endpoints

```rust
// REST API for universe authoring
#[post("/systems")]
async fn create_system(definition: Json<SolarSystemDefinition>) -> Result<Json<String>>;

#[get("/systems/{system_id}")]
async fn get_system(system_id: Path<String>) -> Result<Json<SolarSystem>>;

#[put("/systems/{system_id}")]
async fn update_system(system_id: Path<String>, definition: Json<SolarSystemDefinition>) -> Result<()>;

#[delete("/systems/{system_id}")]
async fn delete_system(system_id: Path<String>) -> Result<()>;

#[post("/systems/{system_id}/asteroids/generate")]
async fn regenerate_asteroids(system_id: Path<String>, params: Json<BeltParams>) -> Result<()>;

// Export/import for version control
#[get("/systems/{system_id}/export")]
async fn export_system(system_id: Path<String>) -> Result<Json<SystemExport>>;

#[post("/systems/import")]
async fn import_system(export: Json<SystemExport>) -> Result<()>;
```

### Phase 3: Client-Side Procedural Generation

Client receives entity metadata and generates visuals:

```rust
// crates/sidereal-client/src/rendering/celestial.rs

/// System that spawns visual representations of celestial bodies
pub fn spawn_celestial_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PlanetMaterial>>,
    query: Query<(Entity, &CelestialBody, &ProceduralMaterial, &RenderMetadata), Added<CelestialBody>>,
) {
    for (entity, celestial, proc_mat, render_meta) in query.iter() {
        match &celestial.body_type {
            CelestialBodyType::Planet { planet_type, radius } => {
                spawn_planet_visual(
                    &mut commands,
                    entity,
                    *planet_type,
                    *radius,
                    celestial.seed,
                    proc_mat,
                    &mut meshes,
                    &mut materials,
                );
            }
            CelestialBodyType::Asteroid { size, .. } => {
                spawn_asteroid_visual(
                    &mut commands,
                    entity,
                    *size,
                    celestial.seed,
                    proc_mat,
                    render_meta,
                    &mut meshes,
                    &mut materials,
                );
            }
            // ... other types
        }
    }
}

fn spawn_asteroid_visual(
    commands: &mut Commands,
    entity: Entity,
    size: f32,
    seed: u32,
    proc_mat: &ProceduralMaterial,
    render_meta: &RenderMetadata,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<AsteroidMaterial>>,
) {
    // Generate procedural mesh
    let mesh = match &render_meta.mesh_type {
        MeshType::ProceduralAsteroid { seed, base_radius, detail } => {
            let generator = AsteroidMeshGenerator::new(*seed);
            generator.generate(*base_radius, *detail)
        }
        _ => Sphere::new(size).mesh().ico(3).unwrap(),
    };
    
    let mesh_handle = meshes.add(mesh);
    
    // Create material from shader parameters
    let material = materials.add(AsteroidMaterial {
        params: deserialize_asteroid_params(proc_mat),
    });
    
    // Attach visual to entity
    commands.entity(entity).insert(MaterialMeshBundle {
        mesh: mesh_handle,
        material,
        ..default()
    });
}
```

### Phase 4: LOD System

Clients only render what's visible with appropriate detail:

```rust
/// LOD management based on distance
pub fn update_celestial_lod(
    camera: Query<&Transform, With<Camera>>,
    mut celestials: Query<(
        &Transform,
        &RenderMetadata,
        &mut Handle<Mesh>,
        &CelestialBody,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let camera_pos = camera.single().translation;
    
    for (transform, metadata, mut mesh_handle, celestial) in celestials.iter_mut() {
        let distance = transform.translation.distance(camera_pos);
        
        // Determine LOD level
        let lod_level = metadata.lod_distances
            .iter()
            .position(|&d| distance < d)
            .unwrap_or(metadata.lod_distances.len());
        
        // Regenerate mesh at appropriate detail
        match &metadata.mesh_type {
            MeshType::ProceduralAsteroid { seed, base_radius, .. } => {
                let detail = match lod_level {
                    0 => 5,  // Near: high detail
                    1 => 3,  // Medium
                    2 => 2,  // Far: low detail
                    _ => 1,  // Very far: billboard or cull
                };
                
                if lod_level < 3 {
                    let generator = AsteroidMeshGenerator::new(*seed);
                    let new_mesh = generator.generate(*base_radius, detail);
                    *mesh_handle = meshes.add(new_mesh);
                } else {
                    // Too far - could remove mesh entirely or use billboard
                }
            }
            _ => {}
        }
    }
}
```

## Data Flow Summary

### Development Time (Orchestrator)
```
Developer/Tool
    ↓
[Orchestrator UI/API]
    ↓
Define Solar System:
  - Star (type, size, temperature)
  - Planets (type, size, orbit, shader params)
  - Moons (orbit around planets)
  - Asteroid Belts (density, mineral types)
  - System Background (nebula colors, stars)
    ↓
Generate Seeds & Entity Definitions
    ↓
[Graph Database] - Persist all entities with components
```

### Runtime (Shard/Replication)
```
[Graph DB] - Load celestial entities
    ↓
[Replication] - Hydrate celestial bodies as ECS entities
    ↓
[Shard] - Simulate orbital mechanics (update positions)
    ↓
[Replication] - Filter visibility (only send nearby celestials)
    ↓
[Client] - Receive entity metadata
```

### Client Rendering
```
Receive: {
  entity_id,
  celestial_type,
  seed,
  position,
  shader_params
}
    ↓
Generate Mesh Procedurally:
  - Planets: Sphere mesh
  - Asteroids: Irregular mesh from seed
    ↓
Apply Procedural Shader:
  - planet_core.wgsl with params
  - asteroid.wgsl with mineral params
    ↓
Render with LOD
```

## Implementation Phases

### Phase 0: Foundations (Week 1)
- [ ] Define celestial body components in `sidereal-game`
- [ ] Add orbital mechanics components
- [ ] Add procedural material parameter storage
- [ ] Add mesh generation components
- [ ] Update persistence to handle new components

### Phase 1: Test Solar System (Week 2)
- [ ] Create hardcoded test system definition
- [ ] Manually insert into graph database
- [ ] Verify replication hydrates entities
- [ ] Verify shard loads entities
- [ ] Test basic orbital position updates

### Phase 2: Client Rendering (Week 3)
- [ ] Add celestial visual spawning system
- [ ] Implement planet sphere generation
- [ ] Implement asteroid procedural mesh generation
- [ ] Wire up shader materials
- [ ] Test LOD system

### Phase 3: Orchestrator Tool (Week 4-5)
- [ ] Build orchestrator service
- [ ] Add REST API for system authoring
- [ ] Add solar system creation logic
- [ ] Add asteroid belt generation
- [ ] Add export/import for version control
- [ ] Create simple web UI or CLI tool

### Phase 4: Full Integration (Week 6)
- [ ] Create multiple test systems
- [ ] Test client transitions between systems
- [ ] Optimize visibility filtering
- [ ] Add system background shader integration
- [ ] Performance testing with many asteroids

## Example: Creating Test Solar System

### Definition File (YAML/JSON)
```yaml
system:
  name: "Sol System"
  id: "sol-001"
  
  star:
    type: main_sequence
    temperature: 5778  # Kelvin (Sun)
    radius: 696000000  # meters
    mass: 1.989e30     # kg
    
  planets:
    - name: "Earth"
      type: rocky
      radius: 6371000  # meters
      mass: 5.972e24   # kg
      orbit:
        semi_major_axis: 149600000000  # 1 AU
        eccentricity: 0.0167
        inclination: 0.0
        period: 31557600  # 1 year in seconds
      shader_params:
        crater_density: 0.15
        continent_size: 0.6
        ocean_level: 0.4
        cloud_coverage: 0.5
        atmosphere_thickness: 0.3
        ice_cap_size: 0.2
        city_lights: 0.3
      moons:
        - name: "Luna"
          radius: 1737000
          orbit:
            semi_major_axis: 384400000
            period: 2360592  # ~27.3 days
          shader_params:
            crater_density: 0.85
            
    - name: "Mars"
      type: desert
      radius: 3389500
      orbit:
        semi_major_axis: 227900000000  # 1.52 AU
        period: 59355072  # ~1.88 years
      shader_params:
        crater_density: 0.45
        color_primary: [0.8, 0.4, 0.3]
        
  asteroid_belts:
    - name: "Main Belt"
      inner_radius: 329000000000   # 2.2 AU
      outer_radius: 478700000000   # 3.2 AU
      density: 0.3
      count: 5000  # How many asteroids to generate
      predominant_type: rocky
      mineral_richness: 0.4
      
  background:
    environment_type: moderate
    nebula_density: 0.3
    nebula_colors:
      - [0.2, 0.3, 0.6]  # Blue
      - [0.4, 0.2, 0.5]  # Purple
    star_density: 0.6
```

### Loading via Orchestrator CLI
```bash
# Start orchestrator
cargo run --bin sidereal-orchestrator

# Import system definition
curl -X POST http://localhost:9000/systems/import \
  -H "Content-Type: application/json" \
  -d @sol_system.json

# Verify created
curl http://localhost:9000/systems/sol-001 | jq

# Regenerate asteroids with different seed
curl -X POST http://localhost:9000/systems/sol-001/asteroids/generate \
  -d '{"belt_id": "main_belt", "seed": 12345, "count": 10000}'
```

## Advantages of This Approach

### ✅ Deterministic
- Same seed = same asteroid/planet appearance
- Reproducible across clients
- No need to store mesh data

### ✅ Scalable
- Thousands of asteroids = just metadata
- Client generates meshes on-demand
- LOD reduces detail for distant objects

### ✅ Authorable
- Developers use tools (orchestrator)
- Version control system definitions (YAML/JSON)
- Easy to iterate and tweak

### ✅ Network Efficient
- Only send: entity_id, seed, position, type
- Client does heavy lifting (mesh gen, shaders)
- Visibility filtering = less data

### ✅ Extensible
- Add new celestial types easily
- New shaders = just parameter changes
- Future: planetary features, rings, etc.

## Questions to Resolve

1. **Orbital Simulation**: Full Kepler or simplified circular orbits?
2. **Persistence Frequency**: How often save celestial positions?
3. **Authority**: Do celestial bodies need authoritative physics or just kinematic?
4. **Collisions**: Can ships collide with planets/asteroids or pass through?
5. **Mining**: How do we handle asteroid destruction/mining?
6. **System Transitions**: Warp gates? Seamless transition? Loading screens?

## Next Steps

**Immediate (This Week)**:
1. Define celestial body components in `sidereal-game`
2. Create single hardcoded test solar system
3. Insert into graph manually
4. Verify it loads in shard/replication

**Short Term (Next 2 Weeks)**:
1. Implement client-side procedural spawning
2. Wire up planet and asteroid shaders
3. Test LOD system

**Medium Term (Next Month)**:
1. Build orchestrator service
2. Create authoring UI/CLI
3. Generate multiple test systems

**Long Term (Future)**:
1. Web-based universe editor
2. In-game system transitions
3. Dynamic events (asteroid impacts, supernovas)
4. Player-built structures in systems

---

**This plan makes `sidereal-orchestrator` the universe authoring tool** while keeping the existing graph→replication→shard→client flow intact. The key insight is that **clients generate visuals procedurally from minimal metadata**, making the entire system scalable and deterministic.

Ready to start with Phase 0?
