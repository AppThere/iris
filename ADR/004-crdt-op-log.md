# ADR 004 — CRDT Op Log

**Status:** Accepted  
**Date:** 2024-11-01  
**Deciders:** AppThere core team

## Context

Iris needs:
- Unlimited undo/redo (no state snapshots — too large for image data)
- Real-time collaborative editing (Phase 5)
- Audit history for shared documents

Loki uses Loro for CRDT-based collaborative editing in its document model. The same decision applies to Iris, with different data types.

## Decision

`iris-ops` owns the op log. It wraps Loro's `LoroDoc` and exposes a typed `Op` enum covering all document mutations. The Loro doc is snapshotted to `history/ops.bin` on save using the header format specified in SPEC.md §4.11.

Op log failure is **recoverable**: if `ops.bin` is corrupt or uses an unsupported version, the document opens without history and the user is warned. Pixel and vector data are always stored independently of the op log and must be readable without it.

**Pixel tile writes** use last-write-wins per tile (not CRDT merge). Concurrent collaborative edits to the same 256×256 tile are resolved by timestamp. This is the correct trade-off: pixel painting is inherently spatial and two collaborative users painting the same pixel simultaneously is an edge case, not a normal workflow.

**Layer tree mutations** (add, remove, move, rename) use Loro's List CRDT, which handles concurrent structural edits correctly (e.g., two users simultaneously adding a layer get both layers, in a deterministic order).

## Consequences

- `iris-ops` must not expose Loro types at its public API boundary. Callers work with the `Op` enum and the `UndoStack` struct; Loro is an implementation detail.
- Phase 1 implements undo/redo only (no collaboration). The CRDT machinery is present but the sync transport is not wired until Phase 5.
- The 200-operation undo depth limit is enforced in `UndoStack`. Op log entries beyond this depth are retained in `ops.bin` for collaboration and audit but are not surfaced in the undo UI.
