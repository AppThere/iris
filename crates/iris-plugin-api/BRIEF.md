# iris-plugin-api — Implementation Brief

See SPEC.md §9 and ADR/005-plugin-wasi.md before implementing.

## Current milestone

**Phase 1–4: Stub only. Phase 5: Full WASI implementation.**

Phase 1 goal: define the trait boundary and iOS cfg gate so the dependency
graph and platform-tiering are correct from the first commit. No wasmtime
dependency. No WASM loading.

## Phase 1 public API

See src/lib.rs — already implemented as part of the platform-tiering change.

## Do not implement yet

- wasmtime dependency (Phase 5)
- Plugin loading, sandboxing, hook dispatch (Phase 5)
- Plugin manifest (plugin.toml) parsing (Phase 5)
- Filter, LayerEffect, Import, Export, Panel hook types (Phase 5)

## Test requirements

Phase 1: one test only — iris_plugin_is_object_safe. Already in src/lib.rs.
