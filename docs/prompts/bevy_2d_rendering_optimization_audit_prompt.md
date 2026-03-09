# Bevy 2D Rendering Optimization Audit Prompt

Perform a complete rendering-performance audit for this repository as a senior Bevy 2D rendering/performance engineer with strong experience in Lightyear, Avian2D, asset streaming, server-authoritative multiplayer architecture, and frame-time analysis.

Your job is not to give generic Bevy advice. Your job is to inspect this codebase and produce a detailed optimization report focused on why the game feels slow, what is actually making it slow, what only appears slow, and what should be changed first.

## Context

- This is Sidereal, a server-authoritative multiplayer game/framework being rebuilt from scratch.
- Architecture and contributor rules are documented in:
  - `docs/sidereal_design_document.md`
  - `docs/decision_register.md`
  - `docs/features/visibility_replication_contract.md`
  - `docs/features/asset_delivery_contract.md`
  - `docs/decisions/dr-0027_lua_authored_render_layers_and_generic_shader_pipeline.md`
  - `docs/features/scripting_support.md`
  - `AGENTS.md`
- Client uses Bevy with a top-down 2D rendering direction.
- Physics uses Avian2D.
- Networking/prediction uses Lightyear.
- Gateway, replication server, persistence, scripting, asset delivery, and client are all in the same workspace.
- The current runtime direction includes:
  - camera-relative world rendering,
  - Lua-authored render layers and rules,
  - streamed assets/shaders,
  - server-authoritative replication with prediction/reconciliation,
  - native client priority, with WASM still needing architectural compatibility.

## Primary Goal

Find every meaningful cause of poor perceived or actual rendering performance across the full stack, not just inside the renderer.

This audit must answer:

1. Why the game may feel slow even if pure GPU load is not the only problem.
2. Which costs are truly render-bound vs simulation-bound vs replication-bound vs asset/shader-bound vs scheduling/frame-pacing-bound.
3. Which server-side or replication-side bottlenecks degrade render smoothness indirectly by starving, stalling, flooding, or destabilizing the client.
4. Which issues are architecture-level problems versus local cleanup opportunities.
5. What should be fixed first for the largest frame-time and perceived-smoothness gain.

## Audit Scope

Audit all client-visible performance contributors, including but not limited to:

1. Bevy render pipeline usage.
2. 2D camera setup and camera-relative transform flow.
3. Sprite/world-layer/fullscreen/post-process rendering paths.
4. Material/shader organization, specialization churn, and bind-group/material duplication.
5. Asset streaming, shader loading, texture loading, cache behavior, and runtime invalidation.
6. ECS schedule shape, system ordering, over-eager queries, and per-frame/per-fixed-tick work.
7. Transform propagation, hierarchy rebuilds, visibility/culling, and render extraction pressure.
8. Entity counts, component counts, and render-relevant entity duplication.
9. Lightyear interpolation, prediction, reconciliation, duplication/winner selection, and visual smoothing paths.
10. Avian-to-render synchronization and any extra transform-copy layers.
11. UI/overlay/debug rendering overhead.
12. Logging, tracing, diagnostics, BRP/debug inspection, and any runtime observability features that may affect frame time.
13. Native-only versus WASM-relevant rendering constraints where architecture choices could hurt later parity.
14. Server-side simulation, visibility, replication delivery, asset metadata flow, or packet/update pressure that can make rendering appear slow.
15. Startup-time and hot-reload-time shader/material/asset behavior that can create hitching or long stalls.
16. Any code paths that keep work alive for hidden/off-screen/non-relevant entities.
17. Any places where the current design docs promise a lower-cost path than the code actually implements.

## Required Investigation Areas

Audit for all of the following.

### 1. Frame pacing and perceived slowness

Find anything that would make the game feel bad even when average FPS looks acceptable, including:

1. fixed-tick visual stepping,
2. missing frame interpolation,
3. conflicting smoothing layers,
4. camera jitter,
5. transform ownership conflicts,
6. long-tail frame spikes,
7. blocking work on the main thread,
8. asset/shader hitching,
9. over-frequent world/UI rebuilds,
10. present-mode or backend choices that may worsen smoothness.

### 2. Bevy render-path efficiency

Review:

1. cameras,
2. render layers,
3. visibility,
4. extraction,
5. sprite/material batching potential,
6. fullscreen passes,
7. post-process passes,
8. off-screen render targets,
9. custom material usage,
10. expensive per-entity render setup.

Call out:

1. unnecessary cameras or passes,
2. avoidable draw-call inflation,
3. avoidable material diversity,
4. avoidable shader specialization proliferation,
5. excessive render-world extraction churn,
6. incorrect use of Bevy APIs that blocks batching/culling,
7. places where more modern/current Bevy functionality should be used.

### 3. 2D-specific rendering issues

Inspect for:

1. sprite batching blockers,
2. texture atlas underuse or misuse,
3. per-entity texture/material uniqueness that defeats batching,
4. parallax/world-layer implementation cost,
5. fullscreen background/foreground implementation cost,
6. shader-heavy planet/effect paths,
7. tactical overlays or debug overlays that redraw too much,
8. excessive z-ordering/depth logic in what should be simple 2D paths,
9. camera-relative position conversion overhead,
10. hidden overdraw risks,
11. alpha blending heavy paths,
12. unnecessary large-screen fullscreen work.

### 4. ECS and schedule overhead affecting rendering

Find:

1. systems running every frame that should be event-driven or gated,
2. duplicated transform sync or visual sync systems,
3. schedule phases doing the same work more than once,
4. expensive wide queries in Update/FixedUpdate/PostUpdate that affect render readiness,
5. resources rebuilt every frame unnecessarily,
6. component churn causing change-detection noise,
7. entity spawn/despawn churn that destabilizes render state,
8. hierarchy rebuild or child-attachment work that is too frequent,
9. diagnostics systems that are too expensive for active runtime.

### 5. Lightyear / networking / replication causes of render slowness

Audit all ways networking can degrade rendering quality or performance, including:

1. duplicate entity presentation,
2. render winner selection hacks,
3. missing or incomplete frame interpolation,
4. correction behavior causing visual snapping or churn,
5. over-delivery of entities/components the client cannot render usefully,
6. visibility/range systems that do not narrow enough,
7. replication frequency or payload shape that creates client-side churn,
8. adoption/despawn lifecycles that thrash render state,
9. prediction/reconciliation flows that generate unnecessary visual work,
10. any places where render safety data is larger or noisier than needed.

### 6. Server-side bottlenecks that indirectly hurt rendering

Do not assume slow feeling is purely a client render issue.

Inspect for server-side contributors such as:

1. visibility queries that scale poorly,
2. replication delivery work that spikes or floods the client,
3. expensive per-tick serialization or filtering,
4. unnecessary component replication,
5. poor delivery-volume culling,
6. asset/bootstrap behavior that delays client readiness,
7. world-init/script/catalog validation work that can stall startup,
8. logging/tracing volume that may hurt local test smoothness,
9. simulation scheduling or fixed-step drift that produces uneven client update cadence,
10. any architecture that causes the client to spend render time compensating for unstable authoritative data.

### 7. Asset, shader, and material pipeline costs

Review:

1. runtime shader compilation risks,
2. streamed shader/material asset churn,
3. fallback shader/material paths,
4. texture residency and duplication,
5. asset cache usage,
6. asset dependency discovery from render-layer definitions and world visual stacks,
7. hot reload or dynamic reload behavior,
8. repeated handle creation or repeated asset lookup work,
9. material instance explosion,
10. content adapters that should be collapsed into more generic cheaper paths.

### 8. Architecture and code health issues that block optimization

Find code-organization or design problems that make render optimization harder, including:

1. monolithic files or plugins mixing unrelated concerns,
2. render logic spread across multiple places with unclear ownership,
3. hardcoded content-specific branches in supposedly generic rendering paths,
4. hacks that preserve duplicate visual systems instead of removing one,
5. temporary or fallback paths still active in runtime,
6. docs/code divergence that hides the intended fast path,
7. missing telemetry/hook points needed to profile or budget rendering properly.

## Specific Things To Confirm Or Refute

Be explicit about whether each of the following appears true in this codebase:

1. The game is GPU-bound in normal gameplay.
2. The game is CPU-bound on the client in normal gameplay.
3. The game is bottlenecked by ECS scheduling/query work more than actual draw submission.
4. The game is bottlenecked by replication/update churn more than rendering itself.
5. The game feels slow mainly because of frame pacing/interpolation issues rather than raw frame time.
6. Shader/material diversity is defeating batching enough to matter.
7. Too many fullscreen or post-process passes are active for the current visual payoff.
8. Off-screen or non-visible entities are still paying too much render-related cost.
9. Asset/shader compilation or loading hitching is a meaningful source of stalls.
10. Server-side visibility/replication behavior is causing client render instability or overload.
11. The current render-layer architecture is directionally correct and should be kept.
12. The current render-layer/material implementation has avoidable transitional cost that should be simplified.

If evidence is incomplete, say so and identify exactly what should be measured next.

## Additional Required Output

After the findings, provide all of the following:

### 1. End-to-end render flow map

Describe the active path for:

1. asset/bootstrap to client-ready rendering,
2. replicated entity arrival to visible draw,
3. camera-relative/world-layer transform derivation,
4. fullscreen background/foreground/post-process execution,
5. prediction/reconciliation/interpolation to final presented motion.

### 2. Performance budget map

Create a budget-oriented breakdown of likely hot paths, grouped by:

1. client CPU,
2. client GPU,
3. client main-thread stalls,
4. server tick cost that affects visual smoothness,
5. network/replication delivery cost that affects render churn.

This may be partly inferential, but each inference must be labeled as such.

### 3. Top remediation plan

Provide:

1. top 5 highest-ROI changes,
2. quick wins that are low risk,
3. medium-size refactors,
4. large architectural changes only if they are justified,
5. dependencies/order of operations,
6. what to measure before and after each major fix.

### 4. Instrumentation and profiling gaps

List the exact missing telemetry, diagnostics, counters, traces, or profiling hooks that should be added to confirm or reject the major hypotheses.

Include recommendations for:

1. frame-time instrumentation,
2. per-system timing,
3. render pass timing,
4. draw/material/entity counts,
5. asset/shader stall telemetry,
6. replication/visibility delivery metrics that correlate with client smoothness.

### 5. Runtime catalog appendix

Catalog the main runtime pieces that materially affect rendering performance, grouped by:

1. client runtime plugins/systems/resources,
2. replication server plugins/systems/resources,
3. gateway/bootstrap/asset-delivery pieces,
4. shared gameplay/render-support crates and modules.

Mark each item as:

1. active runtime,
2. debug/diagnostic,
3. tooling-only,
4. transitional/migration code,
5. likely removable.

## Output Requirements

- Produce the results as a formal audit report.
- Make the report specific to this repository.
- Prioritize findings by severity:
  - Critical
  - High
  - Medium
  - Low
- For each finding include:
  - title
  - severity
  - confidence level (`proven`, `strong inference`, or `weak inference`)
  - why it matters
  - whether the main impact is:
    - frame pacing
    - client CPU
    - client GPU
    - memory/bandwidth
    - startup hitching
    - replication churn
    - architecture/maintainability
  - exact file/path references
  - concrete recommendation
  - expected payoff
  - risk/complexity of fixing
  - whether it is:
    - must fix
    - should fix
    - optional improvement
- Distinguish carefully between:
  - actual measured/observable issue,
  - likely issue inferred from code shape,
  - possible issue that needs instrumentation.
- Call out places where docs and code diverge.
- Call out places where the current architecture is correct and should be preserved.
- Do not give shallow generalized performance advice.
- Do not stop at client rendering if the real issue is upstream.
- Be blunt and technically rigorous.

## Suggested Structure

1. Executive Summary
2. What Most Likely Makes The Game Feel Slow
3. Critical Findings
4. Client Render Pipeline Findings
5. ECS / Schedule / Transform Findings
6. Asset / Shader / Material Findings
7. Lightyear / Replication / Visibility Findings
8. Server-Side Contributors To Render Slowness
9. Documentation / Architecture Divergence
10. End-to-End Render Flow Map
11. Performance Budget Map
12. Prioritized Remediation Plan
13. Instrumentation / Profiling Gaps
14. Runtime Catalog Appendix

## Deliverable

Write the final report to:

- `docs/reports/bevy_2d_rendering_optimization_audit_report_YYYY-MM-DD.md`

Do not write the completed report anywhere else. The final report belongs in `docs/reports/`.

Also include at the top of the report:

- report date,
- prompt source path,
- short statement of audit scope,
- any limitations (for example: no runtime profile capture available, inference-only on GPU cost, no live packet traces, etc.).
