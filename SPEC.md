# AppThere Iris — Architecture & Product Specification

**Status:** Draft v0.1  
**License:** Apache-2.0  
**Maintainer:** AppThere Project  

---

## 1. Product Vision

AppThere Iris is a unified raster and vector creative suite targeting feature parity with Adobe Photoshop and Illustrator, built entirely in Rust using Dioxus Native 0.7, Vello, and wgpu. It is part of the AppThere open-source ecosystem alongside Loki (word processor), Midgard (IDE), and Jotun (3D filmmaking).

Iris's differentiating position:

- **One app, two modes.** Pixel editing and vector editing share a single canvas, layer stack, colour engine, and file format — no round-trip export between "Photoshop" and "Illustrator" documents.
- **Local-first, cloud-optional.** Documents are stored natively as `.aif` (Artisan Interchange Format) files. Real-time collaboration is opt-in via AppThere Cloud.
- **Open format.** Artisan Interchange Format is specified publicly and is OPC/ZIP-based, built on the shared `appthere-opc` container crate used by Loki.
- **Shared infrastructure.** The Vello/wgpu canvas integration layer (`appthere-canvas`) is extracted from Loki and reused, so Iris inherits the solved Dioxus Native / wgpu handoff without rebuilding it.
- **Non-destructive by default.** Every operation is recorded in a CRDT op log. There is no "flatten and you lose history" workflow.

---

## 2. Cargo Workspace Layout

```
iris/
├── Cargo.toml                  # workspace root
├── CLAUDE.md                   # coding conventions (mirrors Loki)
├── ADR/
│   ├── 001-pixel-vector-unified-canvas.md
│   ├── 002-aif-format.md
│   ├── 003-color-pipeline.md
│   ├── 004-crdt-op-log.md
│   ├── 005-plugin-wasi.md
│   ├── 006-shared-canvas-extraction.md
│   ├── 007-file-access-unsafe-exemption.md
│   └── 008-loki-opc-wiring.md
├── crates/
│   ├── iris-app/               # Dioxus Native shell
│   ├── iris-canvas/            # Iris scroll model + scene logic (builds on appthere-canvas)
│   ├── iris-pixel/             # Raster document model
│   ├── iris-vector/            # Vector document model
│   ├── iris-aif/               # Native format (builds on appthere-opc)
│   ├── iris-psd/               # Photoshop PSD adapter
│   ├── iris-ora/               # OpenRaster ORA adapter
│   ├── iris-ai/                # Adobe Illustrator AI adapter
│   ├── iris-svg/               # SVG adapter
│   ├── iris-ops/               # Undo/redo, CRDT, op log
│   └── iris-plugin-api/        # WASI plugin sandbox
└── tools/
    └── aif-inspect/            # CLI for inspecting .aif files
```

**In-workspace path dependencies (cloned into crates/):**

```
crates/loki-opc           # OPC/ZIP container — referenced as loki-opc path dep (ADR 008)
crates/loki-file-access   # File picker + permissions — referenced as appthere-file-access (ADR 007)
```

**External crates consumed from crates.io:**

```
appthere-canvas   # Vello/wgpu + Dioxus Native integration layer — extracted from Loki (ADR 006)
appthere-color    # ICC colour management — already published, extended for Iris
```

### Workspace `Cargo.toml` conventions

- All crates are `edition = "2021"`, `rust-version = "1.80"`
- `#![forbid(unsafe_code)]` in all library crates (see ADR 007 for loki-file-access exemption, ADR 008 notes loki-opc is MIT external dep)
- Typed error enums via `thiserror`; no `anyhow` in library crates
- No `.unwrap()` or `.expect()` in library code
- 300-line file ceiling; split into submodules aggressively
- `// COMPAT(adobe):` annotations on all quirk-handling code
- Apache-2.0 headers on every file

---

## 3. Document Model

### 3.1 Unified Layer Stack

Iris represents all content as a tree of `Layer` nodes shared between pixel and vector modes. This is the key architectural decision (ADR 001) that enables the unified canvas.

```rust
// iris-pixel/src/layer.rs
pub enum LayerContent {
    /// Raster pixel data (tiled u8/u16/f16 bitmap)
    Pixel(PixelLayer),
    /// Vector path graph
    Vector(VectorLayer),
    /// Vector text (rendered to Vello glyph runs)
    Text(TextLayer),
    /// Nested layer group (acts as a clip/blend scope)
    Group(GroupLayer),
    /// Embedded Iris sub-document (Smart Object equivalent)
    SmartObject(SmartObjectLayer),
    /// Adjustment layer (non-destructive filter)
    Adjustment(AdjustmentLayer),
    /// Fill layer (solid, gradient, pattern)
    Fill(FillLayer),
}

pub struct Layer {
    pub id: LayerId,       // stable UUID
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,      // 0.0–1.0
    pub blend_mode: BlendMode,
    pub mask: Option<LayerMask>,
    pub effects: Vec<LayerEffect>,
    pub content: LayerContent,
    pub children: Vec<LayerId>, // for Group nodes
}
```

### 3.2 Pixel Layer (`iris-pixel`)

Stores raster data as a tile cache to keep memory bounded:

- Default tile size: 256 × 256 pixels
- Bit depths: u8 (8bpc), u16 (16bpc), f16 (HDR/float)
- Colour spaces: sRGB, Display P3, ProPhoto RGB, CMYK (via `appthere-color`)
- Channels: RGB, RGBA, L, LA, CMYK, CMYKA, multichannel (spot)
- Alpha is always premultiplied internally

Dirty tile tracking drives the render cache — only modified tiles are re-composited during scroll/zoom.

### 3.3 Vector Layer (`iris-vector`)

A scene graph of path objects, represented as:

```rust
pub struct PathObject {
    pub id: ObjectId,
    pub path: BezPath,          // kurbo::BezPath
    pub fill: Option<Paint>,
    pub stroke: Option<StrokePaint>,
    pub effects: Vec<PathEffect>,
    pub transform: Affine,
}

pub enum Paint {
    Solid(Color),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
    MeshGradient(MeshGradient),  // Adobe-compatible
    Pattern(PatternRef),
}
```

Path boolean operations (union, subtract, intersect, divide, trim) are implemented via the `pathops` crate (based on Skia's pathops algorithm).

### 3.4 Artboards

Vector documents support multiple artboards (equivalent to Illustrator's artboards). Each artboard is a named rectangular clip scope in the scene graph. Pixel documents use a single canvas boundary.

---

## 4. Artisan Interchange Format (AIF) — Full Specification

**ADR 002** — AIF is an OPC/ZIP container built on `loki-opc` (at `crates/loki-opc/`, consumed as a path dep per ADR 008). The OPC plumbing (content types, part addressing, relationship model) is identical between Loki `.loki` files and Iris `.aif` files — only the part schemas differ.

`iris-aif` owns AIF-specific concerns exclusively: the layer tree XML schema, EXR tile parts, the FlatBuffers path store, and the Loro op log part. It contains no ZIP or OPC logic.

---

### 4.1 Design Principles

These principles govern every decision in the format specification. When a future change creates ambiguity, resolve it by returning to these:

1. **Explicit over implicit.** Every value that a reader needs to correctly decode a part must be present in that part or in `document.xml`. No value may be inferred from context, filename patterns, or implementation defaults.
2. **Fail loudly on unknown required fields.** A reader that silently ignores a field it does not understand and produces corrupted output is worse than a reader that refuses to open the file. Required fields are required; unrecognised required fields are a hard error.
3. **Forward compatibility via `x:` extensions.** Unknown XML elements and attributes in the `x:` namespace are preserved on round-trip and ignored on read. Unknown elements outside `x:` in a required part are a hard error.
4. **Format version is immutable once written.** The `formatVersion` in `document.xml` records the AIF version that produced the file. Readers must check it before reading anything else. Writers never silently upgrade a file's version without user action.
5. **No implicit ordering.** The layer tree is an explicit tree, not an ordered list. Tile parts are addressed by coordinate, not position in the ZIP. The op log is a sequence of timestamped entries, not an append order.
6. **All identifiers are UUIDs.** Layer IDs, asset IDs, artboard IDs — all are lowercase hyphenated UUID v4. No sequential integers, no name-derived keys. UUIDs survive copy-paste between documents and collaboration merges without collision.
7. **Tile absence means transparency.** A missing tile part is not an error; it decodes as a fully transparent tile. This rule must be enforced in the reader, not assumed. A tile whose EXR exists but contains only zero-alpha pixels should be omitted on write (sparse optimisation), but its absence on read must always produce transparency, never an error.
8. **Assets are immutable once stored.** An asset part (ICC profile, brush, pattern) is identified by a content-derived UUID (SHA-256 of the asset bytes, truncated to 128 bits, formatted as UUID). Writing the same brush twice produces the same asset UUID and a single part. Readers may cache by UUID safely.

---

### 4.2 Versioning

AIF uses a two-component version: `major.minor`.

- **Minor bump** (`1.0` → `1.1`): new optional XML attributes, new optional part types, new `x:` extension namespaces. Older readers can open newer files and may silently ignore unrecognised optional fields.
- **Major bump** (`1.x` → `2.0`): breaking change to a required part schema, required field type change, or removal of a previously required field. Older readers must refuse to open files with a higher major version and present a user-visible error: *"This file requires Iris version X or later."*

The initial release is `1.0`. This specification describes `1.0`.

`document.xml` carries the version as an attribute on the root element:

```xml
<iris:Document xmlns:iris="https://appthere.dev/iris/2024"
               formatVersion="1.0"
               ...>
```

A reader must validate `formatVersion` as its first act after parsing `document.xml`. If the major component is greater than the reader's supported major, it must return `AifError::UnsupportedMajorVersion { found: u32, supported: u32 }` without reading further.

---

### 4.3 OPC Container Layout

```
document.aif                          (ZIP, stored = no compression on outer envelope)
│
├── [Content_Types].xml               (OPC required — managed by loki-opc)
├── _rels/.rels                       (OPC required — root relationships)
│
├── iris/document.xml                 (REQUIRED — root manifest)
├── iris/_rels/document.xml.rels      (OPC — relationships from document.xml)
│
├── iris/metadata.xml                 (REQUIRED — document metadata)
│
├── iris/history/ops.bin              (REQUIRED — Loro CRDT op log)
│
├── iris/layers/{layer-uuid}/
│   ├── meta.xml                      (REQUIRED per layer)
│   ├── tiles/{tx}_{ty}.exr           (pixel layers only; absent = transparent)
│   └── paths.bin                     (vector layers only; REQUIRED if vector)
│
├── iris/assets/icc/{asset-uuid}.icc  (OPTIONAL — embedded ICC profiles)
├── iris/assets/brushes/{asset-uuid}.brush  (OPTIONAL)
├── iris/assets/patterns/{asset-uuid}.exr   (OPTIONAL — pattern tile)
│
└── iris/preview.png                  (REQUIRED — 256×256 sRGB PNG thumbnail)
```

**ZIP compression policy:**

| Part | ZIP method |
|---|---|
| `[Content_Types].xml`, `_rels/*.rels`, `*.xml` | Deflate (level 6) |
| `*.exr` | Stored (EXR has its own internal compression) |
| `*.bin` (op log, paths) | Deflate (level 6) |
| `preview.png` | Stored (PNG is already compressed) |
| `*.icc` | Deflate (level 6) |
| `*.brush` | Deflate (level 6) |

EXR parts must never be double-compressed. Applying ZIP deflate to an already-compressed EXR yields larger files and wastes CPU on read.

**Part name rules:**
- All part names are lowercase ASCII
- Layer UUIDs in paths are the canonical lowercase hyphenated form: `3f2a1b4c-8d9e-4f0a-b1c2-d3e4f5a6b7c8`
- Tile coordinates are zero-padded to 6 digits: `000000_000003.exr` (tiles at `tx=0, ty=3`)
- No part name may exceed 260 characters (Windows MAX_PATH safety)

---

### 4.4 `iris/document.xml` — Root Manifest

This is the document's spine. Every reader parses this first and uses it to locate all other parts.

**Full schema for a minimal valid document:**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<iris:Document
    xmlns:iris="https://appthere.dev/iris/2024"
    xmlns:x="https://appthere.dev/iris/ext"
    formatVersion="1.0"
    documentId="3f2a1b4c-8d9e-4f0a-b1c2-d3e4f5a6b7c8"
    createdAt="2024-11-01T10:30:00Z"
    savedAt="2024-11-01T14:22:17Z"
    appVersion="0.1.0">

  <iris:Canvas
      mode="pixel"
      widthPx="3508"
      heightPx="4961"
      dpiX="300"
      dpiY="300"
      workingColorSpace="linear-srgb"
      bitDepth="f16" />

  <iris:ColorProfile
      workingSpace="linear-srgb"
      iccPartRef="iris/assets/icc/a1b2c3d4-e5f6-7890-abcd-ef1234567890.icc"
      renderingIntent="relative-colorimetric" />

  <iris:Artboards>
    <!-- pixel mode: exactly one artboard whose bounds equal canvas bounds -->
    <iris:Artboard
        id="7c8d9e0f-1a2b-3c4d-5e6f-7a8b9c0d1e2f"
        name="Canvas"
        xPx="0" yPx="0"
        widthPx="3508" heightPx="4961" />
  </iris:Artboards>

  <iris:LayerTree>
    <iris:Layer id="b2c3d4e5-f6a7-8901-bcde-f12345678901" type="pixel" order="0" />
    <iris:Layer id="c3d4e5f6-a7b8-9012-cdef-123456789012" type="group" order="1">
      <iris:Layer id="d4e5f6a7-b8c9-0123-defa-234567890123" type="pixel" order="0" />
    </iris:Layer>
  </iris:LayerTree>

</iris:Document>
```

**`<iris:Canvas>` attribute definitions:**

| Attribute | Type | Required | Values |
|---|---|---|---|
| `mode` | enum | YES | `pixel` \| `vector` \| `mixed` |
| `widthPx` | u32 | YES | 1 – 300000 |
| `heightPx` | u32 | YES | 1 – 300000 |
| `dpiX` | f32 | YES | > 0.0 |
| `dpiY` | f32 | YES | > 0.0 |
| `workingColorSpace` | string | YES | see §4.9 |
| `bitDepth` | enum | YES | `u8` \| `u16` \| `f16` \| `f32` |

`widthPx` and `heightPx` are the pixel dimensions of the canvas at `dpiX`/`dpiY`. They are fixed at document creation. Resizing the canvas is an explicit user operation that writes a new `document.xml`.

**`<iris:ColorProfile>` attribute definitions:**

| Attribute | Type | Required | Values |
|---|---|---|---|
| `workingSpace` | string | YES | must match `<iris:Canvas workingColorSpace>` |
| `iccPartRef` | string | NO | OPC part name of embedded ICC profile; omit for well-known spaces |
| `renderingIntent` | enum | YES | `perceptual` \| `relative-colorimetric` \| `saturation` \| `absolute-colorimetric` |

If `iccPartRef` is absent, `workingSpace` must be one of the well-known identifiers in §4.9. If `iccPartRef` is present, `workingSpace` must be `icc-embedded` and the reader must load the ICC profile from the referenced part before applying any colour operations.

**`<iris:Artboard>` attribute definitions:**

| Attribute | Type | Required | Notes |
|---|---|---|---|
| `id` | UUID | YES | stable across saves |
| `name` | string | YES | display name, UTF-8, max 255 bytes |
| `xPx` | i32 | YES | origin in canvas space (may be negative in vector mode) |
| `yPx` | i32 | YES | origin in canvas space |
| `widthPx` | u32 | YES | > 0 |
| `heightPx` | u32 | YES | > 0 |

In `pixel` mode there is exactly one artboard whose `xPx=0`, `yPx=0`, `widthPx` and `heightPx` equal the canvas dimensions. Readers must validate this invariant and return `AifError::InvalidPixelModeArtboard` if it is violated.

In `vector` and `mixed` modes, any number of artboards ≥ 1 may be present. Artboard bounds may extend outside the nominal canvas bounds; the canvas bounds are informational in vector mode.

**`<iris:LayerTree>`:**

Layers are declared as a tree of `<iris:Layer>` elements. Each element carries only the layer UUID, type, and sibling order. All other layer data lives in `iris/layers/{id}/meta.xml`. This split keeps `document.xml` compact and allows layer metadata to be updated without rewriting the root manifest.

| Attribute | Type | Required | Notes |
|---|---|---|---|
| `id` | UUID | YES | must have a corresponding `iris/layers/{id}/meta.xml` part |
| `type` | enum | YES | `pixel` \| `vector` \| `text` \| `group` \| `smart-object` \| `adjustment` \| `fill` |
| `order` | u32 | YES | sibling display order, 0 = bottom. Must be unique among siblings. Need not be contiguous. |

Reader must validate that all `id` values are unique across the entire tree and that every declared layer has a `meta.xml` part. Missing `meta.xml` → `AifError::MissingLayerMeta { layer_id: Uuid }`.

---

### 4.5 `iris/metadata.xml` — Document Metadata

```xml
<?xml version="1.0" encoding="UTF-8"?>
<iris:Metadata
    xmlns:iris="https://appthere.dev/iris/2024"
    xmlns:x="https://appthere.dev/iris/ext">

  <iris:Title>Untitled Document</iris:Title>
  <iris:Author>Kevin</iris:Author>
  <iris:Description></iris:Description>
  <iris:Keywords></iris:Keywords>

  <iris:EditingHistory>
    <iris:Session
        startedAt="2024-11-01T10:30:00Z"
        endedAt="2024-11-01T14:22:17Z"
        appVersion="0.1.0"
        platform="windows-x86_64" />
  </iris:EditingHistory>

</iris:Metadata>
```

All fields are optional for readers — a missing `metadata.xml` part is a warning, not an error. Writers must always produce it.

`<iris:Session>` entries accumulate across saves (they are appended, never overwritten). The `platform` attribute uses Rust's `std::env::consts::OS` + `ARCH` joined with `-`.

---

### 4.6 `iris/layers/{id}/meta.xml` — Layer Properties

One file per layer. Contains all layer properties except pixel data and path data.

```xml
<?xml version="1.0" encoding="UTF-8"?>
<iris:LayerMeta
    xmlns:iris="https://appthere.dev/iris/2024"
    xmlns:x="https://appthere.dev/iris/ext"
    id="b2c3d4e5-f6a7-8901-bcde-f12345678901"
    type="pixel"
    name="Background"
    visible="true"
    locked="false"
    opacity="1.0"
    blendMode="normal"
    clippingMask="false">

  <!-- pixel-layer-specific -->
  <iris:PixelData
      tileSize="256"
      channelLayout="rgba"
      colorSpace="linear-srgb"
      bitDepth="f16"
      compression="zip"
      canvasOffsetX="0"
      canvasOffsetY="0" />

  <!-- mask — present only if layer has a mask -->
  <iris:Mask
      type="pixel"
      enabled="true"
      inverted="false"
      density="1.0"
      featherPx="0.0"
      layerRef="e5f6a7b8-c9d0-1234-efab-567890123456" />

  <!-- effects — zero or more, in application order -->
  <iris:Effects>
    <iris:DropShadow
        enabled="true"
        blendMode="multiply"
        opacity="0.75"
        angleDeg="135"
        distancePx="10"
        spreadPct="0"
        sizePx="15"
        color="#000000"
        useGlobalLight="true" />
  </iris:Effects>

</iris:LayerMeta>
```

**Required attributes on `<iris:LayerMeta>`:**

| Attribute | Type | Values / constraints |
|---|---|---|
| `id` | UUID | must match the directory name |
| `type` | enum | `pixel` \| `vector` \| `text` \| `group` \| `smart-object` \| `adjustment` \| `fill` |
| `name` | string | UTF-8, max 255 bytes |
| `visible` | bool | `true` \| `false` |
| `locked` | bool | `true` \| `false` |
| `opacity` | f32 | 0.0 – 1.0 inclusive |
| `blendMode` | enum | see §4.8 |
| `clippingMask` | bool | `true` = this layer clips to the layer immediately below it in the stack |

**`<iris:PixelData>` — required for `type="pixel"` layers:**

| Attribute | Type | Values |
|---|---|---|
| `tileSize` | u32 | must be `256` in format version 1.0 (reserved for future sizes) |
| `channelLayout` | enum | `rgba` \| `rgb` \| `la` \| `l` \| `cmyka` \| `cmyk` \| `multichannel` |
| `colorSpace` | string | see §4.9; must be compatible with `channelLayout` |
| `bitDepth` | enum | `u8` \| `u16` \| `f16` \| `f32` |
| `compression` | enum | `zip` \| `zips` \| `piz` \| `dwab` \| `dwaa` (EXR compression codec) |
| `canvasOffsetX` | i32 | pixel offset of tile `(0,0)` from canvas origin |
| `canvasOffsetY` | i32 | pixel offset of tile `(0,0)` from canvas origin |

`canvasOffsetX` and `canvasOffsetY` allow layers smaller than the canvas (cropped layers). A layer whose pixel data starts at canvas position `(512, 256)` has `canvasOffsetX="512" canvasOffsetY="256"`, and its tile `(0,0)` covers canvas pixels `512–767` × `256–511`.

The number of tiles in each dimension is derived: `tiles_x = ceil((layer_width_px) / tileSize)`, `tiles_y = ceil((layer_height_px) / tileSize)`. The layer's pixel dimensions are not stored explicitly — they are `canvas_width - canvasOffsetX` for a full-canvas layer, or derived from the layer's own crop bounds stored in an optional `<iris:CropBounds>` child element.

**`<iris:CropBounds>` — optional, for layers smaller than the canvas:**

```xml
<iris:CropBounds widthPx="1024" heightPx="768" />
```

If absent, layer dimensions equal canvas dimensions minus the canvas offset (i.e., the layer fills from its offset to the canvas edge).

**`<iris:Mask>` — optional:**

| Attribute | Type | Notes |
|---|---|---|
| `type` | enum | `pixel` \| `vector` |
| `enabled` | bool | |
| `inverted` | bool | |
| `density` | f32 | 0.0–1.0; scales mask intensity |
| `featherPx` | f32 | Gaussian blur radius applied to mask on composite |
| `layerRef` | UUID | ID of the mask layer (pixel mask: another pixel layer; vector mask: a vector layer) |

The mask layer referenced by `layerRef` must exist in the layer tree. Mask layers are regular layers with `visible="false"` by convention; they are not special-cased in the tree structure.

---

### 4.7 Tile Parts (`iris/layers/{id}/tiles/{tx}_{ty}.exr`)

Each tile is a single OpenEXR scanline image of exactly `tileSize × tileSize` pixels.

**EXR header requirements (written by `iris-aif`, validated on read):**

| EXR attribute | Required value | Notes |
|---|---|---|
| `type` | `scanlineimage` | tiled EXR is not used; scanline gives simpler random access |
| `compression` | must match `meta.xml` `compression` | reader must validate consistency |
| `dataWindow` | `(0,0)` to `(tileSize-1, tileSize-1)` | always a full tile, even at canvas edges |
| `displayWindow` | same as `dataWindow` | |
| `pixelAspectRatio` | `1.0` | |
| `channels` | see channel table below | |
| `aif:layerId` | UUID string | must match the parent directory UUID |
| `aif:tileX` | int | tile X coordinate (must match filename) |
| `aif:tileY` | int | tile Y coordinate (must match filename) |
| `aif:colorSpace` | string | must match `meta.xml` `colorSpace` |

The `aif:*` attributes are custom EXR metadata stored using the EXR arbitrary metadata mechanism. Readers must validate that `aif:layerId`, `aif:tileX`, `aif:tileY` match the OPC part path. A mismatch indicates a corrupt or manually modified file and must return `AifError::TileMetadataMismatch`.

**Channel layout → EXR channel names:**

| `channelLayout` | EXR channels | Pixel type |
|---|---|---|
| `rgba` | `R`, `G`, `B`, `A` | matches layer `bitDepth` |
| `rgb` | `R`, `G`, `B` | matches layer `bitDepth` |
| `la` | `Y`, `A` | matches layer `bitDepth` |
| `l` | `Y` | matches layer `bitDepth` |
| `cmyka` | `C`, `M`, `Y`, `K`, `A` | matches layer `bitDepth` |
| `cmyk` | `C`, `M`, `Y`, `K` | matches layer `bitDepth` |
| `multichannel` | `Ch.0`, `Ch.1`, … `Ch.N` | always f16 |

Channels are always stored in alphabetical order within the EXR (EXR convention). Readers must sort channels by name before interpreting them.

**Edge tile padding:**

When a tile falls at the canvas edge and the canvas dimensions are not exact multiples of `tileSize`, the tile is still written as a full `tileSize × tileSize` EXR. The pixels outside the canvas boundary are written as zero-alpha and must be masked on composite using the canvas clip rect. The implementation must not crop edge tiles to the canvas boundary, as this would require special-casing every edge tile on read.

**Sparse tile rule:**

A tile part is omitted if and only if all pixels in that tile have alpha = 0.0 (fully transparent) in all channels. Readers treat absence as transparency. Writers must check the tile before writing and omit it if sparse. A writer that writes transparent tiles wastes space but produces a valid file; a reader that errors on an absent tile is non-conformant.

---

### 4.8 Blend Mode Identifiers

Blend modes are stored as string enumerants in `meta.xml`. The full set for format version 1.0:

| Identifier | Display name | Notes |
|---|---|---|
| `normal` | Normal | |
| `dissolve` | Dissolve | |
| `darken` | Darken | |
| `multiply` | Multiply | |
| `color-burn` | Colour Burn | |
| `linear-burn` | Linear Burn | |
| `darker-color` | Darker Colour | |
| `lighten` | Lighten | |
| `screen` | Screen | |
| `color-dodge` | Colour Dodge | |
| `linear-dodge` | Linear Dodge (Add) | |
| `lighter-color` | Lighter Colour | |
| `overlay` | Overlay | |
| `soft-light` | Soft Light | |
| `hard-light` | Hard Light | |
| `vivid-light` | Vivid Light | |
| `linear-light` | Linear Light | |
| `pin-light` | Pin Light | |
| `hard-mix` | Hard Mix | |
| `difference` | Difference | |
| `exclusion` | Exclusion | |
| `subtract` | Subtract | |
| `divide` | Divide | |
| `hue` | Hue | |
| `saturation` | Saturation | |
| `color` | Colour | |
| `luminosity` | Luminosity | |
| `pass-through` | Pass Through | groups only |

An unrecognised blend mode string on a required layer is `AifError::UnknownBlendMode { value: String }`. Readers must not silently substitute `normal` — this would change the document appearance without user awareness.

---

### 4.9 Colour Space Identifiers

Well-known colour space string values used in `workingColorSpace`, `colorSpace`, and `iccPartRef` contexts:

| Identifier | Description | Valid `channelLayout` |
|---|---|---|
| `linear-srgb` | Linear light sRGB (IEC 61966-2-1) | `rgba`, `rgb`, `la`, `l` |
| `srgb` | Gamma-encoded sRGB | `rgba`, `rgb`, `la`, `l` |
| `display-p3` | Display P3 (DCI-P3 primaries, sRGB gamma) | `rgba`, `rgb` |
| `linear-display-p3` | Linear light Display P3 | `rgba`, `rgb` |
| `prophoto-rgb` | ProPhoto RGB (Kodak) | `rgba`, `rgb` |
| `linear-prophoto-rgb` | Linear ProPhoto RGB | `rgba`, `rgb` |
| `lab` | CIE L\*a\*b\* D50 | `la`, `l` (L channel only) |
| `xyz-d50` | CIE XYZ D50 | `rgba`, `rgb` |
| `gray-linear` | Linear luminance (single channel) | `l`, `la` |
| `gray-srgb` | sRGB-gamma luminance | `l`, `la` |
| `cmyk-generic` | Device CMYK (no profile) | `cmyk`, `cmyka` |
| `icc-embedded` | Custom ICC (see `iccPartRef`) | any |
| `icc-external` | ICC via OS colour management (not embedded) | any |

`cmyk-generic` is a last-resort mode for CMYK data imported without a device profile (e.g., a PSD with no embedded profile). Composite output is undefined for `cmyk-generic` layers; the compositor will issue a visible warning overlay. Users are expected to assign a proper ICC profile.

`icc-external` is used when the document references a named ICC profile installed on the host system but not embedded in the file. This mode is discouraged (files become non-portable) and triggers a warning on open if the profile is not found.

---

### 4.10 Vector Layer Path Store (`iris/layers/{id}/paths.bin`)

Vector layers have no tile directory. Their geometry is stored in a single `paths.bin` FlatBuffers file.

**FlatBuffers schema (`iris_paths.fbs`):**

```fbs
namespace iris.aif;

enum FillRule : byte { NonZero = 0, EvenOdd = 1 }
enum LineCap  : byte { Butt = 0, Round = 1, Square = 2 }
enum LineJoin : byte { Miter = 0, Round = 1, Bevel = 2 }

table ColorStop {
  offset: float;        // 0.0–1.0
  r: float;
  g: float;
  b: float;
  a: float;
}

table LinearGradient {
  x0: float;  y0: float;
  x1: float;  y1: float;
  stops: [ColorStop];
  spread: byte;         // 0=pad, 1=reflect, 2=repeat
}

table RadialGradient {
  cx: float;  cy: float;   // centre
  fx: float;  fy: float;   // focal point
  r: float;
  stops: [ColorStop];
  spread: byte;
}

// Solid fill, gradient fill, or absent (no fill)
union PaintVariant { SolidColor, LinearGradient, RadialGradient }
table SolidColor { r:float; g:float; b:float; a:float; }
table Paint { variant: PaintVariant; }

table StrokePaint {
  paint: Paint;
  width: float;
  cap: LineCap;
  join: LineJoin;
  miterLimit: float = 4.0;
  dashArray: [float];
  dashOffset: float;
}

// Bezier path: sequence of verb + point pairs
// Verbs: 0=MoveTo, 1=LineTo, 2=QuadTo, 3=CubicTo, 4=Close
table PathData {
  verbs: [byte];
  points: [float];    // x,y pairs; count must match verb arity
}

table PathObject {
  id: [ubyte] (required);   // 16 bytes, UUID v4
  path: PathData (required);
  fill: Paint;
  stroke: StrokePaint;
  fillRule: FillRule = NonZero;
  transform: [float];       // 6-element column-major 2D affine [a,b,c,d,e,f]; absent = identity
  visible: bool = true;
  locked: bool = false;
  name: string;
}

table PathStore {
  version: uint = 1;
  colorSpace: string;       // must match layer meta.xml colorSpace
  objects: [PathObject] (required);
}

root_table PathStore;
file_identifier "AIRF";
file_extension "bin";
```

**Rules:**

- `PathStore.version` is `1` for format version AIF 1.0. A reader encountering a value > 1 must return `AifError::UnknownPathStoreVersion`.
- `PathStore.colorSpace` must match the parent layer's `colorSpace` in `meta.xml`. A mismatch is `AifError::ColorSpaceMismatch`.
- `PathObject.transform` when present must be exactly 6 floats. Any other length is `AifError::MalformedTransform`.
- `PathObject.id` is a 16-byte little-endian UUID v4. Readers must validate uniqueness within the `PathStore`.
- `PathData.points` length must equal the sum of arity values for each verb: MoveTo=2, LineTo=2, QuadTo=4, CubicTo=6, Close=0. A mismatch is `AifError::MalformedPathData`.

---

### 4.11 Op Log (`iris/history/ops.bin`)

The op log is a Loro CRDT binary encoded using Loro's native snapshot format. `iris-aif` treats this as an opaque blob on read/write — it passes the bytes to/from `iris-ops`, which owns the Loro document.

The only AIF-level contract on `ops.bin` is:

- The first 8 bytes are a magic header: `AIROPLOG` (ASCII)
- Bytes 8–11 are a little-endian u32 version: `1` for AIF 1.0
- Bytes 12 onward are the Loro snapshot bytes

On read, `iris-aif` strips the 12-byte header and passes the remainder to `loro::LoroDoc::import`. On write, `iris-aif` prepends the header to the bytes returned by `loro::LoroDoc::export_snapshot`.

If the magic is wrong or the version is greater than `1`, `iris-aif` returns `AifError::InvalidOpLog` and the document is opened without history (a recoverable condition — the user is warned, pixel and vector data are intact).

---

### 4.12 Asset Parts

**ICC profiles (`iris/assets/icc/{uuid}.icc`):**

The UUID is derived from the ICC profile bytes: `uuid_v5(NAMESPACE_OID, sha256(profile_bytes)[0..16])`. This ensures the same profile embedded in two documents gets the same UUID, enabling deduplication at the application level.

The raw ICC profile bytes are stored without modification.

**Brush assets (`iris/assets/brushes/{uuid}.brush`):**

`.brush` files are FlatBuffers-encoded brush definitions (schema TBD in a separate ADR for the brush engine). The UUID is content-derived by the same scheme as ICC profiles.

**Pattern tiles (`iris/assets/patterns/{uuid}.exr`):**

Pattern tiles are full OpenEXR images using the same channel and colour space conventions as pixel layer tiles. They are always stored with ZIPS compression (lossless, scanline-block). The UUID is content-derived.

---

### 4.13 `iris/preview.png`

A sRGB PNG thumbnail of the document's composited appearance at 256 × 256 pixels (or smaller if the document is smaller than 256 px in either dimension). It is letterboxed with a transparent background, never cropped.

The preview is always sRGB regardless of the document's working colour space — it exists for file manager display, not colour-accurate reproduction.

Writers must produce the preview on every save. Readers must not error if the preview is absent (some export tools may omit it), but must not display an empty thumbnail — they should composite a fresh preview if the part is missing.

---

### 4.14 Content Types Registration

`[Content_Types].xml` must register the following MIME types:

```xml
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml"  ContentType="application/xml"/>
  <Default Extension="exr"  ContentType="image/x-exr"/>
  <Default Extension="bin"  ContentType="application/octet-stream"/>
  <Default Extension="png"  ContentType="image/png"/>
  <Default Extension="icc"  ContentType="application/vnd.iccprofile"/>
  <Default Extension="brush" ContentType="application/vnd.appthere.brush+flatbuffers"/>
  <Override PartName="/iris/document.xml"
            ContentType="application/vnd.appthere.iris.document+xml"/>
  <Override PartName="/iris/metadata.xml"
            ContentType="application/vnd.appthere.iris.metadata+xml"/>
  <Override PartName="/iris/history/ops.bin"
            ContentType="application/vnd.appthere.iris.oplog"/>
</Types>
```

---

### 4.15 Error Taxonomy (`iris-aif` crate)

All errors returned by `iris-aif` are variants of a single `AifError` enum. No `anyhow` or boxed errors at the crate API boundary.

```rust
#[derive(Debug, thiserror::Error)]
pub enum AifError {
    // Container / OPC errors
    #[error("not a valid AIF file: missing required part {0}")]
    MissingRequiredPart(String),
    #[error("OPC container error: {0}")]
    Opc(#[from] appthere_opc::OpcError),

    // Version errors
    #[error("unsupported AIF major version {found}; this reader supports up to {supported}")]
    UnsupportedMajorVersion { found: u32, supported: u32 },

    // document.xml errors
    #[error("document.xml: missing required attribute '{attr}' on element '{element}'")]
    MissingAttribute { element: String, attr: String },
    #[error("document.xml: invalid pixel mode artboard (must have exactly one artboard matching canvas bounds)")]
    InvalidPixelModeArtboard,
    #[error("document.xml: duplicate layer id {0}")]
    DuplicateLayerId(uuid::Uuid),
    #[error("layer {layer_id} declared in document.xml but meta.xml is missing")]
    MissingLayerMeta { layer_id: uuid::Uuid },

    // Layer meta errors
    #[error("layer {layer_id}: unknown blend mode '{value}'")]
    UnknownBlendMode { layer_id: uuid::Uuid, value: String },
    #[error("layer {layer_id}: unknown colour space '{value}'")]
    UnknownColorSpace { layer_id: uuid::Uuid, value: String },
    #[error("layer {layer_id}: colour space mismatch between meta.xml and paths.bin")]
    ColorSpaceMismatch { layer_id: uuid::Uuid },

    // Tile errors
    #[error("layer {layer_id} tile ({tx},{ty}): EXR metadata mismatch (expected layer {expected_id})")]
    TileMetadataMismatch { layer_id: uuid::Uuid, tx: u32, ty: u32, expected_id: uuid::Uuid },
    #[error("layer {layer_id} tile ({tx},{ty}): {source}")]
    TileReadError { layer_id: uuid::Uuid, tx: u32, ty: u32, source: Box<dyn std::error::Error + Send + Sync> },

    // Path store errors
    #[error("layer {layer_id}: unknown PathStore version {version}")]
    UnknownPathStoreVersion { layer_id: uuid::Uuid, version: u32 },
    #[error("layer {layer_id}: malformed path data (verb/point count mismatch)")]
    MalformedPathData { layer_id: uuid::Uuid },
    #[error("layer {layer_id}: malformed transform (expected 6 floats, got {got})")]
    MalformedTransform { layer_id: uuid::Uuid, got: usize },

    // Op log errors (non-fatal — document opens without history)
    #[error("op log invalid or unreadable; document opened without history: {reason}")]
    InvalidOpLog { reason: String },

    // I/O
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    // XML parse
    #[error("XML parse error in {part}: {source}")]
    XmlParse { part: String, source: Box<dyn std::error::Error + Send + Sync> },
}
```

**Fatal vs. recoverable errors:**

| Error | Fatal? | Behaviour |
|---|---|---|
| `MissingRequiredPart` | YES | Abort open, show error dialog |
| `UnsupportedMajorVersion` | YES | Abort open, suggest upgrade |
| `MissingAttribute` | YES | Abort open |
| `InvalidPixelModeArtboard` | YES | Abort open |
| `DuplicateLayerId` | YES | Abort open |
| `MissingLayerMeta` | YES | Abort open |
| `UnknownBlendMode` | YES | Abort open |
| `UnknownColorSpace` | YES | Abort open |
| `ColorSpaceMismatch` | YES | Abort open |
| `TileMetadataMismatch` | YES | Abort open |
| `TileReadError` | NO | Log warning; substitute transparent tile; continue |
| `UnknownPathStoreVersion` | YES | Abort open |
| `MalformedPathData` | YES | Abort open |
| `InvalidOpLog` | NO | Log warning; open without history; notify user |
| `Io` | YES | Abort open |
| `XmlParse` | YES | Abort open |

---

### 4.16 Format Evolution Rules

These rules govern how the AIF format may be changed in future versions. They are binding on any contributor modifying the format.

1. **Never change the meaning of an existing attribute.** If a field needs new semantics, add a new attribute and deprecate the old one with a `formatVersion` guard.
2. **Never remove a required attribute without a major version bump.** Required attributes may only be removed in a `2.0` release with a documented migration tool.
3. **New optional attributes default to the behaviour of their absence.** A reader that does not recognise a new optional attribute must produce output identical to a reader that treats it as absent.
4. **New layer types in a minor bump require a fallback strategy.** A reader encountering an unknown `type` on a `<iris:Layer>` must treat it as a group (render children normally) and display a warning: *"One or more layers use a feature not supported in this version of Iris."*
5. **New blend modes in a minor bump require a fallback.** A reader encountering an unknown blend mode must substitute `normal` and display a per-layer warning badge.
6. **The FlatBuffers `PathStore.version` field gates path format changes.** A new path feature (e.g. mesh gradients) bumps `PathStore.version` to `2`. Readers supporting only version `1` must skip version-`2` path stores and render the vector layer as empty with a warning.
7. **The op log magic version gates Loro format changes.** Op log format changes bump the u32 at bytes 8–11. Readers encountering a higher version open without history (recoverable error as per §4.11).
8. **Test corpus is mandatory.** Every format change must be accompanied by a new `.aif` fixture file in `iris-aif/tests/fixtures/` that exercises the new feature, plus a round-trip test that writes the fixture and reads it back bit-for-bit equivalent.

---

## 5. Format Adapters

### 5.1 `iris-psd` — Photoshop PSD/PSB

PSD is a complex proprietary format with many undocumented quirks. The adapter is strictly an importer/exporter — it converts to/from the AIF layer model.

**Read support:**
- Layer types: pixel, group, fill, adjustment, type (text), smart object
- Blend modes: all 27 Photoshop blend modes (mapped to Iris equivalents)
- Adjustment layers: Levels, Curves, Hue/Sat, Colour Balance, Brightness/Contrast, Exposure, Vibrance, Photo Filter, Selective Colour, Black & White, Gradient Map, Channel Mixer
- Layer effects: drop shadow, inner shadow, outer glow, inner glow, bevel/emboss, satin, colour overlay, gradient overlay, pattern overlay, stroke
- Masks: pixel masks, vector masks, clipping masks
- Smart objects: embedded (rasterised on import), linked (stubbed with original path)
- Colour modes: RGB 8/16/32bpc, CMYK 8bpc, Grayscale 8/16bpc, Lab 8/16bpc, Indexed, Bitmap
- PSB (large document): files up to 300,000px per dimension

**Write support:**
- Round-trips pixel layers, groups, and basic adjustments
- Vector layers are rasterised on PSD export (noted in export dialog)
- `// COMPAT(adobe):` annotations on all PSD quirk workarounds

**Key dependencies:** `psd` crate (reading), custom writer for complex structures

### 5.2 `iris-ora` — OpenRaster

ORA is already ZIP/XML-based and maps cleanly to IDF.

**Read/write:**
- Full layer stack round-trip (pixel layers, groups)
- Blend modes (ORA subset; extended modes noted as unsupported in UI)
- Layer masks
- Metadata (author, description)
- Compatible with Krita, GIMP, MyPaint exports

### 5.3 `iris-ai` — Adobe Illustrator

AI files in modern versions (CS/CC) are PDF-based ZIP containers. Legacy AI (pre-CS) is PostScript.

**Read support (modern AI):**
- Artboard layout
- Vector paths (converted to `kurbo::BezPath`)
- Fill/stroke paints including gradients
- Text objects (converted to TextLayer; fonts substituted if unavailable)
- Symbols and pattern swatches
- Layer/sublayer structure

**Write support:**
- Exports as SVG 1.1 with Illustrator-compatible namespace extensions (`ai:` prefix)
- Full artboard layout preserved
- Pattern and symbol references preserved

**Key challenge:** AI format is not publicly documented. The adapter is built by parsing the PDF XObject streams and reading the `%%AI_...` DSC comments. Extensive `// COMPAT(adobe):` annotations required.

### 5.4 `iris-svg` — SVG

SVG 1.1 and SVG 2 (partial) read/write.

**Read:**
- Full path, shape, and text element parsing
- CSS styling (inline and `<style>` blocks)
- `transform` attributes
- `<use>` references (expanded to instances in scene graph)
- `<defs>` (gradients, patterns, clip paths, masks)
- Filters (converted to equivalent Iris adjustment layers where possible)

**Write:**
- Produces clean, minified SVG 1.1
- Embeds raster layers as `<image>` data URIs (base64 PNG)
- Vector layers map directly to `<path>` elements
- Text layers output as `<text>` / `<tspan>` with font metadata

---

## 6. Canvas Infrastructure

### 6.1 `appthere-canvas` — shared extraction from Loki (ADR 006)

The hardest integration problem in building a Vello-based Dioxus Native app is the boundary between the Dioxus component tree (which owns layout and input) and the wgpu surface (which owns GPU pixels). Loki solved this through `CustomPaintSource` / `PaintSource` API work, frame pacing, and a render cache state machine. That work lives in `loki-vello` today and should be extracted into `appthere-canvas` before Iris begins, so Iris inherits the solution rather than rediscovering it.

**What `appthere-canvas` provides:**

```
appthere-canvas/
├── src/
│   ├── surface.rs       # wgpu surface lifecycle (init, resize, lost/reclaimed)
│   ├── paint_source.rs  # Dioxus Native CustomPaintSource integration
│   ├── frame.rs         # Frame pacing loop, dirty tracking, present timing
│   ├── input.rs         # Normalised InputEvent (pointer, touch, stylus, keyboard)
│   ├── viewport.rs      # Viewport rect, DPI scale, coordinate transforms
│   ├── cache.rs         # Tiered render cache state machine (Hot/Warm/Cold)
│   └── scene.rs         # VelloScene wrapper + submission queue
```

**What it deliberately does NOT provide:**
- Any scroll model (paginated, infinite, clamped — apps decide)
- Any knowledge of document structure, layers, or tiles
- Any zoom or rotation logic
- Any hit-testing or selection logic

This keeps the crate narrowly useful to any AppThere app (and potentially to third-party Dioxus Native apps) without coupling it to document semantics.

**Extraction from Loki — what changes:**

The `loki-vello` crate today mixes canvas plumbing with Loki-specific page layout concerns. The extraction splits it cleanly:

| Responsibility | Stays in `loki-vello` | Moves to `appthere-canvas` |
|---|---|---|
| wgpu surface init/resize | | ✓ |
| `CustomPaintSource` handoff | | ✓ |
| Frame pacing, vsync | | ✓ |
| Dirty tile tracking (generic) | | ✓ |
| Render cache state machine | | ✓ |
| Input event normalisation | | ✓ |
| Page layout, pagination | ✓ | |
| Scroll-by-page model | ✓ | |
| OOXML line spacing, keep-together | ✓ | |
| Paragraph render cache keys | ✓ | |

After extraction, `loki-vello` becomes a thin crate that imports `appthere-canvas` and adds Loki's paginated scroll model on top.

**Semver and publication plan:**
- `appthere-canvas` is published to crates.io at `0.1.0` once extracted
- Both Loki and Iris pin to the same version via workspace `Cargo.toml`
- Breaking changes to `appthere-canvas` require coordinated semver bumps in both consumers

### 6.2 `iris-canvas` — Iris scroll model and scene logic

`iris-canvas` is the Iris-specific crate that sits on top of `appthere-canvas`. It owns everything the shared crate deliberately excludes.

**Infinite canvas scroll model:**

Unlike Loki's paginated scroll, Iris uses an infinite canvas anchored at `(0, 0)` in document space. The viewport transform is a 2D similarity transform (translation + uniform scale), extended with a rotation angle for the canvas rotation feature.

```rust
// iris-canvas/src/viewport.rs
pub struct CanvasViewport {
    /// Document-space origin at the top-left of the screen viewport
    pub pan: Vec2,
    /// Zoom factor (1.0 = 100%, 0.1 = 10%, 64.0 = 6400%)
    pub zoom: f32,
    /// Canvas rotation in radians (non-destructive; 0 is default)
    pub rotation: f32,
}

impl CanvasViewport {
    pub fn screen_to_doc(&self, screen: Vec2) -> Vec2 { ... }
    pub fn doc_to_screen(&self, doc: Vec2) -> Vec2 { ... }
    pub fn visible_doc_rect(&self, screen_size: UVec2) -> Rect { ... }
}
```

**Two-pass render architecture:**

```rust
pub enum RenderPass {
    /// Bottom-up layer composite into a single wgpu texture
    Composite,
    /// Document-space UI chrome: selection marquee, path anchors,
    /// guides, artboard outlines, rulers — rendered without
    /// contaminating document pixels
    Overlay,
}
```

The composite pass output is a wgpu texture that is then displayed through `appthere-canvas`'s `PaintSource`. The overlay pass is a Vello scene submitted on top of the composited texture, also via `appthere-canvas`.

**Pixel compositor (WGSL compute):**

Each blend mode is a WGSL compute shader operating on premultiplied RGBA f16 values. Shaders live in `iris-canvas/src/shaders/` and are compiled at build time via `wgpu`'s `include_wgsl!` macro.

| Category | Modes |
|---|---|
| Normal | Normal, Dissolve |
| Darken | Darken, Multiply, Colour Burn, Linear Burn, Darker Colour |
| Lighten | Lighten, Screen, Colour Dodge, Linear Dodge (Add), Lighter Colour |
| Contrast | Overlay, Soft Light, Hard Light, Vivid Light, Linear Light, Pin Light, Hard Mix |
| Inversion | Difference, Exclusion, Subtract, Divide |
| Component | Hue, Saturation, Colour, Luminosity |

**Tiered render cache (extends `appthere-canvas` base):**

The base `appthere-canvas` cache state machine is parameterised on a `CacheKey` type. `iris-canvas` instantiates it with `TileKey { layer_id: LayerId, tile: TileCoord }`:

- **Hot zone**: last 2–5 full-resolution composited frames in GPU VRAM (2 on mobile, 5 on desktop, detected via `wgpu::Limits`)
- **Warm zone**: downsampled (50%) composited tiles for the current viewport at sub-100% zoom
- **Cold zone**: per-layer EXR tiles compressed in-memory with LZ4

Target: ≤ 200 MB RAM for a 20-layer, A4@300dpi document.

**ICC colour pipeline:**

All pixel data processed in Linear sRGB working space. Display output converted to monitor ICC profile via `appthere-color`. Soft-proof pass available via a CMYK ICC simulation shader.

---

## 7. Colour Management (`appthere-color`)

Iris uses `appthere-color`, the standalone pure-Rust ICC colour management crate already published to crates.io and used by Loki. It is built on `moxcms` with no C dependencies.

### 7.1 Existing capabilities (from Loki)

- ICC profile parsing (v2 and v4)
- Colour space conversions: sRGB ↔ Linear sRGB ↔ Display P3 ↔ ProPhoto RGB ↔ Lab ↔ XYZ
- Gamut mapping (relative colorimetric + perceptual intents)

### 7.2 Extensions required for Iris

Iris's raster editing workload exposes gaps in the existing crate. The following capabilities are to be added under a `iris` feature flag, guarded by a semver-minor bump:

**CMYK support**
- CMYK ↔ Lab ↔ XYZ conversions via ICC profile LUT lookup
- Device CMYK ↔ sRGB round-trip (for PSD CMYK import and soft-proof display)
- `CmykColour` type with ink percentage representation

**Spot colour handling**
- Pantone name → Lab approximation lookup table (PANTONE+ library, redistributable subset)
- Spot channel compositing model (multiply over base)

**HDR tone mapping**
- Reinhard global tone mapping
- ACES filmic tone mapping
- Linear → PQ (ST.2084) and Linear → HLG transfer function encoding for HDR display output

**High-throughput pixel processing**
- SIMD-optimised batch conversion path for converting entire tile buffers (256 × 256 × f16) in one call, rather than per-pixel
- `convert_tile_buffer(src: &[f16], src_space: ColourSpace, dst: &mut [f16], dst_space: ColourSpace)` API

**OpenEXR channel mapping**
- Named channel → `ColourSpace` inference (`R/G/B/A` → Linear sRGB, `Y` → Linear Grayscale, `C/M/Y/K` → CMYK)
- Validation that embedded EXR chromaticity metadata matches the document's working space

All new APIs follow the existing crate conventions: pure Rust, `#![forbid(unsafe_code)]`, typed errors, Apache-2.0.

---

## 8. Undo / CRDT (`iris-ops`)

Every user operation produces one or more `Op` entries appended to the op log. Undo is implemented by reversing operations, not by storing state snapshots.

```rust
pub enum Op {
    PaintTile { layer: LayerId, tile: TileCoord, before: TileData, after: TileData },
    AddLayer { parent: LayerId, position: usize, layer: Layer },
    RemoveLayer { layer: LayerId },
    MoveLayer { layer: LayerId, new_parent: LayerId, new_position: usize },
    SetLayerProp { layer: LayerId, prop: LayerProp, before: PropValue, after: PropValue },
    AddPath { layer: LayerId, path: PathObject },
    ModifyPath { layer: LayerId, id: ObjectId, before: PathObject, after: PathObject },
    RemovePath { layer: LayerId, id: ObjectId },
    // ... adjustment layer, effect, mask ops
}
```

Undo history depth: 200 operations (configurable). The op log beyond the undo depth is retained for collaboration and audit but not surfaced in the UI.

---

## 9. Plugin System (`iris-plugin-api`)

Plugins run in a WASI sandbox (via `wasmtime`) — they cannot access the file system, network, or GPU directly. They receive pixel data and path data via shared memory segments and return modified data.

**Plugin hook points:**
- **Filter**: receives a pixel tile region, returns a modified region (equivalent to Photoshop filter plugins)
- **Layer effect**: receives composited layer pixels, returns a modified version (drop shadow, glitch effects, etc.)
- **Import**: registers a file extension handler
- **Export**: registers an export format handler
- **Panel**: mounts a Dioxus component into the side panel area (no GPU access)

Plugin manifest is a `plugin.toml`:

```toml
[plugin]
name = "Halftone Pro"
id = "com.example.halftone-pro"
version = "1.0.0"
hooks = ["filter"]
wasm = "plugin.wasm"
```

---

## 10. Platform Targets

Iris targets both desktop and mobile through Dioxus Native's cross-platform renderer. The Vello/wgpu backend supports all five platforms without per-platform render code.

| Platform | Min version | Notes |
|---|---|---|
| Windows | 10 (1903+) | DX12 via wgpu; GPU tile compositor runs natively |
| macOS | 12 (Monterey) | Metal via wgpu; uses `exr` pure-Rust crate for EXR on Mac App Store builds |
| Linux | Ubuntu 22.04+ | Vulkan via wgpu; Wayland primary, X11 via XWayland |
| Android | API 31 (Android 12) | Vulkan via wgpu; touch input mapped to brush/pan/pinch gestures |
| iOS | iOS 16 | Metal via wgpu; Apple Pencil pressure/tilt mapped to brush dynamics |

### Platform-specific constraints

**Mobile tile budget**: Mobile GPUs have far tighter VRAM budgets than desktop. The tiered render cache Hot zone is reduced to 2 frames on mobile (vs 5 on desktop), and the default working bit depth is clamped to f16 (no f32) unless the device reports sufficient VRAM headroom via `wgpu::Limits`.

**EXR on mobile**: The `openexr` crate links against the C++ reference library, which is unsuitable for App Store submission. The `exr` pure-Rust crate is used on iOS and Android builds via a `cfg!(target_os = "ios") || cfg!(target_os = "android")` feature gate. A `// COMPAT(mobile):` annotation marks all affected call sites.

**File access on mobile**: AIF files on iOS and Android are accessed via the platform document picker (no direct file path access). `iris-aif` uses `loki-file-access` (via `appthere-file-access`) which provides `FileAccessToken` + `open_write_truncate()` for all platforms. iOS picker is stubbed pending UIDocumentPickerViewController implementation; Android is functional via Storage Access Framework. See ADR 007.

**Apple Pencil / Stylus input**: Dioxus Native surfaces pressure (`0.0–1.0`) and tilt (`azimuth` + `altitude` angles) from Apple Pencil and Android stylus events. These are mapped to brush dynamics (size, opacity, flow, scattering) in `iris-pixel`. A `StylusEvent` type in `iris-app` abstracts across platforms; mouse input is treated as pressure=1.0, tilt=0.

**Touch canvas gestures**: Two-finger pinch → zoom, two-finger drag → pan, three-finger swipe → undo/redo. These gestures are handled in `iris-canvas` before tool event routing.

---

## 11. UI Shell (`iris-app`)

Built with Dioxus Native 0.7, using the shared `appthere_ui` component library.

### 11.1 Modes

Iris has two primary editing modes selectable from the toolbar:

- **Pixel mode**: activates brush, eraser, clone stamp, heal, smudge, dodge/burn, selection tools
- **Vector mode**: activates pen, curvature, shape, type, anchor tools

Mode switching does not change the document — all layers remain visible in both modes. The tool palette adapts.

### 11.2 Panels

| Panel | Content |
|---|---|
| Layers | Layer tree with visibility toggles, blend mode, opacity |
| Properties | Context-sensitive — selected layer properties, selection stats, path info |
| Colour | Colour picker (HSL, HSB, RGB, Lab, CMYK), swatches, colour history |
| Adjustments | Non-destructive adjustment layer insertion |
| History | Op log display with state jump |
| Channels | Per-channel pixel visibility and editing |
| Paths | Named path storage (saved selections) |
| Assets | Brushes, patterns, gradients, symbols library |
| Export | Live export preview with format/quality settings |

### 11.3 Canvas Controls

**Desktop:**
- Infinite canvas with smooth zoom (10% – 6400%)
- Guides (drag from rulers), smart guides, grid overlay
- Pixel grid display at >400% zoom
- Rotation canvas (non-destructive canvas angle for comfortable drawing)
- Touchpad pinch-to-zoom, two-finger pan on macOS/Windows

**Mobile additions:**
- Touch pinch-to-zoom and two-finger pan replace scroll wheel
- Floating tool radial menu (long-press canvas) replaces toolbar for one-handed use
- Apple Pencil / Android stylus: pressure-sensitive brush size and opacity, tilt-sensitive scattering
- Side panel collapses to slide-in drawer on narrow viewports (< 600px)
- Three-finger swipe left/right → undo/redo

---

## 12. Development Conventions

All Loki coding conventions apply to Iris:

- `cargo check --workspace` checkpoint before every commit
- Audit-only pass before any implementation work (`// AUDIT:` tags)
- `// COMPAT(adobe):` for all PSD/AI quirk workarounds
- `// TODO(iris):` for known gaps
- `// COMPAT(krita):`, `// COMPAT(inkscape):` for interop quirks in ORA/SVG adapters
- `// COMPAT(mobile):` for all platform-conditional code paths
- ADR document required for any cross-crate architectural decision
- Claude Code prompts structured as audit-only then implement-only to avoid timeout

### Extraction gating

| Crate | Gate condition | Status |
|---|---|---|
| `iris-aif` | `loki-opc` path dep at `crates/loki-opc/` wired | ✅ OPEN — ADR 008 |
| `iris-canvas` | `appthere-canvas` published to crates.io; Loki updated to consume it | ⛔ Pending Loki extraction |

The `iris-canvas` extraction is tracked in the Loki ADR backlog since it requires modifying Loki internals.

`iris-aif` file access uses `loki-file-access` (at `crates/loki-file-access/`) consumed as `appthere-file-access`. Android and iOS picker implementations are stubbed; desktop and Android are functional. See ADR 007.

---

## 13. Phased Delivery

### Phase 1 — Canvas & pixel core (MVP)
- **Complete:** `iris-ops` (undo/redo stack, typed Op enum)
- **Complete:** `iris-pixel` (layer tree, TileData, BlendMode, TileCache)
- **Complete:** `loki-opc` wired as path dep; `loki-file-access` wired as path dep
- **In progress:** `iris-aif` — save/load, EXR tile encoding at SampleType::F16
- **Pending Loki extraction:** `appthere-canvas` → then `iris-canvas`
- `iris-app`: minimal shell, layers panel, colour picker; desktop + Android/iOS builds
- `appthere-color` v2: SIMD batch conversion, EXR channel mapping extensions

### Phase 2 — Format round-trips
- `iris-psd`: read (all layer types), write (pixel + group)
- `iris-ora`: full read/write
- `iris-svg`: full read/write

### Phase 3 — Vector editing
- `iris-vector`: scene graph, path objects, bezier editing
- `iris-ai`: read modern AI files, write as SVG+AI namespace
- Artboard support
- Boolean path operations

### Phase 4 — Advanced pixel
- 16bpc and f16 HDR support; DWAB compressed EXR tiles for HDR layers
- Full adjustment layer suite (Curves, Levels, HSL, etc.)
- Layer effects (drop shadow, bevel, glow)
- Smart objects (embedded sub-documents)
- CMYK colour mode (via `appthere-color` CMYK extension)

### Phase 5 — Collaboration & plugins
- Loro CRDT op log
- AppThere Cloud real-time collab presence
- `iris-plugin-api`: WASI sandbox, filter + export hooks

---

## 14. Key Dependencies

| Crate | Purpose |
|---|---|
| `dioxus` 0.7 | UI framework (all 5 platforms) |
| `vello` | Vector/pixel GPU renderer |
| `wgpu` | GPU abstraction (DX12 / Metal / Vulkan) |
| `appthere-canvas` | Dioxus Native / wgpu integration layer (pending Loki extraction, ADR 006) |
| `loki-opc` | OPC/ZIP container — path dep at `crates/loki-opc/` (ADR 008) |
| `loki-file-access` | File picker + permissions — path dep at `crates/loki-file-access/` (ADR 007) |
| `appthere-color` | ICC colour management (published crate, extended for Iris) |
| `kurbo` | Bezier path math |
| `parley` | Text layout |
| `loro` | CRDT op log |
| `moxcms` | Low-level ICC profile math (transitive via appthere-color) |
| `openexr` | EXR read/write — desktop (C++ bindings) |
| `exr` | EXR read/write — mobile (pure Rust fallback) |
| `wasmtime` | WASI plugin sandbox |
| `flatbuffers` | Binary path serialisation |
| `lz4_flex` | In-memory tile compression (Warm/Cold cache zones) |
| `thiserror` | Typed error enums |
| `uuid` | Stable layer/object IDs |

---

*Generated as part of AppThere Iris initial architecture planning.*  
*Apache-2.0 © AppThere Project*
