[← Back to Documentation Index](../README.md) | [Game World Partitioning](./game-world.md) | [Networking Overview](./networking-overview.md)

# Sidereal: Game Entity System

## Overview

This document outlines the design and implementation of Sidereal's entity system, which powers all objects in the game universe. The system is built on Bevy ECS and leverages Bevy 0.15's Required Components feature to create a flexible, maintainable, and performant entity hierarchy.

## Required Components vs. Bundles

Bevy 0.15 introduced a significant change to how entities are created with the Required Components feature. This approach offers several advantages over the traditional Bundle-only approach:

### Required Components

Required Components in Bevy 0.15 are components that are automatically added when certain other components are added. This creates an implicit dependency relationship where adding one component automatically brings in others that are required for it to function properly.

Benefits:

- **Safer Entity Creation**: Ensures entities always have the necessary components
- **Reduced Boilerplate**: No need to repeatedly specify the same core components
- **Clearer Component Relationships**: Makes dependencies between components explicit
- **Better Error Prevention**: Prevents common errors from forgetting core components

### How We Use Both

In Sidereal, we use a hybrid approach:

1. **Required Components**: For fundamental relationships and dependencies
2. **Bundles**: For convenient grouping of related but optional components

## Core Entity Component Structure

### Spatial Tracking Components

All entities that exist in the physical game world need spatial tracking:

```rust
// Marker component indicating an entity should be tracked in the spatial partitioning system
#[derive(Component)]
pub struct SpatialTracked;

// Actual position data - automatically added to any entity with SpatialTracked
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct SpatialPosition {
    pub position: Vec2,       // Actual position in world space
    pub sector_coords: IVec2, // Current sector coordinates
    pub cluster_coords: IVec2, // Current cluster coordinates
}

// Velocity component for physics-based entities
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Velocity {
    pub linvel: Vec2,  // Linear velocity vector
    pub angvel: f32,   // Angular velocity in radians per second
}

// Required Components implementation - this is what makes the magic happen
impl RequiredComponents for SpatialTracked {
    fn register_required(components: &mut RequiredComponentsSet) {
        components.add::<SpatialPosition>();
    }
}
```

This ensures that any entity marked with `SpatialTracked` will automatically have a `SpatialPosition` component without the developer needing to remember to add it explicitly.

### Physics Components

Sidereal uses bevy_rapier2d for physics simulation. Since Rapier's components don't natively support serialization, we've created wrapper structures:

```rust
// PhysicsBody is a marker that indicates an entity participates in physics simulation
#[derive(Component)]
pub struct PhysicsBody;

impl RequiredComponents for PhysicsBody {
    fn register_required(components: &mut RequiredComponentsSet) {
        components.add::<SpatialTracked>(); // Physics bodies must be spatially tracked
        components.add::<RigidBody>();      // Must have a RigidBody from Rapier
        components.add::<Velocity>();       // Must have a Velocity from Rapier
        components.add::<Collider>();       // Must have a Collider from Rapier
    }
}
```

This creates a chain of requirements: adding `PhysicsBody` automatically adds `SpatialTracked`, which in turn adds `SpatialPosition`. It also ensures that physics entities have a `Velocity` component, which is crucial for both physics simulation and boundary detection.

### Ship Components

Ships are complex entities with multiple required components:

```rust
#[derive(Component)]
pub struct Ship;

impl RequiredComponents for Ship {
    fn register_required(components: &mut RequiredComponentsSet) {
        components.add::<PhysicsBody>();        // Ships are physics entities
        components.add::<ShipComponents>();     // Track installed components
        components.add::<ShipStats>();          // Calculated ship statistics
        components.add::<Ownership>();          // All ships have an owner
        components.add::<NetworkReplication>(); // Ships must be replicated
    }
}
```

## Integration with World Partitioning

The entity system integrates with the world partitioning system (described in [Game World Partitioning](./game-world.md)) through the `SpatialPosition` and `Velocity` components:

1. The `SpatialPosition` component stores both the exact world coordinates and the sector/cluster coordinates
2. The `Velocity` component is used to determine boundary awareness thresholds based on entity speed
3. Systems update sector and cluster coordinates when entities move
4. When entities approach sector boundaries, they are flagged for potential transition
5. Boundary detection considers both position and velocity to provide an appropriate transition zone
6. The world partitioning system handles the logistics of transferring entities between sectors and clusters

Example system for updating sector coordinates:

```rust
fn update_entity_sector_coordinates(
    mut query: Query<(Entity, &Transform, &mut SpatialPosition, Option<&Velocity>)>,
    universe_config: Res<UniverseConfig>,
    mut boundary_event_writer: EventWriter<EntityApproachingBoundary>,
) {
    for (entity, transform, mut spatial_pos, velocity) in query.iter_mut() {
        // Calculate sector coordinates from world position
        let new_sector_x = (transform.translation.x / universe_config.sector_size).floor() as i32;
        let new_sector_y = (transform.translation.y / universe_config.sector_size).floor() as i32;
        let new_sector_coords = IVec2::new(new_sector_x, new_sector_y);

        // Calculate cluster coordinates
        let new_cluster_x = (new_sector_x as f32 / universe_config.cluster_dimensions.x as f32).floor() as i32;
        let new_cluster_y = (new_sector_y as f32 / universe_config.cluster_dimensions.y as f32).floor() as i32;
        let new_cluster_coords = IVec2::new(new_cluster_x, new_cluster_y);

        // If sector has changed, update and check if approaching boundary
        if new_sector_coords != spatial_pos.sector_coords {
            // Sector changed
            spatial_pos.sector_coords = new_sector_coords;

            // If cluster changed, this is important for the partitioning system
            if new_cluster_coords != spatial_pos.cluster_coords {
                spatial_pos.cluster_coords = new_cluster_coords;

                // Entity has changed clusters - this is handled by the partitioning system
                // No direct action needed here as the spatial partitioning system will detect this
            }
        }

        // Check if entity is approaching a sector boundary
        check_boundary_approach(entity, &transform.translation, velocity, spatial_pos.sector_coords,
                               &universe_config, &mut boundary_event_writer);
    }
}

// Function to detect entities approaching boundaries
fn check_boundary_approach(
    entity: Entity,
    position: &Vec3,
    velocity: Option<&Velocity>,
    sector_coords: IVec2,
    config: &UniverseConfig,
    boundary_events: &mut EventWriter<EntityApproachingBoundary>,
) {
    // Calculate position within current sector
    let sector_size = config.sector_size;
    let pos_in_sector = Vec2::new(
        position.x - (sector_coords.x as f32 * sector_size),
        position.y - (sector_coords.y as f32 * sector_size)
    );

    // Calculate distances to each boundary
    let dist_to_left = pos_in_sector.x;
    let dist_to_right = sector_size - pos_in_sector.x;
    let dist_to_top = pos_in_sector.y;
    let dist_to_bottom = sector_size - pos_in_sector.y;

    // Determine boundary awareness threshold based on velocity and minimum distance
    let threshold = if let Some(vel) = velocity {
        // Fast-moving entities need a larger awareness zone
        // Use velocity magnitude * time_factor to ensure we have enough time to handle the transition
        (vel.linvel.length() * config.velocity_awareness_factor).max(config.min_boundary_awareness)
    } else {
        // Static or non-physics entities use the minimum threshold
        config.min_boundary_awareness
    };

    // Check each boundary
    if dist_to_left < threshold {
        boundary_events.send(EntityApproachingBoundary {
            entity,
            direction: BoundaryDirection::West,
            distance: dist_to_left,
        });
    }

    if dist_to_right < threshold {
        boundary_events.send(EntityApproachingBoundary {
            entity,
            direction: BoundaryDirection::East,
            distance: dist_to_right,
        });
    }

    if dist_to_top < threshold {
        boundary_events.send(EntityApproachingBoundary {
            entity,
            direction: BoundaryDirection::North,
            distance: dist_to_top,
        });
    }

    if dist_to_bottom < threshold {
        boundary_events.send(EntityApproachingBoundary {
            entity,
            direction: BoundaryDirection::South,
            distance: dist_to_bottom,
        });
    }
}
```

## Shadow Entities

For cross-boundary awareness, Sidereal implements shadow entities:

```rust
// Shadow entities represent entities from neighboring shards/clusters
#[derive(Component)]
pub struct ShadowEntity {
    pub source_shard_id: Uuid,
    pub original_entity: Entity,
    pub is_read_only: bool,
    pub last_updated: f64,  // Timestamp of the last update
}

// Marker indicating entity is visual-only (no physics)
#[derive(Component)]
pub struct VisualOnly;

impl RequiredComponents for ShadowEntity {
    fn register_required(components: &mut RequiredComponentsSet) {
        components.add::<SpatialPosition>(); // Even shadows need positions
        components.add::<Velocity>();        // Shadows track velocity for visual prediction
        components.add::<VisualOnly>();      // Marker indicating no physics processing
    }
}

// Registry to track shadow entities
#[derive(Resource)]
pub struct ShadowEntityRegistry {
    // Maps original entity ID to local shadow entity
    entity_map: HashMap<Entity, ShadowEntityInfo>,
}

#[derive(Clone)]
pub struct ShadowEntityInfo {
    pub local_entity: Entity,
    pub source_shard_id: Uuid,
    pub last_updated: f64,
}
```

These shadow entities allow for visual representation and awareness of entities in neighboring sectors without full physics simulation, supporting seamless gameplay across boundaries. The inclusion of the `Velocity` component enables smooth visual interpolation of shadow entities.

## Serialization & Network Replication

Entity serialization is crucial for both persistence and network replication:

```rust
// Marker component for entities that should be network replicated
#[derive(Component)]
pub struct NetworkReplication;

impl RequiredComponents for NetworkReplication {
    fn register_required(components: &mut RequiredComponentsSet) {
        components.add::<UniqueEntityId>(); // Ensures entity has a consistent ID across the network
    }
}

// Component storing serializable physics data
#[derive(Component, Serialize, Deserialize)]
pub struct PhysicsData {
    // Fields that mirror Rapier components but are serializable
    // See implementation in sidereal-core/src/ecs/components/physics.rs
}
```

## Entity Creation Patterns

With Required Components, entity creation becomes more straightforward:

```rust
// Creating a basic ship
commands.spawn(Ship); // All required components are automatically added

// Creating a shadow entity
commands.spawn((
    ShadowEntity {
        source_shard_id: source_shard,
        original_entity: original_id,
        is_read_only: true
    },
    Transform::from_translation(position.extend(0.0)),
));
```

This ensures that all necessary components are present, making the code both more concise and more error-resistant.

## Bundle Usage

While Required Components handle dependencies, Bundles are still used for convenient grouping:

```rust
// Bundle for creating a new player ship
#[derive(Bundle)]
pub struct NewPlayerShipBundle {
    pub ship: Ship, // This adds all required components
    pub ship_type: ShipType,
    pub player_controlled: PlayerControlled,
    pub name: Name,
    // Additional optional components
}
```

## Required Component Registration

The registration of Required Components happens during app setup:

```rust
fn register_required_components(app: &mut App) {
    // Register all required component relationships
    app.register_required_components::<SpatialTracked>()
       .register_required_components::<PhysicsBody>()
       .register_required_components::<Ship>()
       .register_required_components::<ShadowEntity>()
       .register_required_components::<NetworkReplication>();
}
```

## Conclusion

By leveraging Bevy 0.15's Required Components feature, Sidereal creates a robust entity system that:

1. Ensures entities always have the necessary components
2. Makes dependencies between components explicit
3. Reduces boilerplate code
4. Prevents common errors
5. Integrates seamlessly with the world partitioning system

This approach creates a foundation that is both flexible for development and optimized for performance in a distributed MMO environment.
