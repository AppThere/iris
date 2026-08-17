// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! CPU composite path — the one the Blitz paint loop actually drives.
//!
//! Uses `queue.write_texture()` rather than a `CommandEncoder`, so it cannot
//! corrupt Vello's in-progress encoder (see `paint_bridge.rs`).
//!
//! All compositing happens in a premultiplied **linear-light** f32 accumulation
//! buffer; the buffer is converted to straight-alpha sRGB u8 only at upload.

use iris_pixel::{LayerContent, LayerTree, TileCoord, TileData, TILE_SIZE};

use crate::compositor::encode::{f16_to_f32, upload_acc};
use crate::compositor::CompositorError;
use crate::vector::draw_vector_layer;
use crate::viewport::CanvasViewport;

/// Composite `tree` into an `Rgba8Unorm` texture sized `width_px × height_px`.
///
// COMPAT(blitz): blitz-paint passes physical pixel dimensions; `scale` is the
// DPI factor. Viewport transforms must use logical (CSS) dimensions so they
// match screen_to_doc() in the event handler. Pixel placement uses physical.
#[allow(clippy::too_many_arguments)] // mirrors the CustomPaintSource::render signature
pub(crate) fn composite_to_texture(
    tree: &LayerTree,
    viewport: &CanvasViewport,
    width_px: u32,
    height_px: u32,
    scale: f64,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<wgpu::Texture, CompositorError> {
    let acc = composite_to_buffer(tree, viewport, width_px, height_px, scale);
    let pixel_count = (width_px * height_px) as usize;
    upload_acc(&acc, pixel_count, width_px, height_px, device, queue)
}

/// Composite into a premultiplied linear-light RGBA buffer.
///
/// Split out from [`composite_to_texture`] so the layer walk can be exercised
/// without a GPU adapter — CI has no device to create textures on.
pub(crate) fn composite_to_buffer(
    tree: &LayerTree,
    viewport: &CanvasViewport,
    width_px: u32,
    height_px: u32,
    scale: f64,
) -> Vec<f32> {
    let logical_w = ((width_px as f64) / scale.max(1.0)).round() as u32;
    let logical_h = ((height_px as f64) / scale.max(1.0)).round() as u32;

    let pixel_count = (width_px * height_px) as usize;
    // Premultiplied linear-light f32 RGBA accumulation buffer.
    let mut acc = vec![0.0_f32; pixel_count * 4];

    fill_document_background(
        &mut acc, viewport,
        width_px, height_px, logical_w, logical_h,
        tree.canvas_width, tree.canvas_height,
    );

    let visible_rect = viewport.visible_doc_rect(logical_w, logical_h);
    let layers: Vec<_> = tree.iter_depth_first().collect();

    // Depth-first order is top-first, so walk it in reverse to paint bottom-up.
    for layer in layers.iter().rev() {
        if !layer.visible {
            continue;
        }
        match layer.content {
            LayerContent::Pixel(ref px) => {
                let offset_x = px.canvas_offset_x as f64;
                let offset_y = px.canvas_offset_y as f64;
                let ts = TILE_SIZE as f64;
                let tx_min = ((visible_rect.x0 - offset_x) / ts).floor().max(0.0) as u32;
                let ty_min = ((visible_rect.y0 - offset_y) / ts).floor().max(0.0) as u32;
                let tx_max = ((visible_rect.x1 - offset_x) / ts).ceil().max(0.0) as u32;
                let ty_max = ((visible_rect.y1 - offset_y) / ts).ceil().max(0.0) as u32;
                for ty in ty_min..=ty_max {
                    for tx in tx_min..=tx_max {
                        if let Some(tile_data) = px.tiles.get(TileCoord { tx, ty }) {
                            blit_tile(
                                &mut acc, tile_data, tx, ty,
                                offset_x, offset_y,
                                viewport,
                                width_px, height_px, logical_w, logical_h,
                                layer.opacity,
                            );
                        }
                        // Absent tile = transparent = no contribution to composite.
                    }
                }
            }
            LayerContent::Vector(ref vl) => {
                draw_vector_layer(
                    &mut acc, vl, viewport,
                    width_px, height_px, logical_w, logical_h,
                    layer.opacity,
                );
            }
            // TODO(iris): SPEC.md §6.2 — Phase 3+: text layer compositing.
            _ => {}
        }
    }

    acc
}

// ── CPU composite helpers ─────────────────────────────────────────────────────

/// Forward-map a tile's pixels onto the accumulation buffer using Porter-Duff over.
///
/// Straight-alpha f16 source pixels are premultiplied before compositing.
#[allow(clippy::too_many_arguments)] // coordinate-space plumbing; splitting would obscure it
fn blit_tile(
    acc: &mut [f32],
    tile_data: &TileData,
    tx: u32, ty: u32,
    offset_x: f64, offset_y: f64,
    viewport: &CanvasViewport,
    physical_w: u32, physical_h: u32,
    logical_w: u32, logical_h: u32,
    opacity: f32,
) {
    let scale_x = physical_w as f64 / logical_w as f64;
    let scale_y = physical_h as f64 / logical_h as f64;
    let ts = TILE_SIZE as usize;
    let bytes = &tile_data.0;
    for py in 0..ts {
        for px_local in 0..ts {
            let doc = kurbo::Vec2::new(
                offset_x + (tx as f64 * ts as f64) + px_local as f64 + 0.5,
                offset_y + (ty as f64 * ts as f64) + py as f64 + 0.5,
            );
            let screen = viewport.doc_to_screen(doc, logical_w, logical_h);
            let sx = (screen.x * scale_x) as i64;
            let sy = (screen.y * scale_y) as i64;
            if sx < 0 || sy < 0 || sx >= physical_w as i64 || sy >= physical_h as i64 {
                continue;
            }
            let ib = (py * ts + px_local) * 8; // 4 channels × 2 bytes per f16
            let sr = f16_to_f32(u16::from_le_bytes([bytes[ib],     bytes[ib + 1]]));
            let sg = f16_to_f32(u16::from_le_bytes([bytes[ib + 2], bytes[ib + 3]]));
            let sb = f16_to_f32(u16::from_le_bytes([bytes[ib + 4], bytes[ib + 5]]));
            let sa = f16_to_f32(u16::from_le_bytes([bytes[ib + 6], bytes[ib + 7]])) * opacity;
            let ob = (sy as usize * physical_w as usize + sx as usize) * 4;
            let inv = 1.0 - sa;
            // Porter-Duff "over": dst = src_premul + dst × (1 − src_alpha)
            acc[ob    ] = sr * sa + acc[ob    ] * inv;
            acc[ob + 1] = sg * sa + acc[ob + 1] * inv;
            acc[ob + 2] = sb * sa + acc[ob + 2] * inv;
            acc[ob + 3] =    sa   + acc[ob + 3] * inv;
        }
    }
}

/// Fill the screen-space region corresponding to the document boundary with opaque white.
///
/// Called before layer compositing so painted content composites on top of the
/// white background. Pixels outside the document boundary remain transparent.
#[allow(clippy::too_many_arguments)] // coordinate-space plumbing; see `blit_tile`
fn fill_document_background(
    acc: &mut [f32],
    viewport: &CanvasViewport,
    physical_w: u32, physical_h: u32,
    logical_w: u32, logical_h: u32,
    doc_w: u32,
    doc_h: u32,
) {
    let scale_x = physical_w as f64 / logical_w as f64;
    let scale_y = physical_h as f64 / logical_h as f64;
    // Use logical dims for transform; scale to physical pixel coords.
    let top_left = viewport.doc_to_screen(kurbo::Vec2::new(0.0, 0.0), logical_w, logical_h);
    let bottom_right = viewport.doc_to_screen(
        kurbo::Vec2::new(doc_w as f64, doc_h as f64),
        logical_w, logical_h,
    );
    let x0 = ((top_left.x * scale_x).floor() as i64).max(0) as usize;
    let y0 = ((top_left.y * scale_y).floor() as i64).max(0) as usize;
    let x1 = ((bottom_right.x * scale_x).ceil() as i64).min(physical_w as i64) as usize;
    let y1 = ((bottom_right.y * scale_y).ceil() as i64).min(physical_h as i64) as usize;
    for sy in y0..y1 {
        for sx in x0..x1 {
            let ob = (sy * physical_w as usize + sx) * 4;
            // Premultiplied opaque white: (1, 1, 1, 1).
            acc[ob    ] = 1.0;
            acc[ob + 1] = 1.0;
            acc[ob + 2] = 1.0;
            acc[ob + 3] = 1.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::viewport::CanvasViewport;
    use iris_pixel::{BlendMode, Layer, LayerTree, VectorLayer};
    use iris_vector::{Color, Paint, PathObject, Point};

    const W: u32 = 64;
    const H: u32 = 64;

    fn vector_tree() -> LayerTree {
        let mut tree = LayerTree::new(W, H, 96.0, 96.0);
        let mut path = iris_vector::BezPath::new();
        path.move_to(Point::new(8.0, 8.0));
        path.line_to(Point::new(56.0, 8.0));
        path.line_to(Point::new(56.0, 56.0));
        path.line_to(Point::new(8.0, 56.0));
        path.close_path();
        let obj = PathObject::new(path, Some(Paint::Solid(Color::new(1.0, 0.0, 0.0, 1.0))));
        tree.add_layer(
            None,
            0,
            Layer {
                id: uuid::Uuid::new_v4(),
                name: "Art".into(),
                visible: true,
                locked: false,
                opacity: 1.0,
                blend_mode: BlendMode::Normal,
                clipping_mask: false,
                mask: None,
                content: LayerContent::Vector(VectorLayer {
                    objects: vec![obj],
                    color_space: "srgb".into(),
                }),
            },
        )
        .expect("add vector layer");
        tree
    }

    fn centred_viewport() -> CanvasViewport {
        let mut v = CanvasViewport::new();
        v.pan = kurbo::Vec2::new(W as f64 / 2.0, H as f64 / 2.0);
        v
    }

    #[test]
    fn vector_layers_reach_the_composite_buffer() {
        // Regression guard: the compositor used to skip every non-pixel layer,
        // so an imported SVG rendered as a blank canvas.
        let acc = composite_to_buffer(&vector_tree(), &centred_viewport(), W, H, 1.0);
        let centre = (32 * W as usize + 32) * 4;
        assert!(acc[centre] > 0.9, "red fill must reach the buffer, got {}", acc[centre]);
        assert!(acc[centre + 1] < 0.05, "and carry no green");
        assert!((acc[centre + 3] - 1.0).abs() < 1e-3, "and be opaque");
    }

    #[test]
    fn hidden_vector_layer_leaves_only_the_background() {
        let mut tree = vector_tree();
        let id = tree.root_layer_ids()[0];
        if let Some(l) = tree.get_mut(id) {
            l.visible = false;
        }
        let acc = composite_to_buffer(&tree, &centred_viewport(), W, H, 1.0);
        let centre = (32 * W as usize + 32) * 4;
        // Background fill is opaque white; the red square must not appear.
        assert!(acc[centre + 1] > 0.9, "hidden layer must leave white background");
    }
}
