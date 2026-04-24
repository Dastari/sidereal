use std::collections::HashSet;
use std::io::{self, Stdout};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};

use crate::log_buffer::SharedLogBuffer;
use crate::replication::admin::{
    AdminCommand, AdminCommandBusSender, command_spec, command_specs, parse_admin_command,
};
use crate::replication::health::{
    SharedHealthSnapshot, SharedWorldExplorerSnapshot, SharedWorldMapSnapshot,
    WorldExplorerEntitySnapshot, WorldExplorerGroupSnapshot, WorldExplorerSnapshot,
    WorldMapSnapshot,
};
use bevy::log::error;

type Backend = CrosstermBackend<Stdout>;

const FRAME_INTERVAL: Duration = Duration::from_millis(100);
const WORLD_CELL_ASPECT_Y: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneFocus {
    Logs,
    Sessions,
    World,
    Health,
}

impl PaneFocus {
    fn next(self) -> Self {
        match self {
            Self::Logs => Self::Sessions,
            Self::Sessions => Self::World,
            Self::World => Self::Health,
            Self::Health => Self::Logs,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Logs => Self::Health,
            Self::Sessions => Self::Logs,
            Self::World => Self::Sessions,
            Self::Health => Self::World,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Normal,
    Command,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogLevelFilter {
    All,
    Info,
    Warn,
    Error,
}

impl LogLevelFilter {
    fn next(self) -> Self {
        match self {
            Self::All => Self::Info,
            Self::Info => Self::Warn,
            Self::Warn => Self::Error,
            Self::Error => Self::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    fn matches(self, line: &str) -> bool {
        match self {
            Self::All => true,
            Self::Info => {
                line.contains(" INFO ") || line.contains(" INFO\t") || line.contains("INFO")
            }
            Self::Warn => line.contains(" WARN ") || line.contains("WARN"),
            Self::Error => line.contains(" ERROR ") || line.contains("ERROR"),
        }
    }
}

#[derive(Debug)]
struct TuiApp {
    focus: PaneFocus,
    mode: InputMode,
    level_filter: LogLevelFilter,
    command_input: String,
    search_input: String,
    active_search: String,
    clear_from: usize,
    pending_clear_logs: bool,
    follow_logs: bool,
    log_scroll: usize,
    max_log_scroll: usize,
    log_visible_rows: usize,
    selected_log_line: Option<usize>,
    keep_selected_log_visible: bool,
    should_exit: bool,
    dialog: Option<TuiDialog>,
    tui_last_frame_ms: f64,
    tui_last_log_read_ms: f64,
    tui_max_frame_ms: f64,
    logs_rect: Rect,
    sessions_rect: Rect,
    world_rect: Rect,
    health_rect: Rect,
    world_tree_selected_key: Option<String>,
    world_tree_scroll: usize,
    world_tree_last_rows: usize,
    world_tree_visible_rows: usize,
    world_tree_last_keys: Vec<String>,
    world_tree_last_rows_data: Vec<WorldTreeRow>,
    world_tree_expandable_keys: HashSet<String>,
    world_tree_keep_selected_visible: bool,
    collapsed_world_tree_keys: HashSet<String>,
    world_center_x: f32,
    world_center_y: f32,
    world_zoom: f32,
    world_selected_guid: Option<String>,
    world_selected_name: Option<String>,
    world_cursor_x: f32,
    world_cursor_y: f32,
    world_drag_anchor: Option<(u16, u16)>,
    world_initialized: bool,
}

impl Default for TuiApp {
    fn default() -> Self {
        Self {
            focus: PaneFocus::Logs,
            mode: InputMode::Normal,
            level_filter: LogLevelFilter::All,
            command_input: String::new(),
            search_input: String::new(),
            active_search: String::new(),
            clear_from: 0,
            pending_clear_logs: false,
            follow_logs: true,
            log_scroll: 0,
            max_log_scroll: 0,
            log_visible_rows: 0,
            selected_log_line: None,
            keep_selected_log_visible: false,
            should_exit: false,
            dialog: None,
            tui_last_frame_ms: 0.0,
            tui_last_log_read_ms: 0.0,
            tui_max_frame_ms: 0.0,
            logs_rect: Rect::default(),
            sessions_rect: Rect::default(),
            world_rect: Rect::default(),
            health_rect: Rect::default(),
            world_tree_selected_key: None,
            world_tree_scroll: 0,
            world_tree_last_rows: 0,
            world_tree_visible_rows: 0,
            world_tree_last_keys: Vec::new(),
            world_tree_last_rows_data: Vec::new(),
            world_tree_expandable_keys: HashSet::new(),
            world_tree_keep_selected_visible: false,
            collapsed_world_tree_keys: HashSet::new(),
            world_center_x: 0.0,
            world_center_y: 0.0,
            world_zoom: 250.0,
            world_selected_guid: None,
            world_selected_name: None,
            world_cursor_x: 0.0,
            world_cursor_y: 0.0,
            world_drag_anchor: None,
            world_initialized: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ShortcutHint<'a> {
    key: &'a str,
    label: &'a str,
}

impl<'a> ShortcutHint<'a> {
    const fn new(key: &'a str, label: &'a str) -> Self {
        Self { key, label }
    }
}

#[derive(Debug, Clone)]
enum TuiDialog {
    ConfirmReset,
    ConfirmQuit,
    Help,
}

pub fn start(
    log_buffer: SharedLogBuffer,
    command_sender: AdminCommandBusSender,
    health_snapshot: SharedHealthSnapshot,
    world_explorer_snapshot: SharedWorldExplorerSnapshot,
    world_snapshot: SharedWorldMapSnapshot,
) -> Result<(), String> {
    thread::Builder::new()
        .name("replication-tui".to_string())
        .spawn(move || {
            if let Err(err) = run(
                log_buffer,
                command_sender,
                health_snapshot,
                world_explorer_snapshot,
                world_snapshot,
            ) {
                error!("replication TUI terminated: {err}");
            }
        })
        .map(|_| ())
        .map_err(|err| format!("failed to spawn TUI thread: {err}"))
}

fn run(
    log_buffer: SharedLogBuffer,
    command_sender: AdminCommandBusSender,
    health_snapshot: SharedHealthSnapshot,
    world_explorer_snapshot: SharedWorldExplorerSnapshot,
    world_snapshot: SharedWorldMapSnapshot,
) -> Result<(), String> {
    enable_raw_mode().map_err(|err| format!("enable raw mode failed: {err}"))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|err| format!("enter alternate screen failed: {err}"))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal =
        Terminal::new(backend).map_err(|err| format!("terminal init failed: {err}"))?;
    let mut app = TuiApp::default();

    let result = event_loop(
        &mut terminal,
        &mut app,
        log_buffer,
        command_sender,
        health_snapshot,
        world_explorer_snapshot,
        world_snapshot,
    );

    disable_raw_mode().map_err(|err| format!("disable raw mode failed: {err}"))?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .map_err(|err| format!("leave alternate screen failed: {err}"))?;
    terminal
        .show_cursor()
        .map_err(|err| format!("show cursor failed: {err}"))?;

    result
}

fn event_loop(
    terminal: &mut Terminal<Backend>,
    app: &mut TuiApp,
    log_buffer: SharedLogBuffer,
    command_sender: AdminCommandBusSender,
    health_snapshot: SharedHealthSnapshot,
    world_explorer_snapshot: SharedWorldExplorerSnapshot,
    world_snapshot: SharedWorldMapSnapshot,
) -> Result<(), String> {
    let mut last_draw = Instant::now() - FRAME_INTERVAL;
    while !app.should_exit {
        if last_draw.elapsed() >= FRAME_INTERVAL {
            let frame_started = Instant::now();
            let log_read_started = Instant::now();
            let logs = log_buffer.snapshot();
            app.tui_last_log_read_ms = log_read_started.elapsed().as_secs_f64() * 1000.0;
            let health = health_snapshot.load();
            let world_explorer = world_explorer_snapshot.load();
            let world = world_snapshot.load();
            terminal
                .draw(|frame| render(frame, app, &logs, &health, &world_explorer, &world))
                .map_err(|err| format!("terminal draw failed: {err}"))?;
            app.tui_last_frame_ms = frame_started.elapsed().as_secs_f64() * 1000.0;
            app.tui_max_frame_ms = app.tui_max_frame_ms.max(app.tui_last_frame_ms);
            last_draw = Instant::now();
        }

        if event::poll(Duration::from_millis(25))
            .map_err(|err| format!("event poll failed: {err}"))?
        {
            match event::read().map_err(|err| format!("event read failed: {err}"))? {
                Event::Key(key) => handle_key(app, key, &command_sender)?,
                Event::Mouse(mouse) => handle_mouse(
                    app,
                    mouse,
                    &world_explorer_snapshot,
                    &world_snapshot,
                    &log_buffer,
                )?,
                Event::Resize(_, _) | Event::FocusGained | Event::FocusLost | Event::Paste(_) => {}
            }
        }
    }
    Ok(())
}

fn handle_mouse(
    app: &mut TuiApp,
    mouse: MouseEvent,
    world_explorer_snapshot: &SharedWorldExplorerSnapshot,
    world_snapshot: &SharedWorldMapSnapshot,
    log_buffer: &SharedLogBuffer,
) -> Result<(), String> {
    if let Some(focus) = pane_at_position(app, mouse.column, mouse.row) {
        app.focus = focus;
    }
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if contains(app.logs_rect, (mouse.column, mouse.row)) {
                let logs = log_buffer.snapshot();
                select_log_line_at(app, &logs, mouse.column, mouse.row);
            }
            if contains(app.sessions_rect, (mouse.column, mouse.row)) {
                let snapshot = world_explorer_snapshot.load();
                select_world_tree_row_at(app, &snapshot, mouse.column, mouse.row);
            }
            if contains(app.world_rect, (mouse.column, mouse.row)) {
                let snapshot = world_snapshot.load();
                select_world_entity_at(app, &snapshot, mouse.column, mouse.row);
                app.world_drag_anchor = Some((mouse.column, mouse.row));
            }
        }
        MouseEventKind::Drag(MouseButton::Left) if app.focus == PaneFocus::World => {
            if let Some((last_x, last_y)) = app.world_drag_anchor {
                let dx = mouse.column as i32 - last_x as i32;
                let dy = mouse.row as i32 - last_y as i32;
                app.world_center_x -= dx as f32 * app.world_zoom;
                app.world_center_y += dy as f32 * app.world_zoom * WORLD_CELL_ASPECT_Y;
            }
            app.world_drag_anchor = Some((mouse.column, mouse.row));
        }
        MouseEventKind::Up(MouseButton::Left) => {
            app.world_drag_anchor = None;
        }
        MouseEventKind::ScrollUp => {
            if contains(app.logs_rect, (mouse.column, mouse.row)) {
                app.focus = PaneFocus::Logs;
                scroll_logs_up(app, 3);
                return Ok(());
            }
            if contains(app.sessions_rect, (mouse.column, mouse.row)) {
                app.focus = PaneFocus::Sessions;
                scroll_world_tree(app, -3);
                return Ok(());
            }
            if contains(app.world_rect, (mouse.column, mouse.row)) {
                app.focus = PaneFocus::World;
                app.world_zoom = (app.world_zoom * 0.8).max(10.0);
            }
        }
        MouseEventKind::ScrollDown => {
            if contains(app.logs_rect, (mouse.column, mouse.row)) {
                app.focus = PaneFocus::Logs;
                scroll_logs_down(app, 3);
                return Ok(());
            }
            if contains(app.sessions_rect, (mouse.column, mouse.row)) {
                app.focus = PaneFocus::Sessions;
                scroll_world_tree(app, 3);
                return Ok(());
            }
            if contains(app.world_rect, (mouse.column, mouse.row)) {
                app.focus = PaneFocus::World;
                app.world_zoom = (app.world_zoom * 1.25).min(50_000.0);
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_key(
    app: &mut TuiApp,
    key: KeyEvent,
    command_sender: &AdminCommandBusSender,
) -> Result<(), String> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.dialog = Some(TuiDialog::ConfirmQuit);
        return Ok(());
    }

    if app.dialog.is_some() {
        return handle_dialog_key(app, key, command_sender);
    }

    match app.mode {
        InputMode::Normal => handle_normal_key(app, key, command_sender),
        InputMode::Command => handle_command_key(app, key, command_sender),
        InputMode::Search => handle_search_key(app, key),
    }
}

fn handle_normal_key(
    app: &mut TuiApp,
    key: KeyEvent,
    _command_sender: &AdminCommandBusSender,
) -> Result<(), String> {
    match key.code {
        KeyCode::Char('q') => app.dialog = Some(TuiDialog::ConfirmQuit),
        KeyCode::Tab => app.focus = app.focus.next(),
        KeyCode::BackTab => app.focus = app.focus.previous(),
        KeyCode::Char('h') => {
            if app.focus == PaneFocus::Sessions {
                collapse_world_tree_selection(app);
            } else if app.focus != PaneFocus::Logs {
                app.focus = app.focus.previous();
            }
        }
        KeyCode::Left => {
            if app.focus == PaneFocus::Sessions {
                collapse_world_tree_selection(app);
            } else if app.focus != PaneFocus::Logs {
                app.focus = app.focus.previous();
            }
        }
        KeyCode::Char('l') => {
            if app.focus == PaneFocus::Sessions {
                expand_world_tree_selection(app);
            } else if app.focus != PaneFocus::Logs {
                app.focus = app.focus.next();
            }
        }
        KeyCode::Right => {
            if app.focus == PaneFocus::Sessions {
                expand_world_tree_selection(app);
            } else if app.focus != PaneFocus::Logs {
                app.focus = app.focus.next();
            }
        }
        KeyCode::Char('k') => {
            if app.focus == PaneFocus::Logs {
                move_log_selection(app, -1);
            } else if app.focus == PaneFocus::Sessions {
                move_world_tree_selection(app, -1);
            } else {
                app.focus = PaneFocus::Logs;
            }
        }
        KeyCode::Char('j') => {
            if app.focus == PaneFocus::Logs {
                move_log_selection(app, 1);
            } else if app.focus == PaneFocus::Sessions {
                move_world_tree_selection(app, 1);
            } else {
                app.focus = PaneFocus::Sessions;
            }
        }
        KeyCode::Up => {
            if app.focus == PaneFocus::Logs {
                move_log_selection(app, -1);
            } else if app.focus == PaneFocus::Sessions {
                move_world_tree_selection(app, -1);
            }
        }
        KeyCode::Down => {
            if app.focus == PaneFocus::Logs {
                move_log_selection(app, 1);
            } else if app.focus == PaneFocus::Sessions {
                move_world_tree_selection(app, 1);
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') if app.focus == PaneFocus::Sessions => {
            toggle_world_tree_selection(app);
        }
        KeyCode::Char('[') if app.focus == PaneFocus::Sessions => collapse_all_world_tree(app),
        KeyCode::Char(']') if app.focus == PaneFocus::Sessions => expand_all_world_tree(app),
        KeyCode::PageUp => {
            if app.focus == PaneFocus::Logs {
                page_log_selection(app, -1);
            } else if app.focus == PaneFocus::Sessions {
                page_world_tree_selection(app, -1);
            }
        }
        KeyCode::PageDown => {
            if app.focus == PaneFocus::Logs {
                page_log_selection(app, 1);
            } else if app.focus == PaneFocus::Sessions {
                page_world_tree_selection(app, 1);
            }
        }
        KeyCode::End | KeyCode::Char('G') if app.focus == PaneFocus::Logs => {
            jump_to_latest_log(app)
        }
        KeyCode::Char('?') => app.dialog = Some(TuiDialog::Help),
        KeyCode::Char(':') => {
            app.mode = InputMode::Command;
            app.command_input.clear();
        }
        KeyCode::Char('/') => {
            app.mode = InputMode::Search;
            app.search_input = app.active_search.clone();
        }
        KeyCode::Char('f') => app.level_filter = app.level_filter.next(),
        KeyCode::Char('0') if app.focus == PaneFocus::World => {
            app.world_center_x = 0.0;
            app.world_center_y = 0.0;
            app.world_zoom = 250.0;
            app.world_cursor_x = 0.0;
            app.world_cursor_y = 0.0;
        }
        KeyCode::Char('g') if app.focus == PaneFocus::Sessions => goto_selected_world_entity(app),
        KeyCode::Char('c') => {
            request_clear_logs(app);
        }
        _ => {}
    }
    Ok(())
}

fn handle_command_key(
    app: &mut TuiApp,
    key: KeyEvent,
    command_sender: &AdminCommandBusSender,
) -> Result<(), String> {
    match key.code {
        KeyCode::Esc => {
            app.command_input.clear();
            app.mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            let command = app.command_input.trim().to_string();
            if !command.is_empty() {
                let request = parse_admin_command(&command);
                match request.command {
                    AdminCommand::Clear => request_clear_logs(app),
                    AdminCommand::Filter { ref level } => apply_filter_command(app, level),
                    AdminCommand::Help => app.dialog = Some(TuiDialog::Help),
                    AdminCommand::Reset { force: false }
                        if command_spec("reset").is_some_and(|spec| spec.requires_confirmation) =>
                    {
                        app.dialog = Some(TuiDialog::ConfirmReset)
                    }
                    AdminCommand::Quit => app.dialog = Some(TuiDialog::ConfirmQuit),
                    _ => {
                        let _ = command_sender.send(request);
                    }
                }
            }
            app.command_input.clear();
            app.mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            app.command_input.pop();
        }
        KeyCode::Char(ch)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.command_input.push(ch);
        }
        _ => {}
    }
    Ok(())
}

fn handle_dialog_key(
    app: &mut TuiApp,
    key: KeyEvent,
    command_sender: &AdminCommandBusSender,
) -> Result<(), String> {
    match (&app.dialog, key.code) {
        (_, KeyCode::Esc) => app.dialog = None,
        (Some(TuiDialog::Help), KeyCode::Enter) => app.dialog = None,
        (Some(TuiDialog::ConfirmReset), KeyCode::Enter) => {
            command_sender.send(parse_admin_command("reset force"))?;
            app.dialog = None;
        }
        (Some(TuiDialog::ConfirmQuit), KeyCode::Enter) => {
            command_sender.send(parse_admin_command("quit"))?;
            app.should_exit = true;
            app.dialog = None;
        }
        _ => {}
    }
    Ok(())
}

fn handle_search_key(app: &mut TuiApp, key: KeyEvent) -> Result<(), String> {
    match key.code {
        KeyCode::Esc => {
            app.search_input.clear();
            app.mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            app.active_search = app.search_input.clone();
            app.mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            app.search_input.pop();
        }
        KeyCode::Char(ch)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.search_input.push(ch);
        }
        _ => {}
    }
    Ok(())
}

fn render(
    frame: &mut ratatui::Frame<'_>,
    app: &mut TuiApp,
    logs: &[String],
    health: &crate::replication::health::ReplicationHealthSnapshot,
    world_explorer: &WorldExplorerSnapshot,
    world: &WorldMapSnapshot,
) {
    if app.pending_clear_logs {
        app.clear_from = logs.len();
        app.pending_clear_logs = false;
        app.selected_log_line = None;
        app.log_scroll = 0;
        app.max_log_scroll = 0;
        app.keep_selected_log_visible = false;
        app.follow_logs = true;
    }
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(frame.area());
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(layout[1]);
    app.logs_rect = layout[0];
    app.sessions_rect = bottom[0];
    app.world_rect = bottom[1];
    app.health_rect = bottom[2];
    initialize_world_view(app, world);

    render_logs(frame, app, logs, layout[0]);
    render_world_tree(frame, app, world_explorer, bottom[0]);
    render_world_map(frame, app, world, bottom[1]);
    render_health(frame, app, health, bottom[2]);
    render_command_bar(frame, app, layout[2]);
    render_dialog(frame, app);
}

fn initialize_world_view(app: &mut TuiApp, world: &WorldMapSnapshot) {
    if app.world_initialized || world.entities.is_empty() {
        return;
    }
    if let Some(entity) = world
        .entities
        .iter()
        .find(|entity| entity.glyph == '☻')
        .or_else(|| world.entities.first())
    {
        app.world_center_x = entity.x as f32;
        app.world_center_y = entity.y as f32;
        app.world_cursor_x = entity.x as f32;
        app.world_cursor_y = entity.y as f32;
        app.world_initialized = true;
    }
}

fn pane_at_position(app: &TuiApp, column: u16, row: u16) -> Option<PaneFocus> {
    let point = (column, row);
    if contains(app.logs_rect, point) {
        Some(PaneFocus::Logs)
    } else if contains(app.sessions_rect, point) {
        Some(PaneFocus::Sessions)
    } else if contains(app.world_rect, point) {
        Some(PaneFocus::World)
    } else if contains(app.health_rect, point) {
        Some(PaneFocus::Health)
    } else {
        None
    }
}

fn contains(rect: Rect, point: (u16, u16)) -> bool {
    let (x, y) = point;
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

fn render_logs(
    frame: &mut ratatui::Frame<'_>,
    app: &mut TuiApp,
    logs: &[String],
    area: ratatui::layout::Rect,
) {
    let filtered = filtered_logs(app, logs);
    let block = logs_panel_block(app);
    let inner = inner_rect(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let scrollbar_gutter = 1usize;
    let text_width = inner.width.saturating_sub(scrollbar_gutter as u16) as usize;
    let visible_height = inner.height as usize;
    app.log_visible_rows = visible_height;
    let wrapped_rows = build_wrapped_log_rows(&filtered, text_width);
    let max_scroll = wrapped_rows.len().saturating_sub(visible_height);
    app.max_log_scroll = max_scroll;
    let effective_selected_log_line = resolve_selected_log_line(app, filtered.len());
    let scroll = compute_log_scroll(
        app,
        &wrapped_rows,
        visible_height,
        effective_selected_log_line,
    );

    let mut visible_lines = wrapped_rows
        .iter()
        .skip(scroll)
        .take(visible_height)
        .map(|row| {
            let selected =
                effective_selected_log_line.is_some_and(|selected| selected == row.line_index);
            styled_cells_to_line(&row.cells, selected)
        })
        .collect::<Vec<_>>();
    while visible_lines.len() < visible_height {
        visible_lines.push(Line::from(" ".repeat(text_width)));
    }
    let text_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width.saturating_sub(1),
        height: inner.height,
    };
    frame.render_widget(Paragraph::new(visible_lines), text_area);

    if visible_height > 0 && wrapped_rows.len() > visible_height {
        let scrollbar_position = compute_scrollbar_position(scroll, max_scroll, wrapped_rows.len());
        let mut scrollbar_state = ScrollbarState::new(wrapped_rows.len())
            .viewport_content_length(visible_height)
            .position(scrollbar_position);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .thumb_style(Style::default().fg(Color::Rgb(96, 165, 250)))
                .track_style(Style::default().fg(Color::Rgb(55, 65, 81))),
            Rect {
                x: inner.x + inner.width.saturating_sub(1),
                y: inner.y,
                width: 1,
                height: inner.height,
            },
            &mut scrollbar_state,
        );
    }
}

fn compute_scrollbar_position(scroll: usize, max_scroll: usize, content_len: usize) -> usize {
    if content_len <= 1 || max_scroll == 0 {
        return 0;
    }
    let last_index = content_len.saturating_sub(1);
    scroll.saturating_mul(last_index) / max_scroll
}

#[derive(Debug, Clone)]
struct WorldTreeRow {
    key: String,
    depth: usize,
    label: String,
    kind_label: Option<String>,
    entity_guid: Option<String>,
    entity_display_name: Option<String>,
    entity_position_xy: Option<(f64, f64)>,
    latency_ms: Option<u64>,
    expandable: bool,
    expanded: bool,
    accent: Color,
}

fn build_world_tree_rows(explorer: &WorldExplorerSnapshot, app: &TuiApp) -> Vec<WorldTreeRow> {
    let mut rows = Vec::new();
    for group in &explorer.groups {
        push_group_tree_rows(&mut rows, group, app);
    }
    rows
}

fn push_group_tree_rows(
    rows: &mut Vec<WorldTreeRow>,
    group: &WorldExplorerGroupSnapshot,
    app: &TuiApp,
) {
    let expanded = !app.collapsed_world_tree_keys.contains(&group.key);
    rows.push(WorldTreeRow {
        key: group.key.clone(),
        depth: 0,
        label: format!("{} ({})", group.label, group.entities.len()),
        kind_label: None,
        entity_guid: None,
        entity_display_name: None,
        entity_position_xy: None,
        latency_ms: None,
        expandable: !group.entities.is_empty(),
        expanded,
        accent: Color::Rgb(167, 139, 250),
    });
    if !expanded {
        return;
    }
    for entity in &group.entities {
        push_entity_tree_rows(rows, entity, app, 1);
    }
}

fn push_entity_tree_rows(
    rows: &mut Vec<WorldTreeRow>,
    entity: &WorldExplorerEntitySnapshot,
    app: &TuiApp,
    depth: usize,
) {
    let key = format!("entity:{}", entity.guid);
    let expanded = !app.collapsed_world_tree_keys.contains(&key);
    let mut label = entity
        .display_name
        .clone()
        .unwrap_or_else(|| entity.guid.clone());
    if entity.is_controlled {
        label.push_str(" [controlled]");
    }
    rows.push(WorldTreeRow {
        key: key.clone(),
        depth,
        label,
        kind_label: Some(entity.kind_label.clone()),
        entity_guid: Some(entity.guid.clone()),
        entity_display_name: entity.display_name.clone(),
        entity_position_xy: entity.position_xy,
        latency_ms: entity.latency_ms,
        expandable: !entity.children.is_empty(),
        expanded,
        accent: if entity.is_player_anchor {
            Color::Rgb(125, 211, 252)
        } else if entity.is_controlled {
            Color::Rgb(248, 113, 113)
        } else {
            Color::Rgb(203, 213, 225)
        },
    });
    if !expanded {
        return;
    }
    for child in &entity.children {
        push_entity_tree_rows(rows, child, app, depth + 1);
    }
}

fn world_tree_row_line(
    row: &WorldTreeRow,
    width: usize,
    selected_key: Option<&str>,
) -> Line<'static> {
    let selected = selected_key.is_some_and(|key| key == row.key);
    let row_bg = if selected {
        Color::Rgb(30, 41, 59)
    } else {
        Color::Reset
    };
    let indent = "  ".repeat(row.depth);
    let branch = if row.expandable {
        if row.expanded { "▾ " } else { "▸ " }
    } else {
        "  "
    };
    let kind_prefix = row
        .kind_label
        .as_deref()
        .map(|kind| match kind {
            "player" => "@ ",
            "ship" => "◆ ",
            "landmark" => "◉ ",
            "projectile" => "• ",
            _ => "· ",
        })
        .unwrap_or("");
    let ping_text = row.latency_ms.map(|ping| format!("{ping:>4}ms"));
    let left_text = format!("{indent}{branch}{kind_prefix}{}", row.label);
    let total_right = ping_text
        .as_ref()
        .map(|value| value.chars().count() + 1)
        .unwrap_or(0);
    let left_width = left_text.chars().count();
    let spacer_width = width.saturating_sub(left_width + total_right);
    let mut spans = vec![Span::styled(
        left_text,
        Style::default()
            .fg(row.accent)
            .bg(row_bg)
            .add_modifier(if selected {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
    )];
    if let Some(kind) = &row.kind_label {
        let kind_hint = format!(" <{kind}>");
        if spacer_width > kind_hint.chars().count() + total_right {
            spans.push(Span::styled(
                kind_hint,
                Style::default().fg(Color::DarkGray).bg(row_bg),
            ));
            let remaining = width.saturating_sub(
                spans
                    .iter()
                    .map(|span| span.content.chars().count())
                    .sum::<usize>()
                    + total_right,
            );
            spans.push(Span::styled(
                " ".repeat(remaining),
                Style::default().bg(row_bg),
            ));
        } else {
            spans.push(Span::styled(
                " ".repeat(spacer_width),
                Style::default().bg(row_bg),
            ));
        }
    } else {
        spans.push(Span::styled(
            " ".repeat(spacer_width),
            Style::default().bg(row_bg),
        ));
    }
    if let Some(ping_text) = ping_text {
        spans.push(Span::styled(" ", Style::default().bg(row_bg)));
        spans.push(Span::styled(
            ping_text,
            Style::default()
                .fg(Color::Rgb(196, 181, 253))
                .bg(row_bg)
                .add_modifier(Modifier::BOLD),
        ));
    }
    Line::from(spans)
}

fn render_world_tree(
    frame: &mut ratatui::Frame<'_>,
    app: &mut TuiApp,
    explorer: &WorldExplorerSnapshot,
    area: ratatui::layout::Rect,
) {
    let footer = Some(Line::from(vec![
        Span::styled(
            format!("groups:{} ", explorer.groups.len()),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            app.world_tree_selected_key
                .as_deref()
                .unwrap_or("no selection")
                .to_string(),
            Style::default().fg(Color::Rgb(203, 213, 225)),
        ),
    ]));
    let block = panel_block(
        "world",
        app.focus == PaneFocus::Sessions,
        Color::Rgb(167, 139, 250),
        &[
            ShortcutHint::new("[", "collapse all"),
            ShortcutHint::new("]", "expand all"),
            ShortcutHint::new("g", "goto"),
        ],
        footer,
    );
    let inner = inner_rect(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let rows = build_world_tree_rows(explorer, app);
    app.world_tree_last_rows = rows.len();
    app.world_tree_last_keys = rows.iter().map(|row| row.key.clone()).collect();
    app.world_tree_last_rows_data = rows.clone();
    app.world_tree_expandable_keys = rows
        .iter()
        .filter(|row| row.expandable)
        .map(|row| row.key.clone())
        .collect();
    let visible_height = inner.height as usize;
    app.world_tree_visible_rows = visible_height;
    if app.world_tree_selected_key.is_none() {
        app.world_tree_selected_key = rows.first().map(|row| row.key.clone());
    }
    let selected_index = rows
        .iter()
        .position(|row| app.world_tree_selected_key.as_deref() == Some(row.key.as_str()))
        .unwrap_or(0);
    if app.world_tree_keep_selected_visible {
        if selected_index < app.world_tree_scroll {
            app.world_tree_scroll = selected_index;
        } else if visible_height > 0
            && selected_index >= app.world_tree_scroll.saturating_add(visible_height)
        {
            app.world_tree_scroll = selected_index
                .saturating_add(1)
                .saturating_sub(visible_height);
        }
        app.world_tree_keep_selected_visible = false;
    }
    let max_scroll = rows.len().saturating_sub(visible_height);
    app.world_tree_scroll = app.world_tree_scroll.min(max_scroll);

    let text_width = inner.width.saturating_sub(1) as usize;
    let mut visible_lines = rows
        .iter()
        .skip(app.world_tree_scroll)
        .take(visible_height)
        .map(|row| world_tree_row_line(row, text_width, app.world_tree_selected_key.as_deref()))
        .collect::<Vec<_>>();
    while visible_lines.len() < visible_height {
        visible_lines.push(Line::from(" ".repeat(text_width)));
    }
    let text_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width.saturating_sub(1),
        height: inner.height,
    };
    frame.render_widget(Paragraph::new(visible_lines), text_area);

    if visible_height > 0 && rows.len() > visible_height {
        let scrollbar_position =
            compute_scrollbar_position(app.world_tree_scroll, max_scroll, rows.len());
        let mut scrollbar_state = ScrollbarState::new(rows.len())
            .viewport_content_length(visible_height)
            .position(scrollbar_position);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .thumb_style(Style::default().fg(Color::Rgb(167, 139, 250)))
                .track_style(Style::default().fg(Color::Rgb(55, 65, 81))),
            Rect {
                x: inner.x + inner.width.saturating_sub(1),
                y: inner.y,
                width: 1,
                height: inner.height,
            },
            &mut scrollbar_state,
        );
    }
}

fn render_world_map(
    frame: &mut ratatui::Frame<'_>,
    app: &TuiApp,
    world: &WorldMapSnapshot,
    area: ratatui::layout::Rect,
) {
    let footer = Some(Line::from(vec![
        Span::styled(
            format!(
                "({:.0},{:.0}) cursor=({:.0},{:.0}) zoom={:.0}m",
                app.world_center_x,
                app.world_center_y,
                app.world_cursor_x,
                app.world_cursor_y,
                app.world_zoom
            ),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw(" "),
        Span::styled(
            app.world_selected_name
                .as_deref()
                .or(app.world_selected_guid.as_deref())
                .unwrap_or("no selection")
                .to_string(),
            Style::default().fg(Color::Rgb(203, 213, 225)),
        ),
    ]));
    let block = panel_block(
        "map",
        app.focus == PaneFocus::World,
        Color::Rgb(251, 191, 36),
        &[
            ShortcutHint::new("wheel", "Zoom"),
            ShortcutHint::new("drag", "Pan"),
            ShortcutHint::new("click", "Select"),
            ShortcutHint::new("0", "Reset"),
        ],
        footer,
    );
    let inner = inner_rect(area);
    frame.render_widget(block, area);
    let lines = build_world_lines(app, world, inner.width as usize, inner.height as usize);
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_health(
    frame: &mut ratatui::Frame<'_>,
    app: &TuiApp,
    health: &crate::replication::health::ReplicationHealthSnapshot,
    area: ratatui::layout::Rect,
) {
    let lines = vec![
        Line::from(format!("status: {}", health.status)),
        Line::from(format!("uptime_s: {}", health.uptime_seconds)),
        Line::from(format!("entities: {}", health.world_entity_count)),
        Line::from(format!("physics bodies: {}", health.physics_body_count)),
        Line::from(format!("input drops: {}", health.input_drop_total)),
        Line::from(format!(
            "input dup/oOO: {}",
            health.input_duplicate_or_out_of_order_drop_total
        )),
        Line::from(format!(
            "input future/rate: {}/{}",
            health.input_future_tick_drop_total, health.input_rate_limited_drop_total
        )),
        Line::from(format!(
            "input auth/target: {}/{}",
            health.input_spoofed_player_drop_total, health.input_controlled_target_mismatch_total
        )),
        Line::from(format!(
            "input empty/unbound: {}/{}",
            health.input_empty_after_filter_drop_total, health.input_unbound_client_drop_total
        )),
        Line::from(format!(
            "visibility query ms: {:.2}",
            health.visibility_query_ms
        )),
        Line::from(format!(
            "persistence batches: {}",
            health.persistence_enqueued_batches
        )),
        Line::from(format!(
            "lua interval runs: {}",
            health.lua_runtime.interval_runs
        )),
        Line::from(format!("lua event runs: {}", health.lua_runtime.event_runs)),
        Line::from(format!("lua errors: {}", health.lua_runtime.error_count)),
        Line::from(format!(
            "lua mem limit: {}",
            health.lua_runtime.memory_limit_bytes
        )),
        Line::from(""),
        Line::from(format!("tui frame ms: {:.2}", app.tui_last_frame_ms)),
        Line::from(format!("tui log read ms: {:.2}", app.tui_last_log_read_ms)),
        Line::from(format!("tui max frame ms: {:.2}", app.tui_max_frame_ms)),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(panel_block(
            "health",
            app.focus == PaneFocus::Health,
            Color::Rgb(74, 222, 128),
            &[
                ShortcutHint::new("r", "Refresh"),
                ShortcutHint::new("s", "Section"),
                ShortcutHint::new("c", "Copy"),
            ],
            Some(Line::from(vec![Span::styled(
                "summary-only /health",
                Style::default().fg(Color::DarkGray),
            )])),
        )),
        area,
    );
}

fn logs_panel_block(app: &TuiApp) -> Block<'static> {
    let search_label = if app.active_search.is_empty() {
        "search:off".to_string()
    } else {
        format!("search:{}", app.active_search)
    };
    let status = if app.follow_logs { "tail" } else { "scroll" };
    panel_block(
        "logs",
        app.focus == PaneFocus::Logs,
        Color::Rgb(96, 165, 250),
        &[
            ShortcutHint::new(":", "cmd"),
            ShortcutHint::new("/", "search"),
            ShortcutHint::new("f", "filter"),
            ShortcutHint::new("c", "clear"),
            ShortcutHint::new("q", "quit"),
        ],
        Some(Line::from(vec![
            Span::styled(format!("{} ", status), Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("filter:{} ", app.level_filter.label()),
                Style::default().fg(Color::Rgb(156, 163, 175)),
            ),
            Span::styled(search_label, Style::default().fg(Color::Rgb(156, 163, 175))),
            Span::styled(
                format!(
                    " line:{}",
                    app.selected_log_line
                        .map(|value| value.saturating_add(1).to_string())
                        .unwrap_or_else(|| "-".to_string())
                ),
                Style::default().fg(Color::Rgb(156, 163, 175)),
            ),
        ])),
    )
}

fn render_command_bar(frame: &mut ratatui::Frame<'_>, app: &TuiApp, area: ratatui::layout::Rect) {
    let (prefix, value, style) = match app.mode {
        InputMode::Command => (
            ":",
            app.command_input.as_str(),
            Style::default().fg(Color::Rgb(248, 113, 113)),
        ),
        InputMode::Search => (
            "/",
            app.search_input.as_str(),
            Style::default().fg(Color::Rgb(96, 165, 250)),
        ),
        InputMode::Normal => (
            "",
            if app.follow_logs {
                "ready | : command | / search | f filter | c clear | Tab pane | q quit"
            } else {
                "scroll mode | End follow | k/up back | j/down forward"
            },
            Style::default().fg(Color::Rgb(156, 163, 175)),
        ),
    };
    let line = Line::from(vec![
        Span::styled("└─ ", Style::default().fg(Color::Rgb(75, 85, 99))),
        Span::styled(prefix.to_string(), style.add_modifier(Modifier::BOLD)),
        Span::styled(value.to_string(), style),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_dialog(frame: &mut ratatui::Frame<'_>, app: &TuiApp) {
    let Some(dialog) = app.dialog.as_ref() else {
        return;
    };
    let area = centered_rect(frame.area(), 70, 60);
    let dialog_bg = Color::Rgb(15, 23, 42);
    let dialog_fg = Color::Rgb(241, 245, 249);
    let dialog_border = Color::Rgb(148, 163, 184);
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled("┐ ", Style::default().fg(dialog_border).bg(dialog_bg)),
            Span::styled(
                match dialog {
                    TuiDialog::ConfirmReset => "reset confirmation",
                    TuiDialog::ConfirmQuit => "quit confirmation",
                    TuiDialog::Help => "command help",
                },
                Style::default()
                    .fg(dialog_fg)
                    .bg(dialog_bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ", Style::default().bg(dialog_bg)),
            Span::styled("┌", Style::default().fg(dialog_border).bg(dialog_bg)),
        ]))
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(dialog_border))
        .style(Style::default().bg(dialog_bg).fg(dialog_fg));
    let lines = match dialog {
        TuiDialog::ConfirmReset => vec![
            Line::styled(
                "This will disconnect all active players.",
                Style::default().fg(dialog_fg).bg(dialog_bg),
            ),
            Line::styled(
                "The persisted runtime world state will be reset.",
                Style::default().fg(dialog_fg).bg(dialog_bg),
            ),
            Line::styled(
                "world/world_init.lua will be applied again.",
                Style::default().fg(dialog_fg).bg(dialog_bg),
            ),
            Line::styled("", Style::default().bg(dialog_bg)),
            Line::from(vec![
                Span::styled(
                    "Enter",
                    Style::default()
                        .fg(Color::Rgb(125, 211, 252))
                        .bg(dialog_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" confirm  ", Style::default().fg(dialog_fg).bg(dialog_bg)),
                Span::styled(
                    "Esc",
                    Style::default()
                        .fg(Color::Rgb(248, 250, 252))
                        .bg(dialog_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" cancel", Style::default().fg(dialog_fg).bg(dialog_bg)),
            ]),
        ],
        TuiDialog::ConfirmQuit => vec![
            Line::styled(
                "Terminate sidereal-replication?",
                Style::default().fg(dialog_fg).bg(dialog_bg),
            ),
            Line::styled(
                "This will shut down the replication server process.",
                Style::default().fg(dialog_fg).bg(dialog_bg),
            ),
            Line::styled("", Style::default().bg(dialog_bg)),
            Line::from(vec![
                Span::styled(
                    "Enter",
                    Style::default()
                        .fg(Color::Rgb(125, 211, 252))
                        .bg(dialog_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" confirm  ", Style::default().fg(dialog_fg).bg(dialog_bg)),
                Span::styled(
                    "Esc",
                    Style::default()
                        .fg(Color::Rgb(248, 250, 252))
                        .bg(dialog_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" cancel", Style::default().fg(dialog_fg).bg(dialog_bg)),
            ]),
        ],
        TuiDialog::Help => {
            let mut lines = vec![Line::styled(
                "admin command catalog:",
                Style::default()
                    .fg(Color::Rgb(125, 211, 252))
                    .bg(dialog_bg)
                    .add_modifier(Modifier::BOLD),
            )];
            lines.extend(command_specs().iter().map(|spec| {
                let mut text = format!("{} - {}", spec.usage, spec.summary);
                if !spec.parameters.is_empty() {
                    text.push_str(&format!(" | params: {}", spec.parameters));
                }
                if spec.requires_confirmation {
                    text.push_str(" | confirm");
                }
                Line::styled(text, Style::default().fg(dialog_fg).bg(dialog_bg))
            }));
            lines.push(Line::styled("", Style::default().bg(dialog_bg)));
            lines.push(Line::from(vec![
                Span::styled(
                    "Enter",
                    Style::default()
                        .fg(Color::Rgb(125, 211, 252))
                        .bg(dialog_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("/", Style::default().fg(dialog_fg).bg(dialog_bg)),
                Span::styled(
                    "Esc",
                    Style::default()
                        .fg(Color::Rgb(248, 250, 252))
                        .bg(dialog_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" close", Style::default().fg(dialog_fg).bg(dialog_bg)),
            ]));
            lines
        }
    };
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(dialog_bg).fg(dialog_fg)),
        area,
    );
}

fn build_world_lines(
    app: &TuiApp,
    world: &WorldMapSnapshot,
    width: usize,
    height: usize,
) -> Vec<Line<'static>> {
    if width == 0 || height == 0 {
        return vec![];
    }
    let mut cells = vec![vec![(' ', Style::default()); width]; height];
    let viewport_center_x = width / 2;
    let viewport_center_y = height / 2;
    let axis_x =
        viewport_center_x as isize + ((0.0 - app.world_center_x) / app.world_zoom).round() as isize;
    let axis_y = viewport_center_y as isize
        - ((0.0 - app.world_center_y) / (app.world_zoom * WORLD_CELL_ASPECT_Y)).round() as isize;
    let cursor_x = viewport_center_x as isize
        + ((app.world_cursor_x - app.world_center_x) / app.world_zoom).round() as isize;
    let cursor_y = viewport_center_y as isize
        - ((app.world_cursor_y - app.world_center_y) / (app.world_zoom * WORLD_CELL_ASPECT_Y))
            .round() as isize;
    for (y, row) in cells.iter_mut().enumerate() {
        for (x, cell) in row.iter_mut().enumerate() {
            let grid_style = Style::default().fg(Color::Rgb(120, 120, 120));
            let axis_style = Style::default().fg(Color::Rgb(180, 180, 180));
            let xi = x as isize;
            let yi = y as isize;
            let ch = if xi == axis_x && yi == axis_y {
                ('┼', axis_style)
            } else if xi == axis_x {
                ('│', axis_style)
            } else if yi == axis_y {
                ('─', axis_style)
            } else if x % 4 == 0 && y % 2 == 0 {
                ('┼', grid_style)
            } else if x % 4 == 0 {
                ('┆', grid_style)
            } else if y % 2 == 0 {
                ('┄', grid_style)
            } else {
                (' ', Style::default())
            };
            *cell = ch;
        }
    }

    for entity in &world.entities {
        let screen_x = viewport_center_x as isize
            + ((entity.x - f64::from(app.world_center_x)) / f64::from(app.world_zoom)).round()
                as isize;
        let screen_y = viewport_center_y as isize
            - ((entity.y - f64::from(app.world_center_y))
                / f64::from(app.world_zoom * WORLD_CELL_ASPECT_Y))
            .round() as isize;
        if screen_x < 0 || screen_y < 0 || screen_x >= width as isize || screen_y >= height as isize
        {
            continue;
        }
        let selected = app
            .world_selected_guid
            .as_deref()
            .is_some_and(|guid| guid == entity.guid);
        let style = Style::default()
            .fg(Color::Rgb(
                entity.color_rgb.0,
                entity.color_rgb.1,
                entity.color_rgb.2,
            ))
            .add_modifier(if selected {
                Modifier::BOLD | Modifier::REVERSED
            } else {
                Modifier::BOLD
            });
        cells[screen_y as usize][screen_x as usize] = (entity.glyph, style);
    }

    if cursor_x >= 0 && cursor_y >= 0 && cursor_x < width as isize && cursor_y < height as isize {
        let existing = cells[cursor_y as usize][cursor_x as usize].0;
        let cursor_char = if matches!(existing, ' ' | '┄' | '┆' | '─' | '│' | '┼') {
            '✛'
        } else {
            '◎'
        };
        cells[cursor_y as usize][cursor_x as usize] = (
            cursor_char,
            Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .add_modifier(Modifier::BOLD),
        );
    }

    cells
        .into_iter()
        .map(|row| {
            let spans = row
                .into_iter()
                .map(|(ch, style)| Span::styled(ch.to_string(), style))
                .collect::<Vec<_>>();
            Line::from(spans)
        })
        .collect()
}

fn select_world_entity_at(app: &mut TuiApp, world: &WorldMapSnapshot, column: u16, row: u16) {
    let inner = inner_rect(app.world_rect);
    if inner.width == 0 || inner.height == 0 || !contains(inner, (column, row)) {
        return;
    }
    let inner_x = column.saturating_sub(inner.x) as i32;
    let inner_y = row.saturating_sub(inner.y) as i32;
    let center_x = (inner.width / 2) as i32;
    let center_y = (inner.height / 2) as i32;
    let world_x = app.world_center_x + (inner_x - center_x) as f32 * app.world_zoom;
    let world_y =
        app.world_center_y - (inner_y - center_y) as f32 * app.world_zoom * WORLD_CELL_ASPECT_Y;
    app.world_cursor_x = world_x;
    app.world_cursor_y = world_y;
    app.world_selected_guid = None;
    app.world_selected_name = None;

    let mut best: Option<(&crate::replication::health::WorldMapEntitySnapshot, f64)> = None;
    for entity in &world.entities {
        let dx = entity.x - f64::from(world_x);
        let dy = entity.y - f64::from(world_y);
        let distance_sq = dx * dx + dy * dy;
        let threshold = f64::from(app.world_zoom.max(entity.extent_m) * 1.5).powi(2);
        if distance_sq > threshold {
            continue;
        }
        match best {
            Some((_, best_distance_sq)) if distance_sq >= best_distance_sq => {}
            _ => best = Some((entity, distance_sq)),
        }
    }
    if let Some((entity, _)) = best {
        app.world_selected_guid = Some(entity.guid.clone());
        app.world_selected_name = entity.display_name.clone();
        app.world_cursor_x = entity.x as f32;
        app.world_cursor_y = entity.y as f32;
        app.world_tree_selected_key = Some(format!("entity:{}", entity.guid));
        app.world_tree_keep_selected_visible = true;
    }
}

fn select_world_tree_row_at(
    app: &mut TuiApp,
    explorer: &WorldExplorerSnapshot,
    column: u16,
    row: u16,
) {
    let inner = inner_rect(app.sessions_rect);
    if inner.width <= 1 || inner.height == 0 || !contains(inner, (column, row)) {
        return;
    }
    let rows = build_world_tree_rows(explorer, app);
    let row_index = app
        .world_tree_scroll
        .saturating_add(row.saturating_sub(inner.y) as usize);
    let Some(clicked_row) = rows.get(row_index) else {
        return;
    };
    let already_selected = app.world_tree_selected_key.as_deref() == Some(clicked_row.key.as_str());
    app.world_tree_selected_key = Some(clicked_row.key.clone());
    app.world_tree_keep_selected_visible = true;
    sync_map_selection_from_tree_row(app, clicked_row);
    if already_selected && clicked_row.expandable {
        toggle_world_tree_selection(app);
    }
}

fn move_world_tree_selection(app: &mut TuiApp, delta: isize) {
    let max_index = app.world_tree_last_rows.saturating_sub(1);
    let current = world_tree_selected_index(app).unwrap_or(0);
    let next = if delta.is_negative() {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current.saturating_add(delta as usize).min(max_index)
    };
    app.world_tree_selected_key = Some(world_tree_key_from_index(app, next));
    app.world_tree_keep_selected_visible = true;
    sync_map_selection_from_tree(app);
}

fn page_world_tree_selection(app: &mut TuiApp, direction: isize) {
    let page = app.world_tree_visible_rows.saturating_sub(1).max(1);
    move_world_tree_selection(app, direction.saturating_mul(page as isize));
}

fn scroll_world_tree(app: &mut TuiApp, delta: isize) {
    app.world_tree_keep_selected_visible = false;
    if delta.is_negative() {
        app.world_tree_scroll = app.world_tree_scroll.saturating_sub(delta.unsigned_abs());
    } else {
        app.world_tree_scroll = app
            .world_tree_scroll
            .saturating_add(delta as usize)
            .min(app.world_tree_last_rows.saturating_sub(1));
    }
}

fn toggle_world_tree_selection(app: &mut TuiApp) {
    let Some(key) = app.world_tree_selected_key.clone() else {
        return;
    };
    if !app.world_tree_expandable_keys.contains(&key) {
        return;
    }
    if !app.collapsed_world_tree_keys.insert(key.clone()) {
        app.collapsed_world_tree_keys.remove(&key);
    }
}

fn expand_world_tree_selection(app: &mut TuiApp) {
    if let Some(key) = app.world_tree_selected_key.clone() {
        app.collapsed_world_tree_keys.remove(&key);
    }
}

fn collapse_world_tree_selection(app: &mut TuiApp) {
    if let Some(key) = app.world_tree_selected_key.clone()
        && app.world_tree_expandable_keys.contains(&key)
    {
        app.collapsed_world_tree_keys.insert(key);
    }
}

fn expand_all_world_tree(app: &mut TuiApp) {
    app.collapsed_world_tree_keys.clear();
}

fn collapse_all_world_tree(app: &mut TuiApp) {
    app.collapsed_world_tree_keys = app
        .world_tree_expandable_keys
        .iter()
        .filter(|key| key.starts_with("entity:") || key.starts_with("group:"))
        .cloned()
        .collect();
}

fn world_tree_selected_index(app: &TuiApp) -> Option<usize> {
    app.world_tree_selected_key.as_deref().and_then(|key| {
        app.world_tree_last_keys
            .iter()
            .position(|row_key| row_key == key)
    })
}

fn world_tree_key_from_index(app: &TuiApp, index: usize) -> String {
    app.world_tree_last_keys
        .get(index)
        .cloned()
        .unwrap_or_else(|| "group:world".to_string())
}

fn sync_map_selection_from_tree(app: &mut TuiApp) {
    let Some(selected_key) = app.world_tree_selected_key.as_deref() else {
        return;
    };
    let selected_row = app
        .world_tree_last_rows_data
        .iter()
        .find(|row| row.key == selected_key)
        .cloned();
    if let Some(row) = selected_row.as_ref() {
        sync_map_selection_from_tree_row(app, row);
    }
}

fn sync_map_selection_from_tree_row(app: &mut TuiApp, row: &WorldTreeRow) {
    if let Some(guid) = row.entity_guid.as_ref() {
        app.world_selected_guid = Some(guid.clone());
        app.world_selected_name = row.entity_display_name.clone();
        if let Some((x, y)) = row.entity_position_xy {
            app.world_cursor_x = x as f32;
            app.world_cursor_y = y as f32;
        }
    }
}

fn goto_selected_world_entity(app: &mut TuiApp) {
    let Some(selected_key) = app.world_tree_selected_key.as_deref() else {
        return;
    };
    let Some(row) = app
        .world_tree_last_rows_data
        .iter()
        .find(|row| row.key == selected_key)
        .cloned()
    else {
        return;
    };
    if row.entity_guid.is_none() {
        return;
    }
    if app.world_selected_guid != row.entity_guid {
        sync_map_selection_from_tree_row(app, &row);
    }
    if let Some((x, y)) = row.entity_position_xy {
        app.world_center_x = x as f32;
        app.world_center_y = y as f32;
        app.world_cursor_x = x as f32;
        app.world_cursor_y = y as f32;
    }
    app.focus = PaneFocus::World;
    app.world_zoom = app.world_zoom.min(250.0);
}

fn inner_rect(rect: Rect) -> Rect {
    Rect {
        x: rect.x.saturating_add(1),
        y: rect.y.saturating_add(1),
        width: rect.width.saturating_sub(2),
        height: rect.height.saturating_sub(2),
    }
}

fn panel_block(
    title: &str,
    focused: bool,
    accent: Color,
    shortcuts: &[ShortcutHint<'_>],
    footer: Option<Line<'static>>,
) -> Block<'static> {
    let border_style = if focused {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Rgb(75, 85, 99))
    };
    let mut block = Block::default()
        .title(build_title(title, accent, border_style, shortcuts))
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(border_style);
    if let Some(footer) = footer {
        block = block.title_bottom(footer);
    }
    block
}

fn build_title(
    title: &str,
    accent: Color,
    border_style: Style,
    shortcuts: &[ShortcutHint<'_>],
) -> Line<'static> {
    let mut spans = vec![
        Span::styled("┐ ", border_style),
        Span::styled(
            title.to_string(),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
    ];
    for hint in shortcuts {
        spans.push(Span::raw(" "));
        spans.push(Span::styled("┌┐ ", border_style));
        spans.extend(build_shortcut_spans(hint));
    }
    spans.push(Span::raw(" "));
    spans.push(Span::styled("┌", border_style));
    Line::from(spans)
}

fn filtered_logs(app: &TuiApp, logs: &[String]) -> Vec<String> {
    logs.iter()
        .skip(app.clear_from)
        .filter(|line| app.level_filter.matches(line))
        .filter(|line| {
            if app.active_search.is_empty() {
                true
            } else {
                line.to_ascii_lowercase()
                    .contains(&app.active_search.to_ascii_lowercase())
            }
        })
        .cloned()
        .collect()
}

fn move_log_selection(app: &mut TuiApp, delta: isize) {
    app.follow_logs = false;
    app.keep_selected_log_visible = true;
    match (app.selected_log_line, delta.cmp(&0)) {
        (Some(selected), std::cmp::Ordering::Less) => {
            app.selected_log_line = Some(selected.saturating_sub(delta.unsigned_abs()));
        }
        (Some(selected), std::cmp::Ordering::Greater) => {
            app.selected_log_line = Some(selected.saturating_add(delta as usize));
        }
        (None, std::cmp::Ordering::Less) => app.selected_log_line = Some(0),
        (None, std::cmp::Ordering::Greater) => app.selected_log_line = Some(delta as usize),
        _ => {}
    }
}

fn page_log_selection(app: &mut TuiApp, direction: isize) {
    let page = app.log_visible_rows.saturating_sub(1).max(1);
    move_log_selection(app, direction.saturating_mul(page as isize));
}

fn scroll_logs_up(app: &mut TuiApp, amount: usize) {
    let base = if app.follow_logs {
        app.max_log_scroll
    } else {
        app.log_scroll
    };
    app.follow_logs = false;
    app.keep_selected_log_visible = false;
    app.log_scroll = base.saturating_sub(amount);
}

fn scroll_logs_down(app: &mut TuiApp, amount: usize) {
    let base = if app.follow_logs {
        app.max_log_scroll
    } else {
        app.log_scroll
    };
    app.follow_logs = false;
    app.keep_selected_log_visible = false;
    app.log_scroll = base.saturating_add(amount).min(app.max_log_scroll);
}

fn request_clear_logs(app: &mut TuiApp) {
    app.pending_clear_logs = true;
}

fn apply_filter_command(app: &mut TuiApp, level: &str) {
    app.level_filter = match level.to_ascii_lowercase().as_str() {
        "info" => LogLevelFilter::Info,
        "warn" => LogLevelFilter::Warn,
        "error" => LogLevelFilter::Error,
        _ => LogLevelFilter::All,
    };
}

fn jump_to_latest_log(app: &mut TuiApp) {
    app.follow_logs = true;
    app.keep_selected_log_visible = false;
    app.log_scroll = app.max_log_scroll;
}

fn resolve_selected_log_line(app: &mut TuiApp, filtered_len: usize) -> Option<usize> {
    if filtered_len == 0 {
        app.selected_log_line = None;
        return None;
    }
    if app.follow_logs {
        let latest = filtered_len.saturating_sub(1);
        app.selected_log_line = Some(latest);
        app.keep_selected_log_visible = false;
        return Some(latest);
    }
    let selected = app
        .selected_log_line
        .unwrap_or_else(|| filtered_len.saturating_sub(1))
        .min(filtered_len.saturating_sub(1));
    app.selected_log_line = Some(selected);
    Some(selected)
}

fn compute_log_scroll(
    app: &mut TuiApp,
    wrapped_rows: &[WrappedLogRow],
    visible_height: usize,
    selected_log_line: Option<usize>,
) -> usize {
    let max_scroll = wrapped_rows.len().saturating_sub(visible_height);
    if app.follow_logs {
        app.log_scroll = max_scroll;
        return max_scroll;
    }

    let mut scroll = app.log_scroll.min(max_scroll);
    if scroll == max_scroll {
        app.log_scroll = max_scroll;
        return max_scroll;
    }
    if app.keep_selected_log_visible
        && let Some(selected) = selected_log_line
    {
        let selected_start = wrapped_rows
            .iter()
            .position(|row| row.line_index == selected)
            .unwrap_or(0);
        let selected_end = wrapped_rows
            .iter()
            .rposition(|row| row.line_index == selected)
            .unwrap_or(selected_start);
        if selected_start < scroll {
            scroll = selected_start;
        } else if selected_end >= scroll.saturating_add(visible_height) && visible_height > 0 {
            scroll = selected_end
                .saturating_add(1)
                .saturating_sub(visible_height);
        }
    }
    app.log_scroll = scroll.min(max_scroll);
    app.log_scroll
}

#[derive(Debug, Clone)]
struct WrappedLogRow {
    line_index: usize,
    cells: Vec<StyledCell>,
}

#[derive(Debug, Clone)]
struct StyledCell {
    ch: char,
    style: Style,
}

fn build_wrapped_log_rows(lines: &[String], width: usize) -> Vec<WrappedLogRow> {
    if width == 0 {
        return vec![];
    }
    let mut rows = Vec::new();
    for (line_index, line) in lines.iter().enumerate() {
        let cells = build_log_line_cells(line);
        if cells.is_empty() {
            rows.push(WrappedLogRow {
                line_index,
                cells: vec![
                    StyledCell {
                        ch: ' ',
                        style: Style::default(),
                    };
                    width
                ],
            });
            continue;
        }
        for chunk in cells.chunks(width) {
            let mut row_cells = chunk.to_vec();
            while row_cells.len() < width {
                row_cells.push(StyledCell {
                    ch: ' ',
                    style: Style::default(),
                });
            }
            rows.push(WrappedLogRow {
                line_index,
                cells: row_cells,
            });
        }
    }
    if rows.is_empty() {
        rows.push(WrappedLogRow {
            line_index: 0,
            cells: vec![
                StyledCell {
                    ch: ' ',
                    style: Style::default(),
                };
                width
            ],
        });
    }
    rows
}

fn sanitize_log_text(line: &str) -> String {
    strip_ansi_sequences(&line.replace('\r', "").replace('\t', "    "))
}

fn build_log_line_cells(line: &str) -> Vec<StyledCell> {
    let sanitized = sanitize_log_text(line);
    let parsed = parse_log_line(&sanitized);
    let mut cells = Vec::new();
    for (text, style) in parsed {
        cells.extend(text.chars().map(|ch| StyledCell { ch, style }));
    }
    cells
}

fn parse_log_line(line: &str) -> Vec<(String, Style)> {
    let Some((timestamp_token, rest)) = split_leading_token(line) else {
        return vec![(line.to_string(), message_style())];
    };
    let Some((level_token, remainder)) = split_leading_token(rest.trim_start()) else {
        return vec![
            (format_timestamp(timestamp_token), timestamp_style()),
            (" ".to_string(), Style::default()),
            (rest.trim_start().to_string(), message_style()),
        ];
    };
    let remainder = remainder.trim_start();
    let (target, message) = remainder
        .split_once(": ")
        .map(|(target, message)| (target.trim(), message))
        .unwrap_or((remainder, ""));

    let mut segments = vec![
        (format_timestamp(timestamp_token), timestamp_style()),
        (" ".to_string(), Style::default()),
        (format!("{:<5}", level_token), level_style(level_token)),
        (" ".to_string(), Style::default()),
    ];
    if !target.is_empty() {
        segments.push((target.to_string(), target_style()));
    }
    if !message.is_empty() {
        if !target.is_empty() {
            segments.push((": ".to_string(), target_style()));
        }
        segments.push((message.to_string(), message_style()));
    }
    segments
}

fn split_leading_token(input: &str) -> Option<(&str, &str)> {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.find(char::is_whitespace) {
        Some(index) => Some((&trimmed[..index], &trimmed[index..])),
        None => Some((trimmed, "")),
    }
}

fn format_timestamp(timestamp_token: &str) -> String {
    const FALLBACK_WIDTH: usize = 23;
    match DateTime::parse_from_rfc3339(timestamp_token) {
        Ok(parsed) => parsed
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S%.3f")
            .to_string(),
        Err(_) => {
            let mut text = timestamp_token
                .chars()
                .take(FALLBACK_WIDTH)
                .collect::<String>();
            while text.chars().count() < FALLBACK_WIDTH {
                text.push(' ');
            }
            text
        }
    }
}

fn timestamp_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn level_style(level: &str) -> Style {
    match level {
        "ERROR" => Style::default()
            .fg(Color::Rgb(248, 113, 113))
            .add_modifier(Modifier::BOLD),
        "WARN" => Style::default()
            .fg(Color::Rgb(253, 224, 71))
            .add_modifier(Modifier::BOLD),
        "INFO" => Style::default()
            .fg(Color::Rgb(134, 239, 172))
            .add_modifier(Modifier::BOLD),
        "DEBUG" => Style::default()
            .fg(Color::Rgb(125, 211, 252))
            .add_modifier(Modifier::BOLD),
        _ => Style::default()
            .fg(Color::Rgb(203, 213, 225))
            .add_modifier(Modifier::BOLD),
    }
}

fn target_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn message_style() -> Style {
    Style::default().fg(Color::Rgb(248, 250, 252))
}

fn strip_ansi_sequences(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            result.push(ch);
            continue;
        }
        if chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        }
    }
    result
}

fn styled_cells_to_line(cells: &[StyledCell], selected: bool) -> Line<'static> {
    let mut spans = Vec::new();
    let mut current_text = String::new();
    let mut current_style: Option<Style> = None;
    for cell in cells {
        let style = if selected {
            cell.style.bg(Color::Rgb(30, 41, 59))
        } else {
            cell.style
        };
        match current_style {
            Some(existing) if existing == style => current_text.push(cell.ch),
            Some(existing) => {
                spans.push(Span::styled(current_text.clone(), existing));
                current_text.clear();
                current_text.push(cell.ch);
                current_style = Some(style);
            }
            None => {
                current_text.push(cell.ch);
                current_style = Some(style);
            }
        }
    }
    if let Some(style) = current_style {
        spans.push(Span::styled(current_text, style));
    }
    Line::from(spans)
}

fn centered_rect(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(vertical[1])[1]
}

fn build_shortcut_spans(hint: &ShortcutHint<'_>) -> Vec<Span<'static>> {
    if hint.key.chars().count() == 1 {
        let key = hint.key.chars().next().unwrap_or_default();
        if let Some(byte_index) = hint
            .label
            .to_ascii_lowercase()
            .find(key.to_ascii_lowercase())
        {
            let end = byte_index + key.len_utf8();
            let before = &hint.label[..byte_index];
            let highlighted = &hint.label[byte_index..end];
            let after = &hint.label[end..];
            let mut spans = Vec::new();
            if !before.is_empty() {
                spans.push(Span::styled(
                    before.to_string(),
                    Style::default().fg(Color::Rgb(203, 213, 225)),
                ));
            }
            spans.push(Span::styled(
                highlighted.to_string(),
                Style::default()
                    .fg(Color::Rgb(248, 113, 113))
                    .add_modifier(Modifier::BOLD),
            ));
            if !after.is_empty() {
                spans.push(Span::styled(
                    after.to_string(),
                    Style::default().fg(Color::Rgb(203, 213, 225)),
                ));
            }
            return spans;
        }
    }

    vec![
        Span::styled(
            hint.key.to_string(),
            Style::default()
                .fg(Color::Rgb(248, 113, 113))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            hint.label.to_string(),
            Style::default().fg(Color::Rgb(203, 213, 225)),
        ),
    ]
}

fn select_log_line_at(app: &mut TuiApp, logs: &[String], column: u16, row: u16) {
    let inner = inner_rect(app.logs_rect);
    if inner.width <= 1 || inner.height == 0 || !contains(inner, (column, row)) {
        return;
    }

    let filtered = filtered_logs(app, logs);
    let wrapped_rows = build_wrapped_log_rows(&filtered, inner.width.saturating_sub(1) as usize);
    if wrapped_rows.is_empty() {
        app.selected_log_line = None;
        return;
    }

    let visible_height = inner.height as usize;
    let max_scroll = wrapped_rows.len().saturating_sub(visible_height);
    let scroll = if app.follow_logs {
        max_scroll
    } else {
        app.log_scroll.min(max_scroll)
    };
    let row_index = row.saturating_sub(inner.y) as usize;
    if let Some(wrapped) = wrapped_rows.get(scroll + row_index) {
        app.selected_log_line = Some(wrapped.line_index);
        app.follow_logs = false;
        app.keep_selected_log_visible = true;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InputMode, LogLevelFilter, PaneFocus, Rect, TuiApp, filtered_logs, pane_at_position,
    };

    #[test]
    fn filtered_logs_respects_level_and_search() {
        let app = TuiApp {
            mode: InputMode::Normal,
            level_filter: LogLevelFilter::Warn,
            active_search: "disk".to_string(),
            ..Default::default()
        };
        let logs = vec![
            "2026-03-10 INFO boot ok".to_string(),
            "2026-03-10 WARN disk pressure".to_string(),
            "2026-03-10 ERROR socket".to_string(),
        ];

        let filtered = filtered_logs(&app, &logs);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], "2026-03-10 WARN disk pressure");
    }

    #[test]
    fn pane_selection_maps_click_regions() {
        let app = TuiApp {
            logs_rect: Rect::new(0, 0, 100, 10),
            sessions_rect: Rect::new(0, 10, 33, 20),
            world_rect: Rect::new(33, 10, 34, 20),
            health_rect: Rect::new(67, 10, 33, 20),
            ..Default::default()
        };
        assert_eq!(pane_at_position(&app, 5, 2), Some(PaneFocus::Logs));
        assert_eq!(pane_at_position(&app, 5, 15), Some(PaneFocus::Sessions));
        assert_eq!(pane_at_position(&app, 40, 15), Some(PaneFocus::World));
        assert_eq!(pane_at_position(&app, 80, 15), Some(PaneFocus::Health));
    }
}
