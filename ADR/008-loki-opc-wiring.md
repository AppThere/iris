# ADR 008 — loki-opc as OPC container layer for iris-aif

**Status:** Accepted  
**Date:** 2026-05-18  
**Deciders:** AppThere core team

## Context

iris-aif requires an OPC/ZIP container layer to read and write `.aif` files.
loki-opc is a generic OPC implementation in the AppThere ecosystem, designed
without Loki-specific document assumptions. It lives at `crates/loki-opc/`
inside the iris workspace tree (the same pattern as `crates/loki-file-access/`).

The CLAUDE.md gating rules table originally stated iris-aif was gated on
"appthere-opc published to crates.io at >= 0.1.0". This gate is superseded by
the decision to consume loki-opc directly via a path dependency, consistent with
how `loki-file-access` is consumed today.

An audit (session 2026-05-18) confirmed loki-opc is fully generic: no
Loki-specific content types, relationship types, or part URIs exist in the crate.
The only "loki" string in the public surface was one error message, which was
corrected as part of this wiring.

## Decision

iris-aif depends on loki-opc at path `crates/loki-opc/` with features
`["serde", "strict"]`. No package rename is used; code refers to `loki_opc::`.
The crate is excluded from workspace auto-discovery (exclude list in workspace
`Cargo.toml`).

The `strict` feature is enabled only in iris-aif's dep declaration, not in the
workspace-level dep, so Loki's loki-opc usage is unaffected.

All OPC part URI constants, content type strings, and relationship type URIs used
by iris-aif are isolated in `crates/iris-aif/src/parts.rs`. No OPC string
constants appear in `reader.rs`, `writer.rs`, `xml.rs`, or `tile.rs`.

Per-part ZIP compression is controlled via a callback closure passed to
`Package::write()`. The rule is: `.exr` parts use `Stored`; all others use
`Deflated`. This rule is implemented in `writer.rs`, not in `parts.rs`.

## Consequences

- `iris-aif/Cargo.toml` depends on loki-opc with `serde` + `strict` features.
- loki-opc is excluded from workspace membership; bugs are fixed upstream and
  cherry-picked into `crates/loki-opc/` as needed.
- When loki-opc is eventually published to crates.io, the path dep in workspace
  `Cargo.toml` becomes a version dep; no other iris-aif files change.
- License: loki-opc is MIT; iris-aif is Apache-2.0. MIT is compatible as a
  dependency. No action required until crates.io publication.

## Supersedes

The gating rule in CLAUDE.md ("iris-aif gated on appthere-opc published to
crates.io") is superseded by this ADR. The gate is considered open.
CLAUDE.md gating rules table has been updated accordingly.
