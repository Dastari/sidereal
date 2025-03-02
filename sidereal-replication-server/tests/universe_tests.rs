use bevy::math::{IVec2, Vec2};
use bevy::prelude::*;
use bevy_state::app::StatesPlugin;
use sidereal_core::ecs::components::{
    is_approaching_boundary, BoundaryDirection, EntityApproachingBoundary, SpatialPosition,
    UniverseConfig, UniverseState,
};
use sidereal_replication_server::scene::SceneState;
use sidereal_replication_server::universe::plugin::UniverseManagerPlugin;
use sidereal_replication_server::universe::systems::update_entity_sector_coordinates;

#[test]
fn test_sector_calculation() {
    // Create a universe config with a specific sector size
    let config = UniverseConfig {
        sector_size: 1000.0,
        cluster_dimensions: IVec2::new(3, 3),
        empty_sector_timeout_seconds: 300.0,
        empty_sector_check_interval: 60.0,
        transition_zone_width: 50.0,
        min_boundary_awareness: 30.0, // Add missing fields
        velocity_awareness_factor: 2.0,
    };

    // Test various positions and verify they map to the correct sectors
    let positions = vec![
        (Vec2::new(0.0, 0.0), IVec2::new(0, 0), IVec2::new(0, 0)), // Origin
        (Vec2::new(999.0, 999.0), IVec2::new(0, 0), IVec2::new(0, 0)), // Just inside first sector
        (
            Vec2::new(1000.0, 1000.0),
            IVec2::new(1, 1),
            IVec2::new(0, 0),
        ), // Just inside second sector
        (
            Vec2::new(-1.0, -1.0),
            IVec2::new(-1, -1),
            IVec2::new(-1, -1),
        ), // Negative coordinates
        (
            Vec2::new(3500.0, 2500.0),
            IVec2::new(3, 2),
            IVec2::new(1, 0),
        ), // Further out
    ];

    for (position, expected_sector, expected_cluster) in positions {
        // Calculate sector coordinates
        let sector_x = (position.x / config.sector_size).floor() as i32;
        let sector_y = (position.y / config.sector_size).floor() as i32;
        let sector_coords = IVec2::new(sector_x, sector_y);

        // Calculate cluster coordinates
        let cluster_x = (sector_x as f32 / config.cluster_dimensions.x as f32).floor() as i32;
        let cluster_y = (sector_y as f32 / config.cluster_dimensions.y as f32).floor() as i32;
        let cluster_coords = IVec2::new(cluster_x, cluster_y);

        // Assert sector calculation is correct
        assert_eq!(
            sector_coords, expected_sector,
            "Position {:?} should map to sector {:?}, got {:?}",
            position, expected_sector, sector_coords
        );

        // Assert cluster calculation is correct
        assert_eq!(
            cluster_coords, expected_cluster,
            "Sector {:?} should map to cluster {:?}, got {:?}",
            sector_coords, expected_cluster, cluster_coords
        );
    }
}

#[test]
fn test_universe_state_initialization() {
    // Create a minimal app to test the initialization
    let mut app = App::new();

    // Add the minimum required plugins including StatesPlugin
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin::default());

    // Add our universe plugin
    app.add_plugins(UniverseManagerPlugin);

    // Add the scene state and set it to Ready to trigger initialization
    app.init_state::<SceneState>();
    app.insert_state(SceneState::Ready);

    // Run the startup systems
    app.update();

    // Check that the universe state was initialized
    let universe_state = app.world().resource::<UniverseState>();

    // Verify the initial state
    assert!(
        !universe_state.active_clusters.is_empty(),
        "Universe state should have at least one active cluster"
    );

    // Verify we have the default 3x3 cluster at the origin
    let origin_cluster = universe_state.active_clusters.get(&IVec2::new(0, 0));
    assert!(
        origin_cluster.is_some(),
        "Universe state should have a cluster at the origin"
    );

    // Verify the cluster has the expected sectors
    if let Some(cluster) = origin_cluster {
        assert_eq!(
            cluster.sectors.len(),
            9,
            "Origin cluster should have 9 sectors (3x3)"
        );
    }
}

#[test]
fn test_approaching_boundary_detection() {
    // Create a universe config
    let config = UniverseConfig {
        sector_size: 1000.0,
        cluster_dimensions: IVec2::new(3, 3),
        empty_sector_timeout_seconds: 300.0,
        empty_sector_check_interval: 60.0,
        transition_zone_width: 50.0,
        min_boundary_awareness: 30.0, // Add missing fields
        velocity_awareness_factor: 2.0,
    };

    // Create a spatial position near the eastern boundary
    let eastern_boundary = SpatialPosition {
        position: Vec2::new(990.0, 500.0),
        sector_coords: IVec2::new(0, 0),
        cluster_coords: IVec2::new(0, 0),
    };

    // Check if approaching boundary
    let direction = is_approaching_boundary(&eastern_boundary, None, &config);
    assert_eq!(
        direction,
        Some(BoundaryDirection::East),
        "Entity at {:?} should be approaching the eastern boundary",
        eastern_boundary.position
    );

    // Create a spatial position near the northern boundary
    let northern_boundary = SpatialPosition {
        position: Vec2::new(500.0, 10.0), // Changed from 990.0 to 10.0 to be near the top (North)
        sector_coords: IVec2::new(0, 0),
        cluster_coords: IVec2::new(0, 0),
    };

    // Check if approaching boundary
    let direction = is_approaching_boundary(&northern_boundary, None, &config);
    assert_eq!(
        direction,
        Some(BoundaryDirection::North),
        "Entity at {:?} should be approaching the northern boundary",
        northern_boundary.position
    );

    // Create a spatial position not near any boundary
    let center = SpatialPosition {
        position: Vec2::new(500.0, 500.0),
        sector_coords: IVec2::new(0, 0),
        cluster_coords: IVec2::new(0, 0),
    };

    // Check if approaching boundary
    let direction = is_approaching_boundary(&center, None, &config);
    assert_eq!(
        direction, None,
        "Entity at {:?} should not be approaching any boundary",
        center.position
    );
}

#[test]
fn test_entity_sector_update() {
    // Create a minimal app
    let mut app = App::new();

    // Add required plugins and resources
    app.add_plugins(MinimalPlugins)
        .init_resource::<UniverseConfig>()
        .add_event::<EntityApproachingBoundary>();

    // Spawn an entity with spatial tracking
    let entity = {
        let world = app.world_mut();
        world
            .spawn((
                Transform::from_xyz(1500.0, 500.0, 0.0),
                SpatialPosition {
                    position: Vec2::new(0.0, 0.0),    // Will be updated by the system
                    sector_coords: IVec2::new(0, 0),  // Will be updated
                    cluster_coords: IVec2::new(0, 0), // Will be updated
                },
            ))
            .id()
    };

    // Run the update system
    app.add_systems(Update, update_entity_sector_coordinates);
    app.update();

    // Retrieve the updated entity
    let spatial_pos = {
        let world = app.world();
        world
            .entity(entity)
            .get::<SpatialPosition>()
            .unwrap()
            .clone()
    };

    // Verify the position was updated correctly
    assert_eq!(
        spatial_pos.position,
        Vec2::new(1500.0, 500.0),
        "Entity position should be updated to match transform"
    );

    // Verify the sector coordinates were calculated correctly
    assert_eq!(
        spatial_pos.sector_coords,
        IVec2::new(1, 0),
        "Entity should be in sector (1, 0)"
    );

    // Verify the cluster coordinates were calculated correctly
    assert_eq!(
        spatial_pos.cluster_coords,
        IVec2::new(0, 0),
        "Entity should be in cluster (0, 0)"
    );
}

// Add more tests as needed
