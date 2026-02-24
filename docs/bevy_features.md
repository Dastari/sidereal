# Bevy Feature Watchlist (Project-Relevant)

Status: Curated shortlist for Sidereal v3 planning
Bevy baseline in workspace: `0.18.x`
Last reviewed: 2026-02-19

## Why this file exists

Track newer Bevy features that are likely to reduce custom code in Sidereal or improve tooling/iteration speed.

## High-Relevance Features (Adopt Soon)

1. First-party camera controllers (`FreeCamera`, `PanCamera`)
- Relevance: fast debug camera setup for shard/client world inspection and top-down control experiments.
- Suggested use: dev/debug mode plugin only (not final gameplay camera logic).

2. Automatic directional navigation in UI
- Relevance: auth/login/register UI flow can gain keyboard/gamepad navigation with less custom focus logic.
- Suggested use: enable for auth screen and future in-game menus.

3. Cargo feature collections (`2d`, `3d`, `ui`)
- Relevance: can reduce build surface/compile time for service/tool binaries and tailored client targets.
- Suggested use: evaluate per-crate feature slimming where we currently pull full defaults.

4. Remove systems from schedules (`remove_systems_in_set`)
- Relevance: cleaner runtime toggles for optional systems (debug, dev overlays, temporary scaffolds) without per-frame run-condition overhead.
- Suggested use: use for hard opt-out behavior in dev/prod mode switches.

5. Safe mutable access to multiple arbitrary components
- Relevance: useful for complex ECS update passes touching multiple component sets safely in one operation.
- Suggested use: revisit physics/control/mass pipeline hotspots for cleaner borrow handling.

6. Required Components
- Relevance: enforce spawn-time ECS invariants for core gameplay entities (ship/module/hardpoint chains).
- Suggested use: add to generated/core component registration so invalid entity assemblies fail early.

7. Bevy Remote Protocol (`bevy_remote`) workflow hardening
- Relevance: shard/replication/client already use BRP endpoints and should share consistent inspection patterns.
- Suggested use: define one shared BRP operational profile (auth, endpoint toggles, debug-only helpers).

8. Message/Observer model alignment (0.17+)
- Relevance: clearer separation for event-like observer flows vs queue-like messages in networking/simulation boundaries.
- Suggested use: standardize new systems around current `Message` + observer patterns to avoid legacy assumptions.

## Medium-Relevance Features (Plan In)

1. glTF extensions / coordinate conversion improvements
- Relevance: directly affects streamed model fidelity and transform correctness (`corvette_01`, future assets).
- Suggested use: validate loader settings for current model pipeline and document required extension support.

2. Seekable asset readers
- Relevance: aligns with MMO-style pak/index cache and streamed asset reads.
- Suggested use: evaluate for chunked/offset reads in local cache subsystem.

3. Short-type-path asset processors
- Relevance: can simplify asset processor wiring and reduce type-path friction in tooling.
- Suggested use: assess when formalizing asset preprocessing/import pipeline.

4. Fullscreen materials
- Relevance: can simplify post-process/fullscreen shader workflows (potentially starfield/background passes).
- Suggested use: prototype replacing bespoke fullscreen shader plumbing where possible.

5. Easy screenshot/video recording
- Relevance: useful for QA capture, visual regression evidence, and reproducible bug reports.
- Suggested use: dev-tools command path only.

6. ECS relationships beyond hierarchy
- Relevance: helps model non-tree links (ownership, scan grants, intel links) without overloading parent/child.
- Suggested use: evaluate for visibility/scan-intel data model where `Children` is semantically incorrect.

7. Fallible query patterns (`Query::single*` Result-based)
- Relevance: reduces panic-risk in runtime paths and test harnesses.
- Suggested use: keep all new gameplay/networking systems on fallible access patterns with explicit error handling.

8. State-scoped entities / computed states
- Relevance: useful for auth-screen vs in-world entity lifecycle and temporary debug overlays.
- Suggested use: migrate ad-hoc screen cleanup to state-scoped lifecycle where practical.

## Lower Priority for Current Vertical Slice

- Atmosphere scattering / Solari rendering improvements.
- Advanced standard widget expansion beyond auth flow needs.
- Font-variation enhancements not required for current gameplay-critical UI.

## Suggested immediate actions

1. Add a dev-only camera-controller integration task to implementation checklist.
2. Add auth UI directional-navigation task to the current vertical-slice focus.
3. Add an asset-pipeline task to verify glTF coordinate conversion/extension behavior for streamed models.
4. Add a Required Components adoption task for core generated gameplay entities.
5. Add a visibility/intel data-model task evaluating ECS relationships vs parent/child.
6. Add a code-health task to audit remaining panic-style single-entity access patterns.

## Sources

- Bevy 0.18 release notes: https://bevy.org/news/bevy-0-18/
- Bevy 0.17 -> 0.18 migration guide: https://bevy.org/learn/migration-guides/0-17-to-0-18/
- Bevy 0.17 release notes: https://bevy.org/news/bevy-0-17/
- Bevy 0.16 release notes: https://bevy.org/news/bevy-0-16/
- Bevy 0.15 release notes: https://bevy.org/news/bevy-0-15/
- Bevy 0.14 release notes: https://bevy.org/news/bevy-0-14/
- Migration guide 0.16 -> 0.17: https://bevy.org/learn/migration-guides/0-16-to-0-17/
- Migration guide 0.15 -> 0.16: https://bevy.org/learn/migration-guides/0-15-to-0-16/
- Migration guide 0.14 -> 0.15: https://bevy.org/learn/migration-guides/0-14-to-0-15/
