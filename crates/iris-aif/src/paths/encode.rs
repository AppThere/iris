// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Encoder for `iris/layers/{id}/paths.bin` (SPEC.md §4.10).
//!
//! Built on the upstream `FlatBufferBuilder`, whose API is entirely safe. Output
//! is byte-compatible with `flatc`-generated readers for the §4.10 schema.
//!
//! Precision note: the schema stores coordinates, stroke metrics, and gradient
//! geometry as `float`. `iris-vector` models them as `f64` (kurbo's native
//! type), so a round-trip through `paths.bin` narrows to f32. This is inherent
//! to the format, not a defect in this encoder.

use flatbuffers::{FlatBufferBuilder, WIPOffset};
use iris_vector::{
    BezPath, Color, ColorStop, Paint, PathEl, PathObject, Point, StrokePaint, VectorLayer,
};

use super::schema::{
    color_stop, linear_gradient, paint, paint_variant, path_data, path_object, path_store,
    radial_gradient, solid_color, stroke_paint, DEFAULT_MITER_LIMIT, FILE_IDENTIFIER,
    PATH_STORE_VERSION, VERB_CLOSE, VERB_CUBIC_TO, VERB_LINE_TO, VERB_MOVE_TO, VERB_QUAD_TO,
};

/// Serialise a [`VectorLayer`] into `paths.bin` bytes.
pub(crate) fn write_path_store(layer: &VectorLayer) -> Vec<u8> {
    let mut b = FlatBufferBuilder::new();

    let object_offsets: Vec<_> = layer.objects.iter().map(|o| encode_object(&mut b, o)).collect();
    let objects = b.create_vector(&object_offsets);
    let color_space = b.create_string(&layer.color_space);

    let root = b.start_table();
    b.push_slot::<u32>(slot(path_store::VERSION), PATH_STORE_VERSION, 0);
    b.push_slot_always::<WIPOffset<&str>>(slot(path_store::COLOR_SPACE), color_space);
    b.push_slot_always::<WIPOffset<_>>(slot(path_store::OBJECTS), objects);
    let root = b.end_table(root);

    b.finish(root, Some(FILE_IDENTIFIER));
    b.finished_data().to_vec()
}

/// FlatBuffers vtable offset for a zero-based field slot number.
fn slot(n: u16) -> u16 {
    4 + 2 * n
}

// ── Objects ───────────────────────────────────────────────────────────────────

fn encode_object<'b>(
    b: &mut FlatBufferBuilder<'b>,
    obj: &PathObject,
) -> WIPOffset<flatbuffers::TableFinishedWIPOffset> {
    // Children must be built before the parent table is started.
    let id = b.create_vector(&obj.id.to_bytes_le());
    let path = encode_path_data(b, &obj.path);
    let fill = obj.fill.as_ref().map(|p| encode_paint(b, p));
    let stroke = obj.stroke.as_ref().map(|s| encode_stroke(b, s));
    let coeffs = obj.transform.as_coeffs().map(|c| c as f32);
    let transform = b.create_vector(&coeffs);
    let name = (!obj.name.is_empty()).then(|| b.create_string(&obj.name));

    let t = b.start_table();
    b.push_slot_always::<WIPOffset<_>>(slot(path_object::ID), id);
    b.push_slot_always::<WIPOffset<_>>(slot(path_object::PATH), path);
    if let Some(fill) = fill {
        b.push_slot_always::<WIPOffset<_>>(slot(path_object::FILL), fill);
    }
    if let Some(stroke) = stroke {
        b.push_slot_always::<WIPOffset<_>>(slot(path_object::STROKE), stroke);
    }
    b.push_slot::<i8>(slot(path_object::FILL_RULE), fill_rule_byte(obj.fill_rule), 0);
    b.push_slot_always::<WIPOffset<_>>(slot(path_object::TRANSFORM), transform);
    b.push_slot::<bool>(slot(path_object::VISIBLE), obj.visible, true);
    // `PathObject.locked` has no counterpart in iris_vector::PathObject; the
    // schema default (false) is therefore always elided on write.
    if let Some(name) = name {
        b.push_slot_always::<WIPOffset<&str>>(slot(path_object::NAME), name);
    }
    b.end_table(t)
}

fn encode_path_data<'b>(
    b: &mut FlatBufferBuilder<'b>,
    path: &BezPath,
) -> WIPOffset<flatbuffers::TableFinishedWIPOffset> {
    let mut verbs: Vec<i8> = Vec::new();
    let mut points: Vec<f32> = Vec::new();
    let mut push = |p: Point| {
        points.push(p.x as f32);
        points.push(p.y as f32);
    };
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                verbs.push(VERB_MOVE_TO);
                push(p);
            }
            PathEl::LineTo(p) => {
                verbs.push(VERB_LINE_TO);
                push(p);
            }
            PathEl::QuadTo(p1, p2) => {
                verbs.push(VERB_QUAD_TO);
                push(p1);
                push(p2);
            }
            PathEl::CurveTo(p1, p2, p3) => {
                verbs.push(VERB_CUBIC_TO);
                push(p1);
                push(p2);
                push(p3);
            }
            PathEl::ClosePath => verbs.push(VERB_CLOSE),
        }
    }

    let verbs = b.create_vector(&verbs);
    let points = b.create_vector(&points);
    let t = b.start_table();
    b.push_slot_always::<WIPOffset<_>>(slot(path_data::VERBS), verbs);
    b.push_slot_always::<WIPOffset<_>>(slot(path_data::POINTS), points);
    b.end_table(t)
}

// ── Paints ────────────────────────────────────────────────────────────────────

fn encode_paint<'b>(
    b: &mut FlatBufferBuilder<'b>,
    p: &Paint,
) -> WIPOffset<flatbuffers::TableFinishedWIPOffset> {
    let (tag, variant) = match p {
        Paint::Solid(c) => (paint_variant::SOLID, encode_solid(b, *c)),
        Paint::Linear(g) => {
            let stop_offsets = encode_stops(b, &g.stops);
            let stops = b.create_vector(&stop_offsets);
            let t = b.start_table();
            b.push_slot::<f32>(slot(linear_gradient::X0), g.start.x as f32, 0.0);
            b.push_slot::<f32>(slot(linear_gradient::Y0), g.start.y as f32, 0.0);
            b.push_slot::<f32>(slot(linear_gradient::X1), g.end.x as f32, 0.0);
            b.push_slot::<f32>(slot(linear_gradient::Y1), g.end.y as f32, 0.0);
            b.push_slot_always::<WIPOffset<_>>(slot(linear_gradient::STOPS), stops);
            b.push_slot::<i8>(slot(linear_gradient::SPREAD), spread_byte(g.spread), 0);
            (paint_variant::LINEAR, b.end_table(t))
        }
        Paint::Radial(g) => {
            let stop_offsets = encode_stops(b, &g.stops);
            let stops = b.create_vector(&stop_offsets);
            let t = b.start_table();
            b.push_slot::<f32>(slot(radial_gradient::CX), g.center.x as f32, 0.0);
            b.push_slot::<f32>(slot(radial_gradient::CY), g.center.y as f32, 0.0);
            b.push_slot::<f32>(slot(radial_gradient::FX), g.focus.x as f32, 0.0);
            b.push_slot::<f32>(slot(radial_gradient::FY), g.focus.y as f32, 0.0);
            b.push_slot::<f32>(slot(radial_gradient::RADIUS), g.radius as f32, 0.0);
            b.push_slot_always::<WIPOffset<_>>(slot(radial_gradient::STOPS), stops);
            b.push_slot::<i8>(slot(radial_gradient::SPREAD), spread_byte(g.spread), 0);
            (paint_variant::RADIAL, b.end_table(t))
        }
    };

    let t = b.start_table();
    b.push_slot::<u8>(slot(paint::VARIANT_TYPE), tag, paint_variant::NONE);
    b.push_slot_always::<WIPOffset<_>>(slot(paint::VARIANT), variant);
    b.end_table(t)
}

fn encode_solid<'b>(
    b: &mut FlatBufferBuilder<'b>,
    c: Color,
) -> WIPOffset<flatbuffers::TableFinishedWIPOffset> {
    let t = b.start_table();
    b.push_slot::<f32>(slot(solid_color::R), c.r, 0.0);
    b.push_slot::<f32>(slot(solid_color::G), c.g, 0.0);
    b.push_slot::<f32>(slot(solid_color::B), c.b, 0.0);
    b.push_slot::<f32>(slot(solid_color::A), c.a, 0.0);
    b.end_table(t)
}

/// Encode each stop to its own table. Callers must feed the result to
/// `create_vector` *before* starting the enclosing gradient table — FlatBuffers
/// forbids nested table construction.
fn encode_stops<'b>(
    b: &mut FlatBufferBuilder<'b>,
    stops: &[ColorStop],
) -> Vec<WIPOffset<flatbuffers::TableFinishedWIPOffset>> {
    stops
        .iter()
        .map(|s| {
            let t = b.start_table();
            b.push_slot::<f32>(slot(color_stop::OFFSET), s.offset, 0.0);
            b.push_slot::<f32>(slot(color_stop::R), s.color.r, 0.0);
            b.push_slot::<f32>(slot(color_stop::G), s.color.g, 0.0);
            b.push_slot::<f32>(slot(color_stop::B), s.color.b, 0.0);
            b.push_slot::<f32>(slot(color_stop::A), s.color.a, 0.0);
            b.end_table(t)
        })
        .collect()
}

fn encode_stroke<'b>(
    b: &mut FlatBufferBuilder<'b>,
    s: &StrokePaint,
) -> WIPOffset<flatbuffers::TableFinishedWIPOffset> {
    let p = encode_paint(b, &s.paint);
    let dashes: Vec<f32> = s.dash_array.iter().map(|d| *d as f32).collect();
    let dash_array = (!dashes.is_empty()).then(|| b.create_vector(&dashes));

    let t = b.start_table();
    b.push_slot_always::<WIPOffset<_>>(slot(stroke_paint::PAINT), p);
    b.push_slot::<f32>(slot(stroke_paint::WIDTH), s.width as f32, 0.0);
    b.push_slot::<i8>(slot(stroke_paint::CAP), cap_byte(s.cap), 0);
    b.push_slot::<i8>(slot(stroke_paint::JOIN), join_byte(s.join), 0);
    b.push_slot::<f32>(
        slot(stroke_paint::MITER_LIMIT),
        s.miter_limit as f32,
        DEFAULT_MITER_LIMIT,
    );
    if let Some(dash_array) = dash_array {
        b.push_slot_always::<WIPOffset<_>>(slot(stroke_paint::DASH_ARRAY), dash_array);
    }
    b.push_slot::<f32>(slot(stroke_paint::DASH_OFFSET), s.dash_offset as f32, 0.0);
    b.end_table(t)
}

// ── Enum mapping ──────────────────────────────────────────────────────────────

fn fill_rule_byte(r: iris_vector::FillRule) -> i8 {
    match r {
        iris_vector::FillRule::NonZero => 0,
        iris_vector::FillRule::EvenOdd => 1,
    }
}

fn cap_byte(c: iris_vector::LineCap) -> i8 {
    match c {
        iris_vector::LineCap::Butt => 0,
        iris_vector::LineCap::Round => 1,
        iris_vector::LineCap::Square => 2,
    }
}

fn join_byte(j: iris_vector::LineJoin) -> i8 {
    match j {
        iris_vector::LineJoin::Miter => 0,
        iris_vector::LineJoin::Round => 1,
        iris_vector::LineJoin::Bevel => 2,
    }
}

fn spread_byte(s: iris_vector::SpreadMode) -> i8 {
    match s {
        iris_vector::SpreadMode::Pad => 0,
        iris_vector::SpreadMode::Reflect => 1,
        iris_vector::SpreadMode::Repeat => 2,
    }
}
