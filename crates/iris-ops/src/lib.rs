// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Undo/redo engine and CRDT op log for AppThere Iris.
//!
//! See ADR/004-crdt-op-log.md before implementing.
//!
//! **Phase 1:** in-memory undo/redo stack with typed [`Op`] entries.
//! **Phase 5:** Loro CRDT integration and `ops.bin` serialisation.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod op;
mod snapshot;
mod stack;

pub use op::{LayerOp, LayerProp, LayerSnapshot, Op, PropValue, TileCoord, TileOp};
pub use snapshot::{OpsError, TileSnapshot};
pub use stack::{UndoError, UndoStack};
