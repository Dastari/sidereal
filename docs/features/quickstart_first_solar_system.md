# Quick Start: Building Your First Solar System

**Goal**: Create a test solar system with 1 star, 2 planets, 1 moon, and an asteroid belt

**Time**: ~1-2 hours of setup, then iterate rapidly

## Prerequisites

- [ ] Sidereal v3 repository cloned
- [ ] PostgreSQL + AGE running (`docker compose up -d postgres`)
- [ ] Rust toolchain installed
- [ ] Basic understanding of the architecture

## Step-by-Step Guide

### Step 1: Define the Components (15 min)

Create the celestial body components in `crates/sidereal-game/src/celestial.rs`:

```rust
// Add to crates/sidereal-game/src/lib.rs
pub mod celestial;

// Create crates/sidereal-game/src/celestial.rs
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug)]
#[reflect(Component)]
pub struct CelestialBody {
    pub body_type: CelestialBodyType,
    pub seed: u32,
    pub system_id: String,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug)]
pub enum CelestialBodyType {
    Star {
        temperature: f32,
        radius: f32,
    },
    Planet {
        planet_type: PlanetType,
        radius: f32,
    },
    Moon {
        radius: f32,
    },
    Asteroid {
        asteroid_type: AsteroidType,
        size: f32,
    },
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug)]
pub enum PlanetType {
    Rocky,
    Desert,
    Lava,
    Ice,
    GasGiant,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug)]
pub enum AsteroidType {
    Rocky,
    Metallic,
    Carbonaceous,
    GemRich,
    Ice,
}

#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug)]
#[reflect(Component)]
pub struct OrbitalElements {
    pub semi_major_axis: f32,
    pub eccentricity: f32,
    pub inclination: f32,
    pub mean_anomaly: f32,
    pub orbital_period: f32,
    pub parent_body_id: Option<Uuid>,
}

// Register with Bevy
pub struct CelestialPlugin;

impl Plugin for CelestialPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CelestialBody>()
           .register_type::<OrbitalElements>();
    }
}
```

### Step 2: Create Test System Definition (10 min)

Create `test_systems/sol_test.json`:

```json
{
  "id": "sol-test-001",
  "name": "Test Sol System",
  "star": {
    "name": "Sol",
    "temperature": 5778,
    "radius": 696000000,
    "seed": 12345
  },
  "planets": [
    {
      "name": "Test Earth",
      "type": "Rocky",
      "radius": 6371000,
      "orbit": {
        "semi_major_axis": 149600000000,
        "eccentricity": 0.01,
        "inclination": 0.0,
        "mean_anomaly": 0.0,
        "period": 31557600
      },
      "seed": 12346,
      "shader_params": {
        "planet_type": 0,
        "crater_density": 0.15,
        "continent_size": 0.6,
        "ocean_level": 0.4,
        "cloud_coverage": 0.5
      },
      "moons": [
        {
          "name": "Test Luna",
          "radius": 1737000,
          "orbit": {
            "semi_major_axis": 384400000,
            "period": 2360592
          },
          "seed": 12347,
          "shader_params": {
            "planet_type": 5,
            "crater_density": 0.85
          }
        }
      ]
    },
    {
      "name": "Test Mars",
      "type": "Desert",
      "radius": 3389500,
      "orbit": {
        "semi_major_axis": 227900000000,
        "period": 59355072
      },
      "seed": 12348,
      "shader_params": {
        "planet_type": 1,
        "crater_density": 0.45
      }
    }
  ],
  "asteroid_belt": {
    "name": "Test Belt",
    "inner_radius": 329000000000,
    "outer_radius": 478700000000,
    "count": 100,
    "seed": 12349
  }
}
```

### Step 3: Manual Database Insert (30 min)

For now, manually insert test entities into the graph:

```sql
-- Connect to database
psql -U sidereal -d sidereal

-- Load AGE
LOAD 'age';
SET search_path = ag_catalog, "$user", public;

-- Create star entity
SELECT * FROM cypher('sidereal', $$
  CREATE (star:Entity:Star {
    entity_id: 'star-sol-test-001',
    name: 'Sol',
    component_data: '{
      "CelestialBody": {
        "body_type": {"Star": {"temperature": 5778, "radius": 696000000}},
        "seed": 12345,
        "system_id": "sol-test-001"
      },
      "PositionM": {"x": 0, "y": 0, "z": 0},
      "MassKg": 1.989e30
    }'
  })
  RETURN star
$$) AS (star agtype);

-- Create Earth
SELECT * FROM cypher('sidereal', $$
  MATCH (star:Entity {entity_id: 'star-sol-test-001'})
  CREATE (earth:Entity:Planet {
    entity_id: 'planet-earth-test-001',
    name: 'Test Earth',
    component_data: '{
      "CelestialBody": {
        "body_type": {"Planet": {"planet_type": "Rocky", "radius": 6371000}},
        "seed": 12346,
        "system_id": "sol-test-001"
      },
      "OrbitalElements": {
        "semi_major_axis": 149600000000,
        "eccentricity": 0.01,
        "inclination": 0.0,
        "mean_anomaly": 0.0,
        "orbital_period": 31557600,
        "parent_body_id": "star-sol-test-001"
      },
      "PositionM": {"x": 149600000000, "y": 0, "z": 0}
    }'
  })
  CREATE (earth)-[:ORBITS]->(star)
  RETURN earth
$$) AS (earth agtype);

-- Add more planets, moons, asteroids...
```

### Step 4: Verify Replication Loads It (10 min)

```bash
# Start replication
cargo run --bin sidereal-replication

# Check logs for hydration
# Should see: "Loaded N celestial entities from graph"

# Query replication state
curl http://localhost:9002/world/entities | jq
```

### Step 5: Add Client Rendering (30 min)

In `crates/sidereal-client/src/celestial_rendering.rs`:

```rust
use bevy::prelude::*;
use sidereal_game::celestial::*;

pub struct CelestialRenderingPlugin;

impl Plugin for CelestialRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_celestial_visuals);
    }
}

fn spawn_celestial_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<(Entity, &CelestialBody), Added<CelestialBody>>,
) {
    for (entity, celestial) in query.iter() {
        info!("Spawning visual for celestial: {:?}", celestial);
        
        match &celestial.body_type {
            CelestialBodyType::Star { radius, temperature } => {
                let mesh = meshes.add(Sphere::new(*radius).mesh().ico(3).unwrap());
                let material = materials.add(StandardMaterial {
                    base_color: star_color(*temperature),
                    emissive: star_color(*temperature) * 10.0,
                    ..default()
                });
                
                commands.entity(entity).insert(PbrBundle {
                    mesh,
                    material,
                    ..default()
                });
            }
            
            CelestialBodyType::Planet { planet_type, radius } => {
                let mesh = meshes.add(Sphere::new(*radius).mesh().ico(4).unwrap());
                let material = materials.add(StandardMaterial {
                    base_color: planet_color(planet_type),
                    ..default()
                });
                
                commands.entity(entity).insert(PbrBundle {
                    mesh,
                    material,
                    ..default()
                });
            }
            
            CelestialBodyType::Moon { radius } => {
                let mesh = meshes.add(Sphere::new(*radius).mesh().ico(3).unwrap());
                let material = materials.add(StandardMaterial {
                    base_color: Color::rgb(0.5, 0.5, 0.5),
                    ..default()
                });
                
                commands.entity(entity).insert(PbrBundle {
                    mesh,
                    material,
                    ..default()
                });
            }
            
            CelestialBodyType::Asteroid { size, .. } => {
                // Use simple sphere for now
                let mesh = meshes.add(Sphere::new(*size * 0.5).mesh().ico(2).unwrap());
                let material = materials.add(StandardMaterial {
                    base_color: Color::rgb(0.4, 0.35, 0.3),
                    ..default()
                });
                
                commands.entity(entity).insert(PbrBundle {
                    mesh,
                    material,
                    ..default()
                });
            }
        }
    }
}

fn star_color(temperature: f32) -> Color {
    // Simplified blackbody color
    if temperature < 3500.0 {
        Color::rgb(1.0, 0.4, 0.2)
    } else if temperature < 5500.0 {
        Color::rgb(1.0, 0.9, 0.7)
    } else if temperature < 7500.0 {
        Color::rgb(1.0, 1.0, 0.9)
    } else {
        Color::rgb(0.8, 0.9, 1.0)
    }
}

fn planet_color(planet_type: &PlanetType) -> Color {
    match planet_type {
        PlanetType::Rocky => Color::rgb(0.3, 0.6, 0.3),
        PlanetType::Desert => Color::rgb(0.8, 0.4, 0.3),
        PlanetType::Lava => Color::rgb(0.8, 0.2, 0.1),
        PlanetType::Ice => Color::rgb(0.85, 0.9, 0.95),
        PlanetType::GasGiant => Color::rgb(0.85, 0.65, 0.45),
    }
}
```

Add to client:
```rust
// In crates/sidereal-client/src/main.rs
app.add_plugins(CelestialRenderingPlugin);
```

### Step 6: Test It! (10 min)

```bash
# Terminal 1: Replication
cargo run --bin sidereal-replication

# Terminal 2: Shard
cargo run --bin sidereal-shard

# Terminal 3: Gateway
cargo run --bin sidereal-gateway

# Terminal 4: Client
cargo run --bin sidereal-client

# Login and you should see your solar system!
```

## What You Should See

1. **Star**: Large glowing sphere at origin (0,0,0)
2. **Earth**: Blue-green sphere orbiting the star
3. **Luna**: Small gray sphere orbiting Earth
4. **Mars**: Red-orange sphere in outer orbit
5. **Asteroids**: Small rocks scattered in belt

## Troubleshooting

### Entities don't load
- Check replication logs for hydration errors
- Verify graph queries return data
- Check component registration

### Nothing renders
- Verify `CelestialRenderingPlugin` is added
- Check entity positions (might be very far away!)
- Scale camera view distance

### Performance issues
- Reduce asteroid count
- Lower mesh subdivisions (ico level)
- Implement LOD system

## Next Steps

1. **Add Orbital Motion**: Update positions based on `OrbitalElements`
2. **Add Procedural Shaders**: Replace `StandardMaterial` with planet shaders
3. **Add Procedural Asteroids**: Generate irregular meshes
4. **Build Orchestrator**: Tool to create systems without SQL
5. **Add System Background**: Wire up space_background shader

## Quick Iterations

Once this works, you can rapidly iterate:

```bash
# Change a planet's appearance
UPDATE sol-test planet seed -> regenerates procedurally

# Add more asteroids
INSERT more asteroid entities with different seeds

# Change orbits
UPDATE orbital elements -> planets move differently

# Add new system
Copy test_system JSON, change IDs, insert to graph
```

## Files Created

```
crates/sidereal-game/src/celestial.rs           # Components
crates/sidereal-client/src/celestial_rendering.rs  # Rendering
test_systems/sol_test.json                      # System definition
docs/features/universe_building_plan.md         # This guide
```

## Timeline Estimate

- **Day 1**: Components + manual DB insertion + basic rendering
- **Day 2**: Orbital motion + better materials
- **Day 3**: Procedural shaders integration
- **Week 2**: Orchestrator tool
- **Week 3**: Multiple systems + transitions

You're now ready to build your first universe! 🌌
