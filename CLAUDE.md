# CLAUDE.md — AppThere Iris

This file is the authoritative operating guide for Claude Code working in this repository.
Read it in full before touching any file. Treat every rule here as non-negotiable.

---

## Project identity

AppThere Iris is a unified raster and vector creative suite (Photoshop + Illustrator analog)
built in pure Rust. It is part of the AppThere ecosystem alongside Loki (word processor),
Midgard (IDE), and Jotun (3D filmmaking).

Primary references (read before implementing any crate):
- `SPEC.md` — full product and architecture specification
- `ADR/` — Architecture Decision Records; one per major cross-crate decision
- `crates/{name}/BRIEF.md` — per-crate implementation brief and current milestone

---

## Non-negotiable coding rules

Violations of these rules will cause a prompt to be rejected and restarted from audit.

### File length
- **300-line hard ceiling per file.** When a file approaches 250 lines, split it into
  submodules immediately. Do not wait until it exceeds the limit.
- `mod.rs` files are capped at **100 lines** — they are indices, not implementations.

### Error handling
- **Typed error enums only.** Every crate exposes its own `Error` enum derived with
  `thiserror`. No `anyhow`, no `Box<dyn Error>`, no `String` errors at any public API boundary.
- **No `.unwrap()` or `.expect()`** anywhere in library code (`crates/*/src/`).
  Use `?` with typed errors or explicit match. The only permitted use of `.expect()` is in
  `main.rs` or `bin/*.rs` entrypoints for truly unrecoverable startup failures, and it must
  carry a descriptive message.
- **No `panic!()` in library code.** Use `Result` or `Option`.

### Unsafe code
- `#![forbid(unsafe_code)]` is present in every crate's `lib.rs` except `iris-gpu`.
  Do not remove it. Do not add `unsafe` blocks without an ADR.

### Unwanted imports
- No `use std::collections::HashMap` without a comment explaining why `BTreeMap` was rejected.
  BTreeMap is preferred for deterministic serialisation order.
- No `eprintln!` or `println!` in library code. Use `tracing::warn!`, `tracing::error!`, etc.

### Annotations (mandatory, not optional)
- `// COMPAT(adobe):` — any code path that exists to handle a quirk or undocumented
  behaviour in PSD, PSB, or AI files.
- `// COMPAT(krita):` / `// COMPAT(inkscape):` — quirks in ORA or SVG files from
  those applications.
- `// COMPAT(mobile):` — any `#[cfg(...)]` or runtime branch that differs between
  desktop and iOS/Android.
- `// TODO(iris): <tracker-ref>` — known gap, not yet implemented. Every TODO must
  reference a specific section of SPEC.md or an ADR.
- `// AUDIT: <finding>` — placed during audit-only passes. Never removed during the
  same session; removal is part of the implement pass.
- `// SAFETY:` — required immediately above any `unsafe` block (only in `iris-gpu`).

### License headers
Every `.rs` file begins with:
```rust
// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0
```

### Imports / visibility
- All public types that cross a crate boundary must be re-exported from the crate root
  (`lib.rs`) with a `pub use` statement. Callers import from the crate root, not from
  internal module paths.
- `pub(crate)` for items shared within a crate but not exposed externally.
- No `pub mod` for modules that contain only private implementation detail — use
  `mod` (private) and re-export selectively.

---

## Workflow: audit-first, then implement

Every Claude Code session follows this two-pass structure. Running both passes in the
same prompt context risks API timeout on large crates.

### Pass 1 — Audit only
Prompt template:
```
AUDIT ONLY — do not write any implementation code.
Crate: <crate-name>
Milestone: <milestone from BRIEF.md>

1. Read CLAUDE.md, SPEC.md §<relevant sections>, ADR/<relevant ADRs>, crates/<name>/BRIEF.md
2. List every file that will need to be created or modified, with a one-line description.
3. For each file, list the public types and functions it will expose.
4. Flag any spec ambiguities, missing dependencies, or rule conflicts.
5. Produce a line-count estimate for each file.
Output: an audit report only. No code.
```

### Pass 2 — Implement only
Prompt template:
```
IMPLEMENT — based on the audit report above.
Do not re-read files already loaded in context unless the audit flagged an ambiguity
that requires re-reading.

Rules:
- cargo check --workspace must pass after every file you write.
- Do not exceed 300 lines per file.
- Add // AUDIT: comments for anything deferred.
- Write tests in the same file as the implementation (unit tests in a #[cfg(test)] block).
```

---

## cargo check discipline

Run `cargo check --workspace` after every file written. Do not batch multiple files
and check at the end. If `cargo check` fails, fix it before writing the next file.

Run `cargo test --workspace` at the end of each milestone, not during individual file writes.

---

## Crate dependency rules

These rules prevent circular dependencies and keep the layered architecture intact.

```
iris-app
  └── iris-canvas, iris-pixel, iris-vector, iris-ops, iris-aif, iris-plugin-api

iris-canvas
  └── appthere-canvas, iris-pixel, iris-vector

iris-pixel
  └── appthere-color, iris-ops

iris-vector
  └── appthere-color, iris-ops, kurbo

iris-aif
  └── appthere-opc, iris-pixel, iris-vector, iris-ops

iris-ops
  └── loro, uuid

iris-psd, iris-ora, iris-ai, iris-svg
  └── iris-pixel, iris-vector, iris-aif (read/write only — no canvas deps)

iris-plugin-api
  └── iris-pixel, iris-vector (API surface only — no canvas, no aif)
```

**Forbidden dependencies (will cause a build reject):**
- `iris-aif` must NOT depend on `iris-canvas`
- `iris-pixel` must NOT depend on `iris-canvas`
- `iris-vector` must NOT depend on `iris-canvas`
- Any format adapter (`iris-psd`, `iris-ora`, etc.) must NOT depend on `iris-canvas`
- `iris-ops` must NOT depend on any `iris-*` crate except its own utilities

---

## Gating rules (do not start gated work)

The following crates must not be implemented until their gate condition is met.
Check `ADR/006-shared-canvas-extraction.md` for current status.

| Crate | Gate condition |
|---|---|
| `iris-aif` | loki-opc path dep at `crates/loki-opc/` — GATE OPEN (ADR 008) |
| `iris-canvas` | appthere-canvas at crates/appthere-canvas/ — GATE OPEN (ADR 006) |

Until gates are open, these crates exist as stubs only (`lib.rs` with a `// TODO` comment).

---

## Test requirements

- Every public function in a library crate must have at least one unit test.
- Every `AifError` variant must have a test that triggers it from a malformed fixture file.
- Fixture files for `iris-aif` tests live in `crates/iris-aif/tests/fixtures/`.
- Format round-trip tests (write → read → compare) are mandatory for `iris-aif` before
  any milestone is considered complete.
- Property-based tests (`proptest` crate) are encouraged for path math in `iris-vector`
  and tile coordinate arithmetic in `iris-pixel`.

---

## What Claude Code must never do

- **Never modify `SPEC.md` or any `ADR/*.md`** during an implement pass. If the spec
  appears wrong, flag it in an `// AUDIT:` comment and stop. Spec changes require a
  separate human-reviewed session.
- **Never add a dependency not listed in SPEC.md §14 or approved in an ADR** without
  flagging it in the audit report first.
- **Never implement Phase 2+ features during Phase 1 milestones.** The phased delivery
  in SPEC.md §13 is a constraint, not a suggestion.
- **Never silence a compiler warning with `#[allow(...)]`** without a comment explaining
  why the warning is a false positive in this specific context.
- **Never generate placeholder data** (hardcoded pixel values, fake UUIDs, synthetic
  layer trees) in production code paths. Test fixtures only.
- **Never commit a file where `cargo check` fails.**
