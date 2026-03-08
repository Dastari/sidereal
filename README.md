# sidereal_v3

Sidereal rebuild workspace (server-authoritative architecture, Bevy 0.18, Lightyear transport, Postgres+AGE persistence).

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

Gateway and replication tracing logs are written to both the console and workspace-relative `./logs/`, with a new timestamped file per process start.

## Useful Targets

```bash
make help
make pg-reset          # destructive: resets local postgres volume
```
