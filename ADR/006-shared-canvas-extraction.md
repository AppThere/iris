# ADR 006 — Shared Canvas Extraction from Loki

**Status:** Proposed  
**Date:** 2024-11-01  
**Deciders:** AppThere core team

## Context

Loki has solved several hard problems in integrating Vello/wgpu with Dioxus Native:
- The `CustomPaintSource` / `PaintSource` wgpu texture handoff
- Physical/logical pixel ratio mismatches
- Memory leaks in the renderer lifecycle
- Frame pacing and vsync alignment
- Dirty tile tracking and the render cache state machine

Iris needs all of these. Rebuilding them from scratch would re-pay the cost of debugging that was already done in Loki.

## Decision

Extract the generic canvas plumbing from `loki-vello` into a new `appthere-canvas` crate, publish it to crates.io, and update Loki to consume it. Iris then depends on `appthere-canvas` for the integration layer and adds only Iris-specific canvas logic in `iris-canvas`.

The split is defined in SPEC.md §6.1 (the extraction table). Summary:
- **Moves to `appthere-canvas`:** wgpu surface lifecycle, `CustomPaintSource` handoff, frame pacing, dirty tracking (generic), render cache state machine, input event normalisation
- **Stays in `loki-vello`:** page layout, paginated scroll model, paragraph render cache keys
- **New in `iris-canvas`:** `CanvasViewport` (pan/zoom/rotation), infinite scroll model, two-pass render architecture, WGSL blend mode shaders, tile compositor

## Gate condition

**`iris-canvas` must not be implemented until:**
1. `appthere-canvas` is extracted, published to crates.io at `>= 0.1.0`
2. Loki's CI passes with `appthere-canvas` as a dependency (proving the extraction is clean and no regressions)

This gate is tracked in the Loki ADR backlog (Loki ADR TBD). The status field of this ADR updates from `Proposed` to `Accepted` when the gate opens.

## Consequences

- Iris Phase 1 cannot begin `iris-canvas` implementation until the Loki extraction is complete. The workspace scaffold includes `iris-canvas` as a stub to maintain the dependency graph, but implementation is blocked.
- Any bug fix or improvement to the `CustomPaintSource` integration must be made in `appthere-canvas` and will benefit both Loki and Iris.
- `appthere-canvas` must be kept free of document semantics — it must not know about pages, layers, tiles, or paths.
