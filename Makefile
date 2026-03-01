SHELL := /bin/bash

PG_URL ?= postgres://sidereal:sidereal@127.0.0.1:5432/sidereal
SIDEREAL_PG_PORT ?= 5432
GATEWAY_BIND ?= 0.0.0.0:8080
GATEWAY_CLIENT_URL ?= http://127.0.0.1:8080
GATEWAY_BOOTSTRAP_MODE ?= udp
GATEWAY_JWT_SECRET ?= 0123456789abcdef0123456789abcdef
ASSET_ROOT ?= ./data
WGPU_ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER ?= 1
SIDEREAL_DEBUG_INPUT_LOGS ?= 1
SIDEREAL_DEBUG_CONTROL_LOGS ?= 1
SIDEREAL_CLIENT_MOTION_AUDIT ?= 1
SIDEREAL_CLIENT_WGPU_BACKENDS ?= vulkan
WGPU_POWER_PREF ?= high

REPLICATION_UDP_BIND ?= 0.0.0.0:7001
REPLICATION_UDP_ADDR ?= 127.0.0.1:7001
SHARD_UDP_BIND ?= 127.0.0.1:7002
CLIENT_UDP_BIND ?= 127.0.0.1:7003
CLIENT2_UDP_BIND ?= 127.0.0.1:7004

REPLICATION_CONTROL_UDP_BIND ?= 127.0.0.1:9004
REPLICATION_CONTROL_UDP_ADDR ?= 127.0.0.1:9004
GATEWAY_REPLICATION_CONTROL_UDP_BIND ?= 0.0.0.0:0
BRP_AUTH_TOKEN ?= 0123456789abcdef
REPLICATION_BRP_ENABLED ?= true
REPLICATION_BRP_PORT ?= 15713
REPLICATION_BRP_BIND_ADDR ?= 127.0.0.1
CLIENT_BRP_ENABLED ?= true
CLIENT_BRP_PORT ?= 15714
CLIENT_BRP_BIND_ADDR ?= 127.0.0.1
CLIENT2_BRP_PORT ?= 15715
CLIENT2_BRP_BIND_ADDR ?= 127.0.0.1
REPLICATION_BRP_URL ?= http://127.0.0.1:$(REPLICATION_BRP_PORT)/
CLIENT_BRP_URL ?= http://127.0.0.1:$(CLIENT_BRP_PORT)/
CLIENT2_BRP_URL ?= http://127.0.0.1:$(CLIENT2_BRP_PORT)/
BRP_DUMP_DIR ?= ./data/debug/brp_dumps
DASHBOARD_DIR ?= ./dashboard

.PHONY: help pg-up pg-down pg-logs pg-reset db-reset fmt clippy check test test-gateway test-replication test-client wasm-check windows-check windows-build windows-release target-size clean-lite clean-full run-gateway run-replication run-shard run-client run-client-release run-client-wsl-perf run-client-wsl-safe run-client2 run-client-headless run-dashboard brp-dump-replication brp-dump-client brp-dump-client2 brp-dump-all dev-stack dev-stack-client register-demo

help:
	@echo "Sidereal v3 Make targets"
	@echo ""
	@echo "Infra:"
	@echo "  make pg-up              Start postgres+AGE via docker compose"
	@echo "  make pg-down            Stop postgres+AGE"
	@echo "  make pg-logs            Tail postgres logs"
	@echo "  make pg-reset           Recreate postgres volume (destructive)"
	@echo "  make db-reset           Alias for pg-reset"
	@echo ""
	@echo "Quality:"
	@echo "  make fmt                cargo fmt --all -- --check"
	@echo "  make clippy             cargo clippy --workspace --all-targets -- -D warnings"
	@echo "  make check              cargo check --workspace"
	@echo "  make wasm-check         cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu"
	@echo "  make windows-check      cargo check client for x86_64-pc-windows-gnu"
	@echo "  make windows-build      Debug build client .exe"
	@echo "  make windows-release    Release build client .exe"
	@echo "  make target-size        Show target/ disk usage summary"
	@echo "  make clean-lite         Remove incremental/debug caches (keeps release artifacts)"
	@echo "  make clean-full         Remove all cargo build artifacts (cargo clean)"
	@echo "  make test               Run key crate tests"
	@echo ""
	@echo "Runtime:"
	@echo "  make run-replication    Run replication server"
	@echo "  make run-shard          Run shard server"
	@echo "  make run-gateway        Run gateway API server"
	@echo "  make run-client         Run native client"
	@echo "  make run-client-release Run native client in release mode (recommended for perf)"
	@echo "  make run-client-wsl-perf Run release client with WSL perf-oriented GPU env"
	@echo "  make run-client-wsl-safe Run release client with conservative WSL GPU env"
	@echo "  make run-client2        Run second native client on UDP 7004"
	@echo "  make run-client-headless Run transport-only native client"
	@echo "  make run-dashboard      Run dashboard with BRP env configured"
	@echo "  make brp-dump-replication Dump replication BRP world.query JSON"
	@echo "  make brp-dump-client    Dump client BRP world.query JSON"
	@echo "  make brp-dump-client2   Dump client2 BRP world.query JSON"
	@echo "  make brp-dump-all       Dump replication + both clients BRP snapshots"
	@echo "  make dev-stack          Run replication + shard + gateway in one shell"
	@echo "  make dev-stack-client   Run replication + shard + gateway + native client"
	@echo "  make register-demo      Register demo account via gateway"

pg-up:
	SIDEREAL_PG_PORT=$(SIDEREAL_PG_PORT) docker compose up -d --force-recreate postgres

pg-down:
	docker compose down

pg-logs:
	docker compose logs -f postgres

pg-reset:
	docker compose down -v
	rm -rf data/postgresql
	docker compose up -d postgres

db-reset: pg-reset

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

check:
	cargo check --workspace

wasm-check:
	cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu

windows-check:
	cargo check -p sidereal-client --target x86_64-pc-windows-gnu

windows-build:
	cargo build -p sidereal-client --target x86_64-pc-windows-gnu
	@echo "Built: target/x86_64-pc-windows-gnu/debug/sidereal-client.exe"

windows-release:
	cargo build -p sidereal-client --target x86_64-pc-windows-gnu --release
	@echo "Built: target/x86_64-pc-windows-gnu/release/sidereal-client.exe"

target-size:
	@if [ -d target ]; then \
		echo "target/ total:"; \
		du -sh target; \
		echo ""; \
		echo "target/* breakdown:"; \
		du -sh target/* 2>/dev/null | sort -h; \
	else \
		echo "No target/ directory yet."; \
	fi

clean-lite:
	@echo "Removing incremental/debug caches under target/ ..."
	rm -rf target/debug/incremental target/debug/.fingerprint target/debug/build
	@echo "Done. Use 'make target-size' to inspect remaining artifacts."

clean-full:
	cargo clean
	@echo "Done. Full cargo artifacts removed."

test:
	cargo test -p sidereal-replication
	cargo test -p sidereal-gateway
	cargo test -p sidereal-shard
	cargo test -p sidereal-client

test-gateway:
	cargo test -p sidereal-gateway

test-replication:
	cargo test -p sidereal-replication

test-client:
	cargo test -p sidereal-client

run-replication:
	REPLICATION_DATABASE_URL=$(PG_URL) \
	REPLICATION_UDP_BIND=$(REPLICATION_UDP_BIND) \
	REPLICATION_CONTROL_UDP_BIND=$(REPLICATION_CONTROL_UDP_BIND) \
	SIDEREAL_REPLICATION_BRP_ENABLED=$(REPLICATION_BRP_ENABLED) \
	SIDEREAL_REPLICATION_BRP_BIND_ADDR=$(REPLICATION_BRP_BIND_ADDR) \
	SIDEREAL_REPLICATION_BRP_PORT=$(REPLICATION_BRP_PORT) \
	SIDEREAL_REPLICATION_BRP_AUTH_TOKEN=$(BRP_AUTH_TOKEN) \
	GATEWAY_JWT_SECRET=$(GATEWAY_JWT_SECRET) \
	cargo run -p sidereal-replication

run-shard:
	REPLICATION_UDP_ADDR=$(REPLICATION_UDP_ADDR) \
	SHARD_UDP_BIND=$(SHARD_UDP_BIND) \
	cargo run -p sidereal-shard

run-gateway:
	GATEWAY_DATABASE_URL=$(PG_URL) \
	GATEWAY_BIND=$(GATEWAY_BIND) \
	GATEWAY_BOOTSTRAP_MODE=$(GATEWAY_BOOTSTRAP_MODE) \
	GATEWAY_JWT_SECRET=$(GATEWAY_JWT_SECRET) \
	GATEWAY_REPLICATION_CONTROL_UDP_BIND=$(GATEWAY_REPLICATION_CONTROL_UDP_BIND) \
	REPLICATION_CONTROL_UDP_ADDR=$(REPLICATION_CONTROL_UDP_ADDR) \
	ASSET_ROOT=$(ASSET_ROOT) \
	cargo run -p sidereal-gateway

run-client:
	REPLICATION_UDP_ADDR=$(REPLICATION_UDP_ADDR) \
	CLIENT_UDP_BIND=$(CLIENT_UDP_BIND) \
	GATEWAY_URL=$(GATEWAY_CLIENT_URL) \
	SIDEREAL_CLIENT_WGPU_BACKENDS=$(SIDEREAL_CLIENT_WGPU_BACKENDS) \
	WGPU_POWER_PREF=$(WGPU_POWER_PREF) \
	SIDEREAL_CLIENT_BRP_ENABLED=$(CLIENT_BRP_ENABLED) \
	SIDEREAL_CLIENT_BRP_BIND_ADDR=$(CLIENT_BRP_BIND_ADDR) \
	SIDEREAL_CLIENT_BRP_PORT=$(CLIENT_BRP_PORT) \
	SIDEREAL_CLIENT_BRP_AUTH_TOKEN=$(BRP_AUTH_TOKEN) \
	SIDEREAL_ASSET_ROOT=/home/toby/dev/sidereal_v3 \
	WGPU_ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER=$(WGPU_ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER) \
	cargo run -p sidereal-client

run-client-release:
	REPLICATION_UDP_ADDR=$(REPLICATION_UDP_ADDR) \
	CLIENT_UDP_BIND=$(CLIENT_UDP_BIND) \
	GATEWAY_URL=$(GATEWAY_CLIENT_URL) \
	SIDEREAL_CLIENT_WGPU_BACKENDS=$(SIDEREAL_CLIENT_WGPU_BACKENDS) \
	WGPU_POWER_PREF=$(WGPU_POWER_PREF) \
	SIDEREAL_CLIENT_BRP_ENABLED=$(CLIENT_BRP_ENABLED) \
	SIDEREAL_CLIENT_BRP_BIND_ADDR=$(CLIENT_BRP_BIND_ADDR) \
	SIDEREAL_CLIENT_BRP_PORT=$(CLIENT_BRP_PORT) \
	SIDEREAL_CLIENT_BRP_AUTH_TOKEN=$(BRP_AUTH_TOKEN) \
	SIDEREAL_ASSET_ROOT=/home/toby/dev/sidereal_v3 \
	WGPU_ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER=$(WGPU_ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER) \
	cargo run -p sidereal-client --release

run-client-wsl-perf:
	REPLICATION_UDP_ADDR=$(REPLICATION_UDP_ADDR) \
	CLIENT_UDP_BIND=$(CLIENT_UDP_BIND) \
	GATEWAY_URL=$(GATEWAY_CLIENT_URL) \
	SIDEREAL_CLIENT_WGPU_BACKENDS=vulkan \
	WGPU_POWER_PREF=high \
	MESA_VK_DEVICE_SELECT=10de:27e0 \
	WGPU_ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER=1 \
	SIDEREAL_CLIENT_BRP_ENABLED=$(CLIENT_BRP_ENABLED) \
	SIDEREAL_CLIENT_BRP_BIND_ADDR=$(CLIENT_BRP_BIND_ADDR) \
	SIDEREAL_CLIENT_BRP_PORT=$(CLIENT_BRP_PORT) \
	SIDEREAL_CLIENT_BRP_AUTH_TOKEN=$(BRP_AUTH_TOKEN) \
	SIDEREAL_ASSET_ROOT=/home/toby/dev/sidereal_v3 \
	cargo run -p sidereal-client --release

run-client-wsl-safe:
	REPLICATION_UDP_ADDR=$(REPLICATION_UDP_ADDR) \
	CLIENT_UDP_BIND=$(CLIENT_UDP_BIND) \
	GATEWAY_URL=$(GATEWAY_CLIENT_URL) \
	SIDEREAL_CLIENT_WGPU_BACKENDS=vulkan \
	WGPU_POWER_PREF=low \
	WGPU_ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER=1 \
	SIDEREAL_CLIENT_BRP_ENABLED=$(CLIENT_BRP_ENABLED) \
	SIDEREAL_CLIENT_BRP_BIND_ADDR=$(CLIENT_BRP_BIND_ADDR) \
	SIDEREAL_CLIENT_BRP_PORT=$(CLIENT_BRP_PORT) \
	SIDEREAL_CLIENT_BRP_AUTH_TOKEN=$(BRP_AUTH_TOKEN) \
	SIDEREAL_ASSET_ROOT=/home/toby/dev/sidereal_v3 \
	cargo run -p sidereal-client --release

run-client2:
	$(MAKE) run-client CLIENT_UDP_BIND=$(CLIENT2_UDP_BIND) CLIENT_BRP_PORT=$(CLIENT2_BRP_PORT)

run-client-headless:
	SIDEREAL_CLIENT_HEADLESS=1 \
	REPLICATION_UDP_ADDR=$(REPLICATION_UDP_ADDR) \
	CLIENT_UDP_BIND=$(CLIENT_UDP_BIND) \
	GATEWAY_URL=http://$(GATEWAY_BIND) \
	SIDEREAL_CLIENT_BRP_ENABLED=$(CLIENT_BRP_ENABLED) \
	SIDEREAL_CLIENT_BRP_BIND_ADDR=$(CLIENT_BRP_BIND_ADDR) \
	SIDEREAL_CLIENT_BRP_PORT=$(CLIENT_BRP_PORT) \
	SIDEREAL_CLIENT_BRP_AUTH_TOKEN=$(BRP_AUTH_TOKEN) \
	cargo run -p sidereal-client

run-dashboard:
	REPLICATION_BRP_URL=$(REPLICATION_BRP_URL) \
	CLIENT_BRP_URL=$(CLIENT_BRP_URL) \
	SIDEREAL_REPLICATION_BRP_AUTH_TOKEN=$(BRP_AUTH_TOKEN) \
	SIDEREAL_CLIENT_BRP_AUTH_TOKEN=$(BRP_AUTH_TOKEN) \
	REPLICATION_DATABASE_URL=$(PG_URL) \
	pnpm --dir $(DASHBOARD_DIR) dev

brp-dump-replication:
	@set -euo pipefail; \
	mkdir -p "$(BRP_DUMP_DIR)"; \
	ts=$$(date +%Y%m%d_%H%M%S); \
	out="$(BRP_DUMP_DIR)/replication_$${ts}.json"; \
	curl -sS -X POST "$(REPLICATION_BRP_URL)" \
		-H "content-type: application/json" \
		-H "authorization: Bearer $(BRP_AUTH_TOKEN)" \
		-d '{"jsonrpc":"2.0","id":"sidereal-brp-dump","method":"world.query","params":{"data":{"components":[],"option":"all","has":[]},"filter":{"with":[],"without":[]},"strict":false}}' \
		> "$$out"; \
	echo "$$out"

brp-dump-client:
	@set -euo pipefail; \
	mkdir -p "$(BRP_DUMP_DIR)"; \
	ts=$$(date +%Y%m%d_%H%M%S); \
	out="$(BRP_DUMP_DIR)/client1_$${ts}.json"; \
	curl -sS -X POST "$(CLIENT_BRP_URL)" \
		-H "content-type: application/json" \
		-H "authorization: Bearer $(BRP_AUTH_TOKEN)" \
		-d '{"jsonrpc":"2.0","id":"sidereal-brp-dump","method":"world.query","params":{"data":{"components":[],"option":"all","has":[]},"filter":{"with":[],"without":[]},"strict":false}}' \
		> "$$out"; \
	echo "$$out"

brp-dump-client2:
	@set -euo pipefail; \
	mkdir -p "$(BRP_DUMP_DIR)"; \
	ts=$$(date +%Y%m%d_%H%M%S); \
	out="$(BRP_DUMP_DIR)/client2_$${ts}.json"; \
	curl -sS -X POST "$(CLIENT2_BRP_URL)" \
		-H "content-type: application/json" \
		-H "authorization: Bearer $(BRP_AUTH_TOKEN)" \
		-d '{"jsonrpc":"2.0","id":"sidereal-brp-dump","method":"world.query","params":{"data":{"components":[],"option":"all","has":[]},"filter":{"with":[],"without":[]},"strict":false}}' \
		> "$$out"; \
	echo "$$out"

brp-dump-all:
	@set -euo pipefail; \
	$(MAKE) brp-dump-replication; \
	$(MAKE) brp-dump-client; \
	$(MAKE) brp-dump-client2

dev-stack:
	@set -euo pipefail; \
	echo "[sidereal] starting replication + shard + gateway"; \
	trap 'kill 0' INT TERM EXIT; \
	REPLICATION_DATABASE_URL=$(PG_URL) REPLICATION_UDP_BIND=$(REPLICATION_UDP_BIND) REPLICATION_CONTROL_UDP_BIND=$(REPLICATION_CONTROL_UDP_BIND) SIDEREAL_REPLICATION_BRP_ENABLED=$(REPLICATION_BRP_ENABLED) SIDEREAL_REPLICATION_BRP_PORT=$(REPLICATION_BRP_PORT) SIDEREAL_REPLICATION_BRP_AUTH_TOKEN=$(BRP_AUTH_TOKEN) GATEWAY_JWT_SECRET=$(GATEWAY_JWT_SECRET) cargo run -p sidereal-replication & \
	sleep 1; \
	REPLICATION_UDP_ADDR=$(REPLICATION_UDP_ADDR) SHARD_UDP_BIND=$(SHARD_UDP_BIND) cargo run -p sidereal-shard & \
	sleep 1; \
	GATEWAY_DATABASE_URL=$(PG_URL) GATEWAY_BIND=$(GATEWAY_BIND) GATEWAY_BOOTSTRAP_MODE=$(GATEWAY_BOOTSTRAP_MODE) GATEWAY_JWT_SECRET=$(GATEWAY_JWT_SECRET) GATEWAY_REPLICATION_CONTROL_UDP_BIND=$(GATEWAY_REPLICATION_CONTROL_UDP_BIND) REPLICATION_CONTROL_UDP_ADDR=$(REPLICATION_CONTROL_UDP_ADDR) ASSET_ROOT=$(ASSET_ROOT) cargo run -p sidereal-gateway & \
	wait

dev-stack-client:
	@set -euo pipefail; \
	echo "[sidereal] starting replication + shard + gateway + native client"; \
	trap 'kill 0' INT TERM EXIT; \
	REPLICATION_DATABASE_URL=$(PG_URL) REPLICATION_UDP_BIND=$(REPLICATION_UDP_BIND) REPLICATION_CONTROL_UDP_BIND=$(REPLICATION_CONTROL_UDP_BIND) SIDEREAL_REPLICATION_BRP_ENABLED=$(REPLICATION_BRP_ENABLED) SIDEREAL_REPLICATION_BRP_PORT=$(REPLICATION_BRP_PORT) SIDEREAL_REPLICATION_BRP_AUTH_TOKEN=$(BRP_AUTH_TOKEN) GATEWAY_JWT_SECRET=$(GATEWAY_JWT_SECRET) cargo run -p sidereal-replication & \
	sleep 1; \
	REPLICATION_UDP_ADDR=$(REPLICATION_UDP_ADDR) SHARD_UDP_BIND=$(SHARD_UDP_BIND) cargo run -p sidereal-shard & \
	sleep 1; \
	GATEWAY_DATABASE_URL=$(PG_URL) GATEWAY_BIND=$(GATEWAY_BIND) GATEWAY_BOOTSTRAP_MODE=$(GATEWAY_BOOTSTRAP_MODE) GATEWAY_JWT_SECRET=$(GATEWAY_JWT_SECRET) GATEWAY_REPLICATION_CONTROL_UDP_BIND=$(GATEWAY_REPLICATION_CONTROL_UDP_BIND) REPLICATION_CONTROL_UDP_ADDR=$(REPLICATION_CONTROL_UDP_ADDR) ASSET_ROOT=$(ASSET_ROOT) cargo run -p sidereal-gateway & \
	sleep 2; \
	REPLICATION_UDP_ADDR=$(REPLICATION_UDP_ADDR) CLIENT_UDP_BIND=$(CLIENT_UDP_BIND) GATEWAY_URL=http://$(GATEWAY_BIND) SIDEREAL_CLIENT_WGPU_BACKENDS=$(SIDEREAL_CLIENT_WGPU_BACKENDS) WGPU_POWER_PREF=$(WGPU_POWER_PREF) SIDEREAL_CLIENT_BRP_ENABLED=$(CLIENT_BRP_ENABLED) SIDEREAL_CLIENT_BRP_PORT=$(CLIENT_BRP_PORT) SIDEREAL_CLIENT_BRP_AUTH_TOKEN=$(BRP_AUTH_TOKEN) cargo run -p sidereal-client & \
	wait

register-demo:
	curl -sS -X POST http://$(GATEWAY_BIND)/auth/register \
		-H "Content-Type: application/json" \
		-d '{"email":"pilot@example.com","password":"very-strong-password"}'
