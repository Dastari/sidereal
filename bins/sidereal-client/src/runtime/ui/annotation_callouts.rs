#[allow(clippy::too_many_arguments)]
pub(super) fn sync_debug_entity_callouts_system(
    debug_overlay: Res<'_, DebugOverlayState>,
    tactical_map_state: Res<'_, TacticalMapUiState>,
    camera_motion: Res<'_, CameraMotionState>,
    snapshot: Res<'_, DebugOverlaySnapshot>,
    mut commands: Commands<'_, '_>,
    fonts: Res<'_, EmbeddedFonts>,
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
    mut ui_queries: AnnotationCalloutUiQueries<'_, '_>,
) {
    if !debug_overlay.enabled || tactical_map_state.enabled {
        for (_, _, mut visibility) in &mut ui_queries.root_query {
            *visibility = Visibility::Hidden;
        }
        for (_, _, mut visibility) in &mut ui_queries.line_query {
            *visibility = Visibility::Hidden;
        }
        return;
    }

    let Some((_camera_entity, camera, camera_transform)) =
        select_annotation_callout_camera(&ui_queries.gameplay_camera)
    else {
        for (_, _, mut visibility) in &mut ui_queries.root_query {
            *visibility = Visibility::Hidden;
        }
        for (_, _, mut visibility) in &mut ui_queries.line_query {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let camera_global = GlobalTransform::from(camera_transform);
    let (cursor_px, window_size) = {
        let Ok(window) = ui_queries.window_query.single() else {
            return;
        };
        (
            window.cursor_position(),
            Vec2::new(window.width(), window.height()),
        )
    };
    let controlled_position = snapshot
        .entities
        .iter()
        .find(|entity| {
            entity.is_controlled
                && !matches!(
                    entity.lane,
                    super::resources::DebugEntityLane::Auxiliary
                        | super::resources::DebugEntityLane::ConfirmedGhost
                )
        })
        .map(|entity| entity.position_xy);
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();

    let desired_targets = snapshot
        .entities
        .iter()
        .map(|entity| entity.entity)
        .collect::<HashSet<_>>();
    let stale_targets = ui_queries
        .registry
        .active_by_target
        .keys()
        .copied()
        .filter(|target| !desired_targets.contains(target))
        .collect::<Vec<_>>();
    for target in stale_targets {
        if let Some(entry) = ui_queries.registry.active_by_target.remove(&target) {
            release_annotation_callout_entry(&mut commands, &mut ui_queries.registry, entry);
        }
    }

    for entity in &snapshot.entities {
        let Some(callout_target) = annotation_callout_target(
            entity,
            &ui_queries.target_query,
            &camera,
            &camera_global,
            camera_motion.parallax_position_xy,
        ) else {
            if let Some(entry) = ui_queries
                .registry
                .active_by_target
                .get(&entity.entity)
                .copied()
            {
                hide_annotation_callout_entry(&mut ui_queries, entry);
            }
            continue;
        };
        let viewport_pos = callout_target.center_viewport_pos;
        if viewport_pos.x < -DEBUG_CALLOUT_VIEWPORT_MARGIN_PX
            || viewport_pos.x > window_size.x + DEBUG_CALLOUT_VIEWPORT_MARGIN_PX
            || viewport_pos.y < -DEBUG_CALLOUT_VIEWPORT_MARGIN_PX
            || viewport_pos.y > window_size.y + DEBUG_CALLOUT_VIEWPORT_MARGIN_PX
        {
            if let Some(entry) = ui_queries
                .registry
                .active_by_target
                .get(&entity.entity)
                .copied()
            {
                hide_annotation_callout_entry(&mut ui_queries, entry);
            }
            continue;
        }

        let entry = if let Some(entry) = ui_queries
            .registry
            .active_by_target
            .get(&entity.entity)
            .copied()
        {
            entry
        } else {
            ui_queries.registry.allocated_entries =
                ui_queries.registry.allocated_entries.saturating_add(1);
            let entry = ui_queries.registry.free_entries.pop().unwrap_or_else(|| {
                spawn_annotation_callout_entry(&mut commands, &fonts, theme, glow_intensity)
            });
            ui_queries
                .registry
                .active_by_target
                .insert(entity.entity, entry);
            entry
        };
        let hovered = cursor_px.is_some_and(|cursor_px| {
            cursor_px.distance(viewport_pos) <= DEBUG_CALLOUT_HOVER_RADIUS_PX
        });
        let placement = if entity.is_component {
            AnnotationCalloutPlacement::BottomRight
        } else {
            AnnotationCalloutPlacement::TopLeft
        };
        if entity.is_component && !hovered {
            hide_annotation_callout_entry(&mut ui_queries, entry);
            continue;
        }
        if let Ok((root, mut node, mut visibility)) = ui_queries.root_query.get_mut(entry.root) {
            let text = annotation_callout_text(entity, controlled_position);
            let line_count = text.lines().count().max(1) as f32;
            let height_px =
                line_count * DEBUG_CALLOUT_ROW_HEIGHT_PX + DEBUG_CALLOUT_PADDING_PX * 2.0 + 2.0;
            let callout_rect = annotation_callout_rect(
                callout_target.anchor_viewport_pos,
                height_px,
                window_size,
                placement,
            );
            node.left = px(callout_rect.min.x);
            node.top = px(callout_rect.min.y);
            node.height = px(height_px);
            *visibility = Visibility::Visible;
            if (root.target, root.placement) != (Some(entity.entity), placement)
                && let Ok(mut root_commands) = commands.get_entity(entry.root)
            {
                root_commands.insert(AnnotationCalloutRoot {
                    target: Some(entity.entity),
                    placement,
                });
            }
            if let Ok(mut text_value) = ui_queries.text_query.get_mut(entry.text) {
                text_value.0 = text;
            }
            sync_annotation_callout_line(
                &mut ui_queries.line_query,
                entry.line,
                callout_rect,
                viewport_pos,
            );
        }
    }
}

#[derive(Clone, Copy)]
struct AnnotationCalloutTarget {
    center_viewport_pos: Vec2,
    anchor_viewport_pos: Vec2,
}

#[derive(Clone, Copy)]
struct AnnotationCalloutRect {
    min: Vec2,
    max: Vec2,
}

fn annotation_callout_target(
    entity: &super::resources::DebugOverlayEntity,
    target_query: &AnnotationCalloutTargetQuery<'_, '_>,
    camera: &Camera,
    camera_global: &GlobalTransform,
    parallax_position_xy: Vec2,
) -> Option<AnnotationCalloutTarget> {
    if entity.is_component {
        let (center_world, anchor_world) = annotation_callout_snapshot_world_positions(entity);
        return project_annotation_callout_target(
            camera,
            camera_global,
            center_world,
            anchor_world,
        );
    }
    let (center_world, anchor_world) = if let Ok((
        global_transform,
        maybe_visibility,
        size_m,
        planet_settings,
        resolved_render_layer,
    )) = target_query.get(entity.entity)
    {
        if maybe_visibility
            .is_some_and(|entity_visibility| *entity_visibility == Visibility::Hidden)
        {
            return None;
        }
        if let Some(global_transform) = global_transform {
            let world_pos = global_transform.translation();
            if planet_settings.is_some_and(|settings| settings.enabled) {
                let projected_center_world = super::visuals::planet_camera_relative_translation(
                    resolved_render_layer,
                    world_pos.truncate(),
                    parallax_position_xy,
                );
                let radius_m = size_m
                    .map(|size| size.width.max(size.length) * 0.5)
                    .unwrap_or(128.0);
                let layer_screen_scale = resolved_render_layer
                    .map(|layer| {
                        super::visuals::runtime_layer_screen_scale_factor(&layer.definition)
                    })
                    .unwrap_or(1.0);
                let projected_radius_m = radius_m * layer_screen_scale;
                return project_annotation_callout_target(
                    camera,
                    camera_global,
                    projected_center_world.extend(0.0),
                    Vec3::new(
                        projected_center_world.x - projected_radius_m,
                        projected_center_world.y + projected_radius_m,
                        0.0,
                    ),
                );
            }
            let anchor_world = size_m
                .map(|size| {
                    Vec3::new(
                        world_pos.x - size.width * 0.5,
                        world_pos.y + size.length * 0.5,
                        0.0,
                    )
                })
                .unwrap_or(Vec3::new(world_pos.x, world_pos.y, 0.0));
            (Vec3::new(world_pos.x, world_pos.y, 0.0), anchor_world)
        } else {
            annotation_callout_snapshot_world_positions(entity)
        }
    } else {
        annotation_callout_snapshot_world_positions(entity)
    };
    project_annotation_callout_target(camera, camera_global, center_world, anchor_world)
}

fn project_annotation_callout_target(
    camera: &Camera,
    camera_global: &GlobalTransform,
    center_world: Vec3,
    anchor_world: Vec3,
) -> Option<AnnotationCalloutTarget> {
    let center_viewport_pos = camera.world_to_viewport(camera_global, center_world).ok()?;
    let anchor_viewport_pos = camera
        .world_to_viewport(camera_global, anchor_world)
        .unwrap_or(center_viewport_pos);
    Some(AnnotationCalloutTarget {
        center_viewport_pos,
        anchor_viewport_pos,
    })
}

fn annotation_callout_snapshot_world_positions(
    entity: &super::resources::DebugOverlayEntity,
) -> (Vec3, Vec3) {
    let center = entity.position_xy.extend(0.0);
    let anchor = match &entity.collision {
        super::resources::DebugCollisionShape::Aabb { half_extents } => Vec3::new(
            entity.position_xy.x - half_extents.x,
            entity.position_xy.y + half_extents.y,
            0.0,
        ),
        super::resources::DebugCollisionShape::Outline { points } if !points.is_empty() => {
            let min_x = points
                .iter()
                .map(|point| point.x)
                .fold(f32::INFINITY, f32::min);
            let max_y = points
                .iter()
                .map(|point| point.y)
                .fold(f32::NEG_INFINITY, f32::max);
            Vec3::new(
                entity.position_xy.x + min_x,
                entity.position_xy.y + max_y,
                0.0,
            )
        }
        _ => center,
    };
    (center, anchor)
}

fn annotation_callout_rect(
    anchor_viewport_pos: Vec2,
    height_px: f32,
    window_size: Vec2,
    placement: AnnotationCalloutPlacement,
) -> AnnotationCalloutRect {
    let (unclamped_left, unclamped_top) = match placement {
        AnnotationCalloutPlacement::TopLeft => (
            anchor_viewport_pos.x - DEBUG_CALLOUT_WIDTH_PX - DEBUG_CALLOUT_TARGET_GAP_PX,
            anchor_viewport_pos.y - height_px - DEBUG_CALLOUT_TARGET_GAP_PX,
        ),
        AnnotationCalloutPlacement::BottomRight => (
            anchor_viewport_pos.x + DEBUG_CALLOUT_TARGET_GAP_PX,
            anchor_viewport_pos.y + DEBUG_CALLOUT_TARGET_GAP_PX,
        ),
    };
    let max_left = (window_size.x - DEBUG_CALLOUT_WIDTH_PX - DEBUG_CALLOUT_VIEWPORT_MARGIN_PX)
        .max(DEBUG_CALLOUT_VIEWPORT_MARGIN_PX);
    let max_top = (window_size.y - height_px - DEBUG_CALLOUT_VIEWPORT_MARGIN_PX)
        .max(DEBUG_CALLOUT_VIEWPORT_MARGIN_PX);
    let min = Vec2::new(
        unclamped_left.clamp(DEBUG_CALLOUT_VIEWPORT_MARGIN_PX, max_left),
        unclamped_top.clamp(DEBUG_CALLOUT_VIEWPORT_MARGIN_PX, max_top),
    );
    AnnotationCalloutRect {
        min,
        max: min + Vec2::new(DEBUG_CALLOUT_WIDTH_PX, height_px),
    }
}

fn sync_annotation_callout_line(
    line_query: &mut AnnotationCalloutLineQuery<'_, '_>,
    line_entity: Entity,
    callout_rect: AnnotationCalloutRect,
    target_viewport_pos: Vec2,
) {
    let Ok((mut node, mut transform, mut visibility)) = line_query.get_mut(line_entity) else {
        return;
    };
    let start = closest_point_on_annotation_callout_rect(callout_rect, target_viewport_pos);
    let delta = target_viewport_pos - start;
    let length = delta.length();
    if length <= 1.0 {
        *visibility = Visibility::Hidden;
        return;
    }
    let midpoint = start + delta * 0.5;
    node.left = px(midpoint.x - length * 0.5);
    node.top = px(midpoint.y - DEBUG_CALLOUT_LINE_THICKNESS_PX * 0.5);
    node.width = px(length);
    node.height = px(DEBUG_CALLOUT_LINE_THICKNESS_PX);
    *transform = UiTransform::from_rotation(Rot2::radians(delta.y.atan2(delta.x)));
    *visibility = Visibility::Visible;
}

fn closest_point_on_annotation_callout_rect(rect: AnnotationCalloutRect, target: Vec2) -> Vec2 {
    Vec2::new(
        target.x.clamp(rect.min.x, rect.max.x),
        target.y.clamp(rect.min.y, rect.max.y),
    )
}

fn hide_annotation_callout_entry(
    ui_queries: &mut AnnotationCalloutUiQueries<'_, '_>,
    entry: AnnotationCalloutEntry,
) {
    if let Ok((_, _, mut visibility)) = ui_queries.root_query.get_mut(entry.root) {
        *visibility = Visibility::Hidden;
    }
    if let Ok((_, _, mut visibility)) = ui_queries.line_query.get_mut(entry.line) {
        *visibility = Visibility::Hidden;
    }
}

fn select_annotation_callout_camera(
    gameplay_camera: &Query<'_, '_, (Entity, &'_ Camera, &'_ Transform), With<GameplayCamera>>,
) -> Option<(Entity, Camera, Transform)> {
    let mut selected_camera: Option<(Entity, bool, Camera, Transform)> = None;
    for (entity, camera, transform) in gameplay_camera {
        let candidate = (entity, camera.is_active, camera.clone(), *transform);
        if selected_camera
            .as_ref()
            .is_none_or(|(current_entity, current_active, _, _)| {
                if camera.is_active != *current_active {
                    return camera.is_active;
                }
                entity.to_bits() < current_entity.to_bits()
            })
        {
            selected_camera = Some(candidate);
        }
    }
    selected_camera.map(|(entity, _, camera, transform)| (entity, camera, transform))
}

fn spawn_annotation_callout_entry(
    commands: &mut Commands<'_, '_>,
    fonts: &EmbeddedFonts,
    theme: sidereal_ui::theme::UiTheme,
    glow_intensity: f32,
) -> AnnotationCalloutEntry {
    let (panel_bg, panel_border, panel_shadow) = panel_surface(theme, glow_intensity);
    let root = commands
        .spawn((
            Name::new("AnnotationCallout"),
            Node {
                position_type: PositionType::Absolute,
                width: px(DEBUG_CALLOUT_WIDTH_PX),
                height: px(48.0),
                left: px(0.0),
                top: px(0.0),
                ..layout::panel(
                    px(DEBUG_CALLOUT_WIDTH_PX),
                    DEBUG_CALLOUT_PADDING_PX,
                    0.0,
                    theme.metrics.panel_radius_px,
                    theme.metrics.panel_border_px,
                )
            },
            panel_bg,
            panel_border,
            panel_shadow,
            Visibility::Hidden,
            UiOverlayLayer,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
            AnnotationCalloutRoot {
                target: None,
                placement: AnnotationCalloutPlacement::TopLeft,
            },
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(""),
            text_font(fonts.mono.clone(), 9.5),
            TextColor(Color::srgb(0.78, 1.0, 0.82)),
            AnnotationCalloutText,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        ))
        .id();
    let line = commands
        .spawn((
            Name::new("DebugEntityCalloutLine"),
            Node {
                position_type: PositionType::Absolute,
                width: px(1.0),
                height: px(DEBUG_CALLOUT_LINE_THICKNESS_PX),
                left: px(0.0),
                top: px(0.0),
                ..default()
            },
            UiTransform::IDENTITY,
            BackgroundColor(Color::srgba(0.22, 1.0, 0.4, 0.78)),
            Visibility::Hidden,
            UiOverlayLayer,
            AnnotationCalloutLine,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .id();
    commands.entity(root).add_child(text);
    AnnotationCalloutEntry { root, text, line }
}

fn release_annotation_callout_entry(
    commands: &mut Commands<'_, '_>,
    registry: &mut AnnotationCalloutRegistry,
    entry: AnnotationCalloutEntry,
) {
    registry.free_entries.push(entry);
    if let Ok(mut root_commands) = commands.get_entity(entry.root) {
        root_commands.insert((
            Visibility::Hidden,
            AnnotationCalloutRoot {
                target: None,
                placement: AnnotationCalloutPlacement::TopLeft,
            },
        ));
    }
    if let Ok(mut line_commands) = commands.get_entity(entry.line) {
        line_commands.insert(Visibility::Hidden);
    }
}

fn annotation_callout_text(
    entity: &super::resources::DebugOverlayEntity,
    controlled_position: Option<Vec2>,
) -> String {
    let mut lines = Vec::with_capacity(10);
    lines.push(entity.label.clone());
    lines.push(format!("ID {}", short_uuid(entity.guid)));
    lines.push(format!(
        "POS {:>7.1} {:>7.1}",
        entity.position_xy.x, entity.position_xy.y
    ));
    lines.push(format!("ROT {:>6.1} DEG", entity.rotation_rad.to_degrees()));
    if let Some(controlled_position) = controlled_position {
        let relative = entity.position_xy - controlled_position;
        lines.push(format!("REL {:>7.1} {:>7.1}", relative.x, relative.y));
    }
    lines.push(format!("LANE {:?}", entity.lane).to_ascii_uppercase());
    lines.push(format!("ECS {}", entity.entity.to_bits()));
    lines.push(format!(
        "VEL {:>6.1} {:>6.1}",
        entity.velocity_xy.x, entity.velocity_xy.y
    ));
    lines.push(format!("ANG {:>6.2}", entity.angular_velocity_rps));
    lines.push(format!(
        "COMP {}",
        if entity.is_component { "YES" } else { "NO" }
    ));
    lines.join("\n")
}

fn short_uuid(guid: uuid::Uuid) -> String {
    guid.to_string()
        .chars()
        .take(8)
        .collect::<String>()
        .to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::{
        propagate_ui_overlay_layer_system, split_debug_overlay_text_columns,
        sync_entity_nameplates_system, update_debug_overlay_text_ui_system,
    };
    use crate::runtime::components::{
        CanonicalPresentationEntity, DebugOverlayPanelLabelShadowText, DebugOverlayPanelLabelText,
        DebugOverlayPanelRoot, DebugOverlayPanelSecondaryLabelShadowText,
        DebugOverlayPanelSecondaryLabelText, DebugOverlayPanelSecondaryValueShadowText,
        DebugOverlayPanelSecondaryValueText, DebugOverlayPanelTertiaryLabelShadowText,
        DebugOverlayPanelTertiaryLabelText, DebugOverlayPanelTertiaryValueShadowText,
        DebugOverlayPanelTertiaryValueText, DebugOverlayPanelText,
        DebugOverlayPanelValueShadowText, DebugOverlayPanelValueText, EntityNameplateRoot,
        UiOverlayLayer, WorldEntity,
    };
    use crate::runtime::platform::UI_OVERLAY_RENDER_LAYER;
    use crate::runtime::resources::{
        ClientInputSendState, DebugOverlaySnapshot, DebugOverlayState, HudPerfCounters,
        NameplateRegistry, NameplateUiState, TacticalMapUiState,
    };
    use bevy::camera::visibility::RenderLayers;
    use bevy::diagnostic::DiagnosticsStore;
    use bevy::prelude::*;
    use sidereal_game::{EntityAction, HealthPool};

    #[test]
    fn ui_overlay_layer_propagates_to_new_children_only_when_needed() {
        let mut app = App::new();
        app.add_systems(Update, propagate_ui_overlay_layer_system);

        let root = app.world_mut().spawn(UiOverlayLayer).id();
        let child = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(root).add_child(child);

        app.update();

        let child_ref = app.world().entity(child);
        assert!(child_ref.contains::<UiOverlayLayer>());
        let layers = child_ref
            .get::<RenderLayers>()
            .expect("child render layers should be propagated");
        assert!(layers.intersects(&RenderLayers::layer(UI_OVERLAY_RENDER_LAYER)));
    }

    #[test]
    fn debug_overlay_text_rows_split_evenly_across_two_columns() {
        let rows = vec![
            ("A".to_string(), "1".to_string()),
            ("B".to_string(), "2".to_string()),
            ("C".to_string(), "3".to_string()),
            ("D".to_string(), "4".to_string()),
            ("E".to_string(), "5".to_string()),
        ];

        let columns = split_debug_overlay_text_columns(&rows);

        assert_eq!(columns[0].labels, vec!["A", "B"]);
        assert_eq!(columns[0].values, vec!["1", "2"]);
        assert_eq!(columns[1].labels, vec!["C", "D"]);
        assert_eq!(columns[1].values, vec!["3", "4"]);
        assert_eq!(columns[2].labels, vec!["E"]);
        assert_eq!(columns[2].values, vec!["5"]);
    }

    #[test]
    fn debug_overlay_text_rows_pin_control_data_to_right_column() {
        let rows = vec![
            ("Sent Input".to_string(), "[Long Neutral]".to_string()),
            ("A".to_string(), "1".to_string()),
            (
                "Control GUID".to_string(),
                "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
            ),
            ("B".to_string(), "2".to_string()),
        ];

        let columns = split_debug_overlay_text_columns(&rows);

        assert!(!columns[0].labels.contains(&"Sent Input".to_string()));
        assert!(!columns[1].labels.contains(&"Control GUID".to_string()));
        assert!(columns[2].labels.contains(&"Sent Input".to_string()));
        assert!(columns[2].labels.contains(&"Control GUID".to_string()));
    }

    #[test]
    fn debug_overlay_text_ui_system_initializes_without_query_conflicts() {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.init_resource::<DiagnosticsStore>();
        app.init_resource::<DebugOverlaySnapshot>();
        app.insert_resource(DebugOverlayState {
            enabled: true,
            ..Default::default()
        });
        app.init_resource::<ClientInputSendState>();
        app.add_systems(Update, update_debug_overlay_text_ui_system);

        app.world_mut()
            .spawn((DebugOverlayPanelRoot, Visibility::Hidden));
        app.world_mut().spawn((
            DebugOverlayPanelLabelText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelLabelShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelValueText,
            DebugOverlayPanelText,
            Text::new(""),
            TextColor(Color::WHITE),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelValueShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryLabelText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryLabelShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryValueText,
            DebugOverlayPanelText,
            Text::new(""),
            TextColor(Color::WHITE),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryValueShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryLabelText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryLabelShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryValueText,
            DebugOverlayPanelText,
            Text::new(""),
            TextColor(Color::WHITE),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryValueShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));

        app.update();
    }

    #[test]
    fn debug_overlay_sent_input_moves_to_right_column() {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.init_resource::<DiagnosticsStore>();
        app.insert_resource(DebugOverlayState {
            enabled: true,
            ..Default::default()
        });
        app.insert_resource(DebugOverlaySnapshot {
            text_rows: vec![
                super::super::resources::DebugTextRow {
                    label: "Predicted".to_string(),
                    value: "1".to_string(),
                    severity: super::super::resources::DebugSeverity::Normal,
                },
                super::super::resources::DebugTextRow {
                    label: "Confirmed".to_string(),
                    value: "2".to_string(),
                    severity: super::super::resources::DebugSeverity::Normal,
                },
                super::super::resources::DebugTextRow {
                    label: "Interpolated".to_string(),
                    value: "3".to_string(),
                    severity: super::super::resources::DebugSeverity::Normal,
                },
                super::super::resources::DebugTextRow {
                    label: "Cameras".to_string(),
                    value: "7".to_string(),
                    severity: super::super::resources::DebugSeverity::Normal,
                },
            ],
            ..Default::default()
        });
        app.insert_resource(ClientInputSendState {
            last_sent_actions: vec![
                EntityAction::Left,
                EntityAction::LongitudinalNeutral,
                EntityAction::AfterburnerOff,
            ],
            ..Default::default()
        });
        app.add_systems(Update, update_debug_overlay_text_ui_system);

        app.world_mut()
            .spawn((DebugOverlayPanelRoot, Visibility::Hidden));
        app.world_mut().spawn((
            DebugOverlayPanelLabelText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelLabelShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelValueText,
            DebugOverlayPanelText,
            Text::new(""),
            TextColor(Color::WHITE),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelValueShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryLabelText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryLabelShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryValueText,
            DebugOverlayPanelText,
            Text::new(""),
            TextColor(Color::WHITE),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelSecondaryValueShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryLabelText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryLabelShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryValueText,
            DebugOverlayPanelText,
            Text::new(""),
            TextColor(Color::WHITE),
        ));
        app.world_mut().spawn((
            DebugOverlayPanelTertiaryValueShadowText,
            DebugOverlayPanelText,
            Text::new(""),
        ));

        app.update();

        let primary_labels_value = {
            let world = app.world_mut();
            world
                .query_filtered::<&Text, With<DebugOverlayPanelLabelText>>()
                .single(world)
                .expect("primary labels")
                .0
                .clone()
        };
        let secondary_labels_value = {
            let world = app.world_mut();
            world
                .query_filtered::<&Text, With<DebugOverlayPanelSecondaryLabelText>>()
                .single(world)
                .expect("secondary labels")
                .0
                .clone()
        };

        let tertiary_labels_value = {
            let world = app.world_mut();
            world
                .query_filtered::<&Text, With<DebugOverlayPanelTertiaryLabelText>>()
                .single(world)
                .expect("tertiary labels")
                .0
                .clone()
        };

        assert!(!primary_labels_value.contains("Sent Input"));
        assert!(!secondary_labels_value.contains("Sent Input"));
        assert!(tertiary_labels_value.contains("Sent Input"));
    }

    #[test]
    fn nameplates_default_to_enabled() {
        assert!(NameplateUiState::default().enabled);
    }

    #[test]
    fn sync_entity_nameplates_system_names_spawned_roots() {
        let mut app = App::new();
        app.init_resource::<HudPerfCounters>();
        app.init_resource::<NameplateRegistry>();
        app.init_resource::<NameplateUiState>();
        app.init_resource::<TacticalMapUiState>();
        app.add_systems(Update, sync_entity_nameplates_system);

        let target = app
            .world_mut()
            .spawn((
                WorldEntity,
                CanonicalPresentationEntity,
                HealthPool {
                    current: 10.0,
                    maximum: 10.0,
                },
            ))
            .id();

        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<(&Name, &EntityNameplateRoot), Without<WorldEntity>>();
        let (name, root) = query.single(app.world()).expect("spawned nameplate root");
        assert_eq!(name.as_str(), "Nameplate");
        assert_eq!(root.target, Some(target));
        assert_eq!(
            app.world()
                .resource::<NameplateRegistry>()
                .active_by_target
                .len(),
            1
        );
    }

    #[test]
    fn sync_entity_nameplates_system_reuses_pooled_entries() {
        let mut app = App::new();
        app.init_resource::<HudPerfCounters>();
        app.init_resource::<NameplateRegistry>();
        app.init_resource::<NameplateUiState>();
        app.init_resource::<TacticalMapUiState>();
        app.add_systems(Update, sync_entity_nameplates_system);

        let first_target = app
            .world_mut()
            .spawn((
                WorldEntity,
                CanonicalPresentationEntity,
                HealthPool {
                    current: 10.0,
                    maximum: 10.0,
                },
            ))
            .id();
        let second_target = app
            .world_mut()
            .spawn((
                WorldEntity,
                HealthPool {
                    current: 10.0,
                    maximum: 10.0,
                },
            ))
            .id();

        app.update();
        let first_root =
            app.world().resource::<NameplateRegistry>().active_by_target[&first_target].root;
        assert_eq!(
            app.world()
                .resource::<NameplateRegistry>()
                .allocated_entries,
            1
        );

        app.world_mut()
            .entity_mut(first_target)
            .remove::<CanonicalPresentationEntity>();
        app.world_mut()
            .entity_mut(second_target)
            .insert(CanonicalPresentationEntity);

        app.update();

        let registry = app.world().resource::<NameplateRegistry>();
        assert_eq!(
            registry.allocated_entries, 1,
            "pooled entries should be reused"
        );
        assert_eq!(registry.active_by_target[&second_target].root, first_root);
    }

    #[test]
    fn disabled_nameplates_do_not_allocate_entries() {
        let mut app = App::new();
        app.init_resource::<HudPerfCounters>();
        app.init_resource::<NameplateRegistry>();
        app.insert_resource(NameplateUiState { enabled: false });
        app.init_resource::<TacticalMapUiState>();
        app.add_systems(Update, sync_entity_nameplates_system);

        app.world_mut().spawn((
            WorldEntity,
            CanonicalPresentationEntity,
            HealthPool {
                current: 10.0,
                maximum: 10.0,
            },
        ));

        app.update();

        let registry = app.world().resource::<NameplateRegistry>();
        assert!(registry.active_by_target.is_empty());
        assert!(registry.free_entries.is_empty());
        assert_eq!(registry.allocated_entries, 0);
    }

    #[test]
    fn tactical_map_mode_suppresses_nameplate_allocation_without_disabling_preference() {
        let mut app = App::new();
        app.init_resource::<HudPerfCounters>();
        app.init_resource::<NameplateRegistry>();
        app.init_resource::<NameplateUiState>();
        app.insert_resource(TacticalMapUiState {
            enabled: true,
            ..Default::default()
        });
        app.add_systems(Update, sync_entity_nameplates_system);

        app.world_mut().spawn((
            WorldEntity,
            CanonicalPresentationEntity,
            HealthPool {
                current: 10.0,
                maximum: 10.0,
            },
        ));

        app.update();

        assert!(app.world().resource::<NameplateUiState>().enabled);
        let registry = app.world().resource::<NameplateRegistry>();
        assert!(registry.active_by_target.is_empty());
        assert!(registry.free_entries.is_empty());
        assert_eq!(registry.allocated_entries, 0);
    }
}
