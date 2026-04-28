pub(crate) fn prune_runtime_entity_registry_system(
    mut entity_registry: ResMut<'_, RuntimeEntityHierarchy>,
    mut remote_registry: ResMut<'_, RemoteEntityRegistry>,
    live_entities: Query<'_, '_, ()>,
) {
    entity_registry
        .by_entity_id
        .retain(|_, entity| live_entities.get(*entity).is_ok());
    remote_registry
        .by_entity_id
        .retain(|_, entity| live_entities.get(*entity).is_ok());
}

