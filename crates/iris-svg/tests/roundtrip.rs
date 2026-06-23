// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Read and round-trip tests for `iris-svg`.

use iris_aif::{layer_from_rgba8, layer_to_rgba8, AifArtboard, AifCanvas, AifDocument, CanvasMode};
use iris_pixel::{BitDepth, BlendMode, Layer, LayerContent, LayerTree, VectorLayer};
use iris_vector::{
    Affine, BezPath, Color, ColorStop, FillRule, LinearGradient, Paint, PathObject, Point,
    SpreadMode, StrokePaint,
};
use iris_svg::{SvgReader, SvgWriter};

fn vector_layer(objects: Vec<PathObject>) -> Layer {
    Layer {
        id: uuid::Uuid::new_v4(),
        name: "Vector".to_string(),
        visible: true,
        locked: false,
        opacity: 1.0,
        blend_mode: BlendMode::Normal,
        clipping_mask: false,
        mask: None,
        content: LayerContent::Vector(VectorLayer { objects, color_space: "srgb".to_string() }),
    }
}

fn document(tree: LayerTree, mode: CanvasMode) -> AifDocument {
    let (w, h) = (tree.canvas_width, tree.canvas_height);
    AifDocument {
        canvas: AifCanvas {
            mode,
            width_px: w,
            height_px: h,
            dpi_x: 96.0,
            dpi_y: 96.0,
            working_color_space: "srgb".to_string(),
            bit_depth: BitDepth::F16,
        },
        artboards: vec![AifArtboard {
            id: uuid::Uuid::new_v4(),
            name: "Canvas".to_string(),
            x_px: 0,
            y_px: 0,
            width_px: w,
            height_px: h,
        }],
        layers: tree,
        format_version: (1, 0),
    }
}

fn only_vector(doc: &AifDocument) -> &VectorLayer {
    let root = doc.layers.root_layer_ids();
    let layer = doc.layers.get(root[0]).expect("root layer");
    match &layer.content {
        LayerContent::Vector(vl) => vl,
        other => panic!("expected vector layer, got {other:?}"),
    }
}

#[test]
fn reads_basic_shapes_and_fills() {
    let svg = r##"<svg width="100" height="50">
        <rect x="0" y="0" width="10" height="10" fill="#ff0000"/>
        <circle cx="20" cy="20" r="5" fill="blue"/>
    </svg>"##;
    let doc = SvgReader::from_str(svg).expect("parse");
    assert_eq!((doc.canvas.width_px, doc.canvas.height_px), (100, 50));

    let vl = only_vector(&doc);
    assert_eq!(vl.objects.len(), 2);
    assert_eq!(vl.objects[0].fill, Some(Paint::Solid(Color::from_rgb8(255, 0, 0))));
    assert_eq!(vl.objects[1].fill, Some(Paint::Solid(Color::from_rgb8(0, 0, 255))));
}

#[test]
fn group_transform_is_composed_onto_objects() {
    let svg = r##"<svg width="100" height="100">
        <g transform="translate(5,5)"><rect width="10" height="10"/></g>
    </svg>"##;
    let doc = SvgReader::from_str(svg).expect("parse");
    let vl = only_vector(&doc);
    assert_eq!(vl.objects.len(), 1);
    let mapped = vl.objects[0].transform * Point::new(0.0, 0.0);
    assert_eq!(mapped, Point::new(5.0, 5.0));
}

#[test]
fn round_trips_vector_paths() {
    let path = BezPath::from_svg("M0 0 L10 10 L0 10 Z").expect("path");
    let obj = PathObject {
        id: uuid::Uuid::new_v4(),
        path,
        fill: Some(Paint::Solid(Color::from_rgb8(255, 0, 0))),
        stroke: Some(StrokePaint::new(Paint::Solid(Color::from_rgb8(0, 0, 255)), 2.0)),
        fill_rule: FillRule::EvenOdd,
        transform: Affine::IDENTITY,
        name: "p1".to_string(),
        visible: true,
    };
    let mut tree = LayerTree::new(20, 20, 96.0, 96.0);
    tree.add_layer(None, 0, vector_layer(vec![obj])).expect("add");

    let svg = SvgWriter::to_string(&document(tree, CanvasMode::Vector)).expect("write");
    let back = SvgReader::from_str(&svg).expect("read back");

    let vl = only_vector(&back);
    assert_eq!(vl.objects.len(), 1);
    let o = &vl.objects[0];
    assert_eq!(o.fill, Some(Paint::Solid(Color::from_rgb8(255, 0, 0))));
    assert_eq!(o.fill_rule, FillRule::EvenOdd);
    let st = o.stroke.as_ref().expect("stroke");
    assert_eq!(st.width, 2.0);
    assert_eq!(st.paint, Paint::Solid(Color::from_rgb8(0, 0, 255)));
    // M + L + L + Z = 4 path elements.
    assert_eq!(o.path.elements().len(), 4);
}

#[test]
fn round_trips_linear_gradient_fill() {
    let grad = LinearGradient {
        start: Point::new(0.0, 0.0),
        end: Point::new(100.0, 0.0),
        stops: vec![
            ColorStop { offset: 0.0, color: Color::from_rgb8(255, 0, 0) },
            ColorStop { offset: 1.0, color: Color::from_rgb8(0, 0, 255) },
        ],
        spread: SpreadMode::Pad,
    };
    let obj = PathObject {
        id: uuid::Uuid::new_v4(),
        path: BezPath::from_svg("M0 0 L100 0 L100 100 Z").expect("path"),
        fill: Some(Paint::Linear(grad)),
        stroke: None,
        fill_rule: FillRule::NonZero,
        transform: Affine::IDENTITY,
        name: String::new(),
        visible: true,
    };
    let mut tree = LayerTree::new(100, 100, 96.0, 96.0);
    tree.add_layer(None, 0, vector_layer(vec![obj])).expect("add");

    let svg = SvgWriter::to_string(&document(tree, CanvasMode::Vector)).expect("write");
    assert!(svg.contains("<linearGradient"), "emits a <defs> gradient: {svg}");
    assert!(svg.contains("fill=\"url(#"), "fill references the gradient");

    let back = SvgReader::from_str(&svg).expect("read back");
    match &only_vector(&back).objects[0].fill {
        Some(Paint::Linear(g)) => {
            assert_eq!(g.start, Point::new(0.0, 0.0));
            assert_eq!(g.end, Point::new(100.0, 0.0));
            assert_eq!(g.stops.len(), 2);
            assert_eq!(g.stops[0].color, Color::from_rgb8(255, 0, 0));
            assert_eq!(g.stops[1].color, Color::from_rgb8(0, 0, 255));
        }
        other => panic!("expected linear gradient, got {other:?}"),
    }
}

#[test]
fn reads_rounded_rect_corners() {
    let svg = r##"<svg width="100" height="60">
        <rect x="0" y="0" width="100" height="60" rx="10" ry="8" fill="#888888"/>
    </svg>"##;
    let doc = SvgReader::from_str(svg).expect("parse");
    let obj = &only_vector(&doc).objects[0];
    // Rounded corners introduce curve segments; a sharp rect would have none.
    assert!(
        obj.path.elements().iter().any(|e| matches!(e, kurbo::PathEl::CurveTo(..))),
        "rounded rect should produce curves"
    );
}

#[test]
fn reads_object_bounding_box_gradient_from_defs() {
    // Default gradientUnits=objectBoundingBox: x1=0%..x2=100% map across the
    // rect's bounds (x 10..90).
    let svg = r##"<svg width="100" height="100">
        <defs>
          <linearGradient id="g">
            <stop offset="0" stop-color="#ff0000"/>
            <stop offset="1" stop-color="#0000ff"/>
          </linearGradient>
        </defs>
        <rect x="10" y="20" width="80" height="60" fill="url(#g)"/>
    </svg>"##;
    let doc = SvgReader::from_str(svg).expect("parse");
    match &only_vector(&doc).objects[0].fill {
        Some(Paint::Linear(g)) => {
            assert_eq!(g.start.x, 10.0);
            assert_eq!(g.end.x, 90.0);
            assert_eq!(g.stops.len(), 2);
        }
        other => panic!("expected linear gradient, got {other:?}"),
    }
}

#[test]
fn round_trips_embedded_raster() {
    let swatch = vec![255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255];
    let mut tree = LayerTree::new(2, 2, 96.0, 96.0);
    tree.add_layer(None, 0, layer_from_rgba8(2, 2, &swatch, "Bg")).expect("add");

    let svg = SvgWriter::to_string(&document(tree, CanvasMode::Mixed)).expect("write");
    assert!(svg.contains("data:image/png;base64,"));

    let back = SvgReader::from_str(&svg).expect("read back");
    let root = back.layers.root_layer_ids();
    let layer = back.layers.get(root[0]).expect("layer");
    assert!(matches!(layer.content, LayerContent::Pixel(_)));
    let px = layer_to_rgba8(layer).expect("pixels");
    assert!(px.rgba[0] > 200 && px.rgba[1] < 50, "top-left pixel red");
}
