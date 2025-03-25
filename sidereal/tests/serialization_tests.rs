use bevy::prelude::*;
use bevy_remote::{RemotePlugin, http::RemoteHttpPlugin};
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::RepliconRenetPlugins;
use serde_json::Value;
use sidereal::ecs::components::Object;
use sidereal::ecs::plugins::SiderealPlugin;
use sidereal::serialization::{serialize_entity, update_entity};

#[derive(Component, Default, Debug)]
#[require(Object)]
pub struct TestEntity;

impl TestEntity {
    pub fn mock() -> Self {
        Self::default()
    }
}

fn setup_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        HierarchyPlugin,
        TransformPlugin,
        RemotePlugin::default(),
        RemoteHttpPlugin::default(),
        RepliconPlugins,
        RepliconRenetPlugins,
        SiderealPlugin,
    ));
    debug_assert!(
        app.world().contains_resource::<Time>(),
        "Time resource should be initialized"
    );
    app
}

#[test]
fn test_serialization() {
    let mut app = setup_app();

    // Create and set up original entity
    let test_entity: TestEntity = TestEntity::mock();
    let test_entity_id: Entity = app.world_mut().spawn(test_entity).id();
    app.update();

    // Serialize original entity
    let test_entity_ref = app.world().get_entity(test_entity_id).unwrap();
    let test_entity_serialized = serialize_entity(test_entity_ref, &app.world());
    let test_entity_json: Value = serde_json::from_str(&test_entity_serialized)
        .expect("Original entity should deserialize to valid JSON");

    println!("Test entity serialized: {}", test_entity_serialized);

    // Update world with serialized entity (should find matching ID and update)
    let same_id_entity = update_entity(&test_entity_serialized, &mut app.world_mut())
        .expect("Should successfully update entity from serialized data");

    // In this case, with the same ID, update_entity should find the existing entity
    assert_eq!(
        test_entity_id, same_id_entity,
        "update_entity should find and update the existing entity with matching ID"
    );

    // Verify the serialized data is the same
    let same_id_entity_ref = app
        .world()
        .get_entity(same_id_entity)
        .expect("Updated entity should exist in world");
    let same_id_serialized = serialize_entity(same_id_entity_ref, &app.world());
    let same_id_json: Value = serde_json::from_str(&same_id_serialized)
        .expect("Updated entity should deserialize to valid JSON");

    assert_eq!(
        test_entity_json, same_id_json,
        "Original and updated entity serializations should match"
    );

    // PART 2: Test with a modified ID to ensure a new entity is created

    // Clone and modify the original JSON with a different ID
    let mut modified_json = test_entity_json.clone();
    if let Some(id_value) = modified_json.get_mut("sidereal::ecs::components::id::Id") {
        // Generate a new valid UUID string for testing
        *id_value = Value::String("00000000-0000-0000-0000-000000000000".to_string());
    } else {
        panic!("Original entity should have ID component");
    }

    // Convert back to string
    let modified_serialized =
        serde_json::to_string(&modified_json).expect("Modified JSON should serialize");

    // Update world with modified serialized entity (should create new entity with different ID)
    let new_entity = update_entity(&modified_serialized, &mut app.world_mut())
        .expect("Should successfully create entity from modified serialized data");

    // Verify it's a different entity
    assert_ne!(
        test_entity_id, new_entity,
        "With different ID, update_entity should create a new entity"
    );

    // Check both entities exist in the world
    assert!(
        app.world().get_entity(test_entity_id).is_ok(),
        "Original entity should still exist"
    );
    assert!(
        app.world().get_entity(new_entity).is_ok(),
        "New entity should exist"
    );

    // Serialize the new entity
    let new_entity_ref = app
        .world()
        .get_entity(new_entity)
        .expect("New entity should exist in world");
    let new_entity_serialized = serialize_entity(new_entity_ref, &app.world());
    let new_entity_json: Value = serde_json::from_str(&new_entity_serialized)
        .expect("New entity should deserialize to valid JSON");

    // Verify the ID is different but other components match
    assert_ne!(
        test_entity_json.get("sidereal::ecs::components::id::Id"),
        new_entity_json.get("sidereal::ecs::components::id::Id"),
        "Entity IDs should be different"
    );

    // Check that both entities have the Object component
    assert!(
        test_entity_json
            .get("sidereal::ecs::components::object::Object")
            .is_some(),
        "Original entity should have Object component"
    );
    assert!(
        new_entity_json
            .get("sidereal::ecs::components::object::Object")
            .is_some(),
        "New entity should have Object component"
    );
}
