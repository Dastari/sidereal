use avian2d::prelude::Position;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use sidereal_game::{
    Destructible, EntityGuid, HealthPool, PendingDestruction, PendingDestructionPhase,
    advance_pending_destructions, begin_pending_destructions,
};
use std::time::Duration;
use uuid::Uuid;

#[test]
fn depleted_destructible_enters_pending_then_despawns() {
    let mut app = App::new();
    app.insert_resource(Time::<Fixed>::from_hz(60.0));
    app.add_message::<sidereal_game::EntityDestructionStartedEvent>();
    app.add_message::<sidereal_game::EntityDestroyedEvent>();

    let entity = app
        .world_mut()
        .spawn((
            EntityGuid(Uuid::new_v4()),
            Position(Vec2::new(14.0, -8.0).into()),
            HealthPool {
                current: 0.0,
                maximum: 100.0,
            },
            Destructible {
                destruction_profile_id: "explosion_burst".to_string(),
                destroy_delay_s: 0.18,
            },
        ))
        .id();

    let _ = app.world_mut().run_system_once(begin_pending_destructions);

    let pending = app
        .world()
        .entity(entity)
        .get::<PendingDestruction>()
        .expect("pending destruction should be inserted");
    assert_eq!(pending.phase, PendingDestructionPhase::EffectDelay);

    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .advance_by(Duration::from_millis(200));
    let _ = app
        .world_mut()
        .run_system_once(advance_pending_destructions);

    let pending = app
        .world()
        .entity(entity)
        .get::<PendingDestruction>()
        .expect("entity should stay alive for destroyed-event dispatch");
    assert_eq!(
        pending.phase,
        PendingDestructionPhase::AwaitDestroyedEventDispatch
    );

    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .advance_by(Duration::from_millis(17));
    let _ = app
        .world_mut()
        .run_system_once(advance_pending_destructions);

    assert!(
        app.world().get_entity(entity).is_err(),
        "entity should despawn after the destroyed-event dispatch tick"
    );
}
