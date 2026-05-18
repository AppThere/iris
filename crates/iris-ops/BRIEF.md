## `iris-ops` — Undo/redo engine

**Gate:** None. Implement first.

### Phase 1 milestone

A working in-memory undo/redo stack with typed `Op` entries covering pixel and layer-tree
mutations. No Loro integration yet (Loro is Phase 5). The op log is a `Vec<Op>` with a
configurable depth limit.

### Public API at milestone completion

```rust
// crate root re-exports
pub use op::{Op, LayerOp, TileOp};
pub use stack::{UndoStack, UndoError};

// op.rs
pub enum Op {
    Layer(LayerOp),
    Tile(TileOp),
}

pub enum LayerOp {
    Add    { parent: Uuid, position: usize, snapshot: LayerSnapshot },
    Remove { layer_id: Uuid, snapshot: LayerSnapshot },
    Move   { layer_id: Uuid, new_parent: Uuid, new_position: usize,
             old_parent: Uuid, old_position: usize },
    SetProp { layer_id: Uuid, prop: LayerProp, before: PropValue, after: PropValue },
}

pub enum TileOp {
    Paint { layer_id: Uuid, tile: TileCoord, before: TileSnapshot, after: TileSnapshot },
}

// LayerSnapshot is a compact serialised form of a Layer's metadata,
// sufficient to reconstruct the layer on undo. NOT the full pixel data.
pub struct LayerSnapshot { /* opaque */ }

pub struct TileCoord { pub tx: u32, pub ty: u32 }

// Opaque snapshot of a single tile's pixel data for undo purposes.
// Stored as LZ4-compressed raw f16 RGBA bytes.
pub struct TileSnapshot { /* opaque */ }

// stack.rs
pub struct UndoStack {
    // depth: number of undoable ops. Default 200.
}
impl UndoStack {
    pub fn new(depth: usize) -> Self;
    pub fn push(&mut self, op: Op);
    pub fn undo(&mut self) -> Option<Op>;
    pub fn redo(&mut self) -> Option<Op>;
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;
    pub fn clear(&mut self);
}

#[derive(Debug, thiserror::Error)]
pub enum UndoError {
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("nothing to redo")]
    NothingToRedo,
}
```

### Do not implement yet

- Loro CRDT integration (Phase 5)
- Op log serialisation to `ops.bin` (Phase 1 milestone 2, after `iris-aif` is ready)
- Collaboration transport (Phase 5)

### Test requirements

- `UndoStack::push` → `undo` → verify `Op` returned is the pushed op
- `undo` when empty returns `None`
- `redo` after `undo` returns the op
- Depth limit: pushing 201 ops with depth=200 drops the oldest; verify stack length = 200
- `TileSnapshot` round-trips: compress → decompress → bytes equal original
