## `iris-canvas` — Iris infinite canvas

**Gate:** ADR 006 gate open (`appthere-canvas` published, Loki updated).

### Phase 1 milestone

A scrollable, zoomable Dioxus Native canvas displaying a `LayerTree`'s pixel tiles composited
using `normal` blend mode only. No vector rendering yet.

### Public API at milestone completion

```rust
pub use viewport::{CanvasViewport, ZoomLevel};
pub use compositor::{Compositor, CompositorError};
pub use canvas_widget::IrisCanvas;  // Dioxus component

// viewport.rs
pub struct CanvasViewport {
    pub pan: glam::Vec2,
    pub zoom: f32,       // 0.1 – 64.0
    pub rotation: f32,   // radians
}
impl CanvasViewport {
    pub fn new() -> Self;  // pan=(0,0), zoom=1.0, rotation=0.0
    pub fn screen_to_doc(&self, screen: glam::Vec2, screen_size: glam::UVec2) -> glam::Vec2;
    pub fn doc_to_screen(&self, doc: glam::Vec2, screen_size: glam::UVec2) -> glam::Vec2;
    pub fn visible_doc_rect(&self, screen_size: glam::UVec2) -> kurbo::Rect;
    pub fn zoom_to(&mut self, new_zoom: f32, anchor_screen: glam::Vec2, screen_size: glam::UVec2);
    pub fn clamp_zoom(&mut self);
}

pub struct ZoomLevel(f32);
impl ZoomLevel {
    pub const MIN: f32 = 0.1;
    pub const MAX: f32 = 64.0;
}

// compositor.rs — Phase 1: normal blend only; other blend modes added in Phase 4
pub struct Compositor { /* holds wgpu device/queue refs via appthere-canvas */ }
impl Compositor {
    pub fn composite_tile(
        &self,
        layers: &[(&iris_pixel::Layer, Option<&iris_pixel::TileData>)],
        output: &mut iris_pixel::TileData,
    ) -> Result<(), CompositorError>;
}

// canvas_widget.rs — Dioxus component
// Props: LayerTree ref, CanvasViewport (read/write signal), tool event handler
#[component]
pub fn IrisCanvas(/* ... */) -> Element;
```

### Do not implement yet

- Vector layer rendering (Phase 3)
- Non-normal blend modes (Phase 4)
- Layer effects (Phase 4)
- Overlay pass: selection marquee, path anchors, guides (Phase 3)
- Soft-proof colour pass (Phase 4)

### Test requirements

- `CanvasViewport::screen_to_doc` is the inverse of `doc_to_screen` (round-trip within float tolerance)
- `visible_doc_rect` at zoom=1.0 covers exactly screen_size pixels
- `zoom_to` clamps to `ZoomLevel::MIN` and `ZoomLevel::MAX`
- `Compositor::composite_tile` with a single fully-opaque tile returns that tile's pixel data unchanged
- `Compositor::composite_tile` with two normal-blend layers produces correct alpha-composite result
