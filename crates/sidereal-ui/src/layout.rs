use bevy::prelude::*;

pub fn fullscreen_centered_root() -> Node {
    Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

pub fn fullscreen_backdrop() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    }
}

pub fn panel(width: Val, padding_px: f32, gap_px: f32, radius_px: f32, border_px: f32) -> Node {
    Node {
        width,
        padding: UiRect::all(Val::Px(padding_px)),
        border: UiRect::all(Val::Px(border_px)),
        border_radius: BorderRadius::all(Val::Px(radius_px)),
        overflow: Overflow::visible(),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(gap_px),
        ..default()
    }
}

pub fn vertical_stack(gap_px: f32) -> Node {
    Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(gap_px),
        ..default()
    }
}

pub fn horizontal_stack(gap_px: f32, justify_content: JustifyContent) -> Node {
    Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        justify_content,
        column_gap: Val::Px(gap_px),
        ..default()
    }
}

pub fn grid(columns: usize, min_row_height_px: f32, gap_px: f32) -> Node {
    Node {
        width: Val::Percent(100.0),
        display: Display::Grid,
        grid_template_columns: RepeatedGridTrack::flex(columns as u16, 1.0),
        grid_auto_rows: vec![GridTrack::min_content(), GridTrack::px(min_row_height_px)],
        row_gap: Val::Px(gap_px),
        column_gap: Val::Px(gap_px),
        ..default()
    }
}

pub fn button(width: Val, height_px: f32, radius_px: f32, border_px: f32) -> Node {
    Node {
        width,
        height: Val::Px(height_px),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(border_px)),
        border_radius: BorderRadius::all(Val::Px(radius_px)),
        ..default()
    }
}

pub fn leading_button(
    width: Val,
    height_px: f32,
    radius_px: f32,
    border_px: f32,
    padding_x_px: f32,
) -> Node {
    Node {
        width,
        height: Val::Px(height_px),
        justify_content: JustifyContent::FlexStart,
        align_items: AlignItems::Center,
        padding: UiRect::axes(Val::Px(padding_x_px), Val::Px(0.0)),
        border: UiRect::all(Val::Px(border_px)),
        border_radius: BorderRadius::all(Val::Px(radius_px)),
        ..default()
    }
}

pub fn input_box(height_px: f32, radius_px: f32, border_px: f32) -> Node {
    input_box_with_adornments(height_px, radius_px, border_px, false, false)
}

pub fn input_box_with_adornments(
    height_px: f32,
    radius_px: f32,
    border_px: f32,
    has_start_adornment: bool,
    has_end_adornment: bool,
) -> Node {
    let radius_px = radius_px.max(0.0);
    Node {
        width: Val::Percent(100.0),
        height: Val::Px(height_px),
        padding: UiRect {
            left: Val::Px(if has_start_adornment { 10.0 } else { 12.0 }),
            right: Val::Px(if has_end_adornment { 10.0 } else { 12.0 }),
            top: Val::Px(8.0),
            bottom: Val::Px(8.0),
        },
        justify_content: JustifyContent::FlexStart,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        border: UiRect::all(Val::Px(border_px)),
        border_radius: BorderRadius::all(Val::Px(radius_px)),
        ..default()
    }
}

pub fn input_text_slot() -> Node {
    Node {
        flex_grow: 1.0,
        min_width: Val::Px(0.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        overflow: Overflow::clip(),
        ..default()
    }
}

pub fn input_adornment() -> Node {
    Node {
        height: Val::Percent(100.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        flex_shrink: 0.0,
        ..default()
    }
}
