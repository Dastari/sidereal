# DR-0029: WebTransport-First Browser Runtime Transport

Status: Accepted
Date: 2026-03-08
Owners: client/runtime + networking + replication

## Context

Sidereal's browser transport wording had drifted across active docs:

1. some project rules still said WebRTC-first,
2. the active wasm parity work had moved onto Lightyear's WebTransport support,
3. the browser asset/bootstrap/runtime path now depends on one concrete transport contract instead of placeholder wording.

The project needed one canonical browser transport direction before finishing the shared native/WASM runtime path.

## Decision

1. Browser/WASM runtime transport is WebTransport-first.
2. WebSocket is allowed only as an explicit fallback path and must not be the default browser runtime transport.
3. Gateway auth, world-entry bootstrap, and asset payload delivery remain authenticated HTTP concerns; replication transport does not carry asset payload bytes.
4. Gameplay, prediction, reconciliation, and ECS runtime systems remain shared between native and WASM; only the transport/browser I/O adapter boundary changes by target.
5. Browser runtime asset attachment is byte-backed from validated cache or gateway payload bytes and must not depend on filesystem-style `AssetServer` paths.

## Alternatives Considered

1. Keep WebRTC-first wording: rejected because it no longer matched the active Lightyear/browser transport path in the codebase.
2. Default browser runtime transport to WebSocket: rejected because it is not the preferred low-latency transport direction for authoritative gameplay traffic.
3. Leave transport wording intentionally vague: rejected because the wasm parity implementation now depends on concrete gateway/replication/client transport metadata and bootstrap behavior.

## Consequences

### Positive

1. One canonical browser transport contract now exists across AGENTS, design docs, and the parity plan.
2. Client and replication transport work can proceed without further architecture churn at the browser boundary.
3. The browser runtime keeps gameplay logic shared while isolating target-specific networking at the transport edge only.

### Negative

1. Browser deployment now depends on WebTransport-capable replication listener configuration and certificate handling.
2. Any future WebSocket fallback must remain explicitly documented and cannot quietly become the default path.
3. Browser certificate-hash mode is stricter than generic local TLS: the server certificate must stay within browser WebTransport constraints rather than using an arbitrary long-lived self-signed dev cert.

### Browser Certificate Constraints

For browser WebTransport connections that use `serverCertificateHashes`, the development certificate must satisfy current browser requirements:

1. Use an X.509v3 certificate with a validity period shorter than 2 weeks.
2. Use ECDSA P-256 (`secp256r1` / `prime256v1`) rather than RSA.
3. Include SAN entries for the hostnames/IPs used by the browser client (at minimum `localhost`, `127.0.0.1`, and `::1` for local development).

The repo's development WebTransport certificate generation must preserve those constraints so browser runtime validation does not fail at TLS handshake time before replication/bootstrap can start.

## Follow-Up

1. Keep the wasm parity plan and implementation checklist aligned with the WebTransport-first direction.
2. Add live browser validation coverage for login, world-entry, asset bootstrap, and in-world replication.
3. Continue the asset-delivery contract follow-through on packed-cache storage without reintroducing filesystem-path assumptions into browser runtime code.

## References

- `AGENTS.md`
- `docs/sidereal_design_document.md`
- `docs/sidereal_implementation_checklist.md`
- `docs/plans/wasm_parity_implementation_plan.md`
- `docs/features/asset_delivery_contract.md`
