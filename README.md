# sidereal

Sidereal is a server-authoritative multiplayer space RPG built around:

- deterministic fixed-step simulation,
- capability-driven Bevy ECS gameplay,
- persistent world state,
- smooth client prediction/interpolation for responsive control.

## What This Repo Currently Contains

Sidereal is a server-authoritative multiplayer game rebuild with:

1. Bevy 0.18 client/runtime code,
2. Lightyear-based networking and client prediction/interpolation,
3. Postgres + AGE-backed persistence,
4. Lua-authored content direction for assets, rendering, and scripting-connected systems.

Primary services/workspace areas:

1. `bins/sidereal-client`
2. `bins/sidereal-gateway`
3. `bins/sidereal-replication`
4. `crates/sidereal-game`
5. `crates/sidereal-asset-runtime`

## Quick Start

1. Start database:

```bash
make pg-up
```

2. Run core services:

```bash
make dev-stack
```

3. (Optional) Run native client too:

```bash
make dev-stack-client
```

## Documentation Map

1. Architecture baseline: `docs/sidereal_design_document.md`
2. Implementation tracker: `docs/sidereal_implementation_checklist.md`
3. Decision register: `docs/decision_register.md`
4. Documentation index: `docs/README.md`
5. Active feature contracts/references: `docs/features/`
6. Decision detail docs: `docs/decisions/`
7. Plans and migration docs: `docs/plans/`
8. Audit reports: `docs/reports/`

## Useful Targets

```bash
make help
make pg-reset          # destructive: resets local postgres volume
```
