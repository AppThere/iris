// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Read and write `iris/document.xml` (SPEC.md §4.4).

use std::collections::BTreeSet;

use quick_xml::{events::Event, Reader};
use uuid::Uuid;

use crate::{
    document::{AifArtboard, AifCanvas, CanvasMode},
    error::AifError,
    parts::{AIF_FORMAT_VERSION, AIF_MAJOR, IRIS_EXT_NS, IRIS_NS},
    xml::helpers::{
        local_name, parse_f32, parse_i32, parse_u32, parse_uuid, qualified_name,
        required_attr, xml_escape,
    },
};
use iris_pixel::{BitDepth, Layer, LayerContent, LayerTree};

/// A parsed `<iris:Layer>` entry from `document.xml`.
pub(crate) struct LayerTreeEntry {
    pub id: Uuid,
    // TODO(iris): SPEC.md §4.6 — validate layer_type against meta.xml in Phase 3.
    #[allow(dead_code)]
    pub layer_type: String,
    pub order: u32,
}

// ── Read ──────────────────────────────────────────────────────────────────────

/// Parse `iris/document.xml` bytes.
///
/// Returns `(canvas, artboards, flat_layer_entries)`. The caller assembles the
/// `LayerTree` after cross-validating layer IDs against `meta.xml` parts.
pub(crate) fn read_document_xml(
    bytes: &[u8],
) -> Result<(AifCanvas, Vec<AifArtboard>, Vec<LayerTreeEntry>), AifError> {
    let part = crate::parts::DOCUMENT_XML;
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);

    let mut canvas: Option<AifCanvas> = None;
    let mut artboards: Vec<AifArtboard> = Vec::new();
    let mut entries: Vec<LayerTreeEntry> = Vec::new();

    loop {
        match reader.read_event() {
            Err(e) => {
                return Err(AifError::XmlParse {
                    part: part.into(),
                    message: e.to_string(),
                })
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                match local_name(e).as_str() {
                    "Document" => {
                        let ver = required_attr(e, "formatVersion", part)?;
                        let major = ver
                            .split('.')
                            .next()
                            .unwrap_or("0")
                            .parse::<u32>()
                            .map_err(|err| AifError::XmlParse {
                                part: part.into(),
                                message: format!("invalid formatVersion: {err}"),
                            })?;
                        if major > AIF_MAJOR {
                            return Err(AifError::UnsupportedMajorVersion {
                                found: major,
                                supported: AIF_MAJOR,
                            });
                        }
                    }
                    "Canvas" => canvas = Some(parse_canvas(e, part)?),
                    "Artboard" => artboards.push(parse_artboard(e, part)?),
                    "Layer" => entries.push(parse_layer_entry(e, part)?),
                    // Structural wrapper elements: no attributes needed.
                    "ColorProfile" | "Artboards" | "LayerTree" | "Metadata" => {}
                    _ => {
                        // Unknown elements outside the x: extension namespace
                        // are a hard error per §4.1 rule 3.
                        if !qualified_name(e).contains("x:") && !qualified_name(e).contains(IRIS_EXT_NS) {
                            return Err(AifError::XmlParse {
                                part: part.into(),
                                message: format!(
                                    "unknown element <{}> outside extension namespace",
                                    local_name(e)
                                ),
                            });
                        }
                    }
                }
            }
            Ok(_) => {}
        }
    }

    let canvas = canvas.ok_or_else(|| AifError::MissingAttribute {
        element: "Document".into(),
        attr: "Canvas element".into(),
    })?;

    // Pixel-mode artboard invariant (§4.4).
    if canvas.mode == CanvasMode::Pixel {
        if artboards.len() != 1 {
            return Err(AifError::InvalidPixelModeArtboard);
        }
        let ab = &artboards[0];
        if ab.x_px != 0
            || ab.y_px != 0
            || ab.width_px != canvas.width_px
            || ab.height_px != canvas.height_px
        {
            return Err(AifError::InvalidPixelModeArtboard);
        }
    }

    // Duplicate layer ID check.
    let mut seen = BTreeSet::new();
    for entry in &entries {
        if !seen.insert(entry.id) {
            return Err(AifError::DuplicateLayerId(entry.id));
        }
    }

    Ok((canvas, artboards, entries))
}

fn parse_canvas(e: &quick_xml::events::BytesStart<'_>, part: &str) -> Result<AifCanvas, AifError> {
    let mode = match required_attr(e, "mode", part)?.as_str() {
        "pixel" => CanvasMode::Pixel,
        "vector" => CanvasMode::Vector,
        "mixed" => CanvasMode::Mixed,
        other => {
            return Err(AifError::XmlParse {
                part: part.into(),
                message: format!("unknown canvas mode '{other}'"),
            })
        }
    };
    Ok(AifCanvas {
        mode,
        width_px: parse_u32(&required_attr(e, "widthPx", part)?, "widthPx", part)?,
        height_px: parse_u32(&required_attr(e, "heightPx", part)?, "heightPx", part)?,
        dpi_x: parse_f32(&required_attr(e, "dpiX", part)?, "dpiX", part)?,
        dpi_y: parse_f32(&required_attr(e, "dpiY", part)?, "dpiY", part)?,
        working_color_space: required_attr(e, "workingColorSpace", part)?,
        bit_depth: parse_bit_depth(&required_attr(e, "bitDepth", part)?, part)?,
    })
}

fn parse_artboard(
    e: &quick_xml::events::BytesStart<'_>,
    part: &str,
) -> Result<AifArtboard, AifError> {
    Ok(AifArtboard {
        id: parse_uuid(&required_attr(e, "id", part)?, part)?,
        name: required_attr(e, "name", part)?,
        x_px: parse_i32(&required_attr(e, "xPx", part)?, "xPx", part)?,
        y_px: parse_i32(&required_attr(e, "yPx", part)?, "yPx", part)?,
        width_px: parse_u32(&required_attr(e, "widthPx", part)?, "widthPx", part)?,
        height_px: parse_u32(&required_attr(e, "heightPx", part)?, "heightPx", part)?,
    })
}

fn parse_layer_entry(
    e: &quick_xml::events::BytesStart<'_>,
    part: &str,
) -> Result<LayerTreeEntry, AifError> {
    Ok(LayerTreeEntry {
        id: parse_uuid(&required_attr(e, "id", part)?, part)?,
        layer_type: required_attr(e, "type", part)?,
        order: parse_u32(&required_attr(e, "order", part)?, "order", part)?,
    })
}

// ── Write ─────────────────────────────────────────────────────────────────────

/// Serialise the document state as UTF-8 `iris/document.xml` bytes.
pub(crate) fn write_document_xml(
    canvas: &AifCanvas,
    artboards: &[AifArtboard],
    layer_tree: &LayerTree,
    doc_id: Uuid,
    app_version: &str,
    created_at: &str,
    saved_at: &str,
) -> Result<Vec<u8>, AifError> {
    let mode_str = match canvas.mode {
        CanvasMode::Pixel => "pixel",
        CanvasMode::Vector => "vector",
        CanvasMode::Mixed => "mixed",
    };
    let bd = bit_depth_to_str(canvas.bit_depth);
    let cs = &canvas.working_color_space;

    let mut ab_xml = String::new();
    for ab in artboards {
        ab_xml.push_str(&format!(
            "    <iris:Artboard id=\"{id}\" name=\"{name}\" \
             xPx=\"{x}\" yPx=\"{y}\" widthPx=\"{w}\" heightPx=\"{h}\" />\n",
            id = ab.id,
            name = xml_escape(&ab.name),
            x = ab.x_px,
            y = ab.y_px,
            w = ab.width_px,
            h = ab.height_px,
        ));
    }

    let mut lyr_xml = String::new();
    for (order, &id) in layer_tree.root_layer_ids().iter().enumerate() {
        if let Some(layer) = layer_tree.get(id) {
            write_layer_entry(&mut lyr_xml, layer, order as u32, 4);
        }
    }

    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <iris:Document\n    xmlns:iris=\"{IRIS_NS}\"\n    xmlns:x=\"{IRIS_EXT_NS}\"\n\
         \x20\x20\x20\x20formatVersion=\"{AIF_FORMAT_VERSION}\"\n\
         \x20\x20\x20\x20documentId=\"{doc_id}\"\n\
         \x20\x20\x20\x20createdAt=\"{created_at}\"\n\
         \x20\x20\x20\x20savedAt=\"{saved_at}\"\n\
         \x20\x20\x20\x20appVersion=\"{app_version}\">\n\n\
         \x20\x20<iris:Canvas mode=\"{mode_str}\" widthPx=\"{w}\" heightPx=\"{h}\" \
         dpiX=\"{dpi_x}\" dpiY=\"{dpi_y}\" workingColorSpace=\"{cs}\" bitDepth=\"{bd}\" />\n\n\
         \x20\x20<iris:ColorProfile workingSpace=\"{cs}\" \
         renderingIntent=\"relative-colorimetric\" />\n\n\
         \x20\x20<iris:Artboards>\n{ab_xml}\x20\x20</iris:Artboards>\n\n\
         \x20\x20<iris:LayerTree>\n{lyr_xml}\x20\x20</iris:LayerTree>\n\n\
         </iris:Document>\n",
        w = canvas.width_px,
        h = canvas.height_px,
        dpi_x = canvas.dpi_x,
        dpi_y = canvas.dpi_y,
    );
    Ok(xml.into_bytes())
}

fn write_layer_entry(out: &mut String, layer: &Layer, order: u32, indent: usize) {
    let pad = " ".repeat(indent);
    let t = layer_type_str(&layer.content);
    match &layer.content {
        LayerContent::Group { children } if !children.is_empty() => {
            out.push_str(&format!(
                "{pad}<iris:Layer id=\"{id}\" type=\"{t}\" order=\"{order}\">\n",
                id = layer.id,
            ));
            // Child layer details live in their own meta.xml; document.xml
            // only records id, type, and order.
            for (i, child_id) in children.iter().enumerate() {
                out.push_str(&format!(
                    "{pad}  <iris:Layer id=\"{child_id}\" type=\"pixel\" order=\"{i}\" />\n"
                ));
            }
            out.push_str(&format!("{pad}</iris:Layer>\n"));
        }
        _ => {
            out.push_str(&format!(
                "{pad}<iris:Layer id=\"{id}\" type=\"{t}\" order=\"{order}\" />\n",
                id = layer.id,
            ));
        }
    }
}

// ── Small helpers ─────────────────────────────────────────────────────────────

fn layer_type_str(content: &LayerContent) -> &'static str {
    match content {
        LayerContent::Pixel(_) => "pixel",
        LayerContent::Group { .. } => "group",
        LayerContent::Vector => "vector",
        LayerContent::Text => "text",
        LayerContent::Adjustment => "adjustment",
        LayerContent::Fill => "fill",
        LayerContent::SmartObject => "smart-object",
    }
}

fn bit_depth_to_str(bd: BitDepth) -> &'static str {
    match bd { BitDepth::U8 => "u8", BitDepth::U16 => "u16", BitDepth::F16 => "f16", BitDepth::F32 => "f32" }
}

fn parse_bit_depth(s: &str, part: &str) -> Result<BitDepth, AifError> {
    match s {
        "u8" => Ok(BitDepth::U8),
        "u16" => Ok(BitDepth::U16),
        "f16" => Ok(BitDepth::F16),
        "f32" => Ok(BitDepth::F32),
        other => Err(AifError::XmlParse { part: part.into(), message: format!("unknown bitDepth '{other}'") }),
    }
}

