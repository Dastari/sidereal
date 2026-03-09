# Logout Presence Policy

Status: Proposed decision detail (`DR-0003`)  
Date: 2026-02-24

## Purpose

Define what happens to character entities when a session disconnects/logs out.

## Problem Statement

Logout behavior affects:
- combat logging and exploit prevention,
- persistence load and simulation cost,
- AI crew/offline command gameplay,
- player risk expectations and economy balance.

## Candidate Policies

1. Despawn on logout:
   - simplest runtime model,
   - weakest continuity.
2. Always persist in world:
   - strongest continuity,
   - highest complexity and exploit surface.
3. Conditional persist:
   - docked: protected persisted state,
   - undocked: bounded offline behavior (for example autopilot/AI fallback) with explicit vulnerability rules.

## Current Recommendation Direction (Not accepted yet)

Conditional persist is the leading candidate for v1 planning because it balances continuity and operational risk.

## Decision Gates (must be answered before acceptance)

1. Attackability:
   - always vulnerable, grace window, or location/state dependent?
2. Offline control:
   - no AI, limited AI, or full AI crew authority?
3. Reconnect arbitration:
   - how client reclaims control from offline behavior.
4. Anti-exploit rules:
   - combat logging, rapid dock/undock abuse, disconnect abuse.
5. Persistence model:
   - cadence and authority boundaries while offline.

## Non-Negotiable Constraints

- Authority remains server-side.
- Identity and ownership validation remains fail-closed.
- Policy must be testable with explicit resilience and exploit cases.

## Follow-Up Work After Acceptance

- Update `docs/sidereal_design_document.md`.
- Add implementation contract docs for offline AI behavior if applicable.
- Add topology/resilience tests for accepted logout semantics.

## References

- `docs/decision_register.md` (`DR-0003`)
- `docs/plans/test_topology_and_resilience_plan.md`
