// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Draws a [`VectorLayer`]'s objects into the compositor accumulation buffer.

use iris_vector::{
    Affine, BezPath, FillRule, LineCap, LineJoin, PathObject, StrokePaint, VectorLayer,
};

use crate::viewport::CanvasViewport;

use super::paint::DevicePaint;
use super::raster::fill_path;

/// Device-space flattening tolerance for stroke expansion, in pixels.
const STROKE_TOLERANCE_PX: f64 = 0.1;

/// Rasterise every visible object in `vl` onto `acc`.
///
/// `acc` is premultiplied linear-light RGBA at `physical_w × physical_h`.
/// Objects are drawn in list order (index 0 lowest), fill first then stroke,
/// matching the painter's-algorithm order the pixel path uses.
#[allow(clippy::too_many_arguments)] // coordinate-space plumbing, as in `cpu::blit_tile`
pub(crate) fn draw_vector_layer(
    acc: &mut [f32],
    vl: &VectorLayer,
    viewport: &CanvasViewport,
    physical_w: u32,
    physical_h: u32,
    logical_w: u32,
    logical_h: u32,
    opacity: f32,
) {
    if logical_w == 0 || logical_h == 0 || opacity <= 0.0 {
        return;
    }
    // Viewport transforms are defined in logical (CSS) pixels; the buffer is in
    // physical pixels, so fold the DPI ratio in last.
    let scale_x = physical_w as f64 / logical_w as f64;
    let scale_y = physical_h as f64 / logical_h as f64;
    let doc_to_device = Affine::scale_non_uniform(scale_x, scale_y)
        * viewport.doc_to_screen_affine(logical_w, logical_h);

    for obj in &vl.objects {
        if !obj.visible {
            continue;
        }
        draw_object(acc, obj, doc_to_device, physical_w, physical_h, opacity);
    }
}

fn draw_object(
    acc: &mut [f32],
    obj: &PathObject,
    doc_to_device: Affine,
    width: u32,
    height: u32,
    opacity: f32,
) {
    let object_to_device = doc_to_device * obj.transform;

    if let Some(fill) = obj.fill.as_ref() {
        if let Some(paint) = DevicePaint::new(fill, object_to_device, opacity) {
            let device_path = object_to_device * obj.path.clone();
            let even_odd = obj.fill_rule == FillRule::EvenOdd;
            rasterise(acc, &device_path, even_odd, width, height, &paint);
        }
    }

    if let Some(stroke) = obj.stroke.as_ref() {
        if let Some(paint) = DevicePaint::new(&stroke.paint, object_to_device, opacity) {
            // Expand in object space so the stroke width picks up the object and
            // viewport scale, then map the resulting outline to device space.
            let outline = expand_stroke(&obj.path, stroke, object_to_device);
            let device_path = object_to_device * outline;
            // A stroke outline is always filled non-zero: even-odd would hollow
            // out self-overlapping joins.
            rasterise(acc, &device_path, false, width, height, &paint);
        }
    }
}

/// Convert a stroke into a fillable outline in object space.
fn expand_stroke(path: &BezPath, stroke: &StrokePaint, object_to_device: Affine) -> BezPath {
    let style = kurbo::Stroke {
        width: stroke.width,
        join: match stroke.join {
            LineJoin::Miter => kurbo::Join::Miter,
            LineJoin::Round => kurbo::Join::Round,
            LineJoin::Bevel => kurbo::Join::Bevel,
        },
        miter_limit: stroke.miter_limit,
        start_cap: cap_of(stroke.cap),
        end_cap: cap_of(stroke.cap),
        dash_pattern: stroke.dash_array.iter().copied().collect(),
        dash_offset: stroke.dash_offset,
    };
    // Tolerance is specified in object units, but the error that matters is in
    // device pixels — divide by the transform's linear scale so a zoomed-in path
    // is flattened more finely rather than turning visibly polygonal.
    let scale = object_to_device.determinant().abs().sqrt().max(f64::EPSILON);
    let tolerance = STROKE_TOLERANCE_PX / scale;
    kurbo::stroke(path.elements().iter().copied(), &style, &kurbo::StrokeOpts::default(), tolerance)
}

fn cap_of(cap: LineCap) -> kurbo::Cap {
    match cap {
        LineCap::Butt => kurbo::Cap::Butt,
        LineCap::Round => kurbo::Cap::Round,
        LineCap::Square => kurbo::Cap::Square,
    }
}

/// Rasterise `path` and blend its coverage into `acc` with Porter-Duff "over".
fn rasterise(
    acc: &mut [f32],
    path: &BezPath,
    even_odd: bool,
    width: u32,
    height: u32,
    paint: &DevicePaint,
) {
    fill_path(path, even_odd, width, height, |x, y, coverage| {
        let src = paint.sample(x as f64 + 0.5, y as f64 + 0.5);
        let a = src[3] * coverage;
        if a <= 0.0 {
            return;
        }
        let ob = (y * width as usize + x) * 4;
        let inv = 1.0 - a;
        // `src` is already premultiplied, so scaling by coverage keeps it so.
        acc[ob] = src[0] * coverage + acc[ob] * inv;
        acc[ob + 1] = src[1] * coverage + acc[ob + 1] * inv;
        acc[ob + 2] = src[2] * coverage + acc[ob + 2] * inv;
        acc[ob + 3] = a + acc[ob + 3] * inv;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use iris_vector::{Color, Paint, Point};

    const W: u32 = 32;
    const H: u32 = 32;

    /// A viewport that maps document space 1:1 onto the buffer, origin at (0,0).
    fn identity_viewport() -> CanvasViewport {
        let mut v = CanvasViewport::new();
        v.pan = kurbo::Vec2::new(W as f64 / 2.0, H as f64 / 2.0);
        v
    }

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(x0, y0));
        p.line_to(Point::new(x1, y0));
        p.line_to(Point::new(x1, y1));
        p.line_to(Point::new(x0, y1));
        p.close_path();
        p
    }

    fn draw(vl: &VectorLayer, opacity: f32) -> Vec<f32> {
        let mut acc = vec![0.0_f32; (W * H) as usize * 4];
        draw_vector_layer(&mut acc, vl, &identity_viewport(), W, H, W, H, opacity);
        acc
    }

    fn px(acc: &[f32], x: usize, y: usize) -> [f32; 4] {
        let i = (y * W as usize + x) * 4;
        [acc[i], acc[i + 1], acc[i + 2], acc[i + 3]]
    }

    fn layer_with(objects: Vec<PathObject>) -> VectorLayer {
        VectorLayer { objects, color_space: "srgb".into() }
    }

    #[test]
    fn solid_fill_lands_on_the_buffer() {
        let obj = PathObject::new(square(8.0, 8.0, 24.0, 24.0), Some(Paint::Solid(Color::BLACK)));
        let acc = draw(&layer_with(vec![obj]), 1.0);
        let inside = px(&acc, 16, 16);
        assert!((inside[3] - 1.0).abs() < 1e-3, "interior must be opaque, got {inside:?}");
        assert!(inside[0] < 1e-3, "black fill must have ~zero red");
        assert_eq!(px(&acc, 2, 2)[3], 0.0, "outside the square must stay clear");
    }

    #[test]
    fn invisible_objects_are_skipped() {
        let mut obj = PathObject::new(square(8.0, 8.0, 24.0, 24.0), Some(Paint::Solid(Color::BLACK)));
        obj.visible = false;
        let acc = draw(&layer_with(vec![obj]), 1.0);
        assert_eq!(px(&acc, 16, 16)[3], 0.0);
    }

    #[test]
    fn layer_opacity_scales_the_result() {
        let obj = PathObject::new(square(8.0, 8.0, 24.0, 24.0), Some(Paint::Solid(Color::BLACK)));
        let acc = draw(&layer_with(vec![obj]), 0.5);
        assert!((px(&acc, 16, 16)[3] - 0.5).abs() < 1e-3);
    }

    #[test]
    fn white_fill_converts_through_linear_light() {
        let white = Color::new(1.0, 1.0, 1.0, 1.0);
        let obj = PathObject::new(square(8.0, 8.0, 24.0, 24.0), Some(Paint::Solid(white)));
        let acc = draw(&layer_with(vec![obj]), 1.0);
        let inside = px(&acc, 16, 16);
        assert!((inside[0] - 1.0).abs() < 1e-3, "sRGB white is linear 1.0");
    }

    #[test]
    fn objects_paint_in_list_order() {
        // Index 0 is lowest, so the second object must win where they overlap.
        let under = PathObject::new(square(8.0, 8.0, 24.0, 24.0), Some(Paint::Solid(Color::BLACK)));
        let over = PathObject::new(
            square(8.0, 8.0, 24.0, 24.0),
            Some(Paint::Solid(Color::new(1.0, 1.0, 1.0, 1.0))),
        );
        let acc = draw(&layer_with(vec![under, over]), 1.0);
        assert!(px(&acc, 16, 16)[0] > 0.9, "the later object must be on top");
    }

    #[test]
    fn stroke_marks_the_outline_but_not_the_interior() {
        let mut obj = PathObject::new(square(8.0, 8.0, 24.0, 24.0), None);
        obj.stroke = Some(StrokePaint::new(Paint::Solid(Color::BLACK), 2.0));
        let acc = draw(&layer_with(vec![obj]), 1.0);
        assert!(px(&acc, 16, 16)[3] < 1e-3, "unfilled interior must stay clear");
        assert!(px(&acc, 8, 16)[3] > 0.5, "the stroked edge must be painted");
    }

    #[test]
    fn object_transform_moves_the_geometry() {
        let mut obj = PathObject::new(square(0.0, 0.0, 8.0, 8.0), Some(Paint::Solid(Color::BLACK)));
        obj.transform = Affine::translate((16.0, 16.0));
        let acc = draw(&layer_with(vec![obj]), 1.0);
        assert!(px(&acc, 20, 20)[3] > 0.9, "geometry must follow its transform");
        assert_eq!(px(&acc, 4, 4)[3], 0.0, "and vacate the untransformed position");
    }

    #[test]
    fn zoom_scales_rendered_geometry() {
        let obj = PathObject::new(square(0.0, 0.0, 4.0, 4.0), Some(Paint::Solid(Color::BLACK)));
        let vl = layer_with(vec![obj]);
        let mut v = identity_viewport();
        v.zoom = 4.0;
        v.pan = kurbo::Vec2::new(2.0, 2.0); // centre the square
        let mut acc = vec![0.0_f32; (W * H) as usize * 4];
        draw_vector_layer(&mut acc, &vl, &v, W, H, W, H, 1.0);
        // A 4×4 doc square at 4× zoom covers 16×16 device px around the centre.
        assert!(px(&acc, 16, 16)[3] > 0.9);
        assert!(px(&acc, 9, 9)[3] > 0.9, "zoomed square must reach well past 4px");
    }

    #[test]
    fn empty_layer_leaves_the_buffer_untouched() {
        let acc = draw(&layer_with(Vec::new()), 1.0);
        assert!(acc.iter().all(|&c| c == 0.0));
    }
}
