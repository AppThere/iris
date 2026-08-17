// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Full-package round-trip for vector layers: `LayerTree` → `.aif` → `LayerTree`
//! (SPEC.md §4.10). Guards against the geometry loss that occurred while
//! `paths.bin` was unimplemented and vector layers decoded back as empty groups.

use std::io::{Cursor, Seek, SeekFrom};

use iris_aif::{
    document::{AifArtboard, AifCanvas, AifDocument, CanvasMode},
    AifReader, AifWriter, WriteOptions,
};
use iris_pixel::{
    BitDepth, BlendMode, Layer, LayerContent, LayerTree, VectorLayer,
};
use iris_vector::{
    Affine, BezPath, Color, ColorStop, FillRule, LineCap, LineJoin, LinearGradient, Paint,
    PathObject, Point, StrokePaint,
};
use uuid::Uuid;

fn star_path() -> BezPath {
    let mut p = BezPath::new();
    p.move_to(Point::new(50.0, 0.0));
    p.line_to(Point::new(61.0, 35.0));
    p.quad_to(Point::new(98.0, 35.0), Point::new(68.0, 57.0));
    p.curve_to(Point::new(79.0, 91.0), Point::new(50.0, 70.0), Point::new(21.0, 91.0));
    p.close_path();
    p
}

fn vector_layer() -> VectorLayer {
    let mut filled = PathObject::new(star_path(), Some(Paint::Solid(Color::from_rgb8(255, 128, 0))));
    filled.name = "Star".into();
    filled.fill_rule = FillRule::EvenOdd;
    filled.transform = Affine::translate((12.0, 34.0));
    filled.stroke = Some(StrokePaint {
        paint: Paint::Solid(Color::BLACK),
        width: 3.0,
        cap: LineCap::Square,
        join: LineJoin::Round,
        miter_limit: 2.0,
        dash_array: vec![6.0, 3.0],
        dash_offset: 1.5,
    });

    let gradient = PathObject::new(
        star_path(),
        Some(Paint::Linear(LinearGradient {
            start: Point::new(0.0, 0.0),
            end: Point::new(100.0, 100.0),
            stops: vec![
                ColorStop { offset: 0.0, color: Color::new(1.0, 0.0, 0.0, 1.0) },
                ColorStop { offset: 1.0, color: Color::new(0.0, 0.0, 1.0, 1.0) },
            ],
            spread: iris_vector::SpreadMode::Pad,
        })),
    );

    VectorLayer { objects: vec![filled, gradient], color_space: "srgb".into() }
}

fn doc_with_vector_layer() -> (AifDocument, Uuid) {
    let mut tree = LayerTree::new(256, 256, 72.0, 72.0);
    let layer_id = Uuid::new_v4();
    let layer = Layer {
        id: layer_id,
        name: "Artwork".into(),
        visible: true,
        locked: false,
        opacity: 1.0,
        blend_mode: BlendMode::Normal,
        clipping_mask: false,
        mask: None,
        content: LayerContent::Vector(vector_layer()),
    };
    tree.add_layer(None, 0, layer).expect("add vector layer");

    let doc = AifDocument {
        canvas: AifCanvas {
            mode: CanvasMode::Vector,
            width_px: 256,
            height_px: 256,
            dpi_x: 72.0,
            dpi_y: 72.0,
            working_color_space: "linear-srgb".into(),
            bit_depth: BitDepth::F16,
        },
        artboards: vec![AifArtboard {
            id: Uuid::nil(),
            name: "Artboard 1".into(),
            x_px: 0,
            y_px: 0,
            width_px: 256,
            height_px: 256,
        }],
        layers: tree,
        format_version: (1, 0),
    };
    (doc, layer_id)
}

fn write_then_read(doc: &AifDocument) -> AifDocument {
    let mut buf = Cursor::new(Vec::new());
    AifWriter::write(doc, &mut buf, &WriteOptions::default()).expect("write .aif");
    buf.seek(SeekFrom::Start(0)).expect("rewind");
    AifReader::open(buf).expect("read .aif")
}

#[test]
fn vector_layer_survives_aif_roundtrip() {
    let (doc, layer_id) = doc_with_vector_layer();
    let out = write_then_read(&doc);

    let layer = out.layers.get(layer_id).expect("vector layer must be present after reload");
    let LayerContent::Vector(ref vl) = layer.content else {
        panic!("layer decoded as {:?}, not Vector — geometry was dropped", layer.name);
    };
    assert_eq!(vl.color_space, "srgb");
    assert_eq!(vl.objects.len(), 2, "both path objects must survive");
}

#[test]
fn roundtrip_preserves_geometry_paint_and_stroke() {
    let (doc, layer_id) = doc_with_vector_layer();
    let out = write_then_read(&doc);
    let layer = out.layers.get(layer_id).expect("layer");
    let LayerContent::Vector(ref vl) = layer.content else { panic!("not a vector layer") };

    let star = &vl.objects[0];
    assert_eq!(star.name, "Star");
    assert_eq!(star.fill_rule, FillRule::EvenOdd);
    assert_eq!(star.transform.as_coeffs(), Affine::translate((12.0, 34.0)).as_coeffs());
    // Every coordinate in `star_path` is exactly representable in f32, so the
    // narrowing the §4.10 schema imposes is lossless here and the elements must
    // compare bit-for-bit.
    assert_eq!(star.path.elements(), star_path().elements());
    assert_eq!(star.fill, Some(Paint::Solid(Color::from_rgb8(255, 128, 0))));

    let stroke = star.stroke.as_ref().expect("stroke survives");
    assert_eq!(stroke.width, 3.0);
    assert_eq!(stroke.cap, LineCap::Square);
    assert_eq!(stroke.join, LineJoin::Round);
    assert_eq!(stroke.miter_limit, 2.0);
    assert_eq!(stroke.dash_array, vec![6.0, 3.0]);
    assert_eq!(stroke.dash_offset, 1.5);

    match vl.objects[1].fill.as_ref().expect("gradient fill") {
        Paint::Linear(g) => {
            assert_eq!(g.start, Point::new(0.0, 0.0));
            assert_eq!(g.end, Point::new(100.0, 100.0));
            assert_eq!(g.stops.len(), 2);
        }
        other => panic!("expected linear gradient, got {other:?}"),
    }
}

#[test]
fn object_ids_are_stable_across_roundtrip() {
    let (doc, layer_id) = doc_with_vector_layer();
    let before: Vec<Uuid> = match doc.layers.get(layer_id).map(|l| &l.content) {
        Some(LayerContent::Vector(vl)) => vl.objects.iter().map(|o| o.id).collect(),
        _ => panic!("expected vector layer"),
    };

    let out = write_then_read(&doc);
    let layer = out.layers.get(layer_id).expect("layer");
    let LayerContent::Vector(ref vl) = layer.content else { panic!("not a vector layer") };
    let after: Vec<Uuid> = vl.objects.iter().map(|o| o.id).collect();

    assert_eq!(before, after, "§4.10: object UUIDs must survive a round-trip");
}

#[test]
fn empty_vector_layer_roundtrips_as_vector_not_group() {
    let mut tree = LayerTree::new(64, 64, 72.0, 72.0);
    let layer_id = Uuid::new_v4();
    tree.add_layer(
        None,
        0,
        Layer {
            id: layer_id,
            name: "Empty".into(),
            visible: true,
            locked: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            clipping_mask: false,
            mask: None,
            content: LayerContent::Vector(VectorLayer::new("srgb")),
        },
    )
    .expect("add layer");

    let (mut doc, _) = doc_with_vector_layer();
    doc.layers = tree;

    let out = write_then_read(&doc);
    let layer = out.layers.get(layer_id).expect("layer");
    assert!(
        matches!(layer.content, LayerContent::Vector(_)),
        "an empty vector layer must stay a vector layer, not decay to a group"
    );
}
