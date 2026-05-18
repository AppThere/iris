# ADR 003 — Colour Pipeline

**Status:** Accepted  
**Date:** 2024-11-01  
**Deciders:** AppThere core team

## Context

A professional raster editor must handle:
- Multiple working colour spaces (sRGB, Display P3, ProPhoto RGB, CMYK)
- HDR content (f16/f32 linear light)
- ICC profile round-trips (PSD import, print soft-proofing)
- Accurate display output on monitors with non-sRGB profiles

Iris already has `appthere-color` published to crates.io, built on `moxcms`. However, that crate was built for Loki's needs (document text colour) and lacks several capabilities Iris requires.

## Decision

`appthere-color` is extended (not replaced) with a new `iris` feature flag covering:
- CMYK ↔ Lab ↔ XYZ via ICC profile LUT lookup
- Spot colour (Pantone name → Lab approximation)
- HDR tone mapping (Reinhard, ACES filmic)
- SIMD-optimised tile buffer conversion (`convert_tile_buffer` API)
- OpenEXR channel name → `ColourSpace` inference

All new APIs are pure Rust, `#![forbid(unsafe_code)]`, Apache-2.0. The SIMD paths use `std::simd` (stable in Rust ≥ 1.80 with the `portable-simd` feature).

The working colour space for all compositor operations is **Linear sRGB** (f16). All pixel data is converted to Linear sRGB before compositing and converted back to the layer's native colour space before writing to EXR.

## Consequences

- `appthere-color` requires a semver-minor bump (`0.1` → `0.2`) to add the `iris` feature. Loki is unaffected (it does not enable the `iris` feature).
- CMYK layers cannot be composited in the Phase 1 milestone — they require the `appthere-color` CMYK extension, which is Phase 4 work.
- The `cmyk-generic` colour space (SPEC.md §4.9) produces a visible warning overlay in the compositor to prevent silent colour errors.
