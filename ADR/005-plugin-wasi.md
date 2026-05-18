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

## Platform-tiered plugin strategy

The plugin system operates differently per platform due to App Store policy
constraints on dynamic code execution.

**Desktop (Windows, macOS, Linux) and Android:**
Full `wasmtime` with Cranelift JIT. Maximum performance. No restrictions.
Plugins load from local files or a future Iris plugin registry.
macOS App Store distribution requires the `com.apple.security.cs.allow-jit`
entitlement in the app's entitlements.plist; direct Developer ID distribution
does not require it.

**iOS — Phase 5, Option B first (Option A later):**

*Option B (ship first):* `iris-plugin-api` is excluded from iOS builds via
`#[cfg(not(target_os = "ios"))]`. The iOS app is fully featured for all
built-in tools. User-loadable third-party plugins are not available on iOS.
App Store risk: zero. Precedent: Photoshop on iPad has no third-party plugins.

*Option A (future):* Replace `wasmtime` with its Pulley interpreter backend
(no JIT, no executable memory pages). Plugins are sandboxed identically to
desktop but run slower. Suitable for filter plugins on still images; not
suitable for real-time brush plugins. Transition from B to A requires:
  1. Pulley's iOS track record is established (community validation)
  2. A performance benchmark confirms acceptable filter latency on A17 Pro or later
  3. A test submission to App Store review with a minimal WASM plugin

All iOS-conditional plugin code is annotated `// COMPAT(mobile): ADR-005`.

## Consequences

- Plugin system is Phase 5 only. `iris-plugin-api` exists as a stub crate
  in Phase 1–4 to establish the dependency boundary.
- The WASI sandbox means plugins cannot use GPU directly. Complex
  GPU-accelerated filters must be implemented as built-in Iris features.
- The `Panel` hook produces a Dioxus component that re-renders through the
  normal Dioxus reactive system. Plugins cannot break layout outside their slot.
- iOS ships without user-loadable plugins initially (Option B). The cfg gate
  is in place from day one so no retrofit is needed when Option A is ready.
