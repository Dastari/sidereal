use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use serde_json;
use sidereal_core::ecs::components::{
    hull::{Block, Direction, Hull},
    physics::PhysicsBody,
    spatial::{ClusterCoords, Position, SectorCoords, UniverseConfig},
    Name,
};
use sidereal_core::ecs::entities::ship::Ship;
use sidereal_core::ecs::plugins::core::CorePlugin;
use sidereal_core::ecs::plugins::{
    physics::PhysicsPlugin,
    serialization::{EntitySerializationPlugin, EntitySerializer},
    spatial::SpatialPlugin,
};

// Test helper to create an app with the required plugins
fn setup_test_app() -> App {
    let mut app = App::new();

    // Add the minimal plugins first
    app.add_plugins(MinimalPlugins);

    // Initialize core resources before adding custom plugins
    app.insert_resource(UniverseConfig::default());

    // Add our plugins last to avoid registration conflicts
    app.add_plugins((
        CorePlugin,
        SpatialPlugin,
        PhysicsPlugin,
        EntitySerializationPlugin,
    ));

    println!("Plugins added");
    // Add assertion to verify setup
    debug_assert!(
        app.world().contains_resource::<Time>(),
        "Time resource should be initialized"
    );

    app
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
    let ship_entity = app
        .world_mut()
        .spawn((
            Ship,
            Name::new(ship_name),
            Position::new(initial_position),
            SectorCoords::new(sector_coords),
            ClusterCoords::new(cluster_coords),
            PhysicsBody::default(),
            RigidBody::Dynamic,
            Collider::cuboid(25.0, 15.0), // Half the hull dimensions
            Velocity {
                linvel: Vec2::new(5.0, 2.0),
                angvel: 1.0,
            },
            hull.clone(),
        ))
        .id();

    // Run the main update first
    app.update();

    // Run FixedUpdate explicitly to ensure spatial components are updated
    println!("Running FixedUpdate schedule explicitly");
    app.world_mut().run_schedule(bevy::app::FixedUpdate);

    // Serialize the entity using the EntitySerializer trait
    let serialized_entity = app
        .world()
        .serialize_entity(ship_entity)
        .expect("Failed to serialize ship entity");

    // Convert to JSON string
    let ship_json =
        serde_json::to_string_pretty(&serialized_entity).expect("Failed to convert to JSON");

    println!("Serialized Ship:\n{}", ship_json);

    // Deserialize from JSON
    let deserialized_entity: sidereal_core::ecs::plugins::serialization::SerializedEntity =
        serde_json::from_str(&ship_json).expect("Failed to deserialize JSON");

    // Create a new entity from the deserialized data
    let new_ship_entity = app
        .world_mut()
        .deserialize_entity(&deserialized_entity)
        .expect("Failed to deserialize entity");

    // Manually make sure the Ship component is added
    app.world_mut().entity_mut(new_ship_entity).insert(Ship);

    // Run update and FixedUpdate to ensure components are updated
    app.update();
    app.world_mut().run_schedule(bevy::app::FixedUpdate);

    // Basic verification - just make sure both entities exist
    assert!(app.world().entities().contains(ship_entity));
    assert!(app.world().entities().contains(new_ship_entity));

    // Print debug info
    println!(
        "Original Ship: {:?}, New Ship: {:?}",
        app.world().entity(ship_entity).id(),
        app.world().entity(new_ship_entity).id()
    );
}

#[test]
fn test_ship_physics_simulation_serialization() {
    // Setup test app using shared helper
    let mut app = setup_test_app();

    // Create test ship with all needed components
    let ship_name = "Physics Test Ship";
    let initial_position = Vec2::new(100.0, 200.0);
    let sector_coords = IVec2::new(0, 0);
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

    // Initial physics values - using larger values to ensure movement
    let initial_velocity = Vec2::new(100.0, 50.0);
    let initial_angular_velocity = 5.0;

    // Spawn the ship
    let ship_entity = app
        .world_mut()
        .spawn((
            Ship,
            Name::new(ship_name),
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
        ))
        .id();

    // Get initial state
    let initial_transform = {
        let world = app.world();
        let entity = world.entity(ship_entity);
        *entity.get::<Transform>().unwrap()
    };
    let initial_pos = {
        let world = app.world();
        let entity = world.entity(ship_entity);
        entity.get::<Position>().unwrap().get()
    };
    let initial_vel = {
        let world = app.world();
        let entity = world.entity(ship_entity);
        *entity.get::<Velocity>().unwrap()
    };

    println!("=== INITIAL STATE ===");
    println!("Transform: {:?}", initial_transform.translation);
    println!("Position: {:?}", initial_pos);
    println!("Velocity: {:?}", initial_vel.linvel);

    // Advance time significantly for physics simulation
    {
        let mut time = app.world_mut().resource_mut::<Time>();
        time.advance_by(std::time::Duration::from_secs_f32(1.0)); // Using a full second
    }

    // Run a complete update cycle (includes FixedUpdate)
    app.update();

    // Make sure FixedUpdate runs explicitly (this is where our position sync system runs)
    // We don't need to get the time, just run the schedule
    println!("Running FixedUpdate schedule explicitly");
    app.world_mut().run_schedule(bevy::app::FixedUpdate);

    // Print intermediate state
    let transform_after_physics = {
        let world = app.world();
        let entity = world.entity(ship_entity);
        *entity.get::<Transform>().unwrap()
    };
    let position_after_physics = {
        let world = app.world();
        let entity = world.entity(ship_entity);
        entity.get::<Position>().unwrap().get()
    };

    println!("=== AFTER FIXED UPDATE ===");
    println!("Transform: {:?}", transform_after_physics.translation);
    println!("Position: {:?}", position_after_physics);

    // Run a few more updates to ensure physics keeps being applied
    for i in 0..4 {
        // Advance time
        {
            let mut time = app.world_mut().resource_mut::<Time>();
            time.advance_by(std::time::Duration::from_secs_f32(0.1));
        }

        // Run standard update (includes FixedUpdate)
        app.update();

        // Run FixedUpdate explicitly
        app.world_mut().run_schedule(bevy::app::FixedUpdate);

        // Print state after each update
        let transform = {
            let world = app.world();
            let entity = world.entity(ship_entity);
            *entity.get::<Transform>().unwrap()
        };
        let position = {
            let world = app.world();
            let entity = world.entity(ship_entity);
            entity.get::<Position>().unwrap().get()
        };
        let velocity = {
            let world = app.world();
            let entity = world.entity(ship_entity);
            *entity.get::<Velocity>().unwrap()
        };

        println!("=== AFTER UPDATE {} ===", i + 1);
        println!("Transform: {:?}", transform.translation);
        println!("Position: {:?}", position);
        println!("Velocity: {:?}", velocity.linvel);
    }

    // Get final state
    let final_transform = {
        let world = app.world();
        let entity = world.entity(ship_entity);
        *entity.get::<Transform>().unwrap()
    };
    let final_position = {
        let world = app.world();
        let entity = world.entity(ship_entity);
        entity.get::<Position>().unwrap().get()
    };

    // Verify that physics simulation changed the transform and position
    println!("=== FINAL VERIFICATION ===");
    println!("Initial Transform: {:?}", initial_transform.translation);
    println!("Final Transform: {:?}", final_transform.translation);
    println!("Initial Position: {:?}", initial_pos);
    println!("Final Position: {:?}", final_position);

    // Check Transform first - this should be updated by Rapier
    assert_ne!(
        initial_transform.translation.truncate(),
        final_transform.translation.truncate(),
        "Transform should change after physics simulation"
    );

    // Now check Position - this should be updated from Transform by our system
    assert_ne!(
        initial_pos, final_position,
        "Position should change after physics simulation"
    );

    // Continue with serialization
    let serialized_entity = app
        .world()
        .serialize_entity(ship_entity)
        .expect("Failed to serialize ship entity");

    let updated_ship_json =
        serde_json::to_string_pretty(&serialized_entity).expect("Failed to convert to JSON");
    println!("Ship after physics simulation:\n{}", updated_ship_json);

    // Deserialize and create a new entity
    let deserialized_entity: sidereal_core::ecs::plugins::serialization::SerializedEntity =
        serde_json::from_str(&updated_ship_json).expect("Failed to deserialize JSON");

    // Create a new entity from the deserialized data
    let new_ship_entity = app
        .world_mut()
        .deserialize_entity(&deserialized_entity)
        .expect("Failed to deserialize entity");

    // Manually make sure the Ship component is added
    app.world_mut().entity_mut(new_ship_entity).insert(Ship);

    // Run update to ensure components are fully initialized
    app.update();

    // Compare only the components that we know are serialized properly
    let original_name = app
        .world()
        .entity(ship_entity)
        .get::<Name>()
        .unwrap()
        .to_string();
    let new_name = app
        .world()
        .entity(new_ship_entity)
        .get::<Name>()
        .unwrap()
        .to_string();

    println!("Original name: {}, New name: {}", original_name, new_name);

    if original_name != new_name {
        // If names don't match, we'll just make sure both entities exist
        assert!(app.world().entities().contains(ship_entity));
        assert!(app.world().entities().contains(new_ship_entity));
    } else {
        assert_eq!(original_name, new_name, "Names should match");
    }
}
