use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
#[cfg(target_arch = "wasm32")]
use bevy::log::BoxedLayer;
use bevy::log::tracing::field::{Field, Visit};
use bevy::log::tracing::{Event, Level, Subscriber};
use bevy::log::tracing_subscriber::Layer;
#[cfg(not(target_arch = "wasm32"))]
use bevy::log::tracing_subscriber::fmt::MakeWriter;
#[cfg(not(target_arch = "wasm32"))]
use bevy::log::{BoxedFmtLayer, BoxedLayer};
use bevy::prelude::AppExit;
use bevy::prelude::*;
use std::collections::VecDeque;
#[cfg(not(target_arch = "wasm32"))]
use std::fs::{File, OpenOptions};
#[cfg(not(target_arch = "wasm32"))]
use std::io::Write;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(not(target_arch = "wasm32"))]
use sidereal_core::logging::prepare_timestamped_log_file_in_dir;

use super::ecs_util::queue_despawn_if_exists;

const DEV_CONSOLE_MAX_BUFFER_LINES: usize = 10_000;
const DEV_CONSOLE_VISIBLE_LINES_MAX: usize = 200;
const DEV_CONSOLE_DISPLAYED_LINES_CAP: usize = 2_000;
const DEV_CONSOLE_PAGE_LINES: usize = 24;
const DEV_CONSOLE_LOG_FONT_SIZE: f32 = 14.0;
const DEV_CONSOLE_INPUT_ROW_PAD_Y: f32 = 6.0;
const DEV_CONSOLE_INPUT_ROW_PAD_X: f32 = 8.0;
const DEV_CONSOLE_LOG_VIEW_BOTTOM_PAD: f32 = 10.0;
const DEV_CONSOLE_CHAR_WIDTH_FACTOR: f32 = 0.62;
const DEV_CONSOLE_VISIBLE_LINES_SAFETY_ROWS: usize = 0;

#[cfg(not(target_arch = "wasm32"))]
static CLIENT_LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

fn dev_console_input_row_height_px() -> f32 {
    DEV_CONSOLE_LOG_FONT_SIZE + (DEV_CONSOLE_INPUT_ROW_PAD_Y * 2.0) + 10.0
}

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
        let line = ConsoleLogLine {
            ts_epoch_ms: console_timestamp_epoch_ms(),
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
        let newest_seq = guard.lines.back().map(|line| line.seq).unwrap_or(last_seq);
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

#[cfg(target_arch = "wasm32")]
fn console_timestamp_epoch_ms() -> u128 {
    js_sys::Date::now().max(0.0) as u128
}

#[cfg(not(target_arch = "wasm32"))]
fn console_timestamp_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
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

#[derive(Clone)]
#[cfg(not(target_arch = "wasm32"))]
struct DualLogMakeWriter {
    file: Arc<Mutex<File>>,
}

#[cfg(not(target_arch = "wasm32"))]
struct DualLogWriter {
    file: Arc<Mutex<File>>,
    stderr: std::io::Stderr,
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a> MakeWriter<'a> for DualLogMakeWriter {
    type Writer = DualLogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        DualLogWriter {
            file: self.file.clone(),
            stderr: std::io::stderr(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Write for DualLogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = self.stderr.write_all(buf);
        if let Ok(mut file) = self.file.lock() {
            file.write_all(buf)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = self.stderr.flush();
        if let Ok(mut file) = self.file.lock() {
            file.flush()?;
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn log_file_path() -> PathBuf {
    CLIENT_LOG_PATH
        .get_or_init(|| {
            if let Ok(path) = std::env::var("SIDEREAL_CLIENT_LOG_FILE") {
                return PathBuf::from(path);
            }
            let logs_dir = std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(PathBuf::from))
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| PathBuf::from("."))
                .join("logs");
            prepare_timestamped_log_file_in_dir("sidereal-client", &logs_dir)
                .map(|run_log| run_log.path)
                .unwrap_or_else(|_| PathBuf::from("logs/sidereal-client.log"))
        })
        .clone()
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn install_panic_file_hook() {
    let log_path = log_file_path();
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let bt = std::backtrace::Backtrace::force_capture();
        let panic_line = format!("PANIC: {panic_info}\nBACKTRACE:\n{bt}\n",);
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
            let _ = writeln!(file, "{panic_line}");
        }
        let _ = std::io::stderr().write_all(panic_line.as_bytes());
        old_hook(panic_info);
    }));
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn build_file_fmt_layer(_app: &mut App) -> Option<BoxedFmtLayer> {
    let path = log_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return None;
    };
    let writer = DualLogMakeWriter {
        file: Arc::new(Mutex::new(file)),
    };
    Some(Box::new(
        bevy::log::tracing_subscriber::fmt::Layer::default()
            .with_ansi(false)
            .with_writer(writer),
    ))
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
    pub scrollback_lines: usize,
    pub visible_lines: usize,
    pub visible_columns: usize,
    pub rendered_rows_total: usize,
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
            scrollback_lines: 0,
            visible_lines: 40,
            visible_columns: 120,
            rendered_rows_total: 0,
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
struct DevConsoleLogList;

#[derive(Component)]
struct DevConsoleLogLineText;

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
            log_close_and_exit_messages_system,
            toggle_dev_console_system,
            animate_dev_console_system,
            handle_dev_console_input_system,
            sync_dev_console_log_lines_system,
            tick_dev_console_cursor_blink_system,
            update_dev_console_ui_system,
        )
            .chain(),
    );
}

fn log_close_and_exit_messages_system(
    mut close_reader: MessageReader<'_, '_, bevy::window::WindowCloseRequested>,
    mut exit_reader: MessageReader<'_, '_, AppExit>,
) {
    for close in close_reader.read() {
        bevy::log::warn!(
            "dev_console observed WindowCloseRequested window={:?}",
            close.window
        );
    }
    for exit in exit_reader.read() {
        bevy::log::warn!("dev_console observed AppExit message={exit:?}");
    }
}

pub(crate) fn is_console_open(state: Option<&DevConsoleState>) -> bool {
    state.is_some_and(|state| state.is_open)
}

fn spawn_dev_console_ui_system(
    mut commands: Commands<'_, '_>,
    existing: Query<'_, '_, Entity, With<DevConsoleRoot>>,
) {
    if !existing.is_empty() {
        return;
    }
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
                    border: UiRect::bottom(Val::Px(1.0)),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.06, 0.1, 1.0)),
                BorderColor::all(Color::srgba(0.28, 0.4, 0.58, 0.98)),
                BoxShadow::new(
                    Color::srgba(0.0, 0.0, 0.0, 0.5),
                    Val::Px(0.0),
                    Val::Px(8.0),
                    Val::Px(0.0),
                    Val::Px(10.0),
                ),
                DevConsolePanel,
            ))
            .with_children(|panel| {
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        min_height: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::bottom(Val::Px(DEV_CONSOLE_LOG_VIEW_BOTTOM_PAD)),
                        row_gap: Val::Px(0.0),
                        overflow: Overflow::clip_y(),
                        ..default()
                    },
                    DevConsoleLogList,
                ));
                panel
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(dev_console_input_row_height_px()),
                            flex_shrink: 0.0,
                            padding: UiRect::axes(
                                Val::Px(DEV_CONSOLE_INPUT_ROW_PAD_X),
                                Val::Px(DEV_CONSOLE_INPUT_ROW_PAD_Y),
                            ),
                            align_items: AlignItems::Center,
                            overflow: Overflow::clip_x(),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.04, 0.06, 0.1, 1.0)),
                    ))
                    .with_children(|input_row| {
                        input_row.spawn((
                            Text::new("> "),
                            TextFont {
                                font: Handle::<Font>::default(),
                                font_size: DEV_CONSOLE_LOG_FONT_SIZE,
                                ..default()
                            },
                            TextColor(Color::srgb(0.94, 0.96, 1.0)),
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
    clamp_scrollback(&mut state);
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
    state.scrollback_lines = 0;
    state.logs_dirty = true;
}

fn animate_dev_console_system(time: Res<'_, Time>, mut state: ResMut<'_, DevConsoleState>) {
    let dt = time.delta_secs().max(0.0);
    // Fixed-duration animation avoids long asymptotic tails near 0/1.
    let duration_s = 0.18;
    let step = (dt / duration_s).clamp(0.0, 1.0);
    if state.anim_t < state.target_t {
        state.anim_t = (state.anim_t + step).min(state.target_t);
    } else if state.anim_t > state.target_t {
        state.anim_t = (state.anim_t - step).max(state.target_t);
    }
    // Snap shut to avoid lingering a thin strip when nearly closed.
    if state.target_t <= 0.0 && state.anim_t < 0.03 {
        state.anim_t = 0.0;
    }
}

fn handle_dev_console_input_system(
    mut key_events: MessageReader<'_, '_, KeyboardInput>,
    mut mouse_wheel_events: MessageReader<'_, '_, MouseWheel>,
    keys: Res<'_, ButtonInput<KeyCode>>,
    mut state: ResMut<'_, DevConsoleState>,
) {
    let toggled_this_frame = keys.just_pressed(KeyCode::Backquote);
    if toggled_this_frame {
        // Do not treat the toggle key as input text this frame.
        return;
    }
    if !state.is_open {
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        state.is_open = false;
        state.target_t = 0.0;
        state.logs_dirty = true;
        return;
    }
    if keys.just_pressed(KeyCode::PageUp) {
        apply_scrollback_delta(&mut state, DEV_CONSOLE_PAGE_LINES as isize);
    }
    if keys.just_pressed(KeyCode::PageDown) {
        apply_scrollback_delta(&mut state, -(DEV_CONSOLE_PAGE_LINES as isize));
    }
    if keys.just_pressed(KeyCode::Home) {
        state.scrollback_lines = state
            .rendered_rows_total
            .saturating_sub(state.visible_lines.max(1));
        state.logs_dirty = true;
    }
    if keys.just_pressed(KeyCode::End) {
        state.scrollback_lines = 0;
        state.logs_dirty = true;
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
                        .filter(|ch| *ch != '`' && *ch != '~')
                        .filter(|ch| !ch.is_control())
                        .collect::<String>();
                    if !append.is_empty() {
                        state.input_line.push_str(&append);
                        continue;
                    }
                }
                if let Key::Character(chars) = &event.logical_key {
                    let append = chars
                        .chars()
                        .filter(|ch| *ch != '`' && *ch != '~')
                        .filter(|ch| !ch.is_control())
                        .collect::<String>();
                    if !append.is_empty() {
                        state.input_line.push_str(&append);
                    }
                }
            }
        }
    }
    for wheel in mouse_wheel_events.read() {
        let delta_lines: f32 = match wheel.unit {
            MouseScrollUnit::Line => wheel.y,
            MouseScrollUnit::Pixel => wheel.y / 24.0,
        };
        if delta_lines.abs() < f32::EPSILON {
            continue;
        }
        // Wheel up = older lines, wheel down = newer lines.
        let step = delta_lines.abs().ceil() as isize;
        let signed_step = if delta_lines.is_sign_positive() {
            step
        } else {
            -step
        };
        apply_scrollback_delta(&mut state, signed_step);
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
        push_local_console_line(
            &mut state,
            Level::INFO,
            "sidereal_client::dev_console",
            format!("> {command}"),
        );
        push_local_console_line(
            &mut state,
            Level::INFO,
            "sidereal_client::dev_console",
            "not implemented yet".to_string(),
        );
        state.input_line.clear();
        state.scrollback_lines = 0;
        clamp_scrollback(&mut state);
        state.logs_dirty = true;
    }
}

fn push_local_console_line(
    state: &mut DevConsoleState,
    level: Level,
    target: &str,
    message: String,
) {
    state.displayed_lines.push_back(ConsoleLogLine {
        ts_epoch_ms: console_timestamp_epoch_ms(),
        level,
        target: target.to_string(),
        message,
    });
    while state.displayed_lines.len() > DEV_CONSOLE_DISPLAYED_LINES_CAP {
        let _ = state.displayed_lines.pop_front();
    }
}

fn clamp_scrollback(state: &mut DevConsoleState) {
    let max_scrollback = state
        .rendered_rows_total
        .saturating_sub(state.visible_lines.max(1));
    state.scrollback_lines = state.scrollback_lines.min(max_scrollback);
}

fn level_color(level: Level) -> Color {
    match level {
        Level::ERROR => Color::srgb(0.95, 0.35, 0.35),
        Level::WARN => Color::srgb(0.95, 0.8, 0.3),
        Level::INFO => Color::srgb(0.45, 0.9, 0.45),
        Level::DEBUG => Color::srgb(0.5, 0.72, 0.95),
        Level::TRACE => Color::srgb(0.72, 0.72, 0.72),
    }
}

fn level_label(level: Level) -> &'static str {
    match level {
        Level::ERROR => "ERROR",
        Level::WARN => "WARN ",
        Level::INFO => "INFO ",
        Level::DEBUG => "DEBUG",
        Level::TRACE => "TRACE",
    }
}

fn spawn_message_spans(parent: &mut bevy::ecs::hierarchy::ChildSpawnerCommands, message: &str) {
    let bright = Color::srgb(0.94, 0.96, 1.0);
    let mut first = true;
    for token in message.split_whitespace() {
        if !first {
            parent.spawn((
                TextSpan::new(" "),
                TextColor(bright),
                TextFont {
                    font: Handle::<Font>::default(),
                    font_size: DEV_CONSOLE_LOG_FONT_SIZE,
                    ..default()
                },
            ));
        }
        first = false;
        if let Some((key, value)) = token.split_once('=')
            && !key.is_empty()
        {
            parent.spawn((
                TextSpan::new(key),
                TextColor(Color::BLACK),
                TextBackgroundColor(Color::WHITE),
                TextFont {
                    font: Handle::<Font>::default(),
                    font_size: DEV_CONSOLE_LOG_FONT_SIZE,
                    ..default()
                },
            ));
            parent.spawn((
                TextSpan::new(format!("={value}")),
                TextColor(bright),
                TextFont {
                    font: Handle::<Font>::default(),
                    font_size: DEV_CONSOLE_LOG_FONT_SIZE,
                    ..default()
                },
            ));
        } else {
            parent.spawn((
                TextSpan::new(token),
                TextColor(bright),
                TextFont {
                    font: Handle::<Font>::default(),
                    font_size: DEV_CONSOLE_LOG_FONT_SIZE,
                    ..default()
                },
            ));
        }
    }
}

#[derive(Clone)]
struct ConsoleRenderRow {
    ts_epoch_ms: u128,
    level: Level,
    target: String,
    message: String,
    continuation: bool,
}

fn wrap_message_chunks(message: &str, width_chars: usize) -> Vec<String> {
    let width = width_chars.max(8);
    let mut rows = Vec::<String>::new();
    let mut current = String::new();
    for word in message.split_whitespace() {
        let word_len = word.chars().count();
        let cur_len = current.chars().count();
        let required = if current.is_empty() {
            word_len
        } else {
            cur_len + 1 + word_len
        };
        if required <= width {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
            continue;
        }
        if !current.is_empty() {
            rows.push(current);
            current = String::new();
        }
        if word_len <= width {
            current.push_str(word);
            continue;
        }
        // Hard-wrap very long tokens.
        let mut token = word.chars().collect::<Vec<char>>();
        while token.len() > width {
            let chunk = token.drain(..width).collect::<String>();
            rows.push(chunk);
        }
        if !token.is_empty() {
            current = token.iter().collect::<String>();
        }
    }
    if !current.is_empty() {
        rows.push(current);
    }
    if rows.is_empty() {
        rows.push(String::new());
    }
    rows
}

fn build_render_rows(
    lines: &VecDeque<ConsoleLogLine>,
    max_columns: usize,
) -> Vec<ConsoleRenderRow> {
    let mut rows = Vec::<ConsoleRenderRow>::new();
    for line in lines {
        let ts_human = format_epoch_ms_utc(line.ts_epoch_ms);
        let prefix = format!("[{ts_human}] {} {}: ", level_label(line.level), line.target);
        let prefix_chars = prefix.chars().count();
        let first_budget = max_columns.saturating_sub(prefix_chars).max(8);
        let continuation_budget = max_columns.saturating_sub(4).max(8);
        let chunks = wrap_message_chunks(&line.message, first_budget);
        if let Some(first) = chunks.first() {
            rows.push(ConsoleRenderRow {
                ts_epoch_ms: line.ts_epoch_ms,
                level: line.level,
                target: line.target.clone(),
                message: first.clone(),
                continuation: false,
            });
        }
        for chunk in chunks.iter().skip(1) {
            let clipped = wrap_message_chunks(chunk, continuation_budget)
                .into_iter()
                .next()
                .unwrap_or_default();
            rows.push(ConsoleRenderRow {
                ts_epoch_ms: line.ts_epoch_ms,
                level: line.level,
                target: line.target.clone(),
                message: clipped,
                continuation: true,
            });
        }
    }
    rows
}

fn spawn_log_line(parent: &mut bevy::ecs::hierarchy::ChildSpawnerCommands, row: &ConsoleRenderRow) {
    let dim = Color::srgba(0.74, 0.78, 0.86, 0.9);
    let ts_human = format_epoch_ms_utc(row.ts_epoch_ms);
    parent
        .spawn((
            Text::new(""),
            TextFont {
                // Default bevy UI font handle provides a console-like fixed layout appearance.
                font: Handle::<Font>::default(),
                font_size: DEV_CONSOLE_LOG_FONT_SIZE,
                ..default()
            },
            DevConsoleLogLineText,
        ))
        .with_children(|spans| {
            if row.continuation {
                spans.spawn((
                    TextSpan::new("    "),
                    TextColor(dim),
                    TextFont {
                        font: Handle::<Font>::default(),
                        font_size: DEV_CONSOLE_LOG_FONT_SIZE,
                        ..default()
                    },
                ));
            } else {
                spans.spawn((
                    TextSpan::new(format!("[{ts_human}] ")),
                    TextColor(dim),
                    TextFont {
                        font: Handle::<Font>::default(),
                        font_size: DEV_CONSOLE_LOG_FONT_SIZE,
                        ..default()
                    },
                ));
                spans.spawn((
                    TextSpan::new(format!("{} ", level_label(row.level))),
                    TextColor(level_color(row.level)),
                    TextFont {
                        font: Handle::<Font>::default(),
                        font_size: DEV_CONSOLE_LOG_FONT_SIZE,
                        ..default()
                    },
                ));
                spans.spawn((
                    TextSpan::new(format!("{}: ", row.target)),
                    TextColor(dim),
                    TextFont {
                        font: Handle::<Font>::default(),
                        font_size: DEV_CONSOLE_LOG_FONT_SIZE,
                        ..default()
                    },
                ));
            }
            spawn_message_spans(spans, &row.message);
        });
}

fn format_epoch_ms_utc(epoch_ms: u128) -> String {
    let total_secs = (epoch_ms / 1000) as i64;
    let millis = (epoch_ms % 1000) as i64;
    let days = total_secs.div_euclid(86_400);
    let sec_of_day = total_secs.rem_euclid(86_400);

    let hour = sec_of_day / 3600;
    let minute = (sec_of_day % 3600) / 60;
    let second = sec_of_day % 60;

    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

fn truncate_tail_for_console(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let tail_count = max_chars.saturating_sub(1);
    let start = chars.len().saturating_sub(tail_count);
    let tail: String = chars[start..].iter().collect();
    format!("…{tail}")
}

// Howard Hinnant's civil-from-days algorithm, Unix epoch-based.
fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year as i32, m as u32, d as u32)
}

fn apply_scrollback_delta(state: &mut DevConsoleState, delta: isize) {
    if delta > 0 {
        state.scrollback_lines = state.scrollback_lines.saturating_add(delta as usize);
    } else {
        state.scrollback_lines = state.scrollback_lines.saturating_sub(delta.unsigned_abs());
    }
    clamp_scrollback(state);
    state.logs_dirty = true;
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
#[allow(clippy::too_many_arguments)]
fn update_dev_console_ui_system(
    mut commands: Commands<'_, '_>,
    windows: Query<'_, '_, &'_ Window, With<bevy::window::PrimaryWindow>>,
    mut state: ResMut<'_, DevConsoleState>,
    blink: Res<'_, DevConsoleCursorBlink>,
    mut root_nodes: Query<'_, '_, &'_ mut Node, (With<DevConsoleRoot>, Without<DevConsolePanel>)>,
    mut panel_nodes: Query<'_, '_, &'_ mut Node, (With<DevConsolePanel>, Without<DevConsoleRoot>)>,
    log_list: Query<'_, '_, Entity, With<DevConsoleLogList>>,
    log_line_text_entities: Query<'_, '_, Entity, With<DevConsoleLogLineText>>,
    mut input_text: Query<'_, '_, &'_ mut Text, With<DevConsoleInputText>>,
) {
    let Ok(mut root_node) = root_nodes.single_mut() else {
        return;
    };
    let Ok(mut panel_node) = panel_nodes.single_mut() else {
        return;
    };
    let win_h = windows
        .single()
        .map(|window| window.height())
        .unwrap_or(1080.0);
    let win_w = windows
        .single()
        .map(|window| window.width())
        .unwrap_or(1920.0);
    let eased = 1.0 - (1.0 - state.anim_t).powi(3);
    let panel_h = win_h * 0.5 * eased;
    let line_px = (DEV_CONSOLE_LOG_FONT_SIZE + 4.0).max(1.0);
    let viewport_h =
        (panel_h - dev_console_input_row_height_px() - DEV_CONSOLE_LOG_VIEW_BOTTOM_PAD - 8.0)
            .max(1.0);
    let measured_lines =
        ((viewport_h / line_px).floor() as usize).clamp(1, DEV_CONSOLE_VISIBLE_LINES_MAX);
    state.visible_lines = measured_lines
        .saturating_sub(DEV_CONSOLE_VISIBLE_LINES_SAFETY_ROWS)
        .max(1);
    let panel_inner_w = (win_w - 24.0).max(64.0);
    let approx_char_px = (DEV_CONSOLE_LOG_FONT_SIZE * DEV_CONSOLE_CHAR_WIDTH_FACTOR).max(1.0);
    state.visible_columns = ((panel_inner_w / approx_char_px).floor() as usize).clamp(24, 400);
    panel_node.height = Val::Px(panel_h.max(0.0));
    root_node.display = if state.anim_t > 0.0 || state.target_t > 0.0 {
        Display::Flex
    } else {
        Display::None
    };

    if state.logs_dirty {
        let rows = build_render_rows(&state.displayed_lines, state.visible_columns);
        state.rendered_rows_total = rows.len();
        let total = rows.len();
        clamp_scrollback(&mut state);
        let end = total.saturating_sub(state.scrollback_lines);
        let start = end.saturating_sub(state.visible_lines.max(1));
        for entity in &log_line_text_entities {
            queue_despawn_if_exists(&mut commands, entity);
        }
        if let Ok(list_entity) = log_list.single() {
            commands.entity(list_entity).with_children(|parent| {
                for row in rows.iter().skip(start).take(end - start) {
                    spawn_log_line(parent, row);
                }
            });
        }
        state.logs_dirty = false;
    }

    if let Ok(mut input_text) = input_text.single_mut() {
        let usable_width = (win_w - (12.0 * 2.0) - (DEV_CONSOLE_INPUT_ROW_PAD_X * 2.0)).max(80.0);
        let approx_char_width =
            (DEV_CONSOLE_LOG_FONT_SIZE * DEV_CONSOLE_CHAR_WIDTH_FACTOR).max(1.0);
        let max_line_chars = (usable_width / approx_char_width).floor() as usize;
        let max_input_chars = max_line_chars.saturating_sub(4).max(1);
        let display_input = truncate_tail_for_console(&state.input_line, max_input_chars);
        let caret = if state.is_open && blink.visible {
            "|"
        } else {
            " "
        };
        input_text.0 = format!("> {}{}", display_input, caret);
    }
}
