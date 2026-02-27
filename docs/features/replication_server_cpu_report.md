# Sidereal-Replication Server CPU Efficiency Report

## Summary

The replication server was consuming ~157% of a CPU core because its main loop ran **unconstrained**: no frame pacing, so `Update` (and the rest of the main schedule) executed as fast as the CPU could spin. With only 1–2 entities and persistence every 10–15s, the workload per frame is tiny; the inefficiency was almost entirely from running that loop hundreds of thousands of times per second.

## Root Cause: Unconstrained Main Loop

- The server uses **MinimalPlugins**, which includes **ScheduleRunnerPlugin::default()**.
- **RunMode::default()** is `Loop { wait: None }`: the runner does **not** sleep between iterations.
- So the main loop is: `loop { app.update(); }` with no `thread::sleep`, i.e. a tight spin loop.
- **Update** runs every iteration: transport channel setup, input receive, control requests, asset requests, metrics, bootstrap commands, etc. **FixedUpdate** is driven by elapsed time (30 Hz), but the **frame** rate (iterations per second) is unbounded.

## Why 157% CPU Despite Minimal Work

1. **Update schedule** runs at “frame” rate. With no pacing, that’s effectively max CPU speed (e.g. hundreds of thousands of iterations per second).
2. **FixedUpdate** is correctly capped by Bevy’s fixed timestep (30 Hz), but **First**, **PreUpdate**, **RunFixedMainLoop** (stepper), **Update**, **PostUpdate**, **Last** all run every iteration. Lightyear’s server systems (e.g. message handling, replication) also run every frame.
3. **Multi-core**: Bevy’s main schedule is **single-threaded** by default (`ExecutorKind::SingleThreaded` for Main). So one core is saturated; >100% can come from other threads (e.g. persistence worker, bootstrap listener, Lightyear internals, or kernel).

## Secondary Observations

- **Bevy/Lightyear/Avian2D**: The main schedule and fixed loop are single-threaded; there is no built-in multi-core game loop. Parallelism is via system parallelism within a schedule, not across “frames.” So high CPU on one core is expected if the loop is unconstrained.
- **Persistence**: Correctly offloaded to a worker thread; not the cause of the spin.
- **Visibility**: `update_network_visibility` runs in **FixedUpdate** (30 Hz); not the cause of unbounded CPU.
- **Physics**: 1–2 entities at 30 Hz is negligible.

## Fix: Frame Pacing

Cap the main loop with a sleep so “frame” rate is bounded (e.g. 60–100 Hz). That’s enough for:

- Draining UDP and processing messages.
- Running Lightyear’s Update systems.

**FixedUpdate** remains time-based at 30 Hz regardless of frame rate.

Implementation:

1. Disable the default **ScheduleRunnerPlugin** from MinimalPlugins.
2. Add **ScheduleRunnerPlugin::run_loop(Duration::from_millis(10))** (or similar) to sleep so that each iteration is paced (e.g. ~100 Hz).

Example:

- `run_loop(Duration::from_millis(10))` → ~100 Hz Update, large CPU reduction.
- `run_loop(Duration::from_secs_f64(1.0 / 60.0))` → 60 Hz.

**Implemented:** The server uses `MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(...))` with a default of 100 Hz. Env **`REPLICATION_UPDATE_CAP_HZ`** (clamped 10–1000) overrides the cap without recompile.

## Recommendation

- **Always** use frame pacing for the replication server (run_loop with a non-zero wait).
- Document the chosen cap (e.g. 100 Hz) and that it only affects variable-timestep schedules; FixedUpdate stays at 30 Hz.
- If needed, add a small decision record or note in the runbook that the server must not run with the default MinimalPlugins runner in production.
