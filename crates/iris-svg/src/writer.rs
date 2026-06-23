// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`SvgWriter`] — export an [`AifDocument`] to SVG 1.1.

use std::path::Path;

use iris_aif::{encode_png_rgba8, layer_to_rgba8, AifDocument};
use iris_pixel::{LayerContent, LayerId, LayerTree};
use iris_vector::{Paint, PathObject};

use crate::color::to_hex;
use crate::error::SvgError;
use crate::grad_write::GradientRegistry;
use crate::{base64, transform};

/// Stateless writer that serialises an [`AifDocument`] to SVG 1.1.
pub struct SvgWriter;

impl SvgWriter {
    /// Serialise `doc` to an `.svg` file on disk.
    pub fn write(doc: &AifDocument, path: &Path) -> Result<(), SvgError> {
        std::fs::write(path, Self::to_string(doc)?)?;
        Ok(())
    }

    /// Serialise `doc` to an SVG string.
    pub fn to_string(doc: &AifDocument) -> Result<String, SvgError> {
        let tree = &doc.layers;
        let (w, h) = (tree.canvas_width, tree.canvas_height);
        // Emit the body first so gradient paints register into `defs`, then
        // assemble header + <defs> + body (id references resolve regardless of
        // document order).
        let mut reg = GradientRegistry::default();
        let mut body = String::new();
        // SVG draws first-to-last; the layer tree is top-first, so emit reversed.
        for id in tree.root_layer_ids().iter().rev() {
            emit_layer(tree, *id, 1, &mut body, &mut reg)?;
        }
        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        out.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" \
             width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n"
        ));
        if let Some(defs) = reg.defs_block() {
            out.push_str(&defs);
        }
        out.push_str(&body);
        out.push_str("</svg>\n");
        Ok(out)
    }
}

fn emit_layer(
    tree: &LayerTree,
    id: LayerId,
    depth: usize,
    out: &mut String,
    reg: &mut GradientRegistry,
) -> Result<(), SvgError> {
    let Some(layer) = tree.get(id) else {
        return Ok(());
    };
    let pad = "  ".repeat(depth);
    match &layer.content {
        LayerContent::Vector(vl) => {
            for obj in &vl.objects {
                emit_path(obj, &pad, out, reg);
            }
        }
        LayerContent::Pixel(_) => {
            if let Some(px) = layer_to_rgba8(layer) {
                let png = encode_png_rgba8(px.width, px.height, &px.rgba)?;
                let data = base64::encode(&png);
                out.push_str(&format!(
                    "{pad}<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" \
                     xlink:href=\"data:image/png;base64,{}\"/>\n",
                    px.offset_x, px.offset_y, px.width, px.height, data
                ));
            }
        }
        LayerContent::Group { children } => {
            out.push_str(&format!("{pad}<g>\n"));
            for child in children.iter().rev() {
                emit_layer(tree, *child, depth + 1, out, reg)?;
            }
            out.push_str(&format!("{pad}</g>\n"));
        }
        other => {
            tracing::warn!(?other, "SVG export: skipping unsupported layer");
        }
    }
    Ok(())
}

fn emit_path(obj: &PathObject, pad: &str, out: &mut String, reg: &mut GradientRegistry) {
    let mut attrs = String::new();
    if !obj.name.is_empty() {
        attrs.push_str(&format!(" id=\"{}\"", escape(&obj.name)));
    }
    if let Some(t) = transform::to_svg(obj.transform) {
        attrs.push_str(&format!(" transform=\"{t}\""));
    }
    attrs.push_str(&fill_attrs(obj, reg));
    attrs.push_str(&stroke_attrs(obj, reg));
    if matches!(obj.fill_rule, iris_vector::FillRule::EvenOdd) {
        attrs.push_str(" fill-rule=\"evenodd\"");
    }
    out.push_str(&format!("{pad}<path d=\"{}\"{}/>\n", obj.path.to_svg(), attrs));
}

fn fill_attrs(obj: &PathObject, reg: &mut GradientRegistry) -> String {
    match &obj.fill {
        None => " fill=\"none\"".to_string(),
        Some(paint) => match reg.register(paint) {
            Some(id) => format!(" fill=\"url(#{id})\""),
            None => {
                let c = solid_of(paint);
                let mut s = format!(" fill=\"{}\"", to_hex(c));
                if c.a < 1.0 {
                    s.push_str(&format!(" fill-opacity=\"{}\"", c.a));
                }
                s
            }
        },
    }
}

fn stroke_attrs(obj: &PathObject, reg: &mut GradientRegistry) -> String {
    let Some(st) = &obj.stroke else {
        return String::new();
    };
    let mut s = match reg.register(&st.paint) {
        Some(id) => format!(" stroke=\"url(#{id})\" stroke-width=\"{}\"", st.width),
        None => {
            let c = solid_of(&st.paint);
            let mut t = format!(" stroke=\"{}\" stroke-width=\"{}\"", to_hex(c), st.width);
            if c.a < 1.0 {
                t.push_str(&format!(" stroke-opacity=\"{}\"", c.a));
            }
            t
        }
    };
    match st.cap {
        iris_vector::LineCap::Round => s.push_str(" stroke-linecap=\"round\""),
        iris_vector::LineCap::Square => s.push_str(" stroke-linecap=\"square\""),
        iris_vector::LineCap::Butt => {}
    }
    match st.join {
        iris_vector::LineJoin::Round => s.push_str(" stroke-linejoin=\"round\""),
        iris_vector::LineJoin::Bevel => s.push_str(" stroke-linejoin=\"bevel\""),
        iris_vector::LineJoin::Miter => {}
    }
    if !st.dash_array.is_empty() {
        let dashes: Vec<String> = st.dash_array.iter().map(|d| d.to_string()).collect();
        s.push_str(&format!(" stroke-dasharray=\"{}\"", dashes.join(",")));
    }
    s
}

/// Reduce a paint to a representative solid colour. Gradients are normally
/// emitted as `<defs>` (see [`GradientRegistry`]); this first-stop fallback only
/// applies if a gradient ever reaches the solid path.
fn solid_of(paint: &Paint) -> iris_vector::Color {
    match paint {
        Paint::Solid(c) => *c,
        Paint::Linear(g) => g.stops.first().map(|s| s.color).unwrap_or(iris_vector::Color::BLACK),
        Paint::Radial(g) => g.stops.first().map(|s| s.color).unwrap_or(iris_vector::Color::BLACK),
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
}
