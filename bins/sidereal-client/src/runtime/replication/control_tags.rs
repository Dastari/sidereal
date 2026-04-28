fn control_bootstrap_generation(
    current_generation: u64,
    authoritative_generation: u64,
    target_changed: bool,
) -> u64 {
    if authoritative_generation > 0 {
        authoritative_generation
    } else if target_changed {
        current_generation.saturating_add(1)
    } else {
        current_generation
    }
}

fn rewrite_control_bootstrap_phase_generation(
    phase: &ControlBootstrapPhase,
    generation: u64,
) -> ControlBootstrapPhase {
    match phase {
        ControlBootstrapPhase::Idle => ControlBootstrapPhase::Idle,
        ControlBootstrapPhase::PendingPredicted {
            target_entity_id, ..
        } => ControlBootstrapPhase::PendingPredicted {
            target_entity_id: target_entity_id.clone(),
            generation,
        },
        ControlBootstrapPhase::ActivePredicted {
            target_entity_id,
            entity,
            ..
        } => ControlBootstrapPhase::ActivePredicted {
            target_entity_id: target_entity_id.clone(),
            generation,
            entity: *entity,
        },
    }
}

pub(crate) fn sync_controlled_entity_tags_system(
    mut commands: Commands<'_, '_>,
    time: Res<'_, Time>,
    mut inputs: ControlledEntityTagInputs<'_, '_>,
) {
    let Some(local_player_entity_id) = inputs.session.player_entity_id.as_ref() else {
        return;
    };
    let local_player_wire_id = local_player_entity_id.clone();
    let target_entity_id = resolve_control_target_entity_id(
        &inputs.entity_registry,
        local_player_entity_id,
        inputs.player_view_state.controlled_entity_id.as_deref(),
    );
    let target_changed = inputs
        .control_bootstrap_state
        .authoritative_target_entity_id
        != target_entity_id;
    let desired_generation = control_bootstrap_generation(
        inputs.control_bootstrap_state.generation,
        inputs.player_view_state.controlled_entity_generation,
        target_changed,
    );
    let target_guid = target_entity_id
        .as_ref()
        .and_then(|id| parse_guid_from_entity_id(id));
    if target_changed {
        inputs.control_bootstrap_state.generation = desired_generation;
        inputs
            .control_bootstrap_state
            .authoritative_target_entity_id = target_entity_id.clone();
        inputs.control_bootstrap_state.last_transition_at_s = time.elapsed_secs_f64();
        inputs.control_bootstrap_state.phase = match target_entity_id.as_ref() {
            Some(target_entity_id) => ControlBootstrapPhase::PendingPredicted {
                target_entity_id: target_entity_id.clone(),
                generation: desired_generation,
            },
            None => ControlBootstrapPhase::Idle,
        };
    } else if desired_generation != inputs.control_bootstrap_state.generation {
        inputs.control_bootstrap_state.generation = desired_generation;
        inputs.control_bootstrap_state.phase = rewrite_control_bootstrap_phase_generation(
            &inputs.control_bootstrap_state.phase,
            desired_generation,
        );
    }
    let local_player_guid = parse_guid_from_entity_id(local_player_entity_id);
    let is_player_anchor_target = target_guid
        .zip(local_player_guid)
        .is_some_and(|(target, player)| target == player);
    let target_entity = target_guid.and_then(|guid| {
        let mut best_predicted: Option<(Entity, String)> = None;
        for (entity, entity_guid, _is_player_anchor, is_predicted, _is_interpolated) in
            &inputs.guid_candidates
        {
            if entity_guid.0 != guid {
                continue;
            }
            let runtime_entity_id = entity_guid.0.to_string();
            if is_predicted {
                best_predicted = match best_predicted {
                    Some((current, _)) if current.to_bits() <= entity.to_bits() => {
                        Some((current, runtime_entity_id.clone()))
                    }
                    _ => Some((entity, runtime_entity_id.clone())),
                };
                continue;
            }
        }

        if let Some((predicted, predicted_entity_id)) = best_predicted {
            inputs.adoption_state.missing_predicted_control_entity_id = None;
            inputs.control_bootstrap_state.phase = ControlBootstrapPhase::ActivePredicted {
                target_entity_id: predicted_entity_id,
                generation: inputs.control_bootstrap_state.generation,
                entity: predicted,
            };
            inputs.control_bootstrap_state.last_transition_at_s = time.elapsed_secs_f64();
            return Some(predicted);
        }

        let missing_id = target_entity_id.clone().unwrap_or_else(|| guid.to_string());
        inputs.adoption_state.missing_predicted_control_entity_id = Some(missing_id.clone());
        inputs.control_bootstrap_state.phase = ControlBootstrapPhase::PendingPredicted {
            target_entity_id: missing_id.clone(),
            generation: inputs.control_bootstrap_state.generation,
        };
        let now_s = time.elapsed_secs_f64();
        if now_s - inputs.adoption_state.last_missing_predicted_warn_at_s >= 1.0 {
            let target_lane = if is_player_anchor_target {
                "player anchor"
            } else {
                "controlled entity"
            };
            bevy::log::warn!(
                "controlled runtime target {} ({}) has no Predicted clone yet; refusing to bind local control to confirmed/interpolated fallback",
                missing_id, target_lane
            );
            inputs.adoption_state.last_missing_predicted_warn_at_s = now_s;
        }
        None
    });
    if target_guid.is_none() {
        inputs.control_bootstrap_state.phase = ControlBootstrapPhase::Idle;
    }

    for (entity, controlled) in &inputs.controlled_query {
        if Some(entity) != target_entity {
            commands.entity(entity).remove::<ControlledEntity>();
        } else if controlled.player_entity_id != local_player_wire_id {
            commands.entity(entity).insert(ControlledEntity {
                entity_id: controlled.entity_id.clone(),
                player_entity_id: local_player_wire_id.clone(),
            });
        }
    }
    for entity in &inputs.writer_query {
        if Some(entity) != target_entity {
            commands.entity(entity).remove::<SimulationMotionWriter>();
        }
    }

    if let Some(entity) = target_entity {
        commands.entity(entity).insert((
            ControlledEntity {
                entity_id: target_entity_id.clone().unwrap_or_default(),
                player_entity_id: local_player_wire_id,
            },
            SimulationMotionWriter,
        ));
    }
}
