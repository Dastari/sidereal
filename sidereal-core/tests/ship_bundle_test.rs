use bevy::prelude::*;

use sidereal_core::ecs::components::physics::{Velocity, AngularVelocity};
use sidereal_core::ecs::components::rotation::Rotation;
use sidereal_core::ecs::components::position::Position;
use sidereal_core::ecs::components::hull::Hull;
use sidereal_core::ecs::components::name::Name;

// First, let's make sure we have all the necessary components
// We need to define our own test version of ShipBundle since it's not public in the core library
#[derive(Bundle)]
struct TestShipBundle {
    name: Name,
    position: Position,
    velocity: Velocity,
    rotation: Rotation,
    angular_velocity: AngularVelocity,
    hull: Hull,
}

// This test verifies that a ShipBundle can be spawned correctly
#[test]
fn test_ship_bundle_spawn() {
    // Create a new app with minimal plugins for testing
    let mut app = App::new();
    
    // Add minimal plugins required for entity creation
    app.add_plugins(MinimalPlugins);
    
    // Create a test world and spawn a TestShipBundle
    let ship_id = app.world_mut().spawn(TestShipBundle {
        name: Name::new("Test Ship"),
        position: Position { x: 100.0, y: 200.0 },
        velocity: Velocity { x: 10.0, y: 20.0 },
        rotation: Rotation(0.5),
        angular_velocity: AngularVelocity(0.1),
        hull: Hull {
            width: 50.0,
            height: 100.0,
            blocks: vec![],
        },
    }).id();
    
    // Verify that the entity exists and has the expected components
    let entity = app.world().entity(ship_id);
    
    assert!(entity.contains::<Name>());
    assert!(entity.contains::<Position>());
    assert!(entity.contains::<Velocity>());
    assert!(entity.contains::<Rotation>());
    assert!(entity.contains::<AngularVelocity>());
    assert!(entity.contains::<Hull>());
    
    // Verify component values
    let position = entity.get::<Position>().unwrap();
    assert_eq!(position.x, 100.0);
    assert_eq!(position.y, 200.0);
    
    let velocity = entity.get::<Velocity>().unwrap();
    assert_eq!(velocity.x, 10.0);
    assert_eq!(velocity.y, 20.0);
    
    let rotation = entity.get::<Rotation>().unwrap();
    assert_eq!(rotation.0, 0.5);
    
    let angular_velocity = entity.get::<AngularVelocity>().unwrap();
    assert_eq!(angular_velocity.0, 0.1);
    
    let hull = entity.get::<Hull>().unwrap();
    assert_eq!(hull.width, 50.0);
    assert_eq!(hull.height, 100.0);
    assert_eq!(hull.blocks.len(), 0);
}
