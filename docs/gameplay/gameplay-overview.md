[← Back to Documentation Index](../README.md) | [Design Phases](./design-phases.md) | [Game World Partitioning](../architecture/game-world.md) | [Game Entity System](../architecture/game-entities.md)

# Sidereal: Gameplay Overview

## Project Overview

Sidereal is a massively multiplayer 2D top-down action ARPG set in a vast and dynamic space universe. Players navigate this universe through a web browser, embarking on various adventures as they pilot customizable ships, establish space stations, engage in combat, trade resources, explore unknown regions, and interact with both NPCs and other players.

The game combines the depth of complex space simulation games with the accessibility of browser-based play, allowing for a rich gaming experience without requiring specialized hardware or software installation. The top-down 2D perspective offers clarity in tactical situations while still allowing for complex gameplay mechanics and visually engaging environments.

Sidereal aims to create a living, breathing universe where player actions have meaningful consequences, economies evolve based on supply and demand, factions rise and fall based on player allegiances, and unexplored regions offer adventure and opportunity for those brave enough to venture into them.

## Core Game Systems

### Ship & Station Construction

- **Grid-Based Hull System**: Ships and stations are constructed from a hull grid where components are placed
- **Directional Component Placement**:
  - Components are placed facing one of four directions (Port, Starboard, Fore, Aft)
  - Directional components function based on their orientation (e.g., thrusters push in the opposite direction they face)
  - Tactical placement of directional components creates unique ship capabilities
- **Dynamic Ship Stats**: All ship capabilities are emergent properties of installed components:
  - **Power Generation**: Reactors provide energy for all systems, with total ship power being the sum of all reactor outputs
  - **Shield Protection**: Shield generators create directional protection fields, with overlapping generators providing enhanced coverage
  - **Weapon Systems**: Weapon mounts have firing arcs and energy requirements based on placement and type
  - **Propulsion**: Thrusters generate force in the opposite direction they face, requiring balanced placement for optimal handling
  - **Mass Distribution**: Each component adds to the ship's total mass, affecting maneuverability and inertia
- **Advanced Physics Simulation**:
  - **Realistic Thruster Mechanics**: Ship movement depends on thruster placement and orientation
  - **Rotational Control**: Turning requires thrusters placed at appropriate positions to create torque
  - **Center of Mass**: Component placement affects the ship's center of mass, impacting handling characteristics
  - **Moment of Inertia**: Mass distribution determines how easily a ship rotates
  - **Realistic Vector Physics**: Ships follow Newtonian physics, with forces and torques calculated in real-time
  - **Rapier Physics Integration**: Physical properties are continuously updated and synchronized with the physics engine
- **Ship Handling Characteristics**:
  - **Thrust-to-Weight Ratio**: Heavier ships with fewer thrusters accelerate more slowly
  - **Rotational Agility**: Thrusters placed further from the center of mass provide more efficient rotation
  - **Lateral Movement**: Sideways thrusters enable strafing maneuvers
  - **Balanced Designs**: Well-balanced ships have symmetric thruster placement for predictable handling
  - **Specialized Designs**: Asymmetric thruster layouts can create ships with unique movement capabilities
- **Component Interactions**:
  - Power distribution systems determine which components receive energy when demand exceeds supply
  - Heat management requires proper component spacing and cooling systems
  - Damage to specific grid sections affects the components installed there
  - Energy-intensive components can cause power fluctuations during peak usage
  - Thruster failure can cause unbalanced flight characteristics and rotation
- **Hull Size Variety**: Multiple hull sizes determine the overall capacity for components
- **Component Types**:
  - Reactors (power generation with heat output)
  - Armor plating (directional protection)
  - Weapons (beam weapons, projectiles, missiles)
  - Shield generators (directional energy fields)
  - Tractor beams
  - Jump drives
  - Power distribution nodes
  - Cargo bays (affecting total capacity and access speed)
  - Hangar bays
  - Sensor arrays (with directional sensitivity)
  - Engines and thrusters (vectored thrust based on placement)
  - Life support systems
  - Electronic warfare modules
  - Cooling systems (heat management)
  - Resource harvesters
- **Thruster Types**:
  - **Main Engines**: High-powered but fixed direction, typically aft-facing for forward thrust
  - **Maneuvering Thrusters**: Lower power but essential for rotation and lateral movement
  - **Vectored Thrusters**: Can adjust thrust direction slightly for versatile movement
  - **Omnidirectional Thrusters**: Equal thrust in all directions but less efficient
- **Visual Customization**: Color schemes, decals, insignias, and ship naming
- **Ship Classes**: Hull templates optimized for specific roles (combat, mining, transport, etc.)
- **Component Variations**: Components with different stats, strengths, and weaknesses
- **Blueprint System**: Save and share ship designs with the community

#### Sample Ship Design (Work-in-Progress)

Below is a sample ship design using an 11x11 component grid, demonstrating how the various components can be placed to create a functional ship with balanced capabilities:

```
   0123456789A
  +-----------+
0 |....A^A^...|  SHIP DESIGN: "BALANCED CRUISER"
1 |..A^A^A^A^.|
2 |.A<WW^WW>A.|  C = Command Center
3 |TA<R>C<R>AT|  R = Reactor (500 power each)
4 |TA<R>C<R>AT|  W = Pulse Cannon (requires 300 power)
5 |TA<R>C<R>AT|  E = Main Engine (high forward thrust)
6 |TA<R>C<R>AT|  T = Maneuvering Thruster (rotation/lateral)
7 |.A<RR^RR>A.|  A = Armor Plating (directional protection)
8 |.A<WW^WW>A.|  S = Shield Generator
9 |..A<EE>A...|
A |....EEvv...|  Ship Properties:
  +-----------+  - Balanced thrust in all directions
                 - Strong forward firepower
                 - Symmetrical mass distribution
                 - Good rotational capability
```

This design illustrates several key concepts of the ship construction system:

- Central command center with components arranged around it
- Directional components (arrows indicate facing direction)
- Balanced thruster placement for good handling
- Symmetrical design for predictable flight characteristics
- Armor placement protecting vital systems
- Forward-facing weapons for optimal firing arcs

The symbols represent the direction each component is facing:

- `^` = Component facing Fore (forward)
- `v` = Component facing Aft (backward)
- `<` = Component facing Port (left)
- `>` = Component facing Starboard (right)

### Inventory & Item System

- **Grid-Based Inventory**: Items occupy specific slots in a grid pattern
- **Multi-Slot Items**: Larger items consume multiple grid spaces in specific patterns
- **Inventory Types**:
  - **Ship Cargo**: Large capacity but limited accessibility during combat
  - **Active Storage**: Smaller capacity but items usable during gameplay
  - **Station Storage**: Massive capacity for base operations
  - **Container Storage**: Physical crates that can be moved and traded
  - **Personal Inventory**: Small capacity for character items
- **Item Quality**: Variable grades affecting performance and value
- **Physical Representation**: Items can exist as physical entities in space:
  - Resource deposits appear as asteroids or debris fields
  - Cargo manifests as containers when dropped or ejected
  - Equipment appears as salvageable wreckage after ship destruction
  - Trade goods are represented by standardized container types
- **Non-Physical Items**: Abstract representation in inventory:
  - Data (maps, coordinates, blueprints)
  - Licenses and permits
  - Virtual currencies and reputation tokens
- **Item Stacking**: Similar items can be combined up to a maximum stack size
- **Decay & Condition**: Item deterioration based on usage and damage
- **Transferability**: Rules for which items can be traded, sold, or salvaged

### Universe & Exploration

- **2D Physics System**: Realistic collision, mass, and inertia
- **Gravity Wells**: Stars, planets, and other celestial bodies exert gravitational pull
- **Jump Points Network**: Established routes for fast travel between distant regions
- **Wormhole Generation**: Larger ships can create temporary jump points
- **Fog of War**: Limited visibility based on sensor range
- **Sensor Networks**: Deployable towers to monitor regions of space
- **Stealth Systems**: Technology to avoid detection by other players and NPCs
- **Exploration Mechanics**: Rewards for discovering new areas and phenomena
- **Space Phenomena**: Black holes, nebulae, asteroid fields, radiation zones
- **Weather Events**: Solar flares, ion storms, and other hazards
- **Procedural Generation**: Dynamically created regions, missions, and encounters
- **Wormhole Exploration**: Temporary connections to unknown or isolated regions

### Combat

- **Top-Down Combat**: Real-time tactical battles with skill-based aiming
- **Diverse Weapon Systems**: Energy, ballistic, missile, and specialized weapons
- **Damage Models**: Component-based damage affecting ship functionality
- **Shield Management**: Directional shielding that requires tactical decisions
- **Electronic Warfare**: Jamming, hacking, and disabling enemy systems
- **Boarding Actions**: Taking over enemy ships through targeted operations
- **PvP Flagging System**: Controlled combat zones vs. safe regions
- **Fleet Combat**: Coordinate attacks with allies for tactical advantage
- **Escape Mechanisms**: Pods and emergency systems when ships are destroyed
- **Combat Roles**: Specialized functions within group combat (support, DPS, tank)

### Economic Systems

- **Resource Gathering**: Mining, harvesting, and collection of raw materials
- **Production Chains**: Processing raw materials into components and finished goods
- **Player-Driven Economy**: Prices determined by supply and demand
- **Trading System**: Buy, sell, and transport goods between markets
- **Market Hubs**: Centralized trading locations with dynamic pricing
- **Player-to-Player Trading**: Direct exchange interface with security features
- **Contract System**: Create and fulfill delivery, mining, or combat contracts
- **Insurance**: Protect investments against loss through premium payments
- **Salvage Mechanics**: Recover valuable resources from wrecks and debris
- **Factory Construction**: Build production facilities to process resources
- **Blueprint System**: Discover and use plans for crafting items and ships
- **Resource Management**: Storage, transportation, and inventory systems

### Progression Systems

- **Character Skills**: Personal abilities that develop over time
- **Technology Research**: Unlock new components and capabilities
- **Reputation System**: Standing with various factions affects gameplay options
- **Faction Alignment**: Join major powers and influence their standing
- **Career Paths**: Specialization as trader, explorer, pirate, bounty hunter, etc.
- **Achievement System**: Recognition for accomplishments with tangible rewards
- **Prestige Mechanics**: End-game systems for continued progression
- **Ship Mastery**: Bonuses for experience with specific vessel types
- **Blueprint Collection**: Gather rare designs for unique capabilities
- **Rank Advancement**: Climb hierarchies within organizations
- **Legacy Systems**: Permanent benefits earned across character generations

### Social Systems

- **Guild/Corporation Structure**: Player organizations with shared goals
- **Fleet Formations**: Coordinate movement and actions with allies
- **Communication Channels**: Various text and potentially voice chat options
- **Friend System**: Track and easily locate preferred collaborators
- **Reputation Tracking**: Record of player behavior and trustworthiness
- **Shared Assets**: Group ownership of stations, territories, and resources
- **Recruitment Tools**: Methods to find and join organizations
- **Alliance Networks**: Formal relationships between multiple corporations
- **Command Hierarchy**: Organizational structures with delegated permissions
- **Event Calendar**: Schedule and coordinate group activities
- **Mail System**: Asynchronous communication between players

### NPC & World Interactions

- **Faction System**: Multiple NPC groups with distinct goals and territories
- **Dynamic Events**: Procedurally generated occurrences that affect regions
- **Mission System**: Objectives and tasks provided by NPCs
- **Territory Control**: Contest and claim ownership of valuable regions
- **NPC AI Behavior**: Dynamic responses to player actions and world state
- **Crew Management**: Recruit and develop NPCs to serve on your vessels
- **Encounter Design**: Varied and interesting NPC interactions
- **Story Arcs**: Narrative progressions through connected missions
- **Reactive World**: Environment that changes based on player actions
- **Outpost Development**: Build and upgrade NPC settlements

### User Experience

- **Customizable Controls**: WASD movement with mouse targeting by default
- **Grid-Based Inventory**: Visual representation of cargo and possessions
- **Item Stacking**: Efficient storage of similar resources
- **Multiple Character Support**: Several characters per account
- **Autopilot System**: Automated travel for routine journeys
- **Loadout Presets**: Quick-switching between ship configurations
- **Waypoint Navigation**: Planning and executing complex routes
- **Minimap Interface**: Local tactical information at a glance
- **Scanner Interface**: Detailed analysis of surrounding space
- **Command Interface**: Issue orders to AI ships in your fleet
- **Automated Resource Collection**: Systems for routine gathering operations
- **Quick-Access Menus**: Streamlined interface for common actions

## Player Experience Journey

### New Player Experience

1. **Account Creation**: Register with email and password
2. **Character Creation**: Develop initial persona and appearance
3. **Tutorial Sequence**: Learn basic controls and systems
4. **Starter Ship**: Begin with a default vessel with basic capabilities
5. **Safe Zone Introduction**: Initial gameplay in protected regions
6. **Guided Missions**: Structured introduction to core mechanics
7. **Career Path Introduction**: Overview of possible specializations
8. **Social Integration**: Connection to community resources

### Mid-Game Progression

1. **Ship Upgrades**: Improve and customize vessels
2. **Specialization**: Focus on preferred gameplay styles
3. **Faction Engagement**: Build reputation with chosen groups
4. **Economic Participation**: Establish role in production and trade
5. **PvP Introduction**: Controlled exposure to player combat
6. **Guild Membership**: Join player organizations
7. **Territory Exploration**: Venture into more dangerous regions
8. **Asset Accumulation**: Build collection of ships and resources

### End-Game Activities

1. **Large-Scale Construction**: Major stations and capital ships
2. **Territory Control**: Contest and maintain ownership of regions
3. **Economic Dominance**: Influence on market systems
4. **Guild Leadership**: Direct player organizations
5. **Factional Warfare**: Shape the political landscape
6. **Rare Resource Control**: Manage access to valuable materials
7. **Technological Superiority**: Access to advanced components
8. **Legacy Building**: Create lasting impact on the game world

## World Design

### Universe Structure

- **Central Systems**: Well-protected, resource-poor but stable
- **Mid Regions**: Balanced risk and reward, faction-controlled
- **Frontier**: Dangerous, resource-rich, limited protection
- **Uncharted Space**: Procedurally generated, high-risk/high-reward
- **Wormhole Regions**: Temporary access, exceptional resources
- **Faction Territories**: Areas controlled by major NPC groups
- **Contested Zones**: Regions under active dispute
- **Player Claimable Areas**: Regions available for guild control

### Environmental Features

- **Star Systems**: Central gravity wells with orbiting features
- **Planets**: Major bodies with resources and potential bases
- **Asteroid Fields**: Mining opportunities with navigation hazards
- **Nebulae**: Visibility and sensor limitations, unique resources
- **Space Stations**: Trading hubs and social centers
- **Jump Gates**: Established travel network between regions
- **Anomalies**: Unique phenomena with special properties
- **Debris Fields**: Remnants of battles with salvage opportunities

## Physical Simulation

The game utilizes Rapier physics for realistic space movement and interactions, with the following key elements:

### Ship Movement and Control

Ships move according to Newtonian physics principles, modified slightly for playability:

- Ships generate thrust in the direction they are facing
- Inertia means ships continue moving without constant thrust
- Ships can rotate to change direction
- Velocity is maintained while changing orientation
- Component-based directional thrust system allows for customization

### Gravity and Celestial Movement

The game simulates gravity between celestial bodies and ships:

- Planets, stars, and large objects exert gravitational pull
- Ships and smaller objects are affected by gravity
- Orbital mechanics allow for realistic space movement
- N-body simulation for dynamic celestial interactions

### Collision and Interaction

The physics system handles various types of collisions:

- Ship-to-ship collisions can cause damage
- Ship-to-asteroid collisions for mining
- Projectile collisions for combat
- Docking with stations and larger ships

## Technical Implementation Status

### Ship Thrusters & Movement (✅ Completed)

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
```

### N-body Gravity System (✅ Completed)

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

### Collision System (🔄 In Progress)

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

### Resource Extraction (🔄 In Progress)

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

## Implementation Strategy

The gameplay features are being implemented in phases, aligned with the phases outlined in the Design Phases document:

### Implementation Status

- ✅ **Phase 0**: Core engine architecture and framework (Complete)
- 🔄 **Phase 1**: Basic networked universe with simple ship movement (In Progress)
- 🔄 **Phase 2**: Ship systems and enhanced physics (Partially Started)
- ❌ **Phase 3 - 8**: Future development phases (Not Started)

### Current Focus Areas

1. **Core Physics**:

   - ✅ Basic Newtonian movement
   - ✅ N-body gravity system
   - 🔄 Collision handling

2. **Entity Systems**:

   - 🔄 Ship component framework
   - 🔄 Shadow entity system for cross-boundary awareness
   - 🔄 Entity serialization and persistence

3. **Networking**:
   - ✅ Basic entity replication
   - 🔄 Cross-shard entity transfer
   - 🔄 Event-based communication

### Next Steps

The immediate development priorities are:

1. Completing the shadow entity framework
2. Implementing cross-shard entity transitions
3. Enhancing the ship component system
4. Developing the basic resource extraction mechanics
5. Implementing persistence of player ships and inventories

## Future Expansion Potential

- **Planetary Landings**: Surface exploration and activities
- **Character Avatars**: On-station personal representation
- **Factional Campaigns**: Large-scale narrative events
- **Alliance Warfare**: Structured conflict between player groups
- **Specialized Vessels**: Unique ships with special capabilities
- **Advanced Production**: Complex manufacturing with specialization
- **Cosmic Events**: Universe-wide phenomena affecting all players
- **Alien Encounters**: Non-human factions with unique technology

---

_This design document outlines the scope and vision for Sidereal. The project is ambitious in scale and will be developed in phases, with core systems established first followed by progressive expansion of features and content._
