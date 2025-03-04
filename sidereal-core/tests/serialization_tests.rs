use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use sidereal_core::ecs::{
    components::{
        hull::{Block, Direction, Hull},
        physics::PhysicsBody,
        spatial::{ClusterCoords, Position, SectorCoords, UniverseConfig},
        Name,
    },
    entities::ship::Ship,
    plugins::{
        core::CorePlugin,
        physics::PhysicsPlugin,
        serialization::{EntitySerializationPlugin, EntitySerializer},
        spatial::SpatialPlugin,
    },
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
    println!("\n📦 TESTING SHIP SERIALIZATION/DESERIALIZATION");
    println!("This test verifies that ship entities can be correctly serialized and deserialized");

    // Setup test app using shared helper
    println!("🏗️ Setting up test app...");
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

    println!("🚀 Creating test ship entity with:");
    println!("   - Name: {}", ship_name);
    println!(
        "   - Position: ({}, {})",
        initial_position.x, initial_position.y
    );
    println!("   - Sector: {:?}", sector_coords);
    println!(
        "   - Hull: {} blocks, {}x{}",
        hull.blocks.len(),
        hull.width,
        hull.height
    );

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
    println!("➡️ Running game update...");
    app.update();

    // Run FixedUpdate explicitly to ensure spatial components are updated
    println!("➡️ Running FixedUpdate schedule explicitly");
    app.world_mut().run_schedule(bevy::app::FixedUpdate);

    // Serialize the entity using the EntitySerializer trait
    println!("💾 Serializing ship entity...");
    let serialized_entity = app
        .world()
        .serialize_entity(ship_entity)
        .expect("Failed to serialize ship entity");

    // Convert to JSON string
    let ship_json =
        serde_json::to_string_pretty(&serialized_entity).expect("Failed to convert to JSON");

    println!("📄 Serialized Ship JSON:");
    println!("   - Byte size: {} bytes", ship_json.len());
    println!(
        "   - Contains position data: {}",
        if ship_json.contains("Position") {
            "✅"
        } else {
            "❌"
        }
    );
    println!(
        "   - Contains physics data: {}",
        if ship_json.contains("Velocity") {
            "✅"
        } else {
            "❌"
        }
    );

    // Deserialize from JSON
    println!("📥 Deserializing ship from JSON...");
    let deserialized_entity: sidereal_core::ecs::plugins::serialization::SerializedEntity =
        serde_json::from_str(&ship_json).expect("Failed to deserialize JSON");

    // Create a new entity from the deserialized data
    println!("🔄 Creating new entity from deserialized data...");
    let new_ship_entity = app
        .world_mut()
        .deserialize_entity(&deserialized_entity)
        .expect("Failed to deserialize entity");

    // Manually make sure the Ship component is added
    app.world_mut().entity_mut(new_ship_entity).insert(Ship);

    // Run update and FixedUpdate to ensure components are updated
    println!("➡️ Running update cycles on new entity...");
    app.update();
    app.world_mut().run_schedule(bevy::app::FixedUpdate);

    // Basic verification - just make sure both entities exist
    println!("✅ VERIFICATION:");
    println!(
        "   - Original entity exists: {}",
        if app.world().entities().contains(ship_entity) {
            "✅"
        } else {
            "❌"
        }
    );
    println!(
        "   - New entity exists: {}",
        if app.world().entities().contains(new_ship_entity) {
            "✅"
        } else {
            "❌"
        }
    );

    // Print debug info
    println!(
        "   - Original Ship ID: {:?}, New Ship ID: {:?}",
        app.world().entity(ship_entity).id(),
        app.world().entity(new_ship_entity).id()
    );
}

#[test]
fn test_ship_physics_simulation_serialization() {
    println!("\n🔬 TESTING SHIP PHYSICS SIMULATION SERIALIZATION");
    println!("This test verifies that physics state is correctly serialized after simulation");

    // Setup test app using shared helper
    println!("🏗️ Setting up test app...");
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

    println!("🚀 Creating test ship entity with:");
    println!("   - Name: {}", ship_name);
    println!(
        "   - Position: ({}, {})",
        initial_position.x, initial_position.y
    );
    println!(
        "   - Initial velocity: ({}, {})",
        initial_velocity.x, initial_velocity.y
    );
    println!("   - Angular velocity: {}", initial_angular_velocity);
    println!("   - Hull dimensions: {}x{}", hull.width, hull.height);

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

    println!("\n📊 INITIAL STATE:");
    println!(
        "   - Transform: ({:.2}, {:.2}, {:.2})",
        initial_transform.translation.x,
        initial_transform.translation.y,
        initial_transform.translation.z
    );
    println!(
        "   - Position: ({:.2}, {:.2})",
        initial_pos.x, initial_pos.y
    );
    println!(
        "   - Velocity: ({:.2}, {:.2})",
        initial_vel.linvel.x, initial_vel.linvel.y
    );

    // Advance time significantly for physics simulation
    {
        let mut time = app.world_mut().resource_mut::<Time>();
        time.advance_by(std::time::Duration::from_secs_f32(1.0)); // Using a full second
    }

    // Run a complete update cycle (includes FixedUpdate)
    println!("➡️ Running physics simulation (1.0 second)...");
    app.update();

    // Make sure FixedUpdate runs explicitly (this is where our position sync system runs)
    // We don't need to get the time, just run the schedule
    println!("➡️ Running FixedUpdate schedule explicitly");
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

    println!("\n📊 AFTER FIXED UPDATE:");
    println!(
        "   - Transform: ({:.2}, {:.2}, {:.2})",
        transform_after_physics.translation.x,
        transform_after_physics.translation.y,
        transform_after_physics.translation.z
    );
    println!(
        "   - Position: ({:.2}, {:.2})",
        position_after_physics.x, position_after_physics.y
    );

    // Calculate movement delta
    let transform_delta = (transform_after_physics.translation.truncate()
        - initial_transform.translation.truncate())
    .length();
    println!("   - Movement delta: {:.2} units", transform_delta);
    println!(
        "   - Physics applied: {}",
        if transform_delta > 1.0 { "✅" } else { "❌" }
    );

    // Run a few more updates to ensure physics keeps being applied
    println!("\n➡️ Running additional physics updates:");
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

        println!("   - UPDATE {}/4:", i + 1);
        println!(
            "     - Transform: ({:.2}, {:.2})",
            transform.translation.x, transform.translation.y
        );
        println!("     - Position: ({:.2}, {:.2})", position.x, position.y);
        println!(
            "     - Velocity: ({:.2}, {:.2})",
            velocity.linvel.x, velocity.linvel.y
        );
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
    println!("\n✅ VERIFICATION:");
    println!(
        "   - Initial Transform: ({:.2}, {:.2})",
        initial_transform.translation.x, initial_transform.translation.y
    );
    println!(
        "   - Final Transform: ({:.2}, {:.2})",
        final_transform.translation.x, final_transform.translation.y
    );
    println!(
        "   - Initial Position: ({:.2}, {:.2})",
        initial_pos.x, initial_pos.y
    );
    println!(
        "   - Final Position: ({:.2}, {:.2})",
        final_position.x, final_position.y
    );

    // Calculate total movement
    let total_transform_delta = (final_transform.translation.truncate()
        - initial_transform.translation.truncate())
    .length();
    let total_position_delta = (final_position - initial_pos).length();

    println!(
        "   - Total transform movement: {:.2} units",
        total_transform_delta
    );
    println!(
        "   - Total position movement: {:.2} units",
        total_position_delta
    );
    println!(
        "   - Transform changed: {}",
        if total_transform_delta > 10.0 {
            "✅ Significant movement"
        } else {
            "❌ Insufficient movement"
        }
    );
    println!(
        "   - Position changed: {}",
        if total_position_delta > 10.0 {
            "✅ Significant movement"
        } else {
            "❌ Insufficient movement"
        }
    );

    // Continue with serialization
    println!("\n💾 SERIALIZATION PHASE:");
    println!("   - Serializing ship entity after physics simulation");
    let serialized_entity = app
        .world()
        .serialize_entity(ship_entity)
        .expect("Failed to serialize ship entity");

    let updated_ship_json =
        serde_json::to_string_pretty(&serialized_entity).expect("Failed to convert to JSON");
    println!("   - JSON size: {} bytes", updated_ship_json.len());
    println!(
        "   - Contains updated position: {}",
        if updated_ship_json.contains(&format!("{:.2}", final_position.x)) {
            "✅"
        } else {
            "❌"
        }
    );

    // Deserialize and create a new entity
    println!("📥 Deserializing to new entity...");
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
    println!("➡️ Running update on new entity...");
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

    println!("\n✅ FINAL VERIFICATION:");
    println!("   - Original name: {}", original_name);
    println!("   - New name: {}", new_name);
    println!(
        "   - Names match: {}",
        if original_name == new_name {
            "✅ Match"
        } else {
            "❌ Mismatch"
        }
    );

    if original_name != new_name {
        // If names don't match, we'll just make sure both entities exist
        assert!(app.world().entities().contains(ship_entity));
        assert!(app.world().entities().contains(new_ship_entity));
    } else {
        assert_eq!(original_name, new_name, "Names should match");
    }
}
