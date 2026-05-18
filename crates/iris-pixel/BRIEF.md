## `iris-pixel` — Raster document model

**Gate:** `iris-ops` milestone complete.

### Phase 1 milestone

An in-memory layer tree with pixel tile storage. Layers can be added, removed, reordered.
Pixel data can be written to and read from tiles. No file I/O yet (that's `iris-aif`).

### Public API at milestone completion

```rust
pub use layer::{Layer, LayerId, LayerContent, LayerProp, PropValue};
pub use pixel_layer::{PixelLayer, ChannelLayout, BitDepth};
pub use tile::{TileCoord, TileData, TileCache};
pub use tree::{LayerTree, LayerTreeError};
pub use blend::BlendMode;
pub use mask::LayerMask;

// layer.rs
pub type LayerId = uuid::Uuid;

pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,        // 0.0–1.0
    pub blend_mode: BlendMode,
    pub clipping_mask: bool,
    pub mask: Option<LayerMask>,
    pub content: LayerContent,
}

pub enum LayerContent {
    Pixel(PixelLayer),
    Group { children: Vec<LayerId> },
    // Vector, Text, etc. — stubs only in Phase 1
}

// pixel_layer.rs
pub struct PixelLayer {
    pub channel_layout: ChannelLayout,
    pub bit_depth: BitDepth,
    pub color_space: ColorSpaceId,   // string identifier from SPEC §4.9
    pub compression: ExrCompression,
    pub canvas_offset_x: i32,
    pub canvas_offset_y: i32,
    pub crop_bounds: Option<CropBounds>,
    pub tiles: TileCache,
}

pub enum ChannelLayout { Rgba, Rgb, La, L, /* CMYK stubs — not wired in Phase 1 */ }
pub enum BitDepth { U8, U16, F16, F32 }
pub enum ExrCompression { Zip, Zips, Piz, Dwab, Dwaa }

// tile.rs
pub struct TileCoord { pub tx: u32, pub ty: u32 }

// Raw f16 RGBA pixel data for one tile (256×256 × 4 channels × 2 bytes = 524,288 bytes)
pub struct TileData(pub Box<[u8]>);

impl TileData {
    pub fn transparent(tile_size: u32) -> Self;
    pub fn is_fully_transparent(&self) -> bool;
}

pub struct TileCache { /* LRU map of TileCoord → TileData */ }
impl TileCache {
    pub fn get(&self, coord: TileCoord) -> Option<&TileData>;
    pub fn insert(&mut self, coord: TileCoord, data: TileData);
    pub fn remove(&mut self, coord: TileCoord) -> Option<TileData>;
    pub fn dirty_coords(&self) -> impl Iterator<Item = TileCoord> + '_;
    pub fn mark_clean(&mut self, coord: TileCoord);
}

// tree.rs
pub struct LayerTree {
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub dpi_x: f32,
    pub dpi_y: f32,
}
impl LayerTree {
    pub fn new(canvas_width: u32, canvas_height: u32, dpi_x: f32, dpi_y: f32) -> Self;
    pub fn root_layer_ids(&self) -> &[LayerId];
    pub fn get(&self, id: LayerId) -> Option<&Layer>;
    pub fn get_mut(&mut self, id: LayerId) -> Option<&mut Layer>;
    pub fn add_layer(&mut self, parent: Option<LayerId>, position: usize, layer: Layer)
        -> Result<LayerId, LayerTreeError>;
    pub fn remove_layer(&mut self, id: LayerId) -> Result<Layer, LayerTreeError>;
    pub fn move_layer(&mut self, id: LayerId, new_parent: Option<LayerId>, new_position: usize)
        -> Result<(), LayerTreeError>;
    pub fn iter_depth_first(&self) -> impl Iterator<Item = &Layer> + '_;
}

#[derive(Debug, thiserror::Error)]
pub enum LayerTreeError {
    #[error("layer {0} not found")]
    NotFound(LayerId),
    #[error("cannot move layer {0} into its own descendant")]
    CircularMove(LayerId),
    #[error("position {0} out of range")]
    PositionOutOfRange(usize),
}

// blend.rs — BlendMode enum with all 27 variants from SPEC §4.8
pub enum BlendMode { Normal, Dissolve, Darken, Multiply, /* ... all variants ... */ }
impl BlendMode {
    pub fn from_aif_str(s: &str) -> Option<Self>;
    pub fn to_aif_str(&self) -> &'static str;
}
```

### Do not implement yet

- Vector layer content (Phase 3)
- Text layer content (Phase 3)
- Adjustment/fill layers (Phase 4)
- Smart objects (Phase 4)
- CMYK channel layout (Phase 4)
- Layer effects (Phase 4)

### Test requirements

- `LayerTree::add_layer` → `get` round-trip
- `remove_layer` removes from parent's child list
- `move_layer` detects circular move (layer into its own descendant)
- `BlendMode::from_aif_str(s).unwrap().to_aif_str() == s` for every SPEC §4.8 identifier
- `TileData::is_fully_transparent` returns true for zero-alpha tile, false for non-zero
- `TileCache` LRU eviction: inserting beyond capacity drops oldest entry
