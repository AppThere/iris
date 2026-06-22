// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! In-memory undo/redo stack with a configurable depth limit.

use std::collections::VecDeque;

use crate::op::Op;

/// In-memory undo/redo stack with a configurable depth limit.
///
/// Phase 1 uses a plain pair of [`VecDeque`]s. Loro CRDT integration is Phase 5;
/// see ADR/004-crdt-op-log.md.
// TODO(iris): ADR/004-crdt-op-log.md — Phase 5: wrap the undo deque in a LoroDoc
// so ops are persisted to ops.bin and available for collaborative sync.
#[derive(Debug, Clone)]
pub struct UndoStack {
    undo: VecDeque<Op>,
    redo: VecDeque<Op>,
    depth: usize,
}

impl UndoStack {
    /// Create a new stack with the given maximum undo depth.
    ///
    /// `depth` is the maximum number of ops that can be undone at any one time.
    /// ADR-004 specifies a UI default of 200; pass a different value to override.
    /// A `depth` of 0 means no history is retained.
    pub fn new(depth: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            depth,
        }
    }

    /// Record a new operation.
    ///
    /// Clears the redo stack (branching history is not supported in Phase 1)
    /// and trims the undo stack to the configured depth by discarding the
    /// oldest entry when the limit is exceeded.
    pub fn push(&mut self, op: Op) {
        self.redo.clear();
        self.undo.push_back(op);
        if self.undo.len() > self.depth {
            self.undo.pop_front();
        }
    }

    /// Undo the most recent operation.
    ///
    /// Returns the [`Op`] that was undone so the caller can apply its `before`
    /// state, or `None` if the undo stack is empty.
    /// The op is moved onto the redo stack so [`Self::redo`] can restore it.
    pub fn undo(&mut self) -> Option<Op> {
        let op = self.undo.pop_back()?;
        self.redo.push_back(op.clone());
        Some(op)
    }

    /// Redo the most recently undone operation.
    ///
    /// Returns the [`Op`] to re-apply (caller uses its `after` state),
    /// or `None` if there is nothing to redo.
    /// The op is moved back onto the undo stack.
    pub fn redo(&mut self) -> Option<Op> {
        let op = self.redo.pop_back()?;
        self.undo.push_back(op.clone());
        Some(op)
    }

    /// Returns `true` if there is at least one op that can be undone.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Returns `true` if there is at least one op that can be redone.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Clear both the undo and redo stacks.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

/// Errors produced by the undo/redo engine.
///
/// Defined now for the Phase 5 collaboration layer. The Phase 1 [`UndoStack::undo`]
/// and [`UndoStack::redo`] methods return `Option` rather than `Result` because all
/// Phase 1 failure modes are expressed as `None` (empty stack).
// TODO(iris): ADR/004-crdt-op-log.md — Phase 5: surface these via the CRDT sync
// path when applying remote ops fails (e.g. version conflict, corrupt op log).
#[derive(Debug, thiserror::Error)]
pub enum UndoError {
    /// Undo was requested but the undo stack is empty.
    #[error("nothing to undo")]
    NothingToUndo,
    /// Redo was requested but the redo stack is empty.
    #[error("nothing to redo")]
    NothingToRedo,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::op::{LayerOp, LayerSnapshot, TileCoord, TileOp};
    use crate::snapshot::TileSnapshot;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn layer_op() -> Op {
        Op::Layer(LayerOp::Add {
            parent: None,
            position: 0,
            snapshot: LayerSnapshot::from_bytes(vec![1, 2, 3]),
        })
    }

    fn tile_op() -> Op {
        let raw = vec![0u8; 64];
        Op::Tile(TileOp::Paint {
            layer_id: Uuid::new_v4(),
            tile: TileCoord { tx: 0, ty: 0 },
            before: TileSnapshot::compress(&raw),
            after: TileSnapshot::compress(&raw),
        })
    }

    // ── BRIEF.md test requirements ────────────────────────────────────────────

    /// `push` → `undo` → the returned Op is the one that was pushed.
    #[test]
    fn push_then_undo_returns_same_op() {
        let mut stack = UndoStack::new(200);
        let op = layer_op();
        stack.push(op.clone());
        assert_eq!(stack.undo(), Some(op));
    }

    /// `undo` on an empty stack returns `None`.
    #[test]
    fn undo_empty_stack_returns_none() {
        let mut stack = UndoStack::new(200);
        assert!(stack.undo().is_none());
    }

    /// `redo` after `undo` returns the op that was undone.
    #[test]
    fn redo_after_undo_returns_op() {
        let mut stack = UndoStack::new(200);
        let op = tile_op();
        stack.push(op.clone());
        let undone = stack.undo().expect("undo must succeed after push");
        assert_eq!(undone, op);
        assert!(stack.can_redo());
        let redone = stack.redo().expect("redo must succeed after undo");
        assert_eq!(redone, op);
        assert!(!stack.can_redo());
        assert!(stack.can_undo());
    }

    /// Pushing 201 ops into a depth-200 stack leaves exactly 200 ops undoable.
    #[test]
    fn depth_limit_drops_oldest() {
        const DEPTH: usize = 200;
        let mut stack = UndoStack::new(DEPTH);
        for _ in 0..=DEPTH {
            // 201 pushes total
            stack.push(layer_op());
        }
        let mut count = 0usize;
        while stack.undo().is_some() {
            count += 1;
        }
        assert_eq!(count, DEPTH, "depth={DEPTH} stack must hold exactly {DEPTH} ops");
    }

    /// `push` clears the redo stack (no branching history).
    #[test]
    fn push_clears_redo_stack() {
        let mut stack = UndoStack::new(200);
        stack.push(layer_op());
        stack.undo();
        assert!(stack.can_redo(), "redo must be possible after undo");
        stack.push(layer_op());
        assert!(!stack.can_redo(), "push must clear the redo stack");
    }

    /// `clear` empties both the undo and redo stacks.
    #[test]
    fn clear_empties_both_stacks() {
        let mut stack = UndoStack::new(200);
        stack.push(layer_op());
        stack.push(layer_op());
        stack.undo();
        stack.clear();
        assert!(!stack.can_undo(), "undo stack must be empty after clear");
        assert!(!stack.can_redo(), "redo stack must be empty after clear");
    }
}
