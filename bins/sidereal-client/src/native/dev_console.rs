use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::log::BoxedLayer;
use bevy::log::tracing::field::{Field, Visit};
use bevy::log::tracing::{Event, Level, Subscriber};
use bevy::log::tracing_subscriber::Layer;
use bevy::prelude::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use super::resources::EmbeddedFonts;

const DEV_CONSOLE_MAX_BUFFER_LINES: usize = 10_000;
const DEV_CONSOLE_VISIBLE_LINES: usize = 120;
const DEV_CONSOLE_DISPLAYED_LINES_CAP: usize = 2_000;

#[derive(Debug, Clone)]
pub(crate) struct ConsoleLogLine {
    pub ts_epoch_ms: u128,
    pub level: Level,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Clone)]
struct SequencedConsoleLogLine {
    seq: u64,
    line: ConsoleLogLine,
}

#[derive(Debug)]
struct ConsoleLogBuffer {
    lines: VecDeque<SequencedConsoleLogLine>,
    next_seq: u64,
}

impl Default for ConsoleLogBuffer {
    fn default() -> Self {
        Self {
            lines: VecDeque::new(),
            next_seq: 1,
        }
    }
}

#[derive(Resource, Clone)]
pub(crate) struct SharedConsoleLogBuffer {
    inner: Arc<Mutex<ConsoleLogBuffer>>,
}

impl Default for SharedConsoleLogBuffer {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ConsoleLogBuffer::default())),
        }
    }
}

impl SharedConsoleLogBuffer {
    pub(crate) fn push(&self, level: Level, target: String, message: String) {
        let ts_epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let line = ConsoleLogLine {
            ts_epoch_ms,
            level,
            target,
            message,
        };
        self.push_line(line);
    }

    fn push_line(&self, line: ConsoleLogLine) {
        if let Ok(mut guard) = self.inner.lock() {
            let seq = guard.next_seq;
            guard.next_seq = guard.next_seq.saturating_add(1);
            guard.lines.push_back(SequencedConsoleLogLine { seq, line });
            while guard.lines.len() > DEV_CONSOLE_MAX_BUFFER_LINES {
                let _ = guard.lines.pop_front();
            }
        }
    }

    pub(crate) fn read_since(&self, last_seq: u64) -> (u64, Vec<ConsoleLogLine>) {
        let Ok(guard) = self.inner.lock() else {
            return (last_seq, Vec::new());
        };
        let newest_seq = guard
            .lines
            .back()
            .map(|line| line.seq)
            .unwrap_or(last_seq);
        let first_seq = guard
            .lines
            .front()
            .map(|line| line.seq)
            .unwrap_or(last_seq.saturating_add(1));
        let effective_last_seq = if last_seq < first_seq.saturating_sub(1) {
            first_seq.saturating_sub(1)
        } else {
            last_seq
        };
        let collected = guard
            .lines
            .iter()
            .filter(|line| line.seq > effective_last_seq)
            .map(|line| line.line.clone())
            .collect();
        (newest_seq, collected)
    }
}

pub(crate) struct ConsoleTracingLayer {
    buffer: SharedConsoleLogBuffer,
}

impl<S> Layer<S> for ConsoleTracingLayer
where
    S: Subscriber,
{
    fn on_event(
        &self,
        event: &Event<'_>,
        _ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let mut visitor = ConsoleFieldVisitor::default();
        event.record(&mut visitor);
        let message = visitor.render_message();
        self.buffer.push(
            *metadata.level(),
            metadata.target().to_string(),
            message.trim().to_string(),
        );
    }
}

#[derive(Default)]
struct ConsoleFieldVisitor {
    message: Option<String>,
    extras: Vec<String>,
}

impl ConsoleFieldVisitor {
    fn record_value(&mut self, field_name: &str, value: String) {
        if field_name == "message" {
            self.message = Some(value);
        } else {
            self.extras.push(format!("{field_name}={value}"));
        }
    }

    fn render_message(self) -> String {
        match self.message {
            Some(message) if self.extras.is_empty() => message,
            Some(message) => format!("{message} {}", self.extras.join(" ")),
            None if self.extras.is_empty() => "<event>".to_string(),
            None => self.extras.join(" "),
        }
    }
}

impl Visit for ConsoleFieldVisitor {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_value(field.name(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_value(field.name(), value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_value(field.name(), value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_value(field.name(), value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_value(field.name(), format!("{value:?}"));
    }
}

pub(crate) fn build_log_capture_layer(app: &mut App) -> Option<BoxedLayer> {
    let buffer = app
        .world_mut()
        .get_resource::<SharedConsoleLogBuffer>()
        .cloned()
        .unwrap_or_else(|| {
            let created = SharedConsoleLogBuffer::default();
            app.world_mut().insert_resource(created.clone());
            created
        });
    Some(Box::new(ConsoleTracingLayer { buffer }))
}

#[derive(Debug, Resource)]
pub(crate) struct DevConsoleState {
    pub is_open: bool,
    pub anim_t: f32,
    pub target_t: f32,
    pub input_line: String,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub log_cursor_seq: u64,
    pub displayed_lines: VecDeque<ConsoleLogLine>,
    pub logs_dirty: bool,
}

impl Default for DevConsoleState {
    fn default() -> Self {
        Self {
            is_open: false,
            anim_t: 0.0,
            target_t: 0.0,
            input_line: String::new(),
            history: Vec::new(),
            history_index: None,
            log_cursor_seq: 0,
            displayed_lines: VecDeque::new(),
            logs_dirty: true,
        }
    }
}

#[derive(Debug, Resource)]
pub(crate) struct DevConsoleCursorBlink {
    timer: Timer,
    visible: bool,
}

impl Default for DevConsoleCursorBlink {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
            visible: true,
        }
    }
}

#[derive(Component)]
struct DevConsoleRoot;

#[derive(Component)]
struct DevConsolePanel;

#[derive(Component)]
struct DevConsoleLogText;

#[derive(Component)]
struct DevConsoleInputText;

pub(super) fn register_console(app: &mut App) {
    app.init_resource::<SharedConsoleLogBuffer>();
    app.init_resource::<DevConsoleState>();
    app.init_resource::<DevConsoleCursorBlink>();
    app.add_systems(Startup, spawn_dev_console_ui_system);
    app.add_systems(
        Update,
        (
            sync_dev_console_log_lines_system,
            toggle_dev_console_system,
            animate_dev_console_system,
            handle_dev_console_input_system,
            tick_dev_console_cursor_blink_system,
            update_dev_console_ui_system,
        ),
    );
}

pub(crate) fn is_console_open(state: Option<&DevConsoleState>) -> bool {
    state.is_some_and(|state| state.is_open)
}

fn spawn_dev_console_ui_system(
    mut commands: Commands<'_, '_>,
    fonts: Res<'_, EmbeddedFonts>,
    existing: Query<'_, '_, Entity, With<DevConsoleRoot>>,
) {
    if !existing.is_empty() {
        return;
    }
    let font_regular = fonts.regular.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Stretch,
                display: Display::None,
                ..default()
            },
            ZIndex(950),
            DevConsoleRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(0.0),
                    border: UiRect::all(Val::Px(1.0)),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    overflow: Overflow::clip_y(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.06, 0.1, 0.94)),
                BorderColor::all(Color::srgba(0.24, 0.33, 0.48, 0.92)),
                DevConsolePanel,
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new(""),
                    TextFont {
                        font: font_regular.clone(),
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.84, 0.9, 0.98, 0.95)),
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        ..default()
                    },
                    DevConsoleLogText,
                ));
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(28.0),
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.09, 0.12, 0.18, 0.95)),
                ))
                .with_children(|input_row| {
                    input_row.spawn((
                        Text::new("> "),
                        TextFont {
                            font: font_regular,
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.83, 0.9, 0.98)),
                        DevConsoleInputText,
                    ));
                });
            });
        });
}

fn sync_dev_console_log_lines_system(
    log_buffer: Res<'_, SharedConsoleLogBuffer>,
    mut state: ResMut<'_, DevConsoleState>,
) {
    let (newest_seq, new_lines) = log_buffer.read_since(state.log_cursor_seq);
    if new_lines.is_empty() {
        return;
    }
    state.log_cursor_seq = newest_seq;
    for line in new_lines {
        state.displayed_lines.push_back(line);
    }
    while state.displayed_lines.len() > DEV_CONSOLE_DISPLAYED_LINES_CAP {
        let _ = state.displayed_lines.pop_front();
    }
    state.logs_dirty = true;
}

fn toggle_dev_console_system(
    input: Res<'_, ButtonInput<KeyCode>>,
    mut state: ResMut<'_, DevConsoleState>,
) {
    if !input.just_pressed(KeyCode::Backquote) {
        return;
    }
    state.is_open = !state.is_open;
    state.target_t = if state.is_open { 1.0 } else { 0.0 };
    state.history_index = None;
    state.logs_dirty = true;
}

fn animate_dev_console_system(time: Res<'_, Time>, mut state: ResMut<'_, DevConsoleState>) {
    let dt = time.delta_secs().max(0.0);
    let response = (dt * 14.0).clamp(0.0, 1.0);
    state.anim_t += (state.target_t - state.anim_t) * response;
    if (state.anim_t - state.target_t).abs() < 0.001 {
        state.anim_t = state.target_t;
    }
}

fn handle_dev_console_input_system(
    mut key_events: MessageReader<'_, '_, KeyboardInput>,
    keys: Res<'_, ButtonInput<KeyCode>>,
    mut state: ResMut<'_, DevConsoleState>,
    log_buffer: Res<'_, SharedConsoleLogBuffer>,
) {
    if !state.is_open {
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        state.is_open = false;
        state.target_t = 0.0;
        state.logs_dirty = true;
        return;
    }

    let mut submit = false;
    for event in key_events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        match &event.logical_key {
            Key::Enter => {
                submit = true;
            }
            Key::Backspace => {
                let _ = state.input_line.pop();
            }
            Key::ArrowUp => {
                if state.history.is_empty() {
                    continue;
                }
                let next_index = match state.history_index {
                    Some(index) if index > 0 => index - 1,
                    Some(index) => index,
                    None => state.history.len().saturating_sub(1),
                };
                state.history_index = Some(next_index);
                state.input_line = state.history[next_index].clone();
            }
            Key::ArrowDown => {
                if state.history.is_empty() {
                    continue;
                }
                let next_index = match state.history_index {
                    Some(index) if index + 1 < state.history.len() => Some(index + 1),
                    _ => None,
                };
                state.history_index = next_index;
                state.input_line = next_index
                    .map(|index| state.history[index].clone())
                    .unwrap_or_default();
            }
            _ => {
                if let Some(text) = &event.text {
                    let append = text
                        .chars()
                        .filter(|ch| !ch.is_control())
                        .collect::<String>();
                    if !append.is_empty() {
                        state.input_line.push_str(&append);
                    }
                }
            }
        }
    }

    if submit || keys.just_pressed(KeyCode::Enter) {
        let command = state.input_line.trim().to_string();
        if command.is_empty() {
            state.input_line.clear();
            return;
        }
        if state.history.last() != Some(&command) {
            state.history.push(command.clone());
        }
        state.history_index = None;
        log_buffer.push(
            Level::INFO,
            "sidereal_client::dev_console".to_string(),
            format!("> {command}"),
        );
        log_buffer.push(
            Level::INFO,
            "sidereal_client::dev_console".to_string(),
            "not implemented yet".to_string(),
        );
        state.input_line.clear();
    }
}

fn tick_dev_console_cursor_blink_system(
    time: Res<'_, Time>,
    state: Res<'_, DevConsoleState>,
    mut blink: ResMut<'_, DevConsoleCursorBlink>,
) {
    if !state.is_open && state.anim_t <= 0.01 {
        blink.visible = false;
        return;
    }
    blink.timer.tick(time.delta());
    if blink.timer.just_finished() {
        blink.visible = !blink.visible;
    }
}

#[allow(clippy::type_complexity)]
fn update_dev_console_ui_system(
    windows: Query<'_, '_, &'_ Window, With<bevy::window::PrimaryWindow>>,
    mut state: ResMut<'_, DevConsoleState>,
    blink: Res<'_, DevConsoleCursorBlink>,
    mut root_nodes: Query<'_, '_, &'_ mut Node, (With<DevConsoleRoot>, Without<DevConsolePanel>)>,
    mut panel_nodes: Query<'_, '_, &'_ mut Node, (With<DevConsolePanel>, Without<DevConsoleRoot>)>,
    mut log_text: Query<'_, '_, &'_ mut Text, (With<DevConsoleLogText>, Without<DevConsoleInputText>)>,
    mut input_text: Query<'_, '_, &'_ mut Text, (With<DevConsoleInputText>, Without<DevConsoleLogText>)>,
) {
    let Ok(mut root_node) = root_nodes.single_mut() else {
        return;
    };
    let Ok(mut panel_node) = panel_nodes.single_mut() else {
        return;
    };
    let win_h = windows.single().map(|window| window.height()).unwrap_or(1080.0);
    let eased = 1.0 - (1.0 - state.anim_t).powi(3);
    let panel_h = win_h * 0.5 * eased;
    panel_node.height = Val::Px(panel_h.max(0.0));
    root_node.display = if panel_h > 1.0 || state.target_t > 0.0 {
        Display::Flex
    } else {
        Display::None
    };

    if state.logs_dirty && let Ok(mut log_text) = log_text.single_mut() {
        let total = state.displayed_lines.len();
        let start = total.saturating_sub(DEV_CONSOLE_VISIBLE_LINES);
        let mut rendered = String::new();
        for line in state.displayed_lines.iter().skip(start) {
            let seconds = line.ts_epoch_ms / 1000;
            let millis = line.ts_epoch_ms % 1000;
            let level = match line.level {
                Level::ERROR => "ERROR",
                Level::WARN => "WARN ",
                Level::INFO => "INFO ",
                Level::DEBUG => "DEBUG",
                Level::TRACE => "TRACE",
            };
            rendered.push_str(&format!(
                "[{seconds}.{millis:03}] {level} {}: {}\n",
                line.target, line.message
            ));
        }
        log_text.0 = rendered;
        state.logs_dirty = false;
    }

    if let Ok(mut input_text) = input_text.single_mut() {
        let caret = if blink.visible && state.is_open { "_" } else { " " };
        input_text.0 = format!("> {}{}", state.input_line, caret);
    }
}
