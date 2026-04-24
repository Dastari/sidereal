# Replication Server TUI Backend Plan

Status: Active partial implementation plan  
Date: 2026-03-10  
Owners: replication runtime + diagnostics + tooling

Primary references:
- `AGENTS.md`
- `docs/sidereal_design_document.md`
- `docs/features/brp_debugging_workflow.md`
- `docs/features/visibility_replication_contract.md`
- `bins/sidereal-replication/src/main.rs`
- `bins/sidereal-replication/src/plugins.rs`
- Informational style reference: `https://github.com/Dastari/librarian/tree/main/backend/src/tui`

Update note (2026-03-10):
- This plan defines the target runtime shape for a built-in replication-server TUI with a `--headless` bypass, a non-blocking diagnostics pipeline, and a replication-local `/health` endpoint backed by the same health snapshot model as the TUI stats panel.
- The external Librarian TUI is a style/layout reference only. Sidereal remains responsible for its own runtime contracts, performance isolation, and security defaults.

Update note (2026-03-10, follow-up):
- `/health` is intentionally a summary health surface only. It should expose operational counts and subsystem status, but not player/entity/user-identifying detail beyond coarse aggregate values such as users online.
- The server command path should be implemented as a reusable admin command bus rather than a TUI-only local command handler so future dashboard/dev-tool surfaces can reuse the same execution layer.
- The plan should preserve flexibility for future dashboard transport decisions. No dashboard integration work is part of this plan, and the command/diagnostics contracts must not assume either direct-to-replication or gateway-routed access yet.
- Health/statistics coverage should include Lua runtime telemetry where available, including script processing cost, memory consumption, and parser/runtime diagnostics that can be surfaced safely.

Update note (2026-03-10, implementation progress):
- Initial implementation work has started in `sidereal-replication`.
- Landed foundation pieces:
  - CLI parsing for `--headless` / `--health-bind`,
  - shared bounded log fanout buffer wired into the replication tracing formatter,
  - reusable admin command bus skeleton with parser/executor stubs,
  - replication-local loopback `/health` endpoint backed by a shared summary snapshot,
  - initial summary metrics for sessions/users online, entity counts, physics counts, visibility/input/persistence stats, and Lua runtime counters,
  - a separate `ratatui`/`crossterm` terminal thread that starts on interactive terminals,
  - the initial 33/66 shell layout with rounded panels and title-bar command hints,
  - a functional logs pane with command entry, search, clear, follow/scroll behavior, and level filtering,
  - a live health pane backed by the shared summary snapshot,
  - a first world-pane implementation backed by authoritative replication world-space snapshots,
  - mouse pane selection plus world-pane mouse zoom/pan/select behavior,
  - scaffolded Sessions pane placeholder for the next slice,
  - unit tests for the new log buffer, admin command parsing, health snapshot serialization, and log filtering.
- Not landed yet:
  - session tree view,
  - full Vim-style pane-local navigation parity,
  - rich health-pane sectioning/collapse behavior,
  - bottom-pane command implementations beyond the current scaffold,
  - richer world-map overlays and selection detail views.

Update note (2026-03-10, world pane follow-up):
- The world pane projection now includes approximate terminal-cell aspect compensation so the game space reads less vertically compressed in monospace terminals.
- The solid crosshair axes are aligned to world `0,0`, independent of the viewport center.
- Cursor state is distinct from camera center and should persist at the last operator-selected world coordinate, including empty-space selections.

Update note (2026-03-10, TUI shell follow-up):
- `q` now routes through the reusable admin command bus and terminates the replication process rather than only dropping the alternate-screen UI.
- The logs pane now uses explicit viewport row rendering with a viewport-aware scrollbar model instead of block-scrolled paragraph rendering, so thumb size and position track total history versus visible rows more accurately.
- The health pane now includes local TUI overhead telemetry for log-read cost and frame render cost to help confirm that terminal rendering stays operationally cheap.

Update note (2026-03-10, logs pane follow-up):
- The live TUI log source now comes from the shared in-memory finalized log buffer rather than tailing the persisted log file, aligning more closely with the reference Librarian architecture and avoiding concurrent file-read display artifacts.
- Wrapped-row rendering now tracks logical source lines so operators can click to select and highlight an individual log line even when that line spans multiple terminal rows.
- The log sanitization path now strips ANSI escape sequences before wrapping/rendering so any future colored formatter output does not corrupt TUI row width calculations.

Update note (2026-03-11, command bus follow-up):
- The admin command path now has an indexed command catalog with usage/parameter metadata so TUI help output and future dashboard/dev-tool command documentation can be generated from one source.
- `:clear` is handled as a TUI-local console command that clears the visible log history from the current point without touching persisted log files.
- `:reset` now routes through the reusable admin command bus, disconnects active clients, resets persisted runtime world state/bootstrap markers, and reapplies `world/world_init.lua`; the TUI requires confirmation unless `:reset force` is used.

## 1. Objective

Add a native terminal UI for `sidereal-replication` that starts by default on interactive terminals, preserves current server-authoritative runtime behavior, and exposes live operational data without materially impacting simulation cadence.

Required operator experience:

1. Top 33% of the screen shows tracing logs in the same formatted form the process would normally emit to the console.
2. The log panel includes a bottom input line for server commands; command echoes and results are appended to the log stream.
3. The lower 66% is split into three vertical panels:
   - left: connected sessions/characters, ping, and expandable entity trees,
   - middle: ASCII/extended-ASCII world map with selection, zoom, and pan,
   - right: live server health and subsystem metrics.
4. Vim-style navigation, `/` search, clear/filter actions, resize-aware layout, and title-bar command legends are first-class behavior.
5. TUI runs on startup unless `--headless` is passed or no interactive TTY is available.
6. Server health data shown in the UI is also published via replication-local HTTP `/health`.

## 2. Non-Goals

1. No gameplay-authoritative mutation from the TUI beyond explicit admin/server commands already allowed by the replication runtime.
2. No alternate authoritative state path outside Bevy ECS/resources.
3. No replacement of BRP for deep inspection; BRP remains the richer debugging surface.
4. No requirement to support WASM or dashboard rendering from this TUI codepath.
5. No promise that the first map iteration is a pixel-accurate renderer; the initial map is an operational spatial view.

## 3. Hard Constraints

1. TUI work must never become a writer of authoritative world transforms or gameplay state.
2. TUI rendering/input must not run on the fixed simulation schedule.
3. Tracing output shown in the TUI must come from the same formatter pipeline as stderr/file logging so operators are not comparing divergent views.
4. Diagnostics snapshots must be read-only and bounded in cost.
5. `/health` must default to loopback-only bind unless/until a separately documented authenticated exposure policy exists.
6. `/health` must remain summary-only and must not expose player/session/entity-identifying data beyond coarse aggregate counts.
7. `--headless` must preserve current non-TUI runtime behavior and log/file output.
8. If stdout/stderr is not a TTY, replication should auto-fallback to headless behavior unless explicitly forced into TUI mode later by a separate follow-up decision.

## 4. Operator UX Target

## 4.1 Layout

Use `ratatui` with rounded box styling and title-bar command hints in the same general style direction as the referenced Librarian backend:

1. Top row: 33% height.
   - single full-width `Logs` pane,
   - scrollable log body,
   - pinned input row at pane bottom.
2. Bottom row: 66% height, split into three equal-ish vertical panes.
   - `Sessions`
   - `World`
   - `Health`

All panes must resize proportionally with terminal size changes and degrade gracefully when the terminal is too small.

## 4.2 Navigation Model

Global navigation:

1. `h` / `j` / `k` / `l` move focus and selection in Vim style.
2. `tab` / `shift-tab` cycle panes.
3. `/` opens search mode for the focused pane.
4. `n` / `N` move through search results.
5. `g` / `G` jump top/bottom in list-like panes.
6. `q` quits the TUI view only if quitting the process is explicitly confirmed; otherwise it should behave as a normal pane command or require `:quit`.

Pane-local navigation:

1. Logs: scroll, search, level filters, clear, follow/tail toggle.
2. Sessions: expand/collapse tree nodes, jump between clients, select controlled entity/children.
3. World: pan, zoom, center on selection, cycle render layers/overlays.
4. Health: scroll sections, collapse verbose groups, refresh rate toggle if needed.

## 4.3 Commands in Title Bars

Each pane title bar should draw its active command legend, following the Librarian-style pattern rather than relying on a separate help footer.

Initial legend set:

1. Logs: `[/ Search] [f Filter] [c Clear] [End Follow] [: Cmd]`
2. Sessions: `[/ Search] [Enter Expand] [o Focus World] [p Ping Sort]`
3. World: `[/ Search] [+/- Zoom] [HJKL Pan] [Enter Inspect] [0 Reset]`
4. Health: `[/ Search] [r Refresh] [s Section] [c Copy Key]`

## 5. Runtime Architecture

## 5.1 Module Layout

Create a dedicated replication TUI module tree, intentionally mirroring the successful separation style from the reference project:

1. `bins/sidereal-replication/src/tui/mod.rs`
2. `bins/sidereal-replication/src/tui/app.rs`
3. `bins/sidereal-replication/src/tui/events.rs`
4. `bins/sidereal-replication/src/tui/theme.rs`
5. `bins/sidereal-replication/src/tui/commands.rs`
6. `bins/sidereal-replication/src/tui/panels/logs.rs`
7. `bins/sidereal-replication/src/tui/panels/sessions.rs`
8. `bins/sidereal-replication/src/tui/panels/world.rs`
9. `bins/sidereal-replication/src/tui/panels/health.rs`
10. `bins/sidereal-replication/src/tui/runtime_snapshot.rs`

Recommended split of responsibility:

1. Bevy world owns authoritative diagnostics resources and command ingress.
2. A separate TUI thread owns terminal rendering, keyboard handling, and pane-local UI state.
3. Bounded channels bridge:
   - diagnostics snapshots from Bevy -> TUI,
   - operator commands from TUI -> Bevy.

## 5.2 Startup/CLI Shape

Add a small argument parser to `sidereal-replication` with at minimum:

1. `--headless`
2. optional future-friendly `--health-bind`
3. optional future-friendly `--no-tui` alias to `--headless` only if needed for clarity

Startup rules:

1. Default behavior on an interactive terminal: launch TUI.
2. `--headless`: skip TUI thread entirely.
3. Non-interactive terminal / CI / service manager without TTY: log the reason and continue headless.
4. Any TUI startup failure after simulation bootstrap should degrade to headless mode rather than terminate the replication server.

## 5.3 Performance Isolation

TUI rendering must be isolated from the main game loop:

1. no terminal drawing on `FixedUpdate`,
2. no blocking terminal reads on Bevy schedules,
3. no world scans performed directly by the TUI thread,
4. snapshot generation runs on `Update` or a lower-rate timer resource,
5. expensive metrics are cached and sampled on cadence classes suited to the subsystem.

Initial target cadences:

1. log stream ingest: event-driven,
2. sessions list snapshot: 4-10 Hz,
3. world map snapshot: 2-5 Hz,
4. health snapshot: 1-4 Hz,
5. heavy OS/process sampling: 1 Hz unless proven cheap.

## 6. Logging and Command Console Plan

## 6.1 Exact Console-Format Log Feed

Current replication logging writes through `LogPlugin` using `replication_fmt_layer()` to stderr and the timestamped log file.

Implementation direction:

1. replace the current single-destination fmt setup with a shared formatting sink that fans out to:
   - stderr,
   - timestamped log file,
   - in-memory bounded TUI log ring.
2. keep one canonical formatting path so the TUI log line text matches console output as closely as possible.
3. preserve existing file logging semantics.

The TUI must display:

1. existing tracing levels,
2. existing timestamps,
3. exact message text,
4. command echoes/results interleaved into the same pane.

## 6.2 Log Pane Features

Required:

1. `info` / `warn` / `error` filters at minimum,
2. clear action,
3. search with `/`,
4. tail-follow toggle,
5. bounded retention with oldest-drop behavior,
6. safe handling of high log volume without unbounded allocations.

Recommended follow-up:

1. `debug` / `trace` filters,
2. target/module filters,
3. export selection or copy-last-search result.

## 6.3 Server Command Input

The input row at the bottom of the log pane is the command entry point.

Command architecture:

1. TUI submits parsed command text over a non-blocking bounded channel.
2. Bevy receives commands through a dedicated admin-command resource/event.
3. Command execution results emit structured log events or explicit console-result log lines.
4. Command handling is authoritative on the server side; the TUI is only a local operator client.

This should be implemented as a reusable admin command bus:

1. TUI is one producer/client of the bus.
2. Future dashboard/dev-tool surfaces can become other producers without changing command execution semantics.
3. The bus contract should stay transport-agnostic for now so future routing can be direct to replication, proxied through gateway, or mediated by another authenticated admin surface.
4. The bus should not be limited to BRP semantics or Bevy-only actions; it should be able to dispatch explicit replication admin commands backed by server-owned handlers.

Phase-1 command set:

1. `help`
2. `clear`
3. `filter <level>`
4. `player <player_entity_id>`
5. `entity <entity_guid>`
6. `view <guid>`
7. `health`
8. `quit` or `exit` with confirmation semantics

## 7. Bottom Panel Design

## 7.1 Sessions Pane

Show connected runtime clients and their authoritative session binding:

1. client/session entity id,
2. authenticated `player_entity_id`,
3. current transport kind if available,
4. ping/RTT in ms,
5. controlled entity,
6. expandable tree of relevant in-game entities.

Tree expectations:

1. mimic the dashboard sidebar mental model rather than a flat list,
2. support expansion/collapse,
3. show parent-child and mount relationships where available,
4. preserve UUID/entity-guid based identity display, not raw cross-boundary Bevy ids as the primary label.

Data sources:

1. authenticated client binding resources,
2. control/runtime-state resources,
3. hydrated hierarchy and mount relationships,
4. Lightyear/client transport stats where available.

## 7.2 World Pane

Render an operational ASCII map of the simulated world:

1. centerable camera/viewport in world coordinates,
2. zoom levels that coarsen or refine the glyph grid,
3. pan with Vim-style keys,
4. entity selection from the map,
5. visual distinction for:
   - players/controlled entities,
   - visible dynamic entities,
   - static landmarks,
   - optional partition cell boundaries.

Initial map contract:

1. use shared world-space data (`Position` or `WorldPosition`) only,
2. do not derive authoritative positions from render transforms,
3. clip and aggregate when many entities map to one cell,
4. allow a selection inspector handoff to the Sessions/Health details.

Recommended overlays:

1. spatial partition grid,
2. visibility radii for selected entities,
3. local-view delivery range,
4. discovered-landmark markers.

## 7.3 Health Pane

Expose as much high-signal server telemetry as is practical without destabilizing the server:

1. process CPU %
2. process RSS / memory
3. worker/task counts
4. Bevy world entity count
5. replicated entity count
6. client/session count
7. physics body/object count
8. physics step/load timing
9. networking throughput and drop counters
10. visibility timing/candidate metrics
11. spatial partition occupancy/cell counts
12. persistence queue depth and error counts
13. database connectivity/latency summary where practical
14. BRP enabled/bind summary
15. asset/script reload and persistence worker status
16. Lua runtime/scripting metrics:
   - script interval/event execution counts,
   - execution time totals and recent max/avg timings,
   - Lua memory usage/limit information,
   - script error counts,
   - parser/validation/runtime diagnostics summaries where cheaply available

This pane should prefer grouped sections with stable labels over a noisy wall of counters.

## 8. Shared Diagnostics Model

## 8.1 Canonical Snapshot Resource

Define a canonical health snapshot resource and DTO set, for example:

1. `ReplicationHealthSnapshot`
2. `ReplicationSessionSnapshot`
3. `ReplicationWorldMapSnapshot`
4. `ReplicationLogBufferSnapshot`

Rules:

1. TUI and `/health` consume the same underlying health snapshot model.
2. `/health` should expose machine-readable JSON, not terminal-formatted text.
3. Health snapshot generation must pull from existing resources first, then add new instrumentation only where missing.
4. `/health` DTOs must be redacted/summary-shaped separately from richer TUI/session/world snapshots so operator-local detail is not accidentally exposed over HTTP.

## 8.2 Reuse Existing Metrics First

The first implementation pass should consume existing telemetry/resources before adding new systems:

1. `VisibilityRuntimeMetrics`
2. `ClientInputDropMetrics`
3. `PersistenceWorkerState`
4. `ClientLastActivity`
5. authenticated client bindings
6. control/runtime-state mappings
7. existing BRP config resources

Missing metrics should be added as explicit runtime resources, not ad-hoc string parsing of logs.

Lua/runtime scripting telemetry should be included in the same model when available from:

1. `bins/sidereal-replication/src/replication/runtime_scripting.rs`
2. `bins/sidereal-replication/src/replication/scripting.rs`
3. `crates/sidereal-scripting`

The first pass should inventory what `mlua` and Sidereal's scripting wrapper can expose cheaply:

1. configured memory limit,
2. current memory consumption if available,
3. script execution counts by cadence class,
4. error/failure counts,
5. parse/validation timing and counts where such data already exists or can be instrumented cheaply.

## 9. Replication `/health` Endpoint Plan

Add a lightweight replication-local HTTP server whose primary contract is:

1. `GET /health`
2. loopback-only by default,
3. JSON payload backed by `ReplicationHealthSnapshot`,
4. low-overhead, read-only response path,
5. no gameplay mutation endpoints bundled into this server.

Recommended response structure:

1. process metadata,
2. runtime status,
3. aggregate session/user counts only,
4. world/entity counts,
5. subsystem metrics,
6. degraded/warn/error states,
7. timestamp of snapshot production.

Explicit non-goals for `/health` payload shape:

1. no per-user/session rows,
2. no player/entity trees,
3. no raw command output,
4. no privileged debugging detail that belongs in the local TUI or authenticated tooling.

Follow-up option:

1. `GET /health/live` or `GET /health/summary` if a lighter payload is needed later.

## 10. Phased Implementation Plan

### Phase 0: Baseline and Surface Inventory

Goals:

1. catalog all currently available replication metrics/resources,
2. decide the minimal external crates needed (`ratatui`, `crossterm`, optional `sysinfo`, optional small arg parser),
3. define the canonical health snapshot structs and threading model,
4. inventory available Lua runtime/parser metrics and identify missing instrumentation.

Deliverables:

1. health snapshot schema,
2. TUI thread/channel design note in code comments,
3. dependency review including Windows/Linux terminal behavior.

### Phase 1: Logging Fanout and Headless CLI

Status: partially implemented on 2026-03-10

Goals:

1. introduce `--headless`,
2. preserve stderr + file logs,
3. add bounded in-memory log fanout for TUI consumption.

Acceptance:

1. headless mode reproduces current behavior,
2. TUI mode receives the same formatted log lines as stderr,
3. startup logs appear in the TUI without loss.

Progress note:

1. `--headless` and `--health-bind` parsing landed.
2. Shared bounded log fanout landed and preserves stderr + file logging while capturing the same rendered log lines into memory for future TUI consumption.
3. Full TUI-mode startup is not wired yet; interactive terminals currently log that TUI startup is planned but not yet enabled.

### Phase 2: Health Snapshot and `/health`

Status: partially implemented on 2026-03-10

Goals:

1. build the first canonical `ReplicationHealthSnapshot`,
2. stand up a loopback HTTP server for `/health`,
3. surface existing visibility/input/persistence metrics,
4. keep the HTTP payload summary-only.

Acceptance:

1. `/health` returns stable JSON,
2. the TUI health pane can render from the same snapshot,
3. no expensive per-request world scan is required.

Progress note:

1. Loopback replication `/health` now exists and returns summary JSON from a shared snapshot resource.
2. The snapshot currently exposes aggregate-only operational data and initial Lua runtime counters.
3. The future TUI health pane can consume the same underlying snapshot model, but the pane itself is not implemented yet.

### Phase 3: TUI Shell and Pane Framework

Status: partially implemented on 2026-03-10

Goals:

1. add terminal startup/shutdown lifecycle,
2. create rounded-panel layout and title-bar command legends,
3. add focus management, resize handling, and global key routing.

Acceptance:

1. startup defaults to TUI on interactive terminals,
2. resize works cleanly,
3. no visible simulation hitching is introduced by idle TUI rendering.

Progress note:

1. Interactive terminals now start a separate TUI thread by default unless `--headless` is used.
2. The shell renders the 33/66 layout, rounded borders, pane focus styling, and title-bar command hints.
3. Resize support is handled by `ratatui`'s redraw path; explicit hitch/perf profiling is still pending.

### Phase 4: Logs Pane and Command Entry

Status: partially implemented

Goals:

1. implement log scrolling/search/filter/clear,
2. add command line editing and command dispatch,
3. append command results into the log pane.

Acceptance:

1. `info` / `warn` / `error` filters work,
2. `/` search works,
3. commands round-trip through the Bevy-side executor,
4. clear does not affect file log persistence.

Progress note:

1. The reusable admin command bus and command parser/executor stub landed.
2. The TUI now provides a live log pane with `:` command entry, `/` search, `f` level filter cycling, `c` clear, and scroll/follow behavior.
3. Commands can now be submitted from the TUI producer and executed/logged by the server-side admin bus.
4. More advanced Vim-style search/result navigation and richer command help remain pending.

### Phase 5: Sessions Pane

Goals:

1. show live connected clients and ping,
2. render expandable entity trees,
3. connect session selection to world/health focus.

Acceptance:

1. tree expansion is stable under runtime updates,
2. ping values are visible in ms,
3. selected session/entity can drive context in the other panes.

### Phase 6: World Pane

Status: partially implemented on 2026-03-10

Goals:

1. implement the ASCII map renderer,
2. add pan/zoom/selection,
3. add at least one overlay for partitioning or visibility.

Acceptance:

1. selected entities are locatable on the map,
2. world view remains responsive at target snapshot cadence,
3. map rendering does not read the Bevy world directly from the TUI thread.

Progress note:

1. The world pane now consumes a shared runtime snapshot built on the replication side from `Position` / `WorldPosition`.
2. The map renders a light-grey grid and Unicode glyphs for players, ships, landmarks, projectiles, and asteroid-like entities.
3. Mouse interaction is live for world-pane select, zoom, and pan.
4. More advanced overlays, density handling, and dashboard-parity camera ergonomics are still pending.

### Phase 7: Expanded Health/Optimization Metrics

Status: partially implemented

Goals:

1. add process/system metrics,
2. add subsystem timings and loads,
3. add persistence/database/spatial diagnostics beyond the initial baseline,
4. add Lua/runtime scripting metrics where available.

Acceptance:

1. health pane exposes the key optimization targets already tracked by the team,
2. `/health` stays schema-stable and bounded,
3. Lua/runtime scripting metrics are included where available,
4. newly added metrics are backed by explicit runtime resources/tests.

Progress note:

1. Initial Lua runtime counters landed for reload count, interval/event execution counts, error count, memory limit, and recent run timings.
2. Current Lua memory consumption remains `Option`/best-effort and needs follow-up if a cheap authoritative `mlua` runtime metric is available.

## 11. Testing and Validation Plan

Unit tests:

1. bounded log ring behavior,
2. command parser and dispatch routing,
3. health snapshot serialization,
4. pane-local search/filter logic,
5. tree expansion state reducer,
6. map viewport translation and zoom math,
7. summary `/health` redaction rules for aggregate-only payloads.

Integration tests:

1. replication starts with `--headless` and without TUI side effects,
2. replication auto-falls back to headless when no TTY is present,
3. `/health` returns success and expected keys,
4. log fanout preserves stderr/file/TUI sink behavior,
5. `/health` does not expose per-session/player detail,
6. TUI command ingress does not block server update/fixed schedules.

Manual/runtime validation:

1. compare TUI log lines to console/file output for exact formatting parity,
2. stress-test resize, high log volume, and client connect/disconnect churn,
3. confirm simulation cadence does not regress with TUI active,
4. confirm map/session/health panels remain coherent during heavy visibility and persistence load.

Quality gates for the implementation PR:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
```

If replication client-facing/shared runtime code is touched as part of diagnostics sharing, also run the existing client target checks required by `AGENTS.md`.

## 12. Risks and Mitigations

1. Risk: terminal rendering stalls the runtime.
   Mitigation: separate TUI thread, bounded channels, no blocking world access.
2. Risk: log formatting diverges between console and TUI.
   Mitigation: one canonical formatter fanout path.
3. Risk: `/health` becomes a second ad-hoc diagnostics model.
   Mitigation: one shared snapshot schema for HTTP and TUI.
4. Risk: world pane becomes an expensive mini-renderer.
   Mitigation: coarse glyph map, cadence-limited snapshots, aggregation.
5. Risk: cross-platform terminal quirks create brittle startup behavior.
   Mitigation: degrade to headless on terminal init failure and cover Linux/Windows in validation.
6. Risk: exposing BRP-like detail through `/health` expands attack surface.
   Mitigation: keep `/health` summary-only and loopback-bound by default.
7. Risk: TUI commands become a dead-end local interface that the future dashboard cannot reuse.
   Mitigation: define a reusable admin command bus from the first implementation.
8. Risk: Lua telemetry becomes expensive or noisy.
   Mitigation: sample/cadence-limit scripting metrics and prefer aggregated counters/timers over per-script heavy dumps.

## 13. Recommended Follow-Up Decisions

1. Decide whether replication should also expose a richer authenticated diagnostics endpoint beyond summary `/health`.
2. Define the reusable admin command bus contract so future dashboard/dev-tool clients can target it without coupling to the TUI implementation.
3. Decide whether session/entity tree snapshots should eventually be shared with the dashboard to avoid duplicate diagnostics shaping logic.
4. Decide later whether dashboard-to-server admin traffic should talk directly to replication or route through gateway; this plan intentionally keeps that transport boundary open.

## 2026-03-11 Progress Note

- Increased retained in-memory TUI log history to reduce scrollback loss during active sessions.
- Switched log pane scroll state and scrollbar positioning to track the wrapped-row viewport offset directly so the thumb aligns with actual history position at top/bottom.
- Replaced remaining runtime terminal-print paths in replication-side long-running systems with tracing macros so out-of-band terminal writes do not stomp the alternate-screen TUI.
- Began left explorer-pane implementation using a shared world-explorer snapshot shaped around connected players, owned entities, and `MountedOn` hierarchy.
- Added live-ish per-player latency sourcing from Lightyear `Link.stats.rtt` and exposed it to the TUI explorer rows for right-aligned display.
- Renamed the center pane to `map` and replaced the old placeholder left pane with an expandable `world` tree scaffold that supports selection, mouse hit-testing, and collapse state.
- Corrected explorer nesting to use local Bevy `ChildOf` hierarchy rather than `MountedOn`, and re-enabled local hierarchy rebuild on replication so the diagnostics/TUI tree can mirror the dashboard-style explorer model.
- Snapshot production is now cadence-limited instead of running every uncapped `Update`: summary health defaults to 2 Hz and world/map explorer snapshots default to 5 Hz, with env overrides available for diagnostics tuning.
