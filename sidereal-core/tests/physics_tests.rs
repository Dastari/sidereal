use bevy::prelude::*;
use bevy::prelude::Time;
use std::time::Duration;
use sidereal_core::ecs::components::*;
use sidereal_core::ecs::components::physics::{Velocity, AngularVelocity, Mass};
use sidereal_core::ecs::systems::physics::*;

// Test helper to create an app with test systems
fn setup_test_app() -> App {
    let mut app = App::new();
    
    // Add the bare minimum plugins for testing
    app.add_plugins(MinimalPlugins);
    
    // Register our components
    app.register_type::<Position>()
        .register_type::<Velocity>()
        .register_type::<Mass>()
        .register_type::<Rotation>()
        .register_type::<AngularVelocity>();
    
    app
}

// Custom system to manually set the delta time for testing
fn set_test_time(mut time: ResMut<Time>) {
    // Instead of replacing the time, just advance it
    time.advance_by(Duration::from_secs_f32(1.0));
    // println!("⏱️  Time delta set to: {:?}", time.delta());
}

#[test]
fn test_physics_system() {
    println!("\n📊 TESTING BASIC LINEAR PHYSICS SYSTEM");
    println!("This test verifies that velocity correctly updates position over time");
    
    // Setup test app
    let mut app = setup_test_app();
    
    // Create a test entity with position, velocity, and mass
    let entity_id = {
        let world = app.world_mut();
        world.spawn((
            Position { x: 0.0, y: 0.0 },
            Velocity { x: 10.0, y: 5.0 },
            Mass(2.0),
        )).id()
    };
    
    println!("🚀 Created entity with initial:");
    println!("   - Position: (0.0, 0.0)");
    println!("   - Velocity: (10.0, 5.0)");
    println!("   - Mass: 2.0");
    
    // Initialize Time resource
    app.init_resource::<Time>();
    
    // Add systems to the app
    app.add_systems(Update, (
        set_test_time,
        physics_system,
    ).chain());
    
    // Run the app update
    println!("➡️ Running physics simulation for 1 second...");
    app.update();
    
    // Verify that position was updated correctly
    let position = {
        let world = app.world();
        let pos = world.entity(entity_id).get::<Position>().unwrap().clone();
        pos
    };
    
    // Now the physics system doesn't apply inverse mass, so we expect:
    // position.x = 0.0 + 10.0 * 1.0 = 10.0
    // position.y = 0.0 + 5.0 * 1.0 = 5.0
    println!("✅ VERIFICATION:");
    println!("   - Expected position: (10.0, 5.0)");
    println!("   - Actual position: ({:.2}, {:.2})", position.x, position.y);
    
    let success = position.x == 10.0 && position.y == 5.0;
    println!("🔍 RESULT: {}", if success { "Physics movement working correctly" } else { "Physics movement calculation failed" });
    
    assert_eq!(position.x, 10.0);
    assert_eq!(position.y, 5.0);
}

#[test]
fn test_rotation_system() {
    println!("\n🔄 TESTING ROTATION SYSTEM");
    println!("This test verifies that angular velocity correctly updates rotation over time");
    
    // Setup test app
    let mut app = setup_test_app();
    
    // Create a test entity with rotation, angular velocity, and mass
    let entity_id = {
        let world = app.world_mut();
        world.spawn((
            Rotation(0.0),
            AngularVelocity(2.0),
            Mass(2.0),
        )).id()
    };
    
    println!("🚀 Created entity with initial:");
    println!("   - Rotation: 0.0 radians");
    println!("   - Angular Velocity: 2.0 radians/sec");
    println!("   - Mass: 2.0 (inverse mass: 0.5)");
    
    // Initialize Time resource
    app.init_resource::<Time>();
    
    // Add systems to the app
    app.add_systems(Update, (
        set_test_time,
        rotation_system,
    ).chain());
    
    // Run the app update
    println!("➡️ Running rotation simulation for 1 second...");
    app.update();
    
    // Verify that rotation was updated correctly
    let rotation = {
        let world = app.world();
        let rot = world.entity(entity_id).get::<Rotation>().unwrap().clone();
        rot
    };
    
    // With mass 2.0, the inverse mass is 0.5, so we expect:
    // rotation = 0.0 + 2.0 * 0.5 * 1.0 = 1.0
    println!("✅ VERIFICATION:");
    println!("   - Expected rotation: 1.0 radians");
    println!("   - Actual rotation: {:.2} radians", rotation.0);
    
    let success = rotation.0 == 1.0;
    println!("🔍 RESULT: {}", if success { "Rotation system working correctly" } else { "Rotation calculation failed" });
    
    assert_eq!(rotation.0, 1.0);
}

#[test]
fn test_gravitational_well_system() {
    println!("\n🌍 TESTING GRAVITATIONAL WELL SYSTEM");
    println!("This test verifies that massive objects exert gravitational pull on smaller objects");
    
    // Setup test app
    let mut app = setup_test_app();
    
    // Create a massive object and a smaller object
    let (_massive_entity, affected_entity) = {
        let world = app.world_mut();
        let massive = world.spawn((
            Position { x: 100.0, y: 100.0 },
            Velocity { x: 0.0, y: 0.0 },
            Mass(200.0), // Above MIN_MASS_FOR_GRAVITY (100.0)
        )).id();
        
        let affected = world.spawn((
            Position { x: 110.0, y: 100.0 }, // 10 units to the right of massive object
            Velocity { x: 0.0, y: 0.0 },
            Mass(1.0),
        )).id();
        
        (massive, affected)
    };
    
    println!("🚀 Created two entities:");
    println!("   - Massive entity at (100.0, 100.0) with mass 200.0");
    println!("   - Smaller entity at (110.0, 100.0) with mass 1.0");
    println!("   Both entities start with zero velocity");
    
    // Initialize Time resource
    app.init_resource::<Time>();
    
    // Add systems to the app
    app.add_systems(Update, (
        set_test_time,
        gravitational_well_system,
    ).chain());
    
    // Run the app update
    println!("➡️ Running gravitational simulation for 1 second...");
    app.update();
    
    // Verify that velocity was updated due to gravitational pull
    let velocity = {
        let world = app.world();
        let vel = world.entity(affected_entity).get::<Velocity>().unwrap().clone();
        vel
    };
    
    // The gravitational pull should be in the negative x direction (towards the massive object)
    println!("✅ VERIFICATION:");
    println!("   - Expected velocity direction: towards massive object (negative x)");
    println!("   - Actual velocity: ({:.2}, {:.2})", velocity.x, velocity.y);
    
    let _success = velocity.x < 0.0 && velocity.y == 0.0;
    println!("🔍 RESULT: {}", 
        if velocity.x < 0.0 { 
            "Gravity is correctly pulling the smaller object towards the massive object" 
        } else { 
            "Gravitational pull failed or is in the wrong direction" 
        });
    
    assert!(velocity.x < 0.0, "Gravity should pull in negative x direction, but velocity.x = {}", velocity.x);
    assert_eq!(velocity.y, 0.0, "There should be no y-component of velocity, but velocity.y = {}", velocity.y);
}

#[test]
fn test_integrated_physics() {
    println!("\n🔬 TESTING INTEGRATED PHYSICS SYSTEMS");
    println!("This test verifies that gravity, linear physics, and rotational physics work together");
    
    // Setup test app
    let mut app = setup_test_app();
    
    // Create our test entities
    let entity1 = {
        let world = app.world_mut();
        world.spawn((
            Position { x: 0.0, y: 0.0 },
            Velocity { x: 0.0, y: 3.0 },
            Mass(500.0),
            Rotation(0.0),
            AngularVelocity(0.1),
        )).id()
    };

    let entity2 = {
        let world = app.world_mut();
        world.spawn((
            Position { x: 50.0, y: 0.0 },
            Velocity { x: 0.0, y: -2.0 },
            Mass(250.0),
            Rotation(0.0),
            AngularVelocity(0.2),
        )).id()
    };
    
    println!("🚀 Created two entities:");
    println!("   - Entity 1: Heavy object (mass 500) at (0,0) with upward velocity (0,3) and slow rotation (0.1 rad/s)");
    println!("   - Entity 2: Medium object (mass 250) at (50,0) with downward velocity (0,-2) and faster rotation (0.2 rad/s)");
    
    // Initialize Time resource
    app.init_resource::<Time>();
    
    // Add systems to the app with explicit ordering
    app.add_systems(Update, (
        set_test_time,
        gravitational_well_system.before(physics_system),
        physics_system,
        rotation_system.after(physics_system),
    ).chain());
    
    // Print initial state
    println!("➡️ Initial state:");
    let (pos1, pos2, rot1, rot2, vel1, vel2) = {
        let world = app.world();
        let pos1 = world.entity(entity1).get::<Position>().unwrap().clone();
        let pos2 = world.entity(entity2).get::<Position>().unwrap().clone();
        let rot1 = world.entity(entity1).get::<Rotation>().unwrap().clone();
        let rot2 = world.entity(entity2).get::<Rotation>().unwrap().clone();
        let vel1 = world.entity(entity1).get::<Velocity>().unwrap().clone();
        let vel2 = world.entity(entity2).get::<Velocity>().unwrap().clone();
        (pos1, pos2, rot1, rot2, vel1, vel2)
    };
    println!("Entity 1 (Heavy):");
    println!("   - Position: ({:.2}, {:.2})", pos1.x, pos1.y);
    println!("   - Rotation: {:.2} radians", rot1.0);
    println!("   - Velocity: ({:.2}, {:.2})", vel1.x, vel1.y);
    println!("Entity 2 (Medium):");
    println!("   - Position: ({:.2}, {:.2})", pos2.x, pos2.y);
    println!("   - Rotation: {:.2} radians", rot2.0);
    println!("   - Velocity: ({:.2}, {:.2})", vel2.x, vel2.y);
    
    // Run the simulation for 20 steps
    println!("➡️ Running integrated simulation for 20 seconds...");
    for step in 1..=20 {
        app.update();
        
        if step % 5 == 0 {
            println!("   Step {}/20 completed", step);
        }
    }
    
    // Get final positions
    let (position1, position2, rotation1, rotation2, velocity1, velocity2) = {
        let world = app.world();
        let pos1 = world.entity(entity1).get::<Position>().unwrap().clone();
        let pos2 = world.entity(entity2).get::<Position>().unwrap().clone();
        let rot1 = world.entity(entity1).get::<Rotation>().unwrap().clone();
        let rot2 = world.entity(entity2).get::<Rotation>().unwrap().clone();
        let vel1 = world.entity(entity1).get::<Velocity>().unwrap().clone();
        let vel2 = world.entity(entity2).get::<Velocity>().unwrap().clone();
        (pos1, pos2, rot1, rot2, vel1, vel2)
    };
    
    // Print final state
    println!("\n✅ VERIFICATION AFTER 20 SECONDS:");
    println!("Entity 1 (Heavy):");
    println!("   - Initial position: (0.0, 0.0), final position: ({:.2}, {:.2})", position1.x, position1.y);
    println!("   - Initial rotation: 0.0, final rotation: {:.2} radians", rotation1.0);
    println!("   - Final velocity: ({:.2}, {:.2})", velocity1.x, velocity1.y);
    
    println!("Entity 2 (Medium):");
    println!("   - Initial position: (50.0, 0.0), final position: ({:.2}, {:.2})", position2.x, position2.y);
    println!("   - Initial rotation: 0.0, final rotation: {:.2} radians", rotation2.0);
    println!("   - Final velocity: ({:.2}, {:.2})", velocity2.x, velocity2.y);
    
    // Check if entities have affected each other via gravity
    let distance_moved1 = position1.x.abs() + (position1.y - 60.0).abs(); // Expected y would be ~60 with just velocity
    let distance_moved2 = (position2.x - 50.0).abs() + (position2.y + 40.0).abs(); // Expected y would be ~-40 with just velocity
    
    println!("\n🔍 PHYSICS ANALYSIS:");
    
    // Verify linear motion
    println!("   - Linear motion: {}", 
        if position1.y > 0.0 && position2.y < 0.0 { "✅ Both entities moved in their initial velocity directions" } 
        else { "❌ Entities didn't move as expected by their velocities" });
    
    // Verify rotation
    println!("   - Rotation: {}", 
        if rotation1.0 != 0.0 && rotation2.0 != 0.0 { "✅ Both entities rotated as expected" } 
        else { "❌ Entities didn't rotate properly" });
    
    // Verify gravitational effects
    println!("   - Gravitational effects: {}", 
        if distance_moved1 > 0.1 && distance_moved2 > 0.1 && position2.x != 50.0 { 
            "✅ Gravity affected both entities' trajectories" 
        } else { 
            "❌ Gravity doesn't seem to be working properly" 
        });
    
    // Assertions for test validation
    assert!(position1.y > 0.0, "Entity 1 should have moved in y-direction");
    assert!(position2.x != 50.0, "Entity 2 should have moved in x-direction due to gravity");
    assert!(rotation1.0 != 0.0, "Entity 1 should have rotated");
    assert!(rotation2.0 != 0.0, "Entity 2 should have rotated");
}

#[test]
fn test_orbital_mechanics() {
    println!("\n🌌 TESTING ORBITAL MECHANICS");
    println!("This test verifies that an object with appropriate tangential velocity can orbit a massive object");
    
    // Setup test app
    let mut app = setup_test_app();
    
    // Create a massive central body
    let _central_body = {
        let world = app.world_mut();
        world.spawn((
            Position { x: 0.0, y: 0.0 },
            Velocity { x: 0.0, y: 0.0 },
            Mass(5000.0),
        )).id()
    };

    // Create an orbiting body with tangential velocity
    let orbital_body = {
        let world = app.world_mut();
        world.spawn((
            Position { x: 100.0, y: 0.0 },
            Velocity { x: 0.0, y: 10.0 }, // Tangential velocity for orbit
            Mass(10.0),
        )).id()
    };
    
    println!("🚀 Created two bodies:");
    println!("   - Central massive body (mass 5000) at origin (0,0)");
    println!("   - Orbital body (mass 10) at (100,0) with tangential velocity (0,10)");
    
    // Initialize Time resource
    app.init_resource::<Time>();
    
    // Add systems to the app with explicit ordering
    app.add_systems(Update, (
        set_test_time,
        gravitational_well_system.before(physics_system),
        physics_system,
        // debug_print_system.after(physics_system),
    ).chain());
    
    // Track positions to verify orbital pattern
    let mut positions = Vec::new();
    
    // Run simulation for 100 steps
    println!("➡️ Running orbital simulation for 100 seconds...");
    for i in 0..100 {
        app.update();
        
        // Record position every 5 steps
        if i % 5 == 0 {
            let pos = {
                let world = app.world();
                world.entity(orbital_body).get::<Position>().unwrap().clone()
            };
            positions.push((pos.x, pos.y));
            
            if i % 20 == 0 {
                println!("   Step {}/100: Orbital body at ({:.2}, {:.2})", i, pos.x, pos.y);
            }
        }
    }
    
    // Get final position and velocity
    let (final_pos, final_vel) = {
        let world = app.world();
        let pos = world.entity(orbital_body).get::<Position>().unwrap().clone();
        let vel = world.entity(orbital_body).get::<Velocity>().unwrap().clone();
        (pos, vel)
    };
    
    // Calculate some orbital metrics
    let initial_distance = 100.0; // Initial distance from center
    let final_distance = (final_pos.x.powi(2) + final_pos.y.powi(2)).sqrt();
    let _trajectory_change = positions.len() as f32 / positions.iter()
        .filter(|&&(x, y)| {
            let dx = x - positions[0].0;
            let dy = y - positions[0].1;
            (dx.powi(2) + dy.powi(2)).sqrt() > 20.0
        })
        .count() as f32;
    
    // Define quadrants for position tracking
    let quadrant_counts = positions.iter().fold([0, 0, 0, 0], |mut counts, &(x, y)| {
        if x >= 0.0 && y >= 0.0 { counts[0] += 1; }      // Q1
        else if x < 0.0 && y >= 0.0 { counts[1] += 1; }  // Q2
        else if x < 0.0 && y < 0.0 { counts[2] += 1; }   // Q3
        else if x >= 0.0 && y < 0.0 { counts[3] += 1; }  // Q4
        counts
    });
    
    println!("\n✅ ORBITAL ANALYSIS AFTER 100 SECONDS:");
    println!("   - Initial position: (100.0, 0.0), final position: ({:.2}, {:.2})", final_pos.x, final_pos.y);
    println!("   - Initial distance from center: 100.0, final distance: {:.2}", final_distance);
    println!("   - Final velocity: ({:.2}, {:.2})", final_vel.x, final_vel.y);
    println!("   - Position samples in each quadrant: Q1:{}, Q2:{}, Q3:{}, Q4:{}", 
        quadrant_counts[0], quadrant_counts[1], quadrant_counts[2], quadrant_counts[3]);
    
    // Orbital verification
    let orbital_behavior = quadrant_counts.iter().all(|&count| count > 0);
    let stable_orbit = (final_distance - initial_distance).abs() < 50.0;
    
    println!("\n🔍 ORBITAL MECHANICS ASSESSMENT:");
    println!("   - Object traversal: {}", 
        if orbital_behavior { "✅ Object traversed all four quadrants (completing at least partial orbit)" } 
        else { "❌ Object did not traverse all quadrants (no orbital behavior)" });
    
    println!("   - Orbit stability: {}", 
        if stable_orbit { "✅ Orbit is relatively stable (distance maintained)" } 
        else { "❌ Orbit is unstable (significant change in orbital distance)" });
    
    println!("   - Gravitational influence: {}", 
        if final_vel.x != 0.0 { "✅ Gravity is affecting the object's trajectory" } 
        else { "❌ Gravity is not properly affecting the object" });
    
    // Assertions for test validation
    assert!(final_pos.x < 90.0 || final_pos.x > 110.0, "Expected x position to change from 100, got {}", final_pos.x);
    assert!(final_pos.y.abs() > 10.0, "Expected y position to change from 0, got {}", final_pos.y);
    assert!(positions.len() > 5, "Not enough position recordings");
} 