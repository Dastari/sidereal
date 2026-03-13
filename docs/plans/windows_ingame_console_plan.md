# Windows Native Subsystem + In-Game Console Plan

Status: Proposed implementation plan  
Date: 2026-03-05  
Owners: client/runtime team

Primary references:
- `AGENTS.md`
- `docs/ui_design_guide.md`
- `bins/sidereal-client/src/main.rs`
- `bins/sidereal-client/src/runtime/mod.rs`
- `bins/sidereal-client/src/runtime/plugins.rs`

## 1. Objective

Add a native Windows app behavior and in-game drop-down console that meets these requirements:

1. Native Windows client launches with `#![windows_subsystem = "windows"]`.
2. Pressing tilde/backquote opens a smooth drop-down console panel covering half the screen.
3. Tracing logs are routed into this console and include startup logs emitted before world rendering is ready.
4. Console includes a bottom input line with cursor.
5. Entered commands are accepted and always respond `not implemented yet`.

## 2. Non-Goals

1. No gameplay/admin command execution yet.
2. No remote shell or scripting bridge.
3. No dependency on an external terminal window.

## 3. Constraints and Compatibility

1. Use `cfg(target_os = "windows")` for the subsystem attribute.
2. Keep WASM build behavior unchanged.
3. Preserve existing logging behavior for files/CI tooling where currently used.
4. Do not block startup on UI readiness.

## 4. Recommended Architecture

## 4.1 Entry Attribute

In `bins/sidereal-client/src/main.rs`, add:

```rust
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
```

Recommendation:
1. Keep this always-on for Windows release and dev to match your desired behavior.
2. If interactive terminal debugging is needed later, gate with an env-controlled compile cfg in developer builds, not runtime branching.

## 4.2 Log Capture Pipeline (Startup-Safe)

Implement a custom tracing sink that captures events from process startup onward.

Core pieces:
1. `StartupLogBuffer` shared store (bounded ring buffer, for example 10k lines).
2. `InGameConsoleTracingLayer` (tracing subscriber layer) that formats each event to a compact line and pushes into `StartupLogBuffer`.
3. `InGameConsoleLogState` Bevy resource that references the same shared ring and tracks UI cursor/scroll/filter state.

Data model recommendation:

```rust
struct ConsoleLogLine {
    ts_utc: String,
    level: tracing::Level,
    target: String,
    message: String,
    thread: Option<String>,
}
```

Requirements:
1. Buffer must be initialized before `App::new()` systems start emitting logs.
2. Buffer must be thread-safe (`Arc<Mutex<VecDeque<_>>>` or lock-free queue).
3. Max size must be bounded (drop oldest on overflow).

## 4.3 Bevy/Tracing Integration Strategy

Preferred strategy (cleanest):
1. Install a single custom `tracing_subscriber` pipeline early in `main` for native target.
2. Include both:
- existing stdout/stderr/file behavior (if desired), and
- in-game layer writing to `StartupLogBuffer`.
3. Configure Bevy logging plugin so it does not replace the global subscriber unexpectedly.

Fallback strategy:
1. If Bevy 0.18 plugin lifecycle conflicts with global subscriber setup, configure through `LogPlugin` customization hooks to add the in-game layer there.

Implementation note:
1. Keep formatting stable and cheap (no heavy allocations per frame).
2. Avoid recursive logging from console rendering code.

## 4.4 In-Game Console UI

Create a dedicated module, recommended path:
- `bins/sidereal-client/src/runtime/dev_console.rs`

Register in `native/plugins.rs` under `ClientUiPlugin`.

Suggested ECS resources/components:
1. `DevConsoleState`
- `is_open: bool`
- `anim_t: f32` (0..1)
- `target_t: f32`
- `input: String`
- `cursor_index: usize`
- `history: Vec<String>`
- `history_index: Option<usize>`
- `autoscroll: bool`
2. `DevConsoleUiRoot` marker
3. `DevConsoleLogState` (mirrors shared log buffer)

UI layout:
1. Fullscreen overlay root (`absolute`).
2. Console panel anchored top, height = `screen_height * 0.5 * eased(anim_t)`.
3. Scrollable log region above input row.
4. Input row pinned at panel bottom with caret.

Color/typography:
1. Reuse `docs/ui_design_guide.md` dark panel palette.
2. Level coloring:
- `ERROR`: red tint
- `WARN`: amber tint
- `INFO`: blue/neutral
- `DEBUG/TRACE`: muted gray-blue

## 4.5 Animation and Input Behavior

Toggle key:
1. Bind backquote/tilde key (platform keycode for `). 
2. Add debounce on key press edge.

Animation:
1. Use smooth eased interpolation per frame (`anim_t -> target_t`) with configurable speed.
2. Recommended open/close duration: 160-220 ms.

Input behavior:
1. Focus input when console opens.
2. Enter submits current input.
3. Escape closes console.
4. Up/Down navigates command history.
5. Backspace/Delete/Left/Right editing support (minimum viable line editor).

## 4.6 Command Stub Contract

Command dispatcher shape:

```rust
fn run_console_command(cmd: &str) -> String {
    format!("not implemented yet: {}", cmd.trim())
}
```

Behavior:
1. Empty input does nothing.
2. Non-empty input appends a local echo line (`> command`) and response line.
3. All responses are deterministic stub responses for now.

## 4.7 Startup Log Replay Requirement

At UI creation time:
1. Drain or snapshot `StartupLogBuffer` into `DevConsoleLogState` immediately.
2. Continue tailing new logs each frame/tick.
3. Ensure no logs are lost between startup and first render.

Recommendation:
1. Keep single shared backing buffer and maintain read index in UI state.
2. Do not duplicate full log arrays each frame.

## 5. Detailed Implementation Steps

1. `main.rs`
- add `cfg_attr(target_os = "windows", windows_subsystem = "windows")` crate attribute.

2. New module `native/dev_console.rs`
- define resources, markers, log line model, and systems:
  - `init_dev_console_resources_system`
  - `toggle_dev_console_system`
  - `animate_dev_console_panel_system`
  - `sync_dev_console_log_lines_system`
  - `render_dev_console_text_system`
  - `handle_dev_console_input_system`
  - `submit_dev_console_command_system`

3. Logging integration
- add startup initialization function in native startup path before Bevy systems emit most logs.
- install tracing layer forwarding into shared ring buffer.

4. `native/plugins.rs`
- register console setup/update systems in `ClientUiPlugin`.
- schedule console input before gameplay hotkeys to avoid double-handling while open.

5. `native/mod.rs`
- initialize and insert shared console log resource during app wiring.
- ensure headless mode bypasses UI systems.

6. Optional polish
- add filter toggles (level/target prefix).
- add clear command (`clear`) while still returning `not implemented yet` for all other commands.

## 6. Ordering and Safety Recommendations

1. Implement logging capture first.
2. Add console UI shell and animation second.
3. Add input line editing and command stub last.
4. Validate no impact on existing auth/menu/world-loading transitions.

Risk controls:
1. Bounded memory for logs.
2. Guard against despawned UI entities during state transitions.
3. No panics on malformed UTF-8 or oversized command lines.

## 7. Test Plan

Unit tests (new `dev_console` tests module):
1. ring buffer bounded behavior (drops oldest).
2. command stub returns required response format.
3. animation interpolation converges and clamps.

Integration tests (client-side where practical):
1. startup logs exist in console after first frame.
2. tilde toggles open/closed state.
3. pressing Enter appends `not implemented yet` response.
4. console open state suppresses conflicting gameplay keybind handling.

Manual checks:
1. native Windows build launches without terminal window.
2. console opens smoothly to half-screen.
3. logs keep flowing while open and closed.
4. logout/state transitions do not leave orphan UI.

## 8. WASM and Cross-Target Impact

1. `windows_subsystem` attribute is Windows-only and no-op elsewhere.
2. Console UI systems can remain cross-target compile-safe.
3. If tracing sink uses native-only APIs, gate that part with `cfg(not(target_arch = "wasm32"))` and keep compile-safe no-op on wasm.

Expected impact:
1. Native Windows: functional change.
2. Linux/macOS native: no subsystem attribute effect; console UI can still work if enabled.
3. WASM: no required behavior change unless console UI is intentionally enabled there.

## 9. Quality Gates for Implementation PR

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu
cargo check -p sidereal-client --target x86_64-pc-windows-gnu
```

## 10. Recommended Follow-Up Decisions

1. Decide whether console command parser should become a shared protocol surface (local-only vs gateway-routed).
2. Decide retention/export policy for in-game logs (session-only ring vs persisted file).
3. Decide whether console should be available outside `InWorld` state (auth/menu diagnostics).
