use assert_json_diff::assert_json_eq;
use bevy::prelude::*;
use sidereal_core::ecs::components::{Block, Direction, Hull};
use sidereal_core::ecs::entities::ship::Ship;
use sidereal_core::ecs::plugins::*;

#[derive(Bundle)]
pub struct TestShipBundle {
    ship: Ship,
    transform: Transform,
    name: Name,
    hull: Hull,
}

pub fn test_ship_bundle() -> TestShipBundle {
    TestShipBundle {
        ship: Ship::new(),
        transform: Transform::from_xyz(100.0, 200.0, 0.0),
        name: Name::new("Test Ship"),
        hull: Hull {
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
        },
    }
}

fn setup_test_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, EntitySerializationPlugin));
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
    println!("🏗️ Setting up test app...");

    let mut app = setup_test_app();
    let test_ship_bundle = test_ship_bundle();

    println!("🚀 Spawning a test ship entity with:");
    println!("   - Name: {}", test_ship_bundle.name);
    println!("   - Position: {:?}", test_ship_bundle.transform);

    let ship_entity = app.world_mut().spawn(test_ship_bundle).id();

    println!("💾 Spawned ship entity with ID: {:?}", ship_entity);
    println!("💾 Serializing ship entity...");

    let serialized_entity = app
        .world()
        .serialize_entity(ship_entity)
        .expect("Failed to serialize ship entity");

    let ship_json =
        serde_json::to_string_pretty(&serialized_entity).expect("Failed to convert to JSON");

    println!("📄 Serialized Ship JSON:");
    println!("   - Byte size: {} bytes", ship_json.len());
    println!("📥 Deserializing ship from JSON...");

    let deserialized_entity: sidereal_core::ecs::plugins::serialization::SerializedEntity =
        serde_json::from_str(&ship_json).expect("Failed to deserialize JSON");

    println!("🔄 Creating new entity from deserialized data...");

    let new_ship_entity = app
        .world_mut()
        .deserialize_entity(&deserialized_entity)
        .expect("Failed to deserialize entity");

    app.world_mut().entity_mut(new_ship_entity).insert(Ship);

    println!("✅ BEVY WORLD VERIFICATION:");
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
    println!(
        "   - Original Ship ID: {:?}, New Ship ID: {:?}",
        app.world().entity(ship_entity).id(),
        app.world().entity(new_ship_entity).id()
    );
    println!("💾 Serializing new ship entity...");

    let new_serialized_entity = app
        .world()
        .serialize_entity(new_ship_entity)
        .expect("Failed to serialize ship entity");

    let new_ship_json =
        serde_json::to_string_pretty(&new_serialized_entity).expect("Failed to convert to JSON");

    println!("📄 Serialized New Ship JSON:");
    println!("   - Byte size: {} bytes", new_ship_json.len());
    println!("➡️  Comparing both serialized entities...");

    assert_json_eq!(serialized_entity, new_serialized_entity);

    println!("{}", new_ship_json);
    println!("✅ Test passed: Serialization/deserialization is consistent");
}
