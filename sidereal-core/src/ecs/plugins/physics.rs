use crate::ecs::systems::physics::n_body_gravity_system;
use crate::ecs::components::physics::{PhysicsBody, PhysicsState, BodyType};
use crate::ecs::components::spatial::{UniverseConfig, Position, calculate_entity_cluster, SectorCoords, ClusterCoords};
use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        // Register types
        app.register_type::<PhysicsBody>()
        .register_type::<PhysicsState>();

        // // Register that PhysicsBody requires spatial components
        // app.world_mut().register_required_components::<PhysicsBody, Position>();
        // app.world_mut().register_required_components::<PhysicsBody, SectorCoords>();
        // app.world_mut().register_required_components::<PhysicsBody, ClusterCoords>();

        // Add Rapier physics
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());
        
        // Add gravitational system
        app.add_systems(FixedUpdate, n_body_gravity_system);

        // Add position synchronization system
        // This should run after physics simulation but before boundary checks
        app.add_systems(
            FixedUpdate,
            sync_transform_to_spatial_position.after(PhysicsSet::Writeback)
        );

        // Add sync systems
        app.add_systems(PreUpdate, init_rapier_from_physics_state);
        app.add_systems(
            PostUpdate,
            sync_rapier_to_physics_state.after(PhysicsSet::Writeback)
        );
    }
}

// Add this system to synchronize Transform to spatial components
fn sync_transform_to_spatial_position(
    mut query: Query<(&Transform, &mut Position, &mut SectorCoords, &mut ClusterCoords)>,
    universe_config: Res<UniverseConfig>,
) {
    for (transform, mut position, mut sector_coords, mut cluster_coords) in query.iter_mut() {
        // Update the position from Transform
        position.set(transform.translation.truncate());
        
        // Calculate sector coordinates from position
        let pos = position.get();
        let sector_x = (pos.x / universe_config.sector_size).floor() as i32;
        let sector_y = (pos.y / universe_config.sector_size).floor() as i32;
        sector_coords.set(IVec2::new(sector_x, sector_y));
        
        // Recalculate cluster coordinates based on the new position
        let new_cluster_coords = calculate_entity_cluster(pos, &universe_config);
        cluster_coords.set(new_cluster_coords);
    }
}

// System to initialize Rapier components when a new entity with PhysicsState is spawned
fn init_rapier_from_physics_state(
    mut commands: Commands,
    query: Query<(Entity, &PhysicsState), Added<PhysicsState>>,
    time: Res<Time>,
) {
    let current_time = time.elapsed_secs_f64();
    
    for (entity, physics_state) in query.iter() {
        // Create Rapier rigid body
        let rigid_body = physics_state.to_rapier_body_type();
        
        // Create Rapier collider
        let collider = physics_state.to_rapier_collider();
        
        // Set up Rapier components
        commands.entity(entity)
            .insert(rigid_body)
            .insert(collider)
            .insert(Velocity {
                linvel: physics_state.linear_velocity,
                angvel: physics_state.angular_velocity,
            })
            .insert(Damping {
                linear_damping: physics_state.linear_damping,
                angular_damping: physics_state.angular_damping,
            })
            .insert(AdditionalMassProperties::Mass(physics_state.mass))
            .insert(Sleeping::default());
        
        // Update last sync time
        if let Some(mut state) = commands.get_entity(entity) {
            state.insert(PhysicsState {
                last_sync: current_time,
                ..*physics_state
            });
        }
    }
}

// System to update our PhysicsState from Rapier's components after physics simulations
fn sync_rapier_to_physics_state(
    mut query: Query<(
        &RigidBody,
        &Collider,
        &Velocity,
        Option<&Damping>,
        Option<&ReadMassProperties>,
        &mut PhysicsState,
    )>,
    time: Res<Time>,
) {
    let current_time = time.elapsed_secs_f64();
    
    for (rigid_body, collider, velocity, damping, mass_props, mut physics_state) in query.iter_mut() {
        // Update velocity
        physics_state.linear_velocity = velocity.linvel;
        physics_state.angular_velocity = velocity.angvel;
        
        // Update damping if available
        if let Some(damping) = damping {
            physics_state.linear_damping = damping.linear_damping;
            physics_state.angular_damping = damping.angular_damping;
        }
        
        // Update body type
        physics_state.body_type = match rigid_body {
            RigidBody::Dynamic => BodyType::Dynamic,
            RigidBody::Fixed => BodyType::Static,
            _ => BodyType::Kinematic,
        };
        
        // Update mass if available
        if let Some(mass_props) = mass_props {
            physics_state.mass = mass_props.mass;
            physics_state.center_of_mass = mass_props.local_center_of_mass;
        }
        
        // Update last sync time
        physics_state.last_sync = current_time;
    }
}


