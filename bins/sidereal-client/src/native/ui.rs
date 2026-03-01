//! World HUD and owned-entity panel systems.

use avian2d::prelude::{LinearVelocity, Rotation};
use bevy::camera::visibility::RenderLayers;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use sidereal_game::{DisplayName, EntityGuid, EntityLabels, FuelTank, HealthPool, MountedOn, OwnerId, SizeM};
use std::collections::HashMap;

use super::app_state::{
    ClientAppState, ClientSession, LocalPlayerViewState, OwnedEntitiesPanelState,
};
use super::assets::{LocalAssetManager, RuntimeAssetStreamIndicatorState};
use super::components::{
    ControlledEntity, GameplayCamera, GameplayHud, HudFpsText, HudFuelBarFill, HudHealthBarFill,
    HudPositionValueText, HudSpeedValueText, LoadingOverlayRoot, LoadingOverlayText,
    LoadingProgressBarFill, OwnedEntitiesPanelAction, OwnedEntitiesPanelButton, OwnedEntitiesPanelRoot,
    SegmentedBarSegment, SegmentedBarStyle, SegmentedBarValue, ShipNameplateHealthBar,
    ShipNameplateRoot, UiOverlayLayer, WorldEntity,
};
use super::platform::UI_OVERLAY_RENDER_LAYER;
use super::resources::{ClientControlRequestState, EmbeddedFonts};

/// Propagates the UI overlay render layer to all descendants of HUD roots so they are drawn
/// by the UI overlay camera (fixed scale) instead of the gameplay camera.
pub(super) fn propagate_ui_overlay_layer_system(
    mut commands: Commands,
    roots: Query<(Entity, &Children), With<UiOverlayLayer>>,
) {
    for (_entity, children) in &roots {
        for child in children.iter() {
            commands
                .entity(child)
                .try_insert((RenderLayers::layer(UI_OVERLAY_RENDER_LAYER), UiOverlayLayer));
        }
    }
}

pub(super) fn update_loading_overlay_system(
    asset_manager: Res<'_, LocalAssetManager>,
    mut overlay_query: Query<'_, '_, &mut Visibility, With<LoadingOverlayRoot>>,
    mut text_query: Query<'_, '_, (&mut Text, &mut TextColor), With<LoadingOverlayText>>,
    mut fill_query: Query<'_, '_, (&mut Node, &mut BackgroundColor), With<LoadingProgressBarFill>>,
) {
    let Ok((mut text, mut color)) = text_query.single_mut() else {
        return;
    };
    let Ok((mut fill_node, mut fill_color)) = fill_query.single_mut() else {
        return;
    };
    if asset_manager.bootstrap_complete() {
        if let Ok(mut visibility) = overlay_query.single_mut() {
            *visibility = Visibility::Hidden;
        }
        color.0.set_alpha(0.0);
        text.0 = "".to_string();
        fill_node.width = percent(0.0);
        fill_color.0.set_alpha(0.0);
        return;
    }
    if let Ok(mut visibility) = overlay_query.single_mut() {
        *visibility = Visibility::Visible;
    }
    let pct = (asset_manager.bootstrap_progress() * 100.0).round();
    fill_node.width = percent(pct.clamp(0.0, 100.0));
    fill_color.0.set_alpha(1.0);
    text.0 = if asset_manager.bootstrap_manifest_seen {
        format!("Loading assets... {}%", pct as i32)
    } else {
        "Waiting for asset manifest...".to_string()
    };
    color.0.set_alpha(1.0);
}

pub(super) fn update_runtime_stream_icon_system(
    time: Res<'_, Time>,
    asset_manager: Res<'_, LocalAssetManager>,
    mut indicator_state: ResMut<'_, RuntimeAssetStreamIndicatorState>,
    mut text_query: Query<
        '_,
        '_,
        &mut TextColor,
        With<super::components::RuntimeStreamingIconText>,
    >,
) {
    let Ok(mut color) = text_query.single_mut() else {
        return;
    };
    if !asset_manager.should_show_runtime_stream_indicator() {
        color.0.set_alpha(0.0);
        indicator_state.blinking_phase_s = 0.0;
        return;
    }
    indicator_state.blinking_phase_s += time.delta_secs();
    let pulse = (indicator_state.blinking_phase_s * 8.0).sin().abs();
    color.0 = Color::srgba(0.3 + pulse * 0.7, 0.85, 1.0, 0.5 + pulse * 0.5);
}

#[allow(clippy::type_complexity)]
pub(super) fn update_owned_entities_panel_system(
    mut commands: Commands<'_, '_>,
    fonts: Res<'_, EmbeddedFonts>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    mut panel_state: ResMut<'_, OwnedEntitiesPanelState>,
    existing_panels: Query<'_, '_, Entity, With<OwnedEntitiesPanelRoot>>,
    ships: Query<
        '_,
        '_,
        (&EntityGuid, Option<&OwnerId>, Option<&EntityLabels>, Option<&DisplayName>),
        With<WorldEntity>,
    >,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let mut owned_ship_rows = ships
        .iter()
        .filter_map(|(guid, owner, labels, display_name)| {
            let is_ship = labels.is_some_and(|l| l.0.iter().any(|s| s == "Ship"));
            if !is_ship {
                return None;
            }
            if owner.is_none_or(|owner| owner.0 != *local_player_entity_id) {
                return None;
            }
            let entity_id = guid.0.to_string();
            let label = display_name
                .map(|name| name.0.clone())
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| entity_id.clone());
            Some((entity_id, label))
        })
        .collect::<Vec<_>>();
    owned_ship_rows.sort_by(|a, b| {
        a.1.to_lowercase()
            .cmp(&b.1.to_lowercase())
            .then_with(|| a.0.cmp(&b.0))
    });
    owned_ship_rows.dedup_by(|a, b| a.0 == b.0);
    let entity_ids = owned_ship_rows
        .iter()
        .map(|(entity_id, _)| entity_id.clone())
        .collect::<Vec<_>>();
    let selected_id = player_view_state
        .desired_controlled_entity_id
        .clone()
        .or_else(|| player_view_state.controlled_entity_id.clone());

    if panel_state.last_entity_ids == entity_ids
        && panel_state.last_selected_id == selected_id
        && panel_state.last_detached_mode == player_view_state.detached_free_camera
        && !existing_panels.is_empty()
    {
        return;
    }
    panel_state.last_entity_ids = entity_ids.clone();
    panel_state.last_selected_id = selected_id.clone();
    panel_state.last_detached_mode = player_view_state.detached_free_camera;

    for panel in &existing_panels {
        commands.entity(panel).despawn();
    }

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                top: px(12),
                width: px(280),
                padding: UiRect::all(px(10)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.07, 0.11, 0.88)),
            BorderColor::all(Color::srgba(0.22, 0.34, 0.48, 0.92)),
            OwnedEntitiesPanelRoot,
            GameplayHud,
            UiOverlayLayer,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Owned Ships"),
                TextFont {
                    font: fonts.bold.clone(),
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
            ));

            let free_roam_selected = selected_id.as_deref()
                == Some(local_player_entity_id.as_str())
                && !player_view_state.detached_free_camera;
            panel
                .spawn((
                    Button,
                    OwnedEntitiesPanelButton {
                        action: OwnedEntitiesPanelAction::FreeRoam,
                    },
                    Node {
                        width: percent(100.0),
                        height: px(34),
                        justify_content: JustifyContent::FlexStart,
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(px(10), px(0)),
                        border_radius: BorderRadius::all(px(6)),
                        ..default()
                    },
                    BackgroundColor(if free_roam_selected {
                        Color::srgba(0.26, 0.4, 0.56, 0.96)
                    } else {
                        Color::srgba(0.15, 0.2, 0.28, 0.92)
                    }),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("Free Roam"),
                        TextFont {
                            font: fonts.regular.clone(),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.97, 1.0)),
                    ));
                });
            if owned_ship_rows.is_empty() {
                panel.spawn((
                    Text::new("No owned entities visible"),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.75, 0.82, 0.9, 0.9)),
                ));
            } else {
                for (entity_id, display_label) in owned_ship_rows {
                    let is_selected = selected_id.as_deref() == Some(entity_id.as_str());
                    panel
                        .spawn((
                            Button,
                            OwnedEntitiesPanelButton {
                                action: OwnedEntitiesPanelAction::ControlEntity(entity_id),
                            },
                            Node {
                                width: percent(100.0),
                                height: px(34),
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(px(10), px(0)),
                                border_radius: BorderRadius::all(px(6)),
                                ..default()
                            },
                            BackgroundColor(if is_selected {
                                Color::srgba(0.26, 0.4, 0.56, 0.96)
                            } else {
                                Color::srgba(0.15, 0.2, 0.28, 0.92)
                            }),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new(display_label),
                                TextFont {
                                    font: fonts.regular.clone(),
                                    font_size: 14.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.95, 0.97, 1.0)),
                            ));
                        });
                }
            }
        });
}

#[allow(clippy::type_complexity)]
pub(super) fn handle_owned_entities_panel_buttons(
    mut interactions: Query<
        '_,
        '_,
        (
            &Interaction,
            &OwnedEntitiesPanelButton,
            &mut BackgroundColor,
        ),
        Changed<Interaction>,
    >,
    session: Res<'_, ClientSession>,
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    mut control_request_state: ResMut<'_, ClientControlRequestState>,
    mut panel_state: ResMut<'_, OwnedEntitiesPanelState>,
) {
    for (interaction, button, mut color) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                match &button.action {
                    OwnedEntitiesPanelAction::FreeRoam => {
                        let target = session.player_entity_id.clone();
                        player_view_state.desired_controlled_entity_id = target.clone();
                        control_request_state.next_request_seq =
                            control_request_state.next_request_seq.saturating_add(1);
                        control_request_state.pending_controlled_entity_id = target;
                        control_request_state.pending_request_seq =
                            Some(control_request_state.next_request_seq);
                        control_request_state.last_sent_request_seq = None;
                        control_request_state.last_sent_at_s = 0.0;
                        player_view_state.detached_free_camera = false;
                        player_view_state.selected_entity_id = None;
                    }
                    OwnedEntitiesPanelAction::ControlEntity(entity_id) => {
                        player_view_state.desired_controlled_entity_id = Some(entity_id.clone());
                        control_request_state.next_request_seq =
                            control_request_state.next_request_seq.saturating_add(1);
                        control_request_state.pending_controlled_entity_id =
                            Some(entity_id.clone());
                        control_request_state.pending_request_seq =
                            Some(control_request_state.next_request_seq);
                        control_request_state.last_sent_request_seq = None;
                        control_request_state.last_sent_at_s = 0.0;
                        player_view_state.detached_free_camera = false;
                        player_view_state.selected_entity_id = Some(entity_id.clone());
                    }
                }
                panel_state.last_selected_id = None;
                *color = BackgroundColor(Color::srgba(0.26, 0.4, 0.56, 0.96));
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgba(0.2, 0.29, 0.41, 0.96));
            }
            Interaction::None => {
                let is_selected = match &button.action {
                    OwnedEntitiesPanelAction::FreeRoam => {
                        player_view_state.desired_controlled_entity_id.as_ref()
                            == session.player_entity_id.as_ref()
                            && !player_view_state.detached_free_camera
                    }
                    OwnedEntitiesPanelAction::ControlEntity(entity_id) => {
                        player_view_state.desired_controlled_entity_id.as_ref() == Some(entity_id)
                    }
                };
                *color = BackgroundColor(if is_selected {
                    Color::srgba(0.26, 0.4, 0.56, 0.96)
                } else {
                    Color::srgba(0.15, 0.2, 0.28, 0.92)
                });
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn update_hud_system(
    mut fuel_baseline_by_parent: Local<'_, HashMap<uuid::Uuid, f32>>,
    controlled_query: Query<
        '_,
        '_,
        (
            &EntityGuid,
            &Transform,
            Option<&Rotation>,
            Option<&LinearVelocity>,
            Option<&HealthPool>,
        ),
        With<ControlledEntity>,
    >,
    fuel_tank_query: Query<'_, '_, (&MountedOn, &FuelTank)>,
    camera_query: Query<'_, '_, (&Transform, Option<&Projection>), With<GameplayCamera>>,
    mut text_queries: ParamSet<
        '_,
        '_,
        (
            Query<'_, '_, &mut Text, With<HudFpsText>>,
            Query<'_, '_, &mut Text, With<HudSpeedValueText>>,
            Query<'_, '_, &mut Text, With<HudPositionValueText>>,
        ),
    >,
    mut bar_value_queries: ParamSet<
        '_,
        '_,
        (
            Query<'_, '_, &mut SegmentedBarValue, With<HudHealthBarFill>>,
            Query<'_, '_, &mut SegmentedBarValue, With<HudFuelBarFill>>,
        ),
    >,
    diagnostics: Option<Res<'_, DiagnosticsStore>>,
) {
    let zoom_text = camera_query
        .single()
        .ok()
        .and_then(|(_, projection)| projection)
        .and_then(|projection| match projection {
            Projection::Orthographic(ortho) => Some(format!("{:.2}x", ortho.scale.max(0.01))),
            _ => None,
        })
        .unwrap_or_else(|| "--.--x".to_string());

    let (pos, _heading_rad, vel, health_ratio, fuel_ratio) = if let Ok((
        guid,
        transform,
        maybe_rotation,
        maybe_velocity,
        maybe_health,
    )) = controlled_query.single()
    {
        let vel = maybe_velocity.map_or(Vec2::ZERO, |velocity| velocity.0);
        let heading_rad = maybe_rotation
            .map(|rotation| rotation.as_radians())
            .unwrap_or_else(|| vel.to_angle());
        let health_ratio = if let Some(health) = maybe_health {
            let ratio = if health.maximum > 0.0 {
                (health.current / health.maximum).clamp(0.0, 1.0)
            } else {
                0.0
            };
            ratio
        } else {
            0.0
        };

        let mut fuel_current = 0.0_f32;
        for (mounted_on, fuel_tank) in &fuel_tank_query {
            if mounted_on.parent_entity_id == guid.0 {
                fuel_current += fuel_tank.fuel_kg.max(0.0);
            }
        }
        let baseline_entry = fuel_baseline_by_parent.entry(guid.0).or_insert(fuel_current);
        *baseline_entry = baseline_entry.max(fuel_current);
        let fuel_capacity = (*baseline_entry).max(1.0);
        let fuel_ratio = if fuel_current > 0.0 || fuel_capacity > 1.0 {
            let ratio = (fuel_current / fuel_capacity).clamp(0.0, 1.0);
            ratio
        } else {
            0.0
        };

        (
            transform.translation,
            heading_rad,
            vel,
            health_ratio,
            fuel_ratio,
        )
    } else {
        let Ok((camera_transform, _)) = camera_query.single() else {
            return;
        };
        (camera_transform.translation, 0.0, Vec2::ZERO, 0.0, 0.0)
    };
    let fps_text = diagnostics
        .as_ref()
        .and_then(|store| store.get(&FrameTimeDiagnosticsPlugin::FPS))
        .and_then(|fps| fps.smoothed().or_else(|| fps.value()))
        .map(|fps| format!("{fps:.1}"))
        .unwrap_or_else(|| "--.-".to_string());
    let speed = vel.length();

    if let Ok(mut text) = text_queries.p0().single_mut() {
        text.0 = format!("FPS {}  ZOOM {}", fps_text, zoom_text);
    }
    if let Ok(mut text) = text_queries.p1().single_mut() {
        text.0 = format!("{:.1} m/s", speed);
    }
    if let Ok(mut text) = text_queries.p2().single_mut() {
        text.0 = format!("({:.0}, {:.0})", pos.x, pos.y);
    }
    if let Ok(mut fill) = bar_value_queries.p0().single_mut() {
        fill.ratio = health_ratio;
    }
    if let Ok(mut fill) = bar_value_queries.p1().single_mut() {
        fill.ratio = fuel_ratio;
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn update_segmented_bars_system(
    bar_roots: Query<'_, '_, (&SegmentedBarValue, &SegmentedBarStyle, &Children)>,
    mut segments: Query<'_, '_, (&SegmentedBarSegment, &mut BackgroundColor)>,
) {
    for (value, style, children) in &bar_roots {
        let seg_count = style.segments.max(1);
        let ratio = value.ratio.clamp(0.0, 1.0);
        let active_segments = ((ratio * seg_count as f32).round() as i32).clamp(0, seg_count as i32);
        for child in children.iter() {
            let Ok((segment, mut color)) = segments.get_mut(child) else {
                continue;
            };
            color.0 = if (segment.index as i32) < active_segments {
                style.active_color
            } else {
                style.inactive_color
            };
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn sync_ship_nameplates_system(
    mut commands: Commands<'_, '_>,
    ships: Query<
        '_,
        '_,
        (
            Entity,
            Option<&EntityLabels>,
        ),
        (
            With<WorldEntity>,
            Without<ShipNameplateRoot>,
        ),
    >,
    existing: Query<'_, '_, (Entity, &ShipNameplateRoot)>,
) {
    let mut existing_targets = HashMap::<Entity, Entity>::new();
    for (entity, root) in &existing {
        existing_targets.insert(root.target, entity);
    }

    for (ship_entity, labels) in &ships {
        let is_ship = labels.is_some_and(|labels| labels.0.iter().any(|label| label == "Ship"));
        if !is_ship || existing_targets.contains_key(&ship_entity) {
            continue;
        }
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: px(100),
                    left: px(0),
                    top: px(0),
                    flex_direction: FlexDirection::Row,
                    ..default()
                },
                Visibility::Hidden,
                ShipNameplateRoot { target: ship_entity },
                GameplayHud,
                UiOverlayLayer,
                RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
                WorldEntity,
                DespawnOnExit(ClientAppState::InWorld),
            ))
            .with_children(|plate| {
                plate
                    .spawn((
                        Node {
                            width: percent(100.0),
                            height: px(8.0),
                            column_gap: px(1.0),
                            align_items: AlignItems::Stretch,
                            border: UiRect::all(px(1.0)),
                            padding: UiRect::all(px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.05, 0.08, 0.05, 0.75)),
                        BorderColor::all(Color::srgba(0.18, 0.35, 0.18, 0.8)),
                        SegmentedBarStyle {
                            segments: 16,
                            active_color: Color::srgb(0.22, 0.9, 0.34),
                            inactive_color: Color::srgba(0.08, 0.22, 0.10, 0.85),
                        },
                        SegmentedBarValue { ratio: 1.0 },
                        ShipNameplateHealthBar {
                            target: ship_entity,
                        },
                    ))
                    .with_children(|bar| {
                        for index in 0..16u8 {
                            bar.spawn((
                                Node {
                                    flex_grow: 1.0,
                                    height: percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.15, 0.2, 0.28, 0.85)),
                                SegmentedBarSegment { index },
                            ));
                        }
                    });
            });
    }

    for (nameplate_entity, root) in &existing {
        if ships.get(root.target).is_err() {
            commands.entity(nameplate_entity).despawn();
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn update_ship_nameplate_positions_system(
    mut roots: Query<'_, '_, (&ShipNameplateRoot, &mut Node, &mut Visibility)>,
    mut health_bars: Query<'_, '_, (&ShipNameplateHealthBar, &mut SegmentedBarValue)>,
    ships: Query<
        '_,
        '_,
        (
            Entity,
            &Transform,
            Option<&SizeM>,
            Option<&HealthPool>,
            Option<&EntityLabels>,
        ),
        With<WorldEntity>,
    >,
    controlled_query: Query<'_, '_, &Transform, With<ControlledEntity>>,
    gameplay_camera: Query<'_, '_, (&Camera, &GlobalTransform), With<GameplayCamera>>,
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
) {
    let Ok((camera, camera_transform)) = gameplay_camera.single() else {
        return;
    };
    let Ok(window) = window_query.single() else {
        return;
    };
    let controlled_position = controlled_query
        .single()
        .ok()
        .map(|transform| transform.translation.truncate());

    let mut ship_data_by_entity = HashMap::<Entity, (Vec3, f32, f32)>::new();
    for (entity, transform, size_m, health_pool, labels) in &ships {
        let is_ship = labels.is_some_and(|labels| labels.0.iter().any(|label| label == "Ship"));
        if !is_ship {
            continue;
        }
        let health_ratio = health_pool
            .map(|health| {
                if health.maximum > 0.0 {
                    (health.current / health.maximum).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        let half_height_world = size_m.map(|s| s.length * 0.5).unwrap_or(6.0);
        ship_data_by_entity.insert(
            entity,
            (
                transform.translation,
                half_height_world,
                health_ratio,
            ),
        );
    }

    for (root, mut node, mut visibility) in &mut roots {
        let Some((world_pos, half_height_world, _)) = ship_data_by_entity.get(&root.target) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        if let Some(controlled_pos) = controlled_position {
            let ship_pos = world_pos.truncate();
            let visibility_radius_m = 300.0_f32;
            if (ship_pos - controlled_pos).length_squared() > visibility_radius_m * visibility_radius_m {
                *visibility = Visibility::Hidden;
                continue;
            }
        }
        let center_world = Vec3::new(world_pos.x, world_pos.y, 0.0);
        let Ok(viewport_pos) = camera.world_to_viewport(camera_transform, center_world) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let top_world = Vec3::new(world_pos.x, world_pos.y + *half_height_world, 0.0);
        let Ok(top_viewport_pos) = camera.world_to_viewport(camera_transform, top_world) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        if viewport_pos.x < 0.0
            || viewport_pos.x > window.width()
            || viewport_pos.y < 0.0
            || viewport_pos.y > window.height()
        {
            *visibility = Visibility::Hidden;
            continue;
        }
        let plate_width = 100.0;
        let plate_height = 8.0;
        let vertical_gap = 6.0;
        node.left = px(viewport_pos.x - plate_width * 0.5);
        let ship_top_y_px = viewport_pos.y.min(top_viewport_pos.y);
        node.top = px(ship_top_y_px - plate_height - vertical_gap);
        *visibility = Visibility::Visible;

        if let Some((_, _, health_ratio)) = ship_data_by_entity.get(&root.target) {
            for (bar_target, mut value) in &mut health_bars {
                if bar_target.target == root.target {
                    value.ratio = *health_ratio;
                }
            }
        }
    }
}
