use bevy::prelude::*;

pub mod actions;
pub mod character_movement;
pub mod collision_outline_generation;
pub mod combat;
pub mod component_meta;
pub mod components;
pub mod editor_schema;
pub mod flight;
pub mod generated;
pub mod hierarchy;
pub mod mass;
pub mod procedural_sprite_generation;
pub mod render_layers;
pub mod visibility_range;
pub mod world_spatial;

// Re-export commonly used items
pub use actions::*;
pub use character_movement::{
    process_character_movement_actions, sync_player_to_controlled_entity,
};
pub use collision_outline_generation::{
    compute_collision_half_extents_from_rgba_alpha,
    compute_collision_half_extents_from_sprite_length, generate_rdp_collision_outline_from_rgba,
    generate_rdp_collision_outline_from_sprite_png,
};
pub use combat::{
    BallisticProjectileSpawnedEvent, EntityDestroyedEvent, EntityDestructionStartedEvent,
    ShotFiredEvent, ShotHitEvent, ShotImpactResolvedEvent, advance_pending_destructions,
    apply_damage_from_shot_impacts, begin_pending_destructions, bootstrap_weapon_cooldown_state,
    process_weapon_fire_actions, resolve_shot_impacts, tick_weapon_cooldowns,
    update_ballistic_projectiles,
};
pub use component_meta::*;
pub use components::*;
pub use editor_schema::*;
pub use generated::components::*;
pub use hierarchy::sync_mounted_hierarchy;
pub use mass::{
    bootstrap_collision_profiles_from_aabb, bootstrap_root_dynamic_entity_colliders,
    bootstrap_root_dynamic_mass_components, collider_from_collision_shape, recompute_total_mass,
};
pub use procedural_sprite_generation::{
    ProceduralSpriteImageSet, compute_collision_half_extents_from_procedural_sprite,
    generate_procedural_sprite_image_set, generate_rdp_collision_outline_from_procedural_sprite,
};
pub use render_layers::{
    DEFAULT_MAIN_WORLD_LAYER_ID, default_main_world_render_layer, is_valid_phase_domain_pair,
    is_valid_render_domain, is_valid_render_phase, known_component_kinds,
    validate_runtime_post_process_stack, validate_runtime_render_layer_definition,
    validate_runtime_render_layer_rule, validate_runtime_world_visual_stack,
};
pub use visibility_range::{apply_visibility_range_buff, total_visibility_range_for_parent};
pub use world_spatial::{resolve_world_position, resolve_world_rotation_rad};

#[derive(Resource, Debug, Clone, Copy)]
pub struct CombatAuthorityEnabled(pub bool);

impl Default for CombatAuthorityEnabled {
    fn default() -> Self {
        Self(true)
    }
}

// Re-export flight systems (not components, those come from generated)
pub use flight::{
    angular_inertia_from_size, apply_engine_thrust, clamp_angular_velocity,
    compute_brake_decel_accel_mps2, grant_flight_control_authority_system,
    grant_simulation_motion_writer_system, process_flight_actions,
    revoke_stale_flight_control_authority_system, revoke_stale_simulation_motion_writer_system,
    sanitize_planar_angular_velocity, stabilize_idle_motion,
};

/// Controls whether local Bevy hierarchy reconstruction runs in this runtime.
/// Replication server disables this to avoid leaking ChildOf/Children into network replication.
#[derive(Resource, Debug, Clone, Copy)]
pub struct HierarchyRebuildEnabled(pub bool);

impl Default for HierarchyRebuildEnabled {
    fn default() -> Self {
        Self(true)
    }
}

fn hierarchy_rebuild_enabled(flag: Option<Res<HierarchyRebuildEnabled>>) -> bool {
    flag.map(|v| v.0).unwrap_or(true)
}

/// Registers gameplay component types and reflection metadata only.
/// Does NOT add simulation systems. Use on the client where simulation
/// is server-authoritative and local flight systems must not run.
pub struct SiderealGameCorePlugin;

impl Plugin for SiderealGameCorePlugin {
    fn build(&self, app: &mut App) {
        generated::components::register_generated_components(app);

        app.register_type::<EntityAction>()
            .register_type::<ActionQueue>()
            .register_type::<ActionCapabilities>()
            .register_type::<FlightControlAuthority>()
            .register_type::<SimulationMotionWriter>();
    }
}

/// Full gameplay plugin: component registration + simulation systems.
/// Use on server/replication where authoritative simulation runs.
pub struct SiderealGamePlugin;

impl Plugin for SiderealGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SiderealGameCorePlugin);
        if app
            .world()
            .get_resource::<HierarchyRebuildEnabled>()
            .is_none()
        {
            app.insert_resource(HierarchyRebuildEnabled::default());
        }
        if app
            .world()
            .get_resource::<CombatAuthorityEnabled>()
            .is_none()
        {
            app.insert_resource(CombatAuthorityEnabled::default());
        }
        app.add_message::<ShotFiredEvent>();
        app.add_message::<ShotImpactResolvedEvent>();
        app.add_message::<ShotHitEvent>();
        app.add_message::<BallisticProjectileSpawnedEvent>();
        app.add_message::<EntityDestructionStartedEvent>();
        app.add_message::<EntityDestroyedEvent>();

        let add_hierarchy_rebuild = app
            .world()
            .get_resource::<HierarchyRebuildEnabled>()
            .map(|flag| flag.0)
            .unwrap_or(true);
        if add_hierarchy_rebuild {
            app.add_systems(
                PostUpdate,
                sync_mounted_hierarchy
                    .before(bevy::transform::TransformSystems::Propagate)
                    .run_if(hierarchy_rebuild_enabled),
            );
        }
        app.add_systems(
            PostUpdate,
            (
                bootstrap_root_dynamic_mass_components,
                bootstrap_collision_profiles_from_aabb,
                bootstrap_root_dynamic_entity_colliders,
            ),
        );
        app.add_systems(
            FixedUpdate,
            (
                grant_flight_control_authority_system,
                revoke_stale_flight_control_authority_system,
                grant_simulation_motion_writer_system,
                revoke_stale_simulation_motion_writer_system,
                validate_action_capabilities,
                process_character_movement_actions,
                process_flight_actions,
                bootstrap_weapon_cooldown_state,
                tick_weapon_cooldowns,
                process_weapon_fire_actions,
                update_ballistic_projectiles,
                resolve_shot_impacts,
                apply_damage_from_shot_impacts,
                begin_pending_destructions,
                advance_pending_destructions,
                recompute_total_mass,
                apply_engine_thrust,
            )
                .chain()
                .before(avian2d::prelude::PhysicsSystems::StepSimulation),
        );
        app.add_systems(
            FixedUpdate,
            (
                stabilize_idle_motion,
                clamp_angular_velocity,
                sync_player_to_controlled_entity,
            )
                .chain()
                .after(avian2d::prelude::PhysicsSystems::StepSimulation),
        );
    }
}
