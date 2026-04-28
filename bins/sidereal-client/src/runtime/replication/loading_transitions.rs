pub(crate) fn transition_world_loading_to_in_world(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    session: Res<'_, ClientSession>,
    session_ready: Res<'_, SessionReadyState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    entity_guids: Query<'_, '_, &'_ EntityGuid>,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
) {
    if !app_state
        .as_ref()
        .is_some_and(|state| **state == ClientAppState::WorldLoading)
    {
        return;
    }
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    if session_ready.ready_player_entity_id.as_deref() != Some(local_player_entity_id.as_str()) {
        return;
    }
    let has_local_player_entity = has_local_player_runtime_presence(
        &entity_registry,
        local_player_entity_id,
        entity_guids.iter(),
    );
    if !has_local_player_entity {
        return;
    }
    next_state.set(ClientAppState::AssetLoading);
}

pub(crate) fn transition_asset_loading_to_in_world(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    asset_bootstrap_state: Res<'_, super::auth_net::AssetBootstrapRequestState>,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
) {
    if !app_state
        .as_ref()
        .is_some_and(|state| **state == ClientAppState::AssetLoading)
    {
        return;
    }
    if !asset_bootstrap_state.completed || asset_bootstrap_state.failed {
        return;
    }
    next_state.set(ClientAppState::InWorld);
}

