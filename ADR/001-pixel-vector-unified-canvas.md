# ADR 001 — Pixel + Vector Unified Canvas

**Status:** Accepted  
**Date:** 2024-11-01  
**Deciders:** AppThere core team

## Context

Every major creative suite in existence (Adobe, Affinity, Canva) ships raster editing and vector editing as separate applications or strictly separated modes with no shared layer stack. This forces users to export/import when a design needs both pixel and vector work in the same document.

Iris has an opportunity to be the first serious open-source tool to treat pixel layers and vector layers as first-class peers in a single unified layer tree.

## Decision

Iris uses a single `Layer` enum (`LayerContent`) that accommodates both pixel tile data and vector path graphs. The canvas renders both using the same Vello/wgpu pipeline in a single composite pass. The document model (`iris-pixel` and `iris-vector`) share a common `Layer` struct; only the `content` field differs.

The UI exposes two toolbar modes (Pixel / Vector) that change which tools are active, but the layer tree, blend modes, masks, and effects apply equally to all layer types.

## Consequences

**Positive:**
- Users can work with pixel and vector content in the same document without round-tripping through export.
- The AIF format naturally stores both data types; no "convert to smart object" step is needed.
- Blend modes, masks, and layer effects work identically on both layer types.

**Negative:**
- The compositor is more complex: it must interleave pixel tile compositing with Vello vector rasterisation in the correct order for each frame.
- PSD export must rasterise vector layers (documented limitation).
- The `iris-canvas` render pass is more complex than a pure-pixel compositor.

**Mitigation:** The two-pass render architecture (Composite pass → Overlay pass) keeps pixel and vector rendering separable at the frame level while still compositing them in correct layer order.
