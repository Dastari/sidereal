use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use serde::{Serialize, Deserialize};
use serde_json::{self, json};

use sidereal_core::ecs::components::{
    spatial::{Position, SectorCoords, ClusterCoords, UniverseConfig},
    physics::{PhysicsBody, PhysicsState},
    hull::{Hull, Block, Direction},
    Name,
};
use sidereal_core::ecs::entities::ship::Ship;
use sidereal_core::ecs::plugins::{
    physics::PhysicsPlugin,
    spacial::SpatialPlugin,
};

// Test helper to create an app with the required plugins
fn setup_test_app() -> App {
    let mut app = App::new();
    
    // Add the minimal plugins first
    app.add_plugins(MinimalPlugins);
    
    // Initialize core resources before adding custom plugins
    app.insert_resource(UniverseConfig::default());
    
        // Add our plugins last to avoid registration conflicts
    app.add_plugins(SpatialPlugin)
       .add_plugins(PhysicsPlugin);
       
    // Add assertion to verify setup
    debug_assert!(app.world().contains_resource::<Time>(), "Time resource should be initialized");
       
    app
}

// A serializable representation of a ship's spatial components
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct SerializableSpatial {
    position: Vec2,
    sector_coords: IVec2,
    cluster_coords: IVec2,
}

// A serializable representation of a ship's physics components
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct SerializablePhysics {
    linear_velocity: Vec2,
    angular_velocity: f32,
    collider_size: Vec2,
}

// A complete serializable ship representation
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct SerializableShip {
    name: String,
    spatial: SerializableSpatial,
    hull: Hull,
    physics: SerializablePhysics,
}

#[test]
fn test_ship_serialization_deserialization() {
    // Setup test app using shared helper
    let mut app = setup_test_app();
    
    // Spawn a ship entity with all required components
    let ship_name = "Test Ship";
    let initial_position = Vec2::new(100.0, 200.0);
    let sector_coords = IVec2::new(1, 2);
    let cluster_coords = IVec2::new(0, 0);
    
    // Create a simple hull
    let hull = Hull {
        width: 50.0,
        height: 30.0,
        blocks: vec![
            Block {
                x: 0.0,
                y: 0.0,
                direction: Direction::Fore,
            },
            Block {
                x: 10.0,
                y: 0.0,
                direction: Direction::Starboard,
            },
        ],
    };
    
    // Spawn the ship
    let ship_entity = app.world_mut().spawn((
        Ship,
        Name::new(ship_name),
        Transform::from_translation(Vec3::new(initial_position.x, initial_position.y, 0.0)),
        GlobalTransform::default(),
        PhysicsBody::default(),
        RigidBody::Dynamic,
        Collider::cuboid(25.0, 15.0), // Half the hull dimensions
        Velocity {
            linvel: Vec2::new(5.0, 2.0),
            angvel: 1.0,
        },
        Position::new(initial_position),
        SectorCoords::new(sector_coords),
        ClusterCoords::new(cluster_coords),
        hull.clone(),
    )).id();
    
    // Run one update to ensure all systems have a chance to execute
    app.update();
    
    // Access the entity to get all components for serialization
    let ship_data = {
        let world = app.world();
        let entity = world.entity(ship_entity);
        
        // Get the components we need
        let name = entity.get::<Name>().unwrap().to_string();
        let position = entity.get::<Position>().unwrap();
        let sector_coords = entity.get::<SectorCoords>().unwrap();
        let cluster_coords = entity.get::<ClusterCoords>().unwrap();
        let velocity = entity.get::<Velocity>().unwrap();
        let hull = entity.get::<Hull>().unwrap();
        
        // Create our serializable representation
        SerializableShip {
            name,
            spatial: SerializableSpatial {
                position: position.get(),
                sector_coords: sector_coords.get(),
                cluster_coords: cluster_coords.get(),
            },
            hull: hull.clone(),
            physics: SerializablePhysics {
                linear_velocity: velocity.linvel,
                angular_velocity: velocity.angvel,
                collider_size: Vec2::new(50.0, 30.0), // Using hull dimensions
            },
        }
    };
    
    // Serialize to pretty-printed JSON
    let ship_json = serde_json::to_string_pretty(&ship_data).expect("Failed to serialize ship");
    println!("Serialized Ship:\n{}", ship_json);
    
    // Deserialize from JSON (simulating loading from storage)
    let deserialized_ship: SerializableShip = 
        serde_json::from_str(&ship_json).expect("Failed to deserialize JSON");
    
    // Verify the deserialized ship matches the original
    assert_eq!(ship_data, deserialized_ship, "Deserialized ship should match original");
    
    // Create a new entity from the deserialized data
    let new_ship_entity = app.world_mut().spawn((
        Ship,
        Name::new(&deserialized_ship.name),
        Transform::from_translation(Vec3::new(
            deserialized_ship.spatial.position.x,
            deserialized_ship.spatial.position.y,
            0.0
        )),
        GlobalTransform::default(),
        PhysicsBody::default(),
        RigidBody::Dynamic,
        Collider::cuboid(
            deserialized_ship.physics.collider_size.x / 2.0, 
            deserialized_ship.physics.collider_size.y / 2.0
        ),
        Velocity {
            linvel: deserialized_ship.physics.linear_velocity,
            angvel: deserialized_ship.physics.angular_velocity,
        },
        Position::new(deserialized_ship.spatial.position),
        SectorCoords::new(deserialized_ship.spatial.sector_coords),
        ClusterCoords::new(deserialized_ship.spatial.cluster_coords),
        deserialized_ship.hull.clone(),
    )).id();
    
    // Run another update
    app.update();
    
    // Verify the reconstructed ship has the same components
    let original_ship = app.world().entity(ship_entity);
    let new_ship = app.world().entity(new_ship_entity);
    
    assert_eq!(
        original_ship.get::<Name>().unwrap().to_string(),
        new_ship.get::<Name>().unwrap().to_string(),
        "Names should match"
    );
    
    assert_eq!(
        original_ship.get::<Position>().unwrap().get(),
        new_ship.get::<Position>().unwrap().get(),
        "Positions should match"
    );
    
    assert_eq!(
        original_ship.get::<Velocity>().unwrap().linvel,
        new_ship.get::<Velocity>().unwrap().linvel,
        "Linear velocities should match"
    );
}

#[test]
fn test_ship_physics_simulation_serialization() {
    // Setup test app using shared helper
    let mut app = setup_test_app();
    
    // Create test ship
    let ship_name = "Physics Test Ship";
    let initial_position = Vec2::new(100.0, 200.0);
    let sector_coords = IVec2::new(0, 0);
    let cluster_coords = IVec2::new(0, 0);
    
    // Create a simple hull
    let hull = Hull {
        width: 50.0,
        height: 30.0,
        blocks: vec![
            Block { x: 0.0, y: 0.0, direction: Direction::Fore },
            Block { x: 10.0, y: 0.0, direction: Direction::Starboard },
        ],
    };
    
    // Initial physics values
    let initial_velocity = Vec2::new(10.0, 5.0);
    let initial_angular_velocity = 0.5;
    
    // Spawn the ship
    let ship_entity = app.world_mut().spawn((
        Ship,
        Name::new(ship_name),
        Transform::from_translation(Vec3::new(initial_position.x, initial_position.y, 0.0)),
        GlobalTransform::default(),
        PhysicsBody::default(),
        RigidBody::Dynamic,
        Collider::cuboid(25.0, 15.0), // Half the hull dimensions
        Velocity {
            linvel: initial_velocity,
            angvel: initial_angular_velocity,
        },
        Position::new(initial_position),
        SectorCoords::new(sector_coords),
        ClusterCoords::new(cluster_coords),
        hull.clone(),
    )).id();
    
    // Run one update to initialize components
    app.update();
    
    // Get initial state for comparison
    let initial_state = {
        let world = app.world();
        let entity = world.entity(ship_entity);
        
        SerializableShip {
            name: entity.get::<Name>().unwrap().to_string(),
            spatial: SerializableSpatial {
                position: entity.get::<Position>().unwrap().get(),
                sector_coords: entity.get::<SectorCoords>().unwrap().get(),
                cluster_coords: entity.get::<ClusterCoords>().unwrap().get(),
            },
            hull: entity.get::<Hull>().unwrap().clone(),
            physics: SerializablePhysics {
                linear_velocity: entity.get::<Velocity>().unwrap().linvel,
                angular_velocity: entity.get::<Velocity>().unwrap().angvel,
                collider_size: Vec2::new(50.0, 30.0),
            },
        }
    };
    
    // Advance time for physics simulation
    let mut time = app.world_mut().resource_mut::<Time>();
    time.advance_by(std::time::Duration::from_secs_f32(0.1));
    
    // Run fixed update to simulate physics
    app.update();
    
    // Get updated state after physics
    let updated_state = {
        let world = app.world();
        let entity = world.entity(ship_entity);
        
        SerializableShip {
            name: entity.get::<Name>().unwrap().to_string(),
            spatial: SerializableSpatial {
                position: entity.get::<Position>().unwrap().get(),
                sector_coords: entity.get::<SectorCoords>().unwrap().get(),
                cluster_coords: entity.get::<ClusterCoords>().unwrap().get(),
            },
            hull: entity.get::<Hull>().unwrap().clone(),
            physics: SerializablePhysics {
                linear_velocity: entity.get::<Velocity>().unwrap().linvel,
                angular_velocity: entity.get::<Velocity>().unwrap().angvel,
                collider_size: Vec2::new(50.0, 30.0),
            },
        }
    };
    
    // Verify that physics simulation changed the position
    assert_ne!(
        initial_state.spatial.position, 
        updated_state.spatial.position,
        "Position should change after physics simulation"
    );
    
    // Serialize the updated ship to JSON
    let updated_ship_json = serde_json::to_string_pretty(&updated_state).expect("Failed to serialize updated ship");
    println!("Ship after physics simulation:\n{}", updated_ship_json);
    
    // Deserialize and create a new entity
    let deserialized_ship: SerializableShip = 
        serde_json::from_str(&updated_ship_json).expect("Failed to deserialize JSON");
    
    // Create a new entity from the deserialized data
    let new_ship_entity = app.world_mut().spawn((
        Ship,
        Name::new(&deserialized_ship.name),
        Transform::from_translation(Vec3::new(
            deserialized_ship.spatial.position.x,
            deserialized_ship.spatial.position.y,
            0.0
        )),
        GlobalTransform::default(),
        PhysicsBody::default(),
        RigidBody::Dynamic,
        Collider::cuboid(
            deserialized_ship.physics.collider_size.x / 2.0, 
            deserialized_ship.physics.collider_size.y / 2.0
        ),
        Velocity {
            linvel: deserialized_ship.physics.linear_velocity,
            angvel: deserialized_ship.physics.angular_velocity,
        },
        Position::new(deserialized_ship.spatial.position),
        SectorCoords::new(deserialized_ship.spatial.sector_coords),
        ClusterCoords::new(deserialized_ship.spatial.cluster_coords),
        deserialized_ship.hull.clone(),
    )).id();
    
    // Run update to ensure components are fully initialized
    app.update();
    
    // Verify the new ship matches the state after physics simulation
    let final_ship = app.world().entity(new_ship_entity);
    
    assert_eq!(
        updated_state.name,
        final_ship.get::<Name>().unwrap().to_string(),
        "Names should match"
    );
    
    assert_eq!(
        updated_state.spatial.position,
        final_ship.get::<Position>().unwrap().get(),
        "Positions should match"
    );
    
    assert_eq!(
        updated_state.physics.linear_velocity,
        final_ship.get::<Velocity>().unwrap().linvel,
        "Linear velocities should match"
    );
    
    assert_eq!(
        updated_state.physics.angular_velocity,
        final_ship.get::<Velocity>().unwrap().angvel,
        "Angular velocities should match"
    );
}
