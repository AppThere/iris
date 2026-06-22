# Iris ↔ Adobe Gap Audit

**Date:** 2026-06-22
**Scope:** Functional gaps between AppThere Iris and Adobe Photoshop, Lightroom, and Illustrator.
**Method:** Static inventory of every `crates/*/src` module (public API, wired behaviour, `TODO(iris)`/`AUDIT`/`COMPAT` markers) cross-referenced against `SPEC.md` §3–§13 and the three Adobe products' core feature sets.

> SPEC.md §1 states the explicit product goal: *"feature parity with Adobe Photoshop and Illustrator."* Lightroom is **not** named anywhere in the spec — its entire feature domain (non-destructive RAW develop) is an unscoped gap, called out separately below.

---

## 1. Executive summary

Iris today is a **well-architected Phase 1→2 foundation**, not yet a usable competitor to any of the three Adobe products. The data model, file container, tile cache, undo log, and a CPU compositor are solid and test-covered. What's missing is the **editing surface**: almost every capability that makes Photoshop/Illustrator/Lightroom useful is either a typed-but-empty stub or gated to a later phase.

Maturity by area:

| Area | State | Parity vs Adobe |
|---|---|---|
| Document/layer model (`iris-pixel`) | Solid core; 4 of 7 `LayerContent` variants are stubs | ~25% |
| Raster tools (`iris-tools`) | Brush, eraser, eyedropper, flood-fill, rect marquee | ~10% vs Photoshop |
| Compositing (`iris-canvas`) | CPU Porter-Duff, **Normal blend only** | ~15% |
| Vector model (`iris-vector`) | Path/paint/stroke storage; **no editing, no booleans** | ~10% vs Illustrator |
| Native format (`iris-aif`) | Pixel + group read/write; masks/effects/vector stubbed | ~50% of its own spec |
| PSD (`iris-psd`) | RGB/Grayscale 8-bit pixel+group round-trip | ~30% |
| ORA (`iris-ora`) | Full pixel+group round-trip, 27 blend modes | ~90% |
| SVG (`iris-svg`) | Paths/shapes/strokes round-trip; **gradients & text lost** | ~40% |
| AI (`iris-ai`) | 11-line stub | 0% |
| Lightroom-class develop | **Nonexistent and unspecced** | 0% |
| UI shell (`iris-app`) | Tabs, layers panel, tool palette, status bar | ~20% |
| Color mgmt | Linear-sRGB working space; CMYK shows a warning overlay | partial |
| Plugins / collab | Trait stub / op-log header stub | 0% |

**The single highest-leverage observation:** a large amount of capability is *already modeled and persisted but not surfaced*. All 27 blend modes exist as an enum and round-trip through AIF/PSD/ORA, yet the compositor only paints `Normal`. The Layers panel stores opacity/blend but exposes no controls. SVG gradients have full data types but the writer emits only the first stop. These are presentation/wiring gaps, not architectural ones — i.e. cheap wins.

---

## 2. Gaps vs Adobe Photoshop

### Missing — high impact
- **Blend modes (26 of 27 not composited).** `BlendMode` enum is complete and persisted; `iris-canvas/src/compositor/composite.rs:44` skips every non-`Normal` layer with a warning. Marked Phase 4 / WGSL, but the math is trivially CPU-implementable today.
- **Adjustment layers — none.** `LayerContent::Adjustment` is a stub (`iris-pixel/src/layer.rs:56`). No Curves, Levels, Hue/Sat, Color Balance, B&C, Exposure, Vibrance, Photo Filter, Channel Mixer, Gradient Map, Selective Color, B&W.
- **Layer masks — none.** `LayerMask` is an empty struct (`iris-pixel/src/layer.rs:71`); AIF parses `<iris:Mask>` then discards it.
- **Layer effects / styles — none.** Drop shadow, inner shadow, glow, bevel, satin, overlays, stroke. AIF recognizes the element names then discards them.
- **Selections.** Only an axis-aligned rectangular marquee (stored as `kurbo::Rect`, consumed by the brush). No elliptical, lasso, polygon, magic wand, quick-select, feather, grow/shrink, save-to-channel, or selection-as-mask.
- **Text layers — none.** `LayerContent::Text` stub (`layer.rs:53`); Parley is a dependency but unused.
- **Smart objects — none.** Stub (`layer.rs:62`).
- **Brush engine is minimal.** One hard round tip. No soft/textured/scatter/dual brush, no flow vs opacity, no pressure→opacity/flow (only pressure→size), no clone stamp, healing, smudge, dodge/burn, sponge, gradient, or shape tools.
- **CMYK / 16-bit / 32-bit editing.** Enums exist (`ChannelLayout::Cmyk`, `BitDepth::U16/F32`) but the compositor works only in F16 RGBA; CMYK renders a warning overlay (`color_space.rs:68`).

### Missing — UX shell
- No **Properties** panel, no **History** panel (undo log exists but is invisible — no UI, and `LayerProp::BlendMode` isn't even tracked, `iris-ops/src/op.rs:119`).
- **Layers panel** has visibility/lock/add/delete but **no opacity slider and no blend-mode dropdown** — even though the layer model stores both.
- No real **color picker** dialog (only foreground/background swatch swap, `tool_palette.rs:119`).
- No menu bar, **no keyboard shortcuts**, no command dispatch (ribbon buttons are all `is_disabled`).
- No touch/stylus input wired (`canvas_widget.rs:228`), no rulers/guides/grid/smart-guides, no export dialog.

### Format fidelity (PSD)
- Read: PSD v1 only (**PSB rejected**), RGB + Grayscale 8-bit only (CMYK/Lab/Indexed/16-32bpc rejected). Adjustment/text/smart-object layers are **rasterized**, masks/effects/ICC dropped, **DPI hardcoded to 72** (`convert.rs:18`).
- Write: pixel + group only; everything else rasterized; no masks/effects/resolution.

---

## 3. Gaps vs Adobe Lightroom

**Entirely absent, and not in the spec.** Iris is a destructive pixel/vector editor with no catalog or develop pipeline. None of the following exist:

- RAW decode/demosaic; camera profiles.
- Non-destructive **develop module**: Exposure, Contrast, Highlights/Shadows/Whites/Blacks, Temp/Tint white balance, parametric + point **Tone Curve**, HSL/Color mixer, Texture/Clarity/Dehaze, Vibrance/Saturation, split toning/color grading.
- Detail: noise reduction, capture/output sharpening.
- Lens corrections, geometry/upright, chromatic aberration, vignetting.
- Local adjustments: masking (brush/gradient/radial/AI subject-sky), healing/clone.
- **Library/catalog**: import, DAM, ratings/flags/keywords, collections, virtual copies, presets, history snapshots, batch sync, export presets.

> Note: `iris-pixel` already has F16/F32 bit-depth modeling, wide-gamut spaces (Display-P3, ProPhoto), and a non-destructive op log — a credible substrate for a develop module if the product decides to chase Lightroom. That decision should be made explicitly since the spec currently omits it.

---

## 4. Gaps vs Adobe Illustrator

`iris-vector` stores geometry but has **no editing layer**:

- **No bezier/anchor editing** — `BezPath` is effectively read-only; no add/remove/convert anchor, handle manipulation, pen/curvature tools.
- **No boolean path ops** (union/subtract/intersect/divide/trim) — spec calls for the `pathops` crate; not integrated.
- **No live/parametric shapes** (rect/ellipse/star/polygon as editable primitives) — SVG shapes are flattened to `BezPath` on read and never re-emitted as shapes.
- **No mesh gradients, no pattern fills** (`paint.rs:94` TODO) — only solid/linear/radial.
- **No symbols, no art/scatter/pattern brushes, no blends, no text-on-path, no envelope/warp, no live paint, no image trace, no recolor artwork.**
- **No per-object blend mode / opacity / appearance stack / effects** — those live only at the layer level.
- Artboards exist in `iris-aif` (`AifArtboard`) but there's no artboard editing UX.
- **AI format: 0%** (`iris-ai` is an 11-line stub). No import; export-as-SVG path not built.

### Format fidelity (SVG — the closest-to-usable vector path)
- Round-trips: paths, basic shapes (read→`BezPath`), groups, transforms, solid fill/stroke, dash, fill-rule, embedded raster (`<image>` base64 PNG).
- **Lost: gradients** (read ignores `url(#…)` paint servers `style.rs:43`; writer emits only the first stop `writer.rs:139`), **text, filters, clip paths, masks, patterns, symbols/`<use>`**, rounded-rect `rx/ry` (`shapes.rs:36`).

---

## 5. Cross-cutting infrastructure gaps
- **Compositor** is CPU-only; GPU compute path is a TODO (`compositor/mod.rs:71`). Vector/text layers are not composited at all (`composite.rs:36`).
- **Undo** is in-memory only; no `ops.bin` persistence (12-byte header stub), no Loro CRDT, no collaboration, no branching, blend-mode changes untracked.
- **Plugins**: `iris-plugin-api` is a 2-method trait; no wasmtime loader (Phase 5).
- **`aif-inspect` CLI**: stub that prints "not yet implemented".
- **Mobile**: platform detection exists but essentially no `COMPAT(mobile)` branches; touch/stylus/radial-menu/drawer from SPEC §11.3 unbuilt.

---

## 6. Actionable quick wins

Ranked by (value ÷ effort). Each leverages infrastructure that **already exists**, so these are wiring/surfacing tasks, not new subsystems. Sizes are rough: **S** ≈ ≤1 day, **M** ≈ 2–4 days.

| # | Quick win | Size | Why it's cheap | Impact |
|---|---|---|---|---|
| 1 | **Composite the other 26 blend modes** in the CPU path | M | Enum + AIF/PSD/ORA round-trip already done; just replace the `!= Normal` skip in `composite.rs:44` with per-mode separable math (Multiply/Screen/Overlay/…). Non-separable (Hue/Sat/Color/Lum) need HSL helpers. | Unlocks a headline Photoshop feature set instantly; makes ORA/PSD imports render correctly. |
| 2 | **Layers panel: opacity slider + blend-mode dropdown** | S | Model already stores `opacity`/`blend_mode`; `LayerOp::SetProp` exists for opacity. Pure UI wiring. | Core Photoshop interaction; pairs with #1. |
| 3 | **Track blend-mode changes in undo** (`LayerProp::BlendMode`) | S | Resolve the type-ownership note at `op.rs:119` (move `BlendMode` or add a u8 code); add the variant. | Removes a correctness gap; needed before #2 ships cleanly. |
| 4 | **SVG gradient round-trip** | M | `LinearGradient`/`RadialGradient`/stops already modeled. Emit `<defs><linearGradient>/<radialGradient>` + `fill="url(#id)"`; resolve `url(#…)` on read instead of warning (`style.rs:43`, `writer.rs:139`). | Restores real round-trip fidelity vs Illustrator/Inkscape/Figma SVG. |
| 5 | **SVG: re-emit basic shapes + honor `rx/ry` rounded rects** | S | Reader already parses them; add writer cases + `shapes.rs:36`. | Cleaner, smaller, more interoperable SVG output. |
| 6 | **PSD DPI read/write** (`ResolutionInfo` 0x03ED) | S | Single image-resource block; removes the hardcoded 72 (`convert.rs:18`, `writer/mod.rs:65`). | Fixes silently-wrong document scale on every PSD round-trip. |
| 7 | **Implement `aif-inspect`** (dump canvas, layer tree, tile counts, blend modes) | S | `AifReader` already exposes the full `AifDocument`. | Dev/QA velocity for every subsequent format task. |
| 8 | **Elliptical marquee + polygon/lasso selection** | M | Marquee plumbing (`state.selection`, brush clipping) exists for rects; generalize the selection to a `kurbo::BezPath`/path test. | Real selections are table-stakes; reuses existing clip path in the brush. |
| 9 | **Color picker dialog** (HS V/RGB/Hex) | M | Swatch state + `appthere-ui` tokens exist; needs a picker component bound to foreground color. | Unblocks meaningful brush/fill work. |
| 10 | **Boolean path ops** via the spec'd `pathops`/kurbo | M | Geometry storage exists; add union/subtract/intersect/divide as ops over `BezPath`. | First genuinely "Illustrator" vector capability; high demo value. |
| 11 | **Destructive Brightness/Contrast + Levels as a tile filter** | M | Tiles are F16 RGBA; apply a per-pixel transfer curve + undo via existing `TileOp` snapshots. | A first taste of adjustments without building the full adjustment-layer system; also the kernel for a future Lightroom develop slider. |
| 12 | **Keyboard shortcuts for existing tools** (B/E/I/G/M, ⌘Z/⇧⌘Z) | S | Tools already dispatch via `ToolEvent`; add a key handler that sets `tool_mode`/triggers undo. | Disproportionate UX uplift for trivial effort. |

**Suggested first sprint:** #1 + #2 + #3 together (blend modes end-to-end, visible and undoable) is the strongest single demo, followed by #4 (SVG gradients) and #7 (`aif-inspect`) as low-risk wins.

---

## 7. Larger efforts (scoped, not quick)
- Adjustment-layer subsystem (non-destructive, GPU LUTs) → then the Lightroom develop module + RAW pipeline (**requires a product decision**, currently unspecced).
- Layer masks + layer effects/styles.
- Full vector editing tools (pen/anchor/handles), live shapes, mesh gradients, patterns, symbols, text, type-on-path.
- Text engine (Parley) for raster + vector text.
- GPU compute compositor (WGSL) replacing the CPU path.
- CMYK / wide-gamut ICC compositing.
- PSB + CMYK/Lab/16-32bpc PSD; AI import.
- Loro CRDT op-log persistence, collaboration, WASI plugin host.
- Mobile touch/stylus UX (SPEC §11.3).

---

*Generated by an audit-only pass. No implementation code was written. File:line citations refer to the working tree at audit time.*
