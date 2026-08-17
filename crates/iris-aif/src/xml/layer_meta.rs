// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Types and read path for `iris/layers/{id}/meta.xml` (SPEC.md §4.6).
//!
//! The write path and `layer_from_spec` live in [`super::layer_meta_write`].

use quick_xml::{events::Event, Reader};
use uuid::Uuid;

use crate::{
    error::AifError,
    xml::helpers::{
        local_name, optional_attr, parse_bool, parse_f32, parse_i32, parse_u32, parse_uuid,
        required_attr,
    },
};
use iris_pixel::{BitDepth, BlendMode, ChannelLayout, ExrCompression};

// ── Public types ──────────────────────────────────────────────────────────────

/// Decoded representation of a single `meta.xml` file.
#[derive(Debug)]
pub(crate) struct LayerMetaSpec {
    pub id: Uuid,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub clipping_mask: bool,
    pub content_spec: LayerContentSpec,
}

/// The layer type / content-specific fields decoded from `meta.xml`.
#[derive(Debug)]
pub(crate) enum LayerContentSpec {
    Pixel(PixelDataSpec),
    /// `type="vector"` — geometry lives in `paths.bin` (§4.10), not in meta.xml.
    Vector,
    Group,
    /// Per §4.16 rule 4: unknown type treated as Group with a warning.
    // The String is the original type name logged via tracing::warn! during parse.
    #[allow(dead_code)]
    UnknownFallback(String),
}

/// Decoded `<iris:PixelData>` element.
#[derive(Debug)]
pub(crate) struct PixelDataSpec {
    // Validated during parse (must equal 256 for AIF 1.0); stored for future versions.
    #[allow(dead_code)]
    pub tile_size: u32,
    pub channel_layout: ChannelLayout,
    pub color_space: String,
    pub bit_depth: BitDepth,
    pub compression: ExrCompression,
    pub canvas_offset_x: i32,
    pub canvas_offset_y: i32,
    /// `(width, height)` from `<iris:CropBounds>` if present.
    pub crop_bounds: Option<(u32, u32)>,
}

// ── Read ──────────────────────────────────────────────────────────────────────

/// Parse `iris/layers/{layer_id}/meta.xml` bytes.
///
/// `layer_id` is from the OPC part path and must match the `id` attribute.
pub(crate) fn read_layer_meta_xml(
    bytes: &[u8],
    layer_id: Uuid,
) -> Result<LayerMetaSpec, AifError> {
    let part_owned = crate::parts::layer_meta_xml(&layer_id);
    let part = part_owned.as_str();
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);

    let mut spec: Option<LayerMetaSpec> = None;
    let mut pixel_spec: Option<PixelDataSpec> = None;
    let mut crop_wh: Option<(u32, u32)> = None;

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
                    "LayerMeta" => {
                        spec = Some(parse_layer_meta_root(e, layer_id, part)?);
                    }
                    "PixelData" => {
                        pixel_spec = Some(parse_pixel_data(e, layer_id, part)?);
                    }
                    "CropBounds" => {
                        let w = parse_u32(
                            &required_attr(e, "widthPx", part)?,
                            "widthPx",
                            part,
                        )?;
                        let h = parse_u32(
                            &required_attr(e, "heightPx", part)?,
                            "heightPx",
                            part,
                        )?;
                        crop_wh = Some((w, h));
                    }
                    // Mask and Effects: forward-compat stubs.
                    "Mask" | "Effects" | "DropShadow" | "InnerShadow" => {}
                    _ => {}
                }
            }
            Ok(_) => {}
        }
    }

    let mut spec = spec.ok_or_else(|| AifError::XmlParse {
        part: part.into(),
        message: "missing <iris:LayerMeta> root element".into(),
    })?;

    if let Some(mut px) = pixel_spec {
        px.crop_bounds = crop_wh;
        spec.content_spec = LayerContentSpec::Pixel(px);
    }

    Ok(spec)
}

fn parse_layer_meta_root(
    e: &quick_xml::events::BytesStart<'_>,
    layer_id: Uuid,
    part: &str,
) -> Result<LayerMetaSpec, AifError> {
    let xml_id = parse_uuid(&required_attr(e, "id", part)?, part)?;
    if xml_id != layer_id {
        return Err(AifError::XmlParse {
            part: part.into(),
            message: format!(
                "meta.xml id {xml_id} does not match directory UUID {layer_id}"
            ),
        });
    }

    let type_str = required_attr(e, "type", part)?;
    let content_spec = match type_str.as_str() {
        "pixel" => LayerContentSpec::Pixel(PixelDataSpec {
            tile_size: 256,
            channel_layout: ChannelLayout::Rgba,
            color_space: String::new(),
            bit_depth: BitDepth::F16,
            compression: ExrCompression::Zip,
            canvas_offset_x: 0,
            canvas_offset_y: 0,
            crop_bounds: None,
        }),
        "vector" => LayerContentSpec::Vector,
        "group" => LayerContentSpec::Group,
        other => {
            // §4.16 rule 4: unknown type → treat as group, warn.
            tracing::warn!(
                layer_id = %layer_id,
                layer_type = other,
                "unknown layer type; treating as group (§4.16 rule 4)"
            );
            LayerContentSpec::UnknownFallback(other.into())
        }
    };

    let blend_str = required_attr(e, "blendMode", part)?;
    let blend_mode = BlendMode::from_aif_str(&blend_str).ok_or_else(|| {
        AifError::UnknownBlendMode { layer_id, value: blend_str.clone() }
    })?;

    Ok(LayerMetaSpec {
        id: xml_id,
        name: required_attr(e, "name", part)?,
        visible: parse_bool(&required_attr(e, "visible", part)?, "visible", part)?,
        locked: parse_bool(&required_attr(e, "locked", part)?, "locked", part)?,
        opacity: parse_f32(&required_attr(e, "opacity", part)?, "opacity", part)?,
        blend_mode,
        clipping_mask: parse_bool(
            &required_attr(e, "clippingMask", part)?,
            "clippingMask",
            part,
        )?,
        content_spec,
    })
}

fn parse_pixel_data(
    e: &quick_xml::events::BytesStart<'_>,
    _layer_id: Uuid,
    part: &str,
) -> Result<PixelDataSpec, AifError> {
    let tile_size = parse_u32(&required_attr(e, "tileSize", part)?, "tileSize", part)?;
    if tile_size != 256 {
        return Err(AifError::XmlParse {
            part: part.into(),
            message: format!("tileSize must be 256 in AIF 1.0, got {tile_size}"),
        });
    }
    let ch_str = required_attr(e, "channelLayout", part)?;
    let channel_layout = parse_channel_layout(&ch_str, part)?;
    let color_space = required_attr(e, "colorSpace", part)?;
    let bit_depth = parse_bit_depth(&required_attr(e, "bitDepth", part)?, part)?;
    let compression = parse_exr_compression(&required_attr(e, "compression", part)?, part)?;
    let ox = optional_attr(e, "canvasOffsetX", part)?.unwrap_or_else(|| "0".into());
    let oy = optional_attr(e, "canvasOffsetY", part)?.unwrap_or_else(|| "0".into());
    Ok(PixelDataSpec {
        tile_size,
        channel_layout,
        color_space,
        bit_depth,
        compression,
        canvas_offset_x: parse_i32(&ox, "canvasOffsetX", part)?,
        canvas_offset_y: parse_i32(&oy, "canvasOffsetY", part)?,
        crop_bounds: None,
    })
}

// ── String-mapping helpers (shared with write path) ───────────────────────────

pub(crate) fn parse_channel_layout(s: &str, part: &str) -> Result<ChannelLayout, AifError> {
    match s {
        "rgba" => Ok(ChannelLayout::Rgba),
        "rgb" => Ok(ChannelLayout::Rgb),
        "la" => Ok(ChannelLayout::La),
        "l" => Ok(ChannelLayout::L),
        "cmyk" => Ok(ChannelLayout::Cmyk),
        // TODO(iris): SPEC.md §4.7 — Phase 4: cmyka, multichannel
        other => Err(AifError::XmlParse {
            part: part.into(),
            message: format!("unknown channelLayout '{other}'"),
        }),
    }
}

pub(crate) fn parse_bit_depth(s: &str, part: &str) -> Result<BitDepth, AifError> {
    match s {
        "u8" => Ok(BitDepth::U8), "u16" => Ok(BitDepth::U16),
        "f16" => Ok(BitDepth::F16), "f32" => Ok(BitDepth::F32),
        other => Err(AifError::XmlParse {
            part: part.into(),
            message: format!("unknown bitDepth '{other}'"),
        }),
    }
}

pub(crate) fn parse_exr_compression(s: &str, part: &str) -> Result<ExrCompression, AifError> {
    match s {
        "zip" => Ok(ExrCompression::Zip), "zips" => Ok(ExrCompression::Zips),
        "piz" => Ok(ExrCompression::Piz), "dwab" => Ok(ExrCompression::Dwab),
        "dwaa" => Ok(ExrCompression::Dwaa),
        other => Err(AifError::XmlParse {
            part: part.into(),
            message: format!("unknown EXR compression '{other}'"),
        }),
    }
}

pub(crate) fn channel_layout_str(cl: ChannelLayout) -> &'static str {
    match cl {
        ChannelLayout::Rgba => "rgba", ChannelLayout::Rgb => "rgb",
        ChannelLayout::La => "la", ChannelLayout::L => "l",
        ChannelLayout::Cmyk => "cmyk",
    }
}

pub(crate) fn bit_depth_str(bd: BitDepth) -> &'static str {
    match bd {
        BitDepth::U8 => "u8", BitDepth::U16 => "u16",
        BitDepth::F16 => "f16", BitDepth::F32 => "f32",
    }
}

pub(crate) fn exr_compression_str(c: ExrCompression) -> &'static str {
    match c {
        ExrCompression::Zip => "zip", ExrCompression::Zips => "zips",
        ExrCompression::Piz => "piz", ExrCompression::Dwab => "dwab",
        ExrCompression::Dwaa => "dwaa",
    }
}
