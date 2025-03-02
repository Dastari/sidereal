use bevy::math::Vec2;
use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json;

/// Serialize/deserialize friendly representation of physics components
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PhysicsData {
    // Transform components
    pub position: Option<[f32; 2]>,
    pub rotation: Option<f32>,

    // Rapier components
    pub rigid_body_type: Option<String>, // "dynamic", "fixed", "kinematic"
    pub velocity: Option<[f32; 3]>,      // [linvel.x, linvel.y, angvel]
    pub collider_shape: Option<ColliderShapeData>,
    pub mass: Option<f32>,
    pub friction: Option<f32>,
    pub restitution: Option<f32>,
    pub gravity_scale: Option<f32>,
}

/// Representation of different collider shapes
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ColliderShapeData {
    Ball { radius: f32 },
    Cuboid { hx: f32, hy: f32 },
    Capsule { half_height: f32, radius: f32 },
    // Add more shapes as needed
}

impl PhysicsData {
    /// Create physics data from Bevy/Rapier components
    pub fn from_components(
        transform: Option<&Transform>,
        rigid_body: Option<&RigidBody>,
        velocity: Option<&Velocity>,
        _collider: Option<&Collider>, // Currently unused but will be used for collider shape extraction
        mass_props: Option<&AdditionalMassProperties>,
        friction: Option<&Friction>,
        restitution: Option<&Restitution>,
        gravity_scale: Option<&GravityScale>,
    ) -> Self {
        let position = transform.map(|t| [t.translation.x, t.translation.y]);
        let rotation = transform.map(|t| t.rotation.to_euler(EulerRot::XYZ).2);

        let rigid_body_type = rigid_body.map(|rb| match rb {
            RigidBody::Dynamic => "dynamic".to_string(),
            RigidBody::Fixed => "fixed".to_string(),
            RigidBody::KinematicPositionBased => "kinematic_position".to_string(),
            RigidBody::KinematicVelocityBased => "kinematic_velocity".to_string(),
        });

        let velocity = velocity.map(|v| [v.linvel.x, v.linvel.y, v.angvel]);

        // For now, we'll just use a simplified approach for colliders
        // This is a placeholder that will need to be revised based on the actual API
        let collider_shape = None; // We'll implement this properly once we understand the API better

        // Extract mass from AdditionalMassProperties if available
        let mass = mass_props.and_then(|mp| match mp {
            AdditionalMassProperties::Mass(m) => Some(*m),
            AdditionalMassProperties::MassProperties(mp) => Some(mp.mass),
        });

        let friction = friction.map(|f| f.coefficient);
        let restitution = restitution.map(|r| r.coefficient);
        let gravity_scale = gravity_scale.map(|g| g.0);

        Self {
            position,
            rotation,
            rigid_body_type,
            velocity,
            collider_shape,
            mass,
            friction,
            restitution,
            gravity_scale,
        }
    }

    /// Apply physics data to an entity command
    pub fn apply_to_entity(&self, entity: &mut EntityCommands) {
        // Apply Transform if position or rotation is specified
        if self.position.is_some() || self.rotation.is_some() {
            let mut transform = Transform::default();

            if let Some(pos) = self.position {
                transform.translation.x = pos[0];
                transform.translation.y = pos[1];
            }

            if let Some(rot) = self.rotation {
                transform.rotation = Quat::from_rotation_z(rot);
            }

            entity.insert(transform);
        }

        // Apply RigidBody if specified
        if let Some(ref body_type) = self.rigid_body_type {
            let rigid_body = match body_type.as_str() {
                "dynamic" => RigidBody::Dynamic,
                "fixed" => RigidBody::Fixed,
                "kinematic_position" => RigidBody::KinematicPositionBased,
                "kinematic_velocity" => RigidBody::KinematicVelocityBased,
                _ => RigidBody::Dynamic, // Default to dynamic if unknown
            };

            entity.insert(rigid_body);
        }

        // Apply Velocity if specified
        if let Some(vel) = self.velocity {
            entity.insert(Velocity {
                linvel: Vec2::new(vel[0], vel[1]),
                angvel: vel[2],
            });
        }

        // Apply Collider if shape is specified
        if let Some(ref shape) = self.collider_shape {
            let collider = match shape {
                ColliderShapeData::Ball { radius } => Collider::ball(*radius),
                ColliderShapeData::Cuboid { hx, hy } => Collider::cuboid(*hx, *hy),
                ColliderShapeData::Capsule {
                    half_height,
                    radius,
                } => Collider::capsule_y(*half_height, *radius),
            };

            entity.insert(collider);
        }

        // Apply optional physics properties
        if let Some(mass) = self.mass {
            entity.insert(AdditionalMassProperties::Mass(mass));
        }

        if let Some(friction) = self.friction {
            entity.insert(Friction::coefficient(friction));
        }

        if let Some(restitution) = self.restitution {
            entity.insert(Restitution::coefficient(restitution));
        }

        if let Some(gravity_scale) = self.gravity_scale {
            entity.insert(GravityScale(gravity_scale));
        }
    }

    /// Convert physics data to JSON for database storage
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Create physics data from JSON (from database)
    pub fn from_json(json: &serde_json::Value) -> Option<Self> {
        serde_json::from_value(json.clone()).ok()
    }
}

// Extension trait for easy extraction of physics data from an entity
pub trait PhysicsExtractExt {
    fn extract_physics_data(&self) -> PhysicsData;
}

impl PhysicsExtractExt for EntityRef<'_> {
    fn extract_physics_data(&self) -> PhysicsData {
        PhysicsData::from_components(
            self.get::<Transform>(),
            self.get::<RigidBody>(),
            self.get::<Velocity>(),
            self.get::<Collider>(),
            self.get::<AdditionalMassProperties>(),
            self.get::<Friction>(),
            self.get::<Restitution>(),
            self.get::<GravityScale>(),
        )
    }
}
