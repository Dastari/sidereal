[← Back to Documentation Index](../README.md) | [Architecture Documentation](./networking-overview.md)

# Sidereal Gameplay Overview

## Game Concept

Sidereal is a massively multiplayer online space simulation game that combines exploration, resource gathering, trading, and combat in a vast procedurally generated universe. Players navigate their ships through space, discover celestial bodies, extract resources, build stations, trade goods, and engage in space combat, all within a persistent, player-influenced galaxy.

## Core Gameplay Pillars

1. **Exploration**: Discovering new star systems, planets, and anomalies.
2. **Resource Harvesting**: Mining asteroids and planets for valuable materials.
3. **Crafting & Building**: Creating ships, stations, and tools.
4. **Trading**: Buying and selling goods across different star systems.
5. **Combat**: Engaging in tactical ship-to-ship combat.
6. **Advancement**: Advancing character skills and ship capabilities.

## Physical Simulation

The game utilizes Rapier physics for realistic space movement and interactions, with the following key elements:

### Ship Movement and Control

Ships move according to Newtonian physics principles, modified slightly for playability:

- Ships generate thrust in the direction they are facing
- Inertia means ships continue moving without constant thrust
- Ships can rotate to change direction
- Velocity is maintained while changing orientation
- Component-based directional thrust system allows for customization

```rust
// Component representing ship thrusters
#[derive(Component)]
pub struct ShipThrusters {
    // Maximum thrust force in Newtons
    pub max_thrust: f32,
    // Current thrust level (0.0 to 1.0)
    pub current_thrust: f32,
    // Thruster positions and directions for torque calculations
    pub thruster_config: Vec<ThrusterMount>,
}

// Individual thruster mounting point
#[derive(Clone)]
pub struct ThrusterMount {
    // Position relative to ship center
    pub position: Vec2,
    // Direction of thrust
    pub direction: Vec2,
    // Maximum force this thruster can apply
    pub max_force: f32,
}

// System for applying thrust to ships based on player input
fn apply_ship_thrust(
    mut query: Query<(&ShipThrusters, &mut Velocity, &Transform)>,
    input: Res<InputState>,
    time: Res<Time>,
) {
    for (thrusters, mut velocity, transform) in query.iter_mut() {
        // Calculate thrust direction in world space
        let thrust_direction = transform.rotation * Vec3::X;

        // Apply thrust force based on input
        if input.thrust > 0.0 {
            let thrust_force = thrusters.max_thrust * input.thrust * thrust_direction;

            // F = ma, so a = F/m (assuming mass of 1.0 for simplicity)
            let acceleration = thrust_force;

            // v = v₀ + at
            velocity.linvel += acceleration.truncate() * time.delta_seconds();
        }

        // Handle rotation input
        if input.rotation != 0.0 {
            velocity.angvel = input.rotation * thrusters.max_rotational_speed;
        }
    }
}
```

### Gravity and Celestial Movement

The game simulates gravity between celestial bodies and ships:

- Planets, stars, and large objects exert gravitational pull
- Ships and smaller objects are affected by gravity
- Orbital mechanics allow for realistic space movement
- N-body simulation for dynamic celestial interactions

```rust
// N-body gravity system implemented using Rapier's force generators
fn n_body_gravity_system(
    mut query: Query<(Entity, &Transform, &MassProperties, &mut Velocity)>,
    planets: Query<(Entity, &Transform, &CelestialBody)>,
    time: Res<Time>,
) {
    // For each entity with physics
    for (entity_a, transform_a, mass_a, mut velocity_a) in query.iter_mut() {
        let pos_a = transform_a.translation.truncate();

        // Calculate gravitational force from all celestial bodies
        for (entity_b, transform_b, celestial) in planets.iter() {
            if entity_a == entity_b {
                continue; // Skip self
            }

            let pos_b = transform_b.translation.truncate();
            let offset = pos_b - pos_a;
            let distance_squared = offset.length_squared();

            if distance_squared < 0.0001 {
                continue; // Avoid division by near-zero
            }

            // Calculate gravitational force: F = G * m1 * m2 / r^2
            let force_magnitude = GRAVITATIONAL_CONSTANT * mass_a.mass * celestial.mass / distance_squared;
            let force_direction = offset.normalize();
            let force = force_direction * force_magnitude;

            // Apply force as acceleration (F = ma, so a = F/m)
            let acceleration = force / mass_a.mass;
            velocity_a.linvel += acceleration * time.delta_seconds();
        }
    }
}
```

### Collision and Interaction

The physics system handles various types of collisions:

- Ship-to-ship collisions can cause damage
- Ship-to-asteroid collisions for mining
- Projectile collisions for combat
- Docking with stations and larger ships

```rust
// Collision event handling
fn handle_collisions(
    mut collision_events: EventReader<CollisionEvent>,
    mut query: Query<(Entity, &mut Health, &RigidBody)>,
) {
    for collision in collision_events.iter() {
        if let CollisionEvent::Started(entity_a, entity_b, _flags) = collision {
            // Handle damage based on collision velocity and object types
        }
    }
}
```

## Resources and Economy

### Resource Types

The game features various resources:

- **Common materials**: Iron, Titanium, Silicon
- **Rare materials**: Platinum, Iridium, Exotic Matter
- **Energy resources**: Hydrogen, Helium-3, Antimatter
- **Manufactured goods**: Electronics, Weapons, Ship parts

### Resource Extraction

Players can extract resources through:

- **Mining**: Using mining equipment on asteroids and planets
- **Harvesting**: Collecting gases from planets or stars
- **Salvaging**: Recovering materials from derelict ships or stations

```rust
// Resource extraction system
fn mining_system(
    mut commands: Commands,
    mut query: Query<(Entity, &Transform, &mut MiningEquipment, &mut Energy)>,
    asteroids: Query<(Entity, &Transform, &mut Asteroid)>,
    input: Res<InputState>,
    time: Res<Time>,
) {
    for (entity, transform, mut equipment, mut energy) in query.iter_mut() {
        if !input.mining_active {
            continue;
        }

        // Check if we have enough energy
        if energy.current < equipment.energy_per_second * time.delta_seconds() {
            continue; // Not enough energy
        }

        // Find closest asteroid in range
        let mining_position = transform.translation;
        let mining_range = equipment.range;

        for (asteroid_entity, asteroid_transform, mut asteroid) in asteroids.iter_mut() {
            let distance = (asteroid_transform.translation - mining_position).length();

            if distance <= mining_range {
                // We can mine this asteroid
                let mining_rate = equipment.mining_rate * time.delta_seconds();
                let extracted = asteroid.extract_resources(mining_rate);

                // Consume energy
                energy.current -= equipment.energy_per_second * time.delta_seconds();

                // Create resource objects
                spawn_extracted_resources(&mut commands, extracted, transform.translation);

                break; // Only mine one asteroid at a time
            }
        }
    }
}
```

### Economy and Trading

The game's economy is dynamic and player-influenced:

- **Supply and Demand**: Prices fluctuate based on local availability
- **Trade Routes**: Establishing efficient paths between systems
- **Markets**: Trading posts and stations where goods can be bought and sold
- **Production**: Creating finished goods from raw materials

## Ship Customization and Progression

### Ship Components

Players can customize their ships with various components:

- **Engines**: Different thrust capabilities, fuel efficiency
- **Power Plants**: Energy generation for systems
- **Weapons**: Offensive capabilities
- **Shields**: Defensive systems
- **Special Equipment**: Mining lasers, scanners, etc.

```rust
// Ship component system
#[derive(Component)]
pub struct Ship {
    pub hull_type: HullType,
    pub power_plant: PowerPlantType,
    pub engine: EngineType,
    pub shield_generator: Option<ShieldGeneratorType>,
    pub weapons: Vec<WeaponMount>,
    pub special_equipment: Vec<SpecialEquipment>,
}

// Power management system
fn power_management_system(
    mut query: Query<(&Ship, &mut Energy, &mut PowerDistribution)>,
    time: Res<Time>,
) {
    for (ship, mut energy, mut power_distribution) in query.iter_mut() {
        // Calculate base power generation from the ship's power plant
        let power_generation = get_power_generation(ship.power_plant) * time.delta_seconds();

        // Add power
        energy.current = (energy.current + power_generation).min(energy.maximum);

        // Distribute power to systems based on player settings
        distribute_power(&ship, &mut energy, &power_distribution);
    }
}
```

### Progression Systems

Players advance through:

- **Ship Upgrades**: Improving components and capabilities
- **Skill Development**: Becoming more proficient in various activities
- **Reputation**: Building standing with different factions
- **Wealth**: Accumulating resources and credits

## Combat System

### Weapon Types

Various weapon systems are available:

- **Energy Weapons**: Lasers, plasma cannons
- **Projectile Weapons**: Railguns, missile launchers
- **Defensive Systems**: Point defense, ECM

### Tactical Combat

Combat is skill-based and tactical:

- **Positioning**: Maneuvering for tactical advantage
- **Energy Management**: Balancing weapons, shields, and engines
- **Target Selection**: Focusing fire on specific ship systems
- **Team Coordination**: Working with allies for combined effectiveness

```rust
// Weapon firing system
fn weapon_firing_system(
    mut commands: Commands,
    mut query: Query<(Entity, &Transform, &mut Ship, &mut Energy)>,
    input: Res<InputState>,
    time: Res<Time>,
) {
    for (entity, transform, mut ship, mut energy) in query.iter_mut() {
        // Check if weapons are being fired
        if input.fire_primary {
            // For each weapon that can fire
            for weapon in ship.weapons.iter_mut().filter(|w| w.is_primary && w.cooldown <= 0.0) {
                // Check if we have enough energy
                if energy.current >= weapon.energy_cost {
                    // Spawn projectile or effect
                    let weapon_transform = calculate_weapon_world_transform(transform, &weapon.mount_position);

                    commands.spawn(ProjectileBundle::new(
                        weapon.projectile_type,
                        weapon_transform.translation,
                        weapon_transform.rotation,
                        entity, // Source entity
                    ));

                    // Consume energy
                    energy.current -= weapon.energy_cost;

                    // Set cooldown
                    weapon.cooldown = weapon.base_cooldown;
                }
            }
        }

        // Update weapon cooldowns
        for weapon in ship.weapons.iter_mut() {
            weapon.cooldown = (weapon.cooldown - time.delta_seconds()).max(0.0);
        }
    }
}
```

## Multiplayer and Social Systems

### Player Interaction

Players can interact in various ways:

- **Cooperative Activities**: Mining, trading, exploration
- **Competitive Activities**: Racing, combat, resource control
- **Communication**: Chat, voice (future)
- **Organizations**: Forming corporations or alliances

### Factions and Influence

The game world has various factions:

- **Major Powers**: Established civilizations with territory
- **Corporations**: Business entities with economic interests
- **Independent Groups**: Pirates, rebels, fringe organizations
- **Player Organizations**: Groups formed by players

## Technical Implementation Considerations

The gameplay systems described above are designed to work with the Sidereal architecture:

1. **World Partitioning Integration**:

   - Physics simulation is handled by individual shard servers
   - Entity interactions span sector boundaries using the shadow entity system
   - The replication server coordinates cross-shard activities and persistence

2. **Performance Considerations**:

   - Physics simulations scale with entity count and interaction complexity
   - N-body calculations can be optimized for distant objects
   - Relevance filtering means players only receive updates about nearby entities

3. **Networking Requirements**:
   - Combat and movement require low-latency updates
   - Economic systems can use less frequent synchronization
   - Large-scale events may need special handling for many concurrent players

## Implementation Strategy

The gameplay features will be implemented in phases:

1. **Phase 1**: Basic movement, simple resource gathering, primitive combat
2. **Phase 2**: Enhanced physics, basic economy, improved combat
3. **Phase 3**: Advanced features, full economy, complex interactions
4. **Phase 4**: Optimization, balance adjustments, content expansion

## Next Steps

The immediate focus is on:

1. Completing the core physics implementation
2. Basic ship control and movement
3. Initial resource extraction mechanics
4. Simplified combat system
5. Persistence of player ships and inventories
