use crate::ecs::components::id::Id;
use crate::ecs::components::*;
use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_replicon::prelude::*;
pub struct SiderealPlugin;

impl Plugin for SiderealPlugin {
    fn build(&self, app: &mut App) {
        app.replicate::<Name>()
            .replicate::<Transform>()
            .replicate::<Id>()
            .replicate::<LinearVelocity>()
            .replicate::<AngularVelocity>()
            .replicate::<RigidBody>()
            .replicate::<Object>()
            .replicate::<Sector>();

        app.register_type::<Name>()
            .register_type::<Transform>()
            .register_type::<Id>()
            .register_type::<LinearVelocity>()
            .register_type::<AngularVelocity>()
            .register_type::<RigidBody>()
            .register_type::<Object>()
            .register_type::<Sector>();
    }
} 