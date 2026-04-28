/// Computes fullscreen world-space values used by fullscreen shaders. Runs in Last.
#[allow(clippy::type_complexity)]
fn preferred_controlled_velocity(
    controlled_vel_query: &Query<
        '_,
        '_,
        (
            Entity,
            &'static LinearVelocity,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
        ),
        With<ControlledEntity>,
    >,
) -> Option<Vec2> {
    controlled_vel_query
        .iter()
        .fold(
            None::<(Vec2, i32, u64)>,
            |winner, (entity, velocity, is_predicted, is_interpolated)| {
                let score = if is_predicted {
                    3
                } else if is_interpolated {
                    2
                } else {
                    1
                };
                let entity_bits = entity.to_bits();
                if winner.is_none_or(|(_, best_score, best_entity_bits)| {
                    score > best_score || (score == best_score && entity_bits > best_entity_bits)
                }) {
                    Some((velocity.0.as_vec2(), score, entity_bits))
                } else {
                    winner
                }
            },
        )
        .map(|(velocity, _, _)| velocity)
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn compute_fullscreen_external_world_system(
    time: Res<'_, Time>,
    player_view_state: Res<'_, super::app_state::LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    controlled_vel_query: Query<
        '_,
        '_,
        (
            Entity,
            &'static LinearVelocity,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
        ),
        With<ControlledEntity>,
    >,
    velocity_query: Query<'_, '_, &'static LinearVelocity>,
    gameplay_camera_projection: Query<'_, '_, &'static Projection, With<GameplayCamera>>,
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
    mut motion: ResMut<'_, StarfieldMotionState>,
    mut world_data: ResMut<'_, FullscreenExternalWorldData>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(render_size) = platform::safe_render_target_size(window) else {
        return;
    };

    let velocity_vector = preferred_controlled_velocity(&controlled_vel_query)
        .or_else(|| {
            let controlled_id = player_view_state.controlled_entity_id.as_ref()?;
            let entity = entity_registry.by_entity_id.get(controlled_id.as_str())?;
            velocity_query
                .get(*entity)
                .ok()
                .map(|velocity| velocity.0.as_vec2())
        })
        .unwrap_or(Vec2::ZERO);

    let magnitude = velocity_vector.length();
    let heading = if magnitude > 0.01 {
        velocity_vector / magnitude
    } else {
        Vec2::Y
    };

    let dt = time.delta_secs().max(0.0);
    let zoom_scale = gameplay_camera_projection
        .single()
        .ok()
        .and_then(|projection| match projection {
            Projection::Orthographic(ortho) => Some(ortho.scale.max(0.01)),
            _ => None,
        })
        .unwrap_or(1.0);

    if !motion.initialized {
        motion.initialized = true;
        motion.prev_speed = magnitude;
        motion.smoothed_warp = 0.0;
    }

    // Starfield from controlled entity: vector = velocity, magnitude = speed, heading = unit direction.
    // Parallax is distance-over-time: we need the accumulator so scroll reflects integrated displacement (continual smooth motion).
    // Do not wrap at 1.0 (caused visible reset). Shader uses fract() so pattern is periodic. Wrap at large period to avoid f32 precision loss over long sessions.
    const STARFIELD_WORLD_TO_UV: f32 = 0.024;
    const SCROLL_WRAP_PERIOD: f32 = 4096.0;

    let frame_displacement = velocity_vector * dt;
    let delta_uv = frame_displacement * STARFIELD_WORLD_TO_UV;
    motion.accumulated_scroll_uv += delta_uv;
    if motion.accumulated_scroll_uv.x.abs() >= SCROLL_WRAP_PERIOD {
        motion.accumulated_scroll_uv.x -=
            motion.accumulated_scroll_uv.x.signum() * SCROLL_WRAP_PERIOD;
    }
    if motion.accumulated_scroll_uv.y.abs() >= SCROLL_WRAP_PERIOD {
        motion.accumulated_scroll_uv.y -=
            motion.accumulated_scroll_uv.y.signum() * SCROLL_WRAP_PERIOD;
    }

    let travel_uv = motion.accumulated_scroll_uv;
    motion.starfield_drift_uv = travel_uv;
    motion.background_drift_uv = travel_uv * 0.32;

    let target_warp = ((magnitude - 480.0) / 1650.0).clamp(0.0, 1.25);
    let warp_alpha = 1.0 - (-7.5 * dt).exp();
    motion.smoothed_warp = motion.smoothed_warp.lerp(target_warp, warp_alpha);

    let warp = motion.smoothed_warp;

    world_data.viewport_time = Vec4::new(render_size.x, render_size.y, time.elapsed_secs(), warp);
    // Y-flip so world Y-up matches screen: stars stream opposite travel (e.g. 223° -> 43°).
    world_data.drift_intensity = Vec4::new(travel_uv.x, -travel_uv.y, 1.0, 1.0);
    world_data.velocity_dir = Vec4::new(heading.x, heading.y, zoom_scale, 0.0);
}

