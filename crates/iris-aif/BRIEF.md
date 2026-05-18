## `iris-aif` — Artisan Interchange Format read/write

**Gate:** OPEN — loki-opc path dep at `crates/loki-opc/` satisfies the OPC container
requirement (ADR 008). `iris-pixel` milestone must still be complete before
implementing the full layer-tree round-trip test.

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

## OPC container

iris-aif uses loki-opc (at `crates/loki-opc/`) for all OPC/ZIP container operations.
The crate is depended on with features `["serde", "strict"]`. Public API surface
relevant to iris-aif:

```rust
Package::new()                             // create empty container
Package::open(impl Read + Seek)            // open existing .aif
Package::write(w, compression_fn)          // write with per-part compression
Package::set_part(PartName, PartData)      // add/replace a part
Package::part(&PartName)                   // read a part by URI
Package::part_names()                      // enumerate all parts
Package::content_type_map_mut()            // register MIME types
Package::relationships_mut()               // write root relationships
CompressionMethod                          // Stored | Deflated (re-exported)
```

**Per-part compression rule** (enforced in `writer.rs`, never in `parts.rs`):

- EXR tile parts (`.exr`) → `CompressionMethod::Stored`
- All other parts → `CompressionMethod::Deflated`

All OPC part URI string constants live in `src/parts.rs` exclusively. No part URI
strings may be hardcoded in `reader.rs`, `writer.rs`, `xml.rs`, or `tile.rs`.
This isolates the OPC vocabulary so a future backend swap requires only `parts.rs`
changes.

iris-aif enables the `strict` loki-opc feature for reading third-party OPC files.
Deviation warnings from first-party `.aif` reads are logged via `tracing::warn!`
and do not return errors.

## File access

`iris-aif` uses `appthere-file-access` (package: `loki-file-access`) for all
file I/O on all platforms.

API shape (from PROMPT 2C audit + PROMPT 2D additions):

- `FilePicker` (zero-size struct) with async `pick_file_to_open` / `pick_file_to_save`
- `FileAccessToken` with:
  - `open_read() -> Box<dyn ReadSeek>` — streaming, seekable
  - `open_write_truncate() -> Box<dyn WriteSeek>` — overwrites from byte 0 (added PROMPT 2D)
  - `serialize() / deserialize()` — URL-safe base64 for recent-files storage
- No public platform trait — dispatch is `cfg`-gated inside the crate

`AifReader::open` accepts:
- `&FileAccessToken` on all platforms (primary API)
- `&std::path::Path` on desktop only (convenience; returns `AifError::PathAccessDenied` on mobile)

`AifWriter::write` accepts the same two variants.  
Tests always use the `&Path` variant (desktop host; no picker needed in tests).

Decisions encoded in PROMPT 2D:
- Desktop overwrites use `open_write_truncate()` — not `open_write()`
- iOS and WASM writes return `AccessError::Platform` (stub) until fully implemented
- WASM read is functional (in-memory `Cursor`); WASM write is not supported yet
- Android read and write are functional via SAF + `ContentResolver`

### Do not implement yet

- Op log read/write (Phase 1 milestone 2 — requires `iris-ops` Loro integration)
- Vector path store (`paths.bin`) read/write (Phase 3)
- Asset parts: brushes, patterns (Phase 4)
- ICC profile embedding beyond well-known colour spaces (Phase 4)

### Test requirements

- Every `AifError` variant has a fixture file that triggers it
- Round-trip: write minimal document → read → canvas dimensions, DPI, layer count match
- Round-trip: write layer with non-zero tile data → read → tile bytes match
- Sparse tile: write layer with fully-transparent tile → `.aif` must not contain that tile part
- Edge tile: layer whose dimensions are not multiples of 256 → tiles written as full 256×256
- EXR metadata validation: `aif:layerId` in tile header matches directory UUID
- `formatVersion` check: fabricate a file with `formatVersion="99.0"` → `UnsupportedMajorVersion`
- Missing required part: fabricate a `.aif` with no `document.xml` → `MissingRequiredPart`
