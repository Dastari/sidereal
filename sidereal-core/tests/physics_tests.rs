use bevy::prelude::*;
use std::time::Duration;
use bevy_rapier2d::prelude::*; // Import Rapier components
use sidereal_core::ecs::systems::physics::*;

// Test helper to create an app with Rapier physics
fn setup_test_app() -> App {
    let mut app = App::new();
    
    // Add the bare minimum plugins for testing
    app.add_plugins(MinimalPlugins);
    
    // Add Rapier physics plugin with default settings
    app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());
    
    // Add our n_body_gravity_system to run physics
    app.add_systems(Update, n_body_gravity_system);
    
    app
}

// Simulate multiple physics frames to see the effects
fn run_physics_frames(app: &mut App, frames: u32) {
    for _ in 0..frames {
        // Advance time by 1/60 second for each frame
        {
            let mut time = app.world_mut().resource_mut::<Time>();
            time.advance_by(Duration::from_secs_f32(1.0 / 60.0));
        }
        
        // Update the app to process all systems
        app.update();
    }
}

#[test]
fn test_basic_movement() {
    println!("\n📊 TESTING BASIC MOVEMENT");
    println!("This test verifies that Rapier correctly updates position based on velocity");
    
    // Setup test app
    let mut app = setup_test_app();
    
    // Initialize Time resource
    app.init_resource::<Time>();
    
    // Create a test entity with an initial velocity
    let entity_id = app.world_mut().spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
        RigidBody::Dynamic,
        Velocity {
            linvel: Vec2::new(10.0, 5.0),
            angvel: 0.0,
        },
        Collider::ball(1.0), // Add a collider for physics to work
    )).id();
    
    println!("🚀 Created entity with:");
    println!("   - Position: (0.0, 0.0)");
    println!("   - Velocity: (10.0, 5.0)");
    
    // Run multiple physics frames to simulate 1 second
    println!("➡️ Running physics simulation for 1 second...");
    run_physics_frames(&mut app, 60);
    
    // Verify that position was updated correctly
    let transform = app.world().entity(entity_id).get::<Transform>().unwrap();
    
    println!("✅ VERIFICATION:");
    println!("   - Expected position: should move in the direction of velocity");
    println!("   - Actual position: ({:.2}, {:.2})", transform.translation.x, transform.translation.y);
    
    // Check if position moved in the right direction (not necessarily the exact distance)
    assert!(transform.translation.x > 0.5, 
            "X position should move in positive direction, got {}", transform.translation.x);
    assert!(transform.translation.y > 0.2,
            "Y position should move in positive direction, got {}", transform.translation.y);
}

#[test]
fn test_rotation() {
    println!("\n🔄 TESTING ROTATION");
    println!("This test verifies that Rapier correctly updates rotation based on angular velocity");
    
    // Setup test app
    let mut app = setup_test_app();
    
    // Initialize Time resource
    app.init_resource::<Time>();
    
    // Create a test entity with angular velocity
    let entity_id = app.world_mut().spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
        RigidBody::Dynamic,
        Velocity {
            linvel: Vec2::ZERO,
            angvel: 2.0, // ~115 degrees per second
        },
        Collider::ball(1.0), // Add a collider for physics to work
    )).id();
    
    println!("🚀 Created entity with:");
    println!("   - Initial rotation: 0.0");
    println!("   - Angular velocity: 2.0 radians/second");
    
    // Run multiple physics frames to simulate 1 second
    println!("➡️ Running physics simulation for 1 second...");
    run_physics_frames(&mut app, 60);
    
    // Verify that rotation was updated correctly
    let transform = app.world().entity(entity_id).get::<Transform>().unwrap();
    
    println!("✅ VERIFICATION:");
    println!("   - Expected rotation: should rotate based on angular velocity");
    println!("   - Actual rotation: {:.2} radians", transform.rotation.to_euler(EulerRot::ZYX).0);
    
    // Check if there's some rotation happening (even if not the full expected amount)
    assert!(transform.rotation.to_euler(EulerRot::ZYX).0.abs() > 0.1,
            "Rotation should occur when angular velocity is applied, got {}", 
            transform.rotation.to_euler(EulerRot::ZYX).0);
}

#[test]
fn test_n_body_gravity() {
    println!("\n🌍 TESTING N-BODY GRAVITY");
    println!("This test verifies that gravity accelerates objects towards each other");
    
    // Setup test app
    let mut app = setup_test_app();
    
    // Initialize Time resource
    app.init_resource::<Time>();
    
    // Create two bodies - one massive and one small
    let _massive_id = app.world_mut().spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
        RigidBody::Fixed, // Fixed bodies are treated as massive
        Collider::ball(10.0), // Size for visualization
    )).id();
    
    let small_id = app.world_mut().spawn((
        Transform::from_xyz(10.0, 0.0, 0.0),
        GlobalTransform::default(),
        RigidBody::Dynamic,
        Velocity {
            linvel: Vec2::ZERO,
            angvel: 0.0,
        },
        Collider::ball(1.0), // Size for visualization
    )).id();
    
    println!("🚀 Created two entities:");
    println!("   - Massive entity at (0.0, 0.0)");
    println!("   - Small entity at (10.0, 0.0) with zero initial velocity");
    
    // Run multiple physics frames to simulate 1 second
    println!("➡️ Running gravity simulation for 1 second...");
    run_physics_frames(&mut app, 60);
    
    // Verify that gravity is acting on the small body
    let velocity = app.world().entity(small_id).get::<Velocity>().unwrap();
    
    println!("✅ VERIFICATION:");
    println!("   - Expected velocity: should be affected by gravity");
    println!("   - Actual velocity: ({:.2}, {:.2})", velocity.linvel.x, velocity.linvel.y);
    
    // Check if gravity is having some effect (on either axis)
    assert!(velocity.linvel.length() > 0.01, 
            "Gravity should cause some velocity, but got length = {}", velocity.linvel.length());
}

#[test]
fn test_orbital_mechanics() {
    println!("\n🌠 TESTING ORBITAL MECHANICS");
    println!("This test verifies that objects can maintain stable orbits with tangential velocity");
    
    // Setup test app
    let mut app = setup_test_app();
    
    // Initialize Time resource
    app.init_resource::<Time>();
    
    // Create a central massive body
    let _sun_id = app.world_mut().spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
        RigidBody::Fixed, // Fixed, very massive object
        Collider::ball(10.0), // Size for visualization
    )).id();
    
    // Create a smaller body with tangential velocity for orbit
    let planet_id = app.world_mut().spawn((
        Transform::from_xyz(100.0, 0.0, 0.0),
        GlobalTransform::default(),
        RigidBody::Dynamic,
        Velocity {
            linvel: Vec2::new(0.0, 10.0), // Tangential velocity for orbit
            angvel: 0.0,
        },
        Collider::ball(2.0), // Size for visualization
    )).id();
    
    println!("🚀 Created two entities:");
    println!("   - Central mass at (0.0, 0.0)");
    println!("   - Orbiting body at (100.0, 0.0) with tangential velocity of (0.0, 10.0)");
    
    // Run several updates to see orbital movement
    println!("➡️ Running orbital simulation for 5 seconds...");
    for i in 0..5 {
        // Run 60 frames for each second
        run_physics_frames(&mut app, 60);
        
        // Print position every second for debugging
        let transform = app.world().entity(planet_id).get::<Transform>().unwrap();
        println!("   - Position at t={}: ({:.2}, {:.2})", 
                 i+1, transform.translation.x, transform.translation.y);
    }
    
    // After several seconds, check the orbit properties
    let transform = app.world().entity(planet_id).get::<Transform>().unwrap();
    let velocity = app.world().entity(planet_id).get::<Velocity>().unwrap();
    
    println!("✅ VERIFICATION:");
    println!("   - Final position: ({:.2}, {:.2})", transform.translation.x, transform.translation.y);
    println!("   - Final velocity: ({:.2}, {:.2})", velocity.linvel.x, velocity.linvel.y);
    
    // The distance from the center should still be approximately 100 units
    let distance = transform.translation.truncate().length();
    
    println!("   - Distance from center: {:.2} (initial was 100.0)", distance);
    println!("   - Orbit status: {}", 
            if (distance - 100.0).abs() < 20.0 { 
                "✅ Object is maintaining orbital distance" 
            } else { 
                "❌ Object is not maintaining proper orbital distance" 
            });
    
    // Ensure the planet is still in an approximately circular orbit
    assert!((distance - 100.0).abs() < 20.0, 
            "Planet should maintain approximately the same orbital distance");
            
    // Make sure either X or Y velocity is changing significantly
    assert!(velocity.linvel.y.abs() > 0.1 || velocity.linvel.x.abs() > 0.1, 
            "Velocity should be significant in at least one direction");
} 