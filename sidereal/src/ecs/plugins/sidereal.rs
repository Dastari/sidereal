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

        app.register_type::<Transform>();
        app.register_type::<Id>();
        app.register_type::<LinearVelocity>();
        app.register_type::<AngularVelocity>();
        app.register_type::<RigidBody>();
        app.register_type::<Object>();
        app.register_type::<Sector>();
    }
}
