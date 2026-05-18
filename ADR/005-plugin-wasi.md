# ADR 005 — Plugin System (WASI Sandbox)

**Status:** Accepted  
**Date:** 2024-11-01  
**Deciders:** AppThere core team

## Context

Iris needs extensibility (custom filters, export formats, UI panels) without allowing plugins to destabilise the application or access the user's file system arbitrarily.

## Decision

`iris-plugin-api` defines a WASI sandbox using `wasmtime`. Plugins are compiled to WASM32-WASI and loaded at runtime. They receive pixel data and path data via shared memory segments (no direct GPU access, no file system access, no network access).

Plugin hook types (Phase 5):
- `Filter`: receives a pixel tile region, returns modified pixels
- `LayerEffect`: receives composited layer pixels, returns modified pixels
- `Import`: registers a file extension handler
- `Export`: registers an export format handler
- `Panel`: mounts a Dioxus component into a side panel slot

Plugin manifest is `plugin.toml` (see SPEC.md §9).

## Consequences

- Plugin system is Phase 5 only. `iris-plugin-api` exists as a stub crate in Phase 1–4 to establish the dependency boundary.
- The WASI sandbox means plugins cannot use GPU directly. Complex GPU-accelerated filters must be implemented as built-in Iris features, not plugins.
- The `Panel` hook produces a Dioxus component, which means plugin panels re-render through the normal Dioxus reactive system. This is intentional — plugins cannot break layout outside their panel slot.
