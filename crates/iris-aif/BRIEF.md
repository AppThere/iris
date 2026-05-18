## `iris-aif` — Artisan Interchange Format read/write

**Gate:** `appthere-opc >= 0.1.0` published; `iris-pixel` milestone complete.

### Phase 1 milestone

Write and read a minimal valid AIF file containing:
- `document.xml` with a single pixel-mode canvas, one artboard, and a flat layer tree
- `metadata.xml`
- `iris/layers/{id}/meta.xml` for each layer
- EXR tiles for pixel layers (8bpc u8, RGBA, ZIP compression)
- `preview.png`
- All content types registered in `[Content_Types].xml`

Round-trip test: write a `LayerTree` → `.aif` → read → compare layer tree and tile data.

### Public API at milestone completion

```rust
pub use reader::{AifReader, AifError};
pub use writer::{AifWriter, WriteOptions};
pub use document::{AifDocument, AifCanvas, AifArtboard};

pub struct AifDocument {
    pub canvas: AifCanvas,
    pub artboards: Vec<AifArtboard>,
    pub layers: iris_pixel::LayerTree,
}

pub struct AifCanvas {
    pub mode: CanvasMode,
    pub width_px: u32,
    pub height_px: u32,
    pub dpi_x: f32,
    pub dpi_y: f32,
    pub working_color_space: String,
    pub bit_depth: iris_pixel::BitDepth,
}

pub enum CanvasMode { Pixel, Vector, Mixed }

pub struct AifArtboard {
    pub id: uuid::Uuid,
    pub name: String,
    pub x_px: i32,
    pub y_px: i32,
    pub width_px: u32,
    pub height_px: u32,
}

// reader.rs
pub struct AifReader;
impl AifReader {
    pub fn open(path: &std::path::Path) -> Result<AifDocument, AifError>;
    // Internal: open from an AsyncReadWrite trait object (for mobile)
    pub fn from_reader<R: std::io::Read + std::io::Seek>(r: R)
        -> Result<AifDocument, AifError>;
}

// writer.rs
pub struct WriteOptions {
    pub compression: iris_pixel::ExrCompression,
    pub omit_op_log: bool,  // true = write without history (export path)
}
pub struct AifWriter;
impl AifWriter {
    pub fn write(doc: &AifDocument, path: &std::path::Path, opts: WriteOptions)
        -> Result<(), AifError>;
    pub fn to_writer<W: std::io::Write + std::io::Seek>(
        doc: &AifDocument, w: W, opts: WriteOptions
    ) -> Result<(), AifError>;
}

// Full AifError taxonomy — all variants as specified in SPEC.md §4.15
pub use error::AifError;
```

### Do not implement yet

- Op log read/write (Phase 1 milestone 2 — requires `iris-ops` Loro integration)
- Vector path store (`paths.bin`) read/write (Phase 3)
- Asset parts: brushes, patterns (Phase 4)
- ICC profile embedding beyond well-known colour spaces (Phase 4)
- Mobile `AsyncReadWrite` platform abstraction (Phase 1 mobile milestone)

### Test requirements

- Every `AifError` variant has a fixture file that triggers it
- Round-trip: write minimal document → read → canvas dimensions, DPI, layer count match
- Round-trip: write layer with non-zero tile data → read → tile bytes match
- Sparse tile: write layer with fully-transparent tile → `.aif` must not contain that tile part
- Edge tile: layer whose dimensions are not multiples of 256 → tiles written as full 256×256
- EXR metadata validation: `aif:layerId` in tile header matches directory UUID
- `formatVersion` check: fabricate a file with `formatVersion="99.0"` → `UnsupportedMajorVersion`
- Missing required part: fabricate a `.aif` with no `document.xml` → `MissingRequiredPart`
