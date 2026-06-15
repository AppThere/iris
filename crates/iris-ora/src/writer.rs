// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`OraWriter`] — export an [`AifDocument`] to an OpenRaster `.ora` file.

use std::io::{Cursor, Write};
use std::path::Path;

use iris_aif::{encode_png_rgba8, flatten_to_rgba8, layer_to_rgba8, AifDocument};
use iris_pixel::{LayerContent, LayerId, LayerTree};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::composite_op::to_ora;
use crate::error::OraError;

/// Stateless writer that serialises an [`AifDocument`] to OpenRaster bytes.
pub struct OraWriter;

impl OraWriter {
    /// Serialise `doc` to an `.ora` file on disk.
    pub fn write(doc: &AifDocument, path: &Path) -> Result<(), OraError> {
        let bytes = Self::to_bytes(doc)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Serialise `doc` to OpenRaster bytes in memory.
    pub fn to_bytes(doc: &AifDocument) -> Result<Vec<u8>, OraError> {
        let tree = &doc.layers;
        let (w, h) = (tree.canvas_width, tree.canvas_height);

        let mut xml = String::new();
        xml.push_str("<?xml version='1.0' encoding='UTF-8'?>\n");
        xml.push_str(&format!("<image version=\"0.0.3\" w=\"{w}\" h=\"{h}\">\n  <stack>\n"));
        let mut parts: Vec<(String, Vec<u8>)> = Vec::new();
        let mut counter = 0usize;
        for id in tree.root_layer_ids() {
            emit_node(tree, *id, 2, &mut xml, &mut parts, &mut counter)?;
        }
        xml.push_str("  </stack>\n</image>\n");

        let merged_png = encode_png_rgba8(w, h, &flatten_to_rgba8(tree, w, h))?;

        let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        // mimetype must be the first entry and stored uncompressed.
        zw.start_file("mimetype", stored)?;
        zw.write_all(b"image/openraster")?;
        zw.start_file("stack.xml", deflated)?;
        zw.write_all(xml.as_bytes())?;
        for (src, png) in &parts {
            zw.start_file(src.as_str(), stored)?; // PNG is already compressed
            zw.write_all(png)?;
        }
        zw.start_file("mergedimage.png", stored)?;
        zw.write_all(&merged_png)?;
        // TODO(iris): SPEC.md §5.2 — also emit Thumbnails/thumbnail.png (≤256px).

        Ok(zw.finish()?.into_inner())
    }
}

/// Emit one node to `stack.xml`, collecting `(src, png)` parts for pixel layers.
fn emit_node(
    tree: &LayerTree,
    id: LayerId,
    depth: usize,
    xml: &mut String,
    parts: &mut Vec<(String, Vec<u8>)>,
    counter: &mut usize,
) -> Result<(), OraError> {
    let Some(layer) = tree.get(id) else {
        return Ok(());
    };
    let indent = "  ".repeat(depth);
    match &layer.content {
        LayerContent::Pixel(_) => {
            let Some(px) = layer_to_rgba8(layer) else {
                return Ok(());
            };
            let png = encode_png_rgba8(px.width, px.height, &px.rgba)?;
            let src = format!("data/{}.png", *counter);
            *counter += 1;
            xml.push_str(&format!(
                "{indent}<layer name=\"{}\" src=\"{}\" x=\"{}\" y=\"{}\" opacity=\"{}\" visibility=\"{}\" composite-op=\"{}\"/>\n",
                escape(&layer.name),
                src,
                px.offset_x,
                px.offset_y,
                layer.opacity.clamp(0.0, 1.0),
                visibility(layer.visible),
                to_ora(layer.blend_mode),
            ));
            parts.push((src, png));
        }
        LayerContent::Group { children } => {
            xml.push_str(&format!(
                "{indent}<stack name=\"{}\" opacity=\"{}\" visibility=\"{}\" composite-op=\"{}\">\n",
                escape(&layer.name),
                layer.opacity.clamp(0.0, 1.0),
                visibility(layer.visible),
                to_ora(layer.blend_mode),
            ));
            for child in children {
                emit_node(tree, *child, depth + 1, xml, parts, counter)?;
            }
            xml.push_str(&format!("{indent}</stack>\n"));
        }
        other => {
            tracing::warn!(?other, name = layer.name, "ORA export: skipping unsupported layer");
        }
    }
    Ok(())
}

fn visibility(visible: bool) -> &'static str {
    if visible {
        "visible"
    } else {
        "hidden"
    }
}

/// Escape the five XML predefined entities for use in an attribute value.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
