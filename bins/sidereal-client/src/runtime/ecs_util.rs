use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;

use super::app_state::ClientAppState;

/// Queue a despawn that silently no-ops if the entity no longer exists.
pub(super) fn queue_despawn_if_exists(commands: &mut Commands<'_, '_>, entity: Entity) {
    commands.queue(move |world: &mut World| {
        // Optional/manual cleanup should not race with state-scoped teardown.
        // If the entity is already owned by `DespawnOnExit`, let that path remain the single writer.
        if world
            .get_entity(entity)
            .is_ok_and(|entity_ref| entity_ref.contains::<DespawnOnExit<ClientAppState>>())
        {
            return;
        }
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
    });
}

/// Queue a despawn that silently no-ops if the entity no longer exists, even when
/// the entity is state-scoped via `DespawnOnExit`.
pub(super) fn queue_despawn_if_exists_force(commands: &mut Commands<'_, '_>, entity: Entity) {
    commands.queue(move |world: &mut World| {
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
    });
}
