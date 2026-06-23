// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`SvgReader`] — import an SVG 1.1 document into an [`AifDocument`].

use std::collections::BTreeMap;
use std::path::Path;

use iris_aif::{import_raster_image, AifArtboard, AifCanvas, AifDocument, CanvasMode};
use iris_pixel::{BlendMode, Layer, LayerContent, LayerTree, VectorLayer};
use iris_vector::{Affine, PathObject};
use kurbo::Shape;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use uuid::Uuid;

use crate::attrs::{self, f64_of, str_of, Attrs};
use crate::error::SvgError;
use crate::gradient::{self, GradKind, GradientDef};
use crate::{shapes, style, transform};

/// Stateless reader that imports an SVG document into Iris's native model.
pub struct SvgReader;

impl SvgReader {
    /// Read and convert an SVG file from disk.
    pub fn read(path: &Path) -> Result<AifDocument, SvgError> {
        let text = std::fs::read_to_string(path)?;
        Self::from_str(&text)
    }

    /// Read and convert an SVG document from a string.
    pub fn from_str(svg: &str) -> Result<AifDocument, SvgError> {
        let mut p = Parser::new();
        p.run(svg)?;
        p.finish()
    }
}

/// Accumulates geometry and raster layers while walking the SVG element tree.
struct Parser {
    width: u32,
    height: u32,
    ctm: Vec<Affine>,
    style: Vec<Attrs>,
    defs_depth: u32,
    objects: Vec<PathObject>,
    raster: Vec<Layer>,
    /// Gradient definitions by id, collected from `<linearGradient>` /
    /// `<radialGradient>` (usually inside `<defs>`).
    gradients: BTreeMap<String, GradientDef>,
    /// Id of the gradient currently receiving `<stop>` children, if any.
    cur_grad: Option<String>,
}

impl Parser {
    fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            ctm: vec![Affine::IDENTITY],
            style: vec![Attrs::new()],
            defs_depth: 0,
            objects: Vec::new(),
            raster: Vec::new(),
            gradients: BTreeMap::new(),
            cur_grad: None,
        }
    }

    fn run(&mut self, svg: &str) -> Result<(), SvgError> {
        let mut reader = Reader::from_str(svg);
        reader.config_mut().expand_empty_elements = true;
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Err(e) => return Err(SvgError::Xml(e.to_string())),
                Ok(Event::Eof) => break,
                Ok(Event::Start(e)) => self.start(&e),
                Ok(Event::End(e)) => self.end(e.local_name().as_ref()),
                _ => {}
            }
            buf.clear();
        }
        Ok(())
    }

    fn start(&mut self, e: &BytesStart) {
        let tag = e.local_name().as_ref().to_vec();
        // Gradient elements are parsed wherever they appear (normally inside
        // <defs>), so they are matched before the defs-skip below.
        match tag.as_slice() {
            b"linearGradient" => return self.begin_gradient(GradKind::Linear, e),
            b"radialGradient" => return self.begin_gradient(GradKind::Radial, e),
            b"stop" => return self.add_stop(e),
            b"defs" => {
                self.defs_depth += 1;
                return;
            }
            _ => {}
        }
        if self.defs_depth > 0 {
            return; // skip other <defs> content (clipPath, pattern, mask, …)
        }
        let own = attrs::collect(e);
        match tag.as_slice() {
            b"svg" => self.open_svg(&own),
            b"g" => {
                self.ctm.push(self.cur_ctm() * transform::parse(own.get("transform").map(String::as_str).unwrap_or("")));
                let merged = style::merge(self.cur_style(), &own);
                self.style.push(merged);
            }
            b"image" => self.add_image(&own),
            other => {
                if let Some(path) = shapes::shape_to_path(other, &own) {
                    self.add_shape(path, &own);
                }
            }
        }
    }

    fn end(&mut self, tag: &[u8]) {
        if tag == b"linearGradient" || tag == b"radialGradient" {
            self.cur_grad = None;
            return;
        }
        if tag == b"defs" {
            self.defs_depth = self.defs_depth.saturating_sub(1);
            return;
        }
        if self.defs_depth == 0 && tag == b"g" {
            if self.ctm.len() > 1 {
                self.ctm.pop();
            }
            if self.style.len() > 1 {
                self.style.pop();
            }
        }
    }

    /// Begin a gradient definition, registering it by `id` and making it the
    /// target for subsequent `<stop>` children.
    fn begin_gradient(&mut self, kind: GradKind, e: &BytesStart) {
        let a = attrs::collect(e);
        match a.get("id") {
            Some(id) => {
                let id = id.clone();
                self.gradients.insert(id.clone(), GradientDef { kind, attrs: a, stops: Vec::new() });
                self.cur_grad = Some(id);
            }
            None => self.cur_grad = None, // unreferenceable without an id
        }
    }

    /// Add a `<stop>` to the gradient currently being parsed.
    fn add_stop(&mut self, e: &BytesStart) {
        let Some(id) = self.cur_grad.clone() else {
            return;
        };
        if let Some(stop) = gradient::parse_stop(&attrs::collect(e)) {
            if let Some(def) = self.gradients.get_mut(&id) {
                def.stops.push(stop);
            }
        }
    }

    fn cur_ctm(&self) -> Affine {
        *self.ctm.last().unwrap_or(&Affine::IDENTITY)
    }
    fn cur_style(&self) -> &Attrs {
        self.style.last().expect("style stack is never empty")
    }

    fn open_svg(&mut self, a: &Attrs) {
        let view_box: Vec<f64> = str_of(a, "viewBox")
            .map(|v| v.split([',', ' ']).filter(|t| !t.is_empty()).filter_map(|t| t.parse().ok()).collect())
            .unwrap_or_default();
        let (w, h) = match (f64_of(a, "width"), f64_of(a, "height")) {
            (Some(w), Some(h)) => (w, h),
            _ if view_box.len() == 4 => (view_box[2], view_box[3]),
            _ => (0.0, 0.0),
        };
        self.width = w.max(0.0) as u32;
        self.height = h.max(0.0) as u32;
        if view_box.len() == 4 && view_box[2] > 0.0 && view_box[3] > 0.0 {
            let base = Affine::scale_non_uniform(w / view_box[2], h / view_box[3])
                * Affine::translate((-view_box[0], -view_box[1]));
            self.ctm[0] = base;
        }
        self.style[0] = style::merge(&Attrs::new(), a);
    }

    fn add_shape(&mut self, path: iris_vector::BezPath, own: &Attrs) {
        let eff = style::merge(self.cur_style(), own);
        let xf = self.cur_ctm() * transform::parse(own.get("transform").map(String::as_str).unwrap_or(""));
        // objectBoundingBox gradients need the path's bounds in object space.
        let bbox = path.bounding_box();
        let fill = match str_of(&eff, "fill") {
            Some(v) if is_url(v) => gradient::resolve(&self.gradients, v, bbox, style::opacity(&eff, "fill-opacity")),
            _ => style::fill(&eff),
        };
        let stroke = match str_of(&eff, "stroke") {
            Some(v) if is_url(v) => gradient::resolve(&self.gradients, v, bbox, style::opacity(&eff, "stroke-opacity"))
                .map(|p| style::stroke_geom(&eff, p)),
            _ => style::stroke(&eff),
        };
        self.objects.push(PathObject {
            id: Uuid::new_v4(),
            path,
            fill,
            stroke,
            fill_rule: style::fill_rule(&eff),
            transform: xf,
            name: str_of(own, "id").unwrap_or_default().to_string(),
            visible: true,
        });
    }

    fn add_image(&mut self, a: &Attrs) {
        let href = str_of(a, "href").or_else(|| str_of(a, "xlink:href")).unwrap_or("");
        let Some(b64) = href.split_once("base64,").map(|(_, d)| d) else {
            tracing::warn!("SVG <image> without an embedded base64 data URI; skipping");
            return;
        };
        let Some(bytes) = crate::base64::decode(b64) else {
            tracing::warn!("SVG <image> has invalid base64; skipping");
            return;
        };
        match import_raster_image(&bytes, str_of(a, "id").unwrap_or("Image")) {
            Ok(mut layer) => {
                if let LayerContent::Pixel(ref mut px) = layer.content {
                    px.canvas_offset_x = f64_of(a, "x").unwrap_or(0.0) as i32;
                    px.canvas_offset_y = f64_of(a, "y").unwrap_or(0.0) as i32;
                }
                self.raster.push(layer);
            }
            Err(e) => tracing::warn!("SVG <image> failed to decode: {e}"),
        }
    }

    fn finish(self) -> Result<AifDocument, SvgError> {
        if self.width == 0 || self.height == 0 {
            return Err(SvgError::MissingDimensions);
        }
        let mut tree = LayerTree::new(self.width, self.height, 96.0, 96.0);
        // Raster layers form the background; the vector layer sits on top.
        for layer in self.raster {
            let _ = tree.add_layer(None, 0, layer);
        }
        let has_raster = !tree.root_layer_ids().is_empty();
        if !self.objects.is_empty() {
            let vl = VectorLayer { objects: self.objects, color_space: "srgb".to_string() };
            let layer = Layer {
                id: Uuid::new_v4(),
                name: "Vector".to_string(),
                visible: true,
                locked: false,
                opacity: 1.0,
                blend_mode: BlendMode::Normal,
                clipping_mask: false,
                mask: None,
                content: LayerContent::Vector(vl),
            };
            let _ = tree.add_layer(None, 0, layer);
        }
        let mode = if has_raster { CanvasMode::Mixed } else { CanvasMode::Vector };
        Ok(AifDocument {
            canvas: AifCanvas {
                mode,
                width_px: self.width,
                height_px: self.height,
                dpi_x: 96.0,
                dpi_y: 96.0,
                working_color_space: "srgb".to_string(),
                bit_depth: iris_pixel::BitDepth::F16,
            },
            artboards: vec![AifArtboard {
                id: Uuid::new_v4(),
                name: "Canvas".to_string(),
                x_px: 0,
                y_px: 0,
                width_px: self.width,
                height_px: self.height,
            }],
            layers: tree,
            format_version: (1, 0),
        })
    }
}

/// Whether a paint value is a `url(#…)` paint-server reference.
fn is_url(v: &str) -> bool {
    v.trim_start().starts_with("url(")
}
