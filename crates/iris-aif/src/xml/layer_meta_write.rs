// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Write path for `iris/layers/{id}/meta.xml` and `Layer` construction
//! from a decoded [`LayerMetaSpec`] (SPEC.md §4.6).

use crate::{
    error::AifError,
    parts::{IRIS_EXT_NS, IRIS_NS},
    xml::{
        helpers::xml_escape,
        layer_meta::{
            bit_depth_str, channel_layout_str, exr_compression_str, LayerContentSpec,
            LayerMetaSpec, PixelDataSpec,
        },
    },
};
use iris_pixel::{
    ColorSpaceId, CropBounds, Layer, LayerContent, PixelLayer, TileCache, VectorLayer, LINEAR_SRGB,
};

// ── Write ─────────────────────────────────────────────────────────────────────

/// Serialise `iris/layers/{id}/meta.xml` bytes for the given layer.
pub(crate) fn write_layer_meta_xml(layer: &Layer) -> Result<Vec<u8>, AifError> {
    let type_str = layer_type_attr(&layer.content);
    let blend_str = layer.blend_mode.to_aif_str();
    let pixel_xml = match &layer.content {
        LayerContent::Pixel(px) => write_pixel_data_xml(px),
        _ => String::new(),
    };

    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <iris:LayerMeta\n\
         \x20\x20\x20\x20xmlns:iris=\"{IRIS_NS}\"\n\
         \x20\x20\x20\x20xmlns:x=\"{IRIS_EXT_NS}\"\n\
         \x20\x20\x20\x20id=\"{id}\"\n\
         \x20\x20\x20\x20type=\"{type_str}\"\n\
         \x20\x20\x20\x20name=\"{name}\"\n\
         \x20\x20\x20\x20visible=\"{vis}\"\n\
         \x20\x20\x20\x20locked=\"{locked}\"\n\
         \x20\x20\x20\x20opacity=\"{opacity}\"\n\
         \x20\x20\x20\x20blendMode=\"{blend}\"\n\
         \x20\x20\x20\x20clippingMask=\"{clip}\">\n\
         {pixel_xml}\
         </iris:LayerMeta>\n",
        id = layer.id,
        name = xml_escape(&layer.name),
        vis = layer.visible,
        locked = layer.locked,
        opacity = layer.opacity,
        blend = blend_str,
        clip = layer.clipping_mask,
    );
    Ok(xml.into_bytes())
}

fn write_pixel_data_xml(px: &PixelLayer) -> String {
    let ch = channel_layout_str(px.channel_layout);
    let bd = bit_depth_str(px.bit_depth);
    let comp = exr_compression_str(px.compression);
    let header = format!(
        "\x20\x20<iris:PixelData tileSize=\"256\" channelLayout=\"{ch}\" \
         colorSpace=\"{cs}\" bitDepth=\"{bd}\" compression=\"{comp}\" \
         canvasOffsetX=\"{ox}\" canvasOffsetY=\"{oy}\"",
        cs = px.color_space.as_str(),
        ox = px.canvas_offset_x,
        oy = px.canvas_offset_y,
    );
    if let Some(cb) = &px.crop_bounds {
        format!(
            "{header}>\n    <iris:CropBounds widthPx=\"{}\" heightPx=\"{}\" />\n\
             \x20\x20</iris:PixelData>\n",
            cb.width, cb.height
        )
    } else {
        format!("{header} />\n")
    }
}

// ── Layer construction ────────────────────────────────────────────────────────

/// Construct a minimal [`Layer`] from a decoded [`LayerMetaSpec`].
///
/// Tile data is populated separately by the tile reader (PROMPT 3C).
pub(crate) fn layer_from_spec(spec: LayerMetaSpec) -> Layer {
    let content = match spec.content_spec {
        LayerContentSpec::Pixel(px) => {
            LayerContent::Pixel(pixel_layer_from_spec(&px))
        }
        // Geometry is loaded separately from `paths.bin` by the reader (§4.10);
        // meta.xml carries no vector payload of its own.
        LayerContentSpec::Vector => LayerContent::Vector(VectorLayer::default()),
        LayerContentSpec::Group | LayerContentSpec::UnknownFallback(_) => {
            LayerContent::Group { children: Vec::new() }
        }
    };
    Layer {
        id: spec.id,
        name: spec.name,
        visible: spec.visible,
        locked: spec.locked,
        opacity: spec.opacity,
        blend_mode: spec.blend_mode,
        clipping_mask: spec.clipping_mask,
        mask: None,
        content,
    }
}

fn pixel_layer_from_spec(px: &PixelDataSpec) -> PixelLayer {
    PixelLayer {
        channel_layout: px.channel_layout,
        bit_depth: px.bit_depth,
        color_space: ColorSpaceId::from_aif_str(&px.color_space).unwrap_or(LINEAR_SRGB),
        compression: px.compression,
        canvas_offset_x: px.canvas_offset_x,
        canvas_offset_y: px.canvas_offset_y,
        crop_bounds: px.crop_bounds.map(|(w, h)| CropBounds {
            x: 0,
            y: 0,
            width: w,
            height: h,
        }),
        tiles: TileCache::default(),
    }
}

fn layer_type_attr(content: &LayerContent) -> &'static str {
    match content {
        LayerContent::Pixel(_) => "pixel",
        LayerContent::Group { .. } => "group",
        LayerContent::Vector(_) => "vector",
        LayerContent::Text => "text",
        LayerContent::Adjustment => "adjustment",
        LayerContent::Fill => "fill",
        LayerContent::SmartObject => "smart-object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iris_pixel::{BitDepth, BlendMode, ChannelLayout, ExrCompression};

    fn make_pixel_layer() -> Layer {
        use iris_pixel::TileCache;
        Layer {
            id: uuid::Uuid::nil(),
            name: "Background".into(),
            visible: true,
            locked: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            clipping_mask: false,
            mask: None,
            content: LayerContent::Pixel(PixelLayer {
                channel_layout: ChannelLayout::Rgba,
                bit_depth: BitDepth::F16,
                color_space: iris_pixel::LINEAR_SRGB,
                compression: ExrCompression::Zip,
                canvas_offset_x: 0,
                canvas_offset_y: 0,
                crop_bounds: None,
                tiles: TileCache::default(),
            }),
        }
    }

    #[test]
    fn write_contains_required_attrs() {
        let layer = make_pixel_layer();
        let bytes = write_layer_meta_xml(&layer).unwrap();
        let xml = std::str::from_utf8(&bytes).unwrap();
        assert!(xml.contains("type=\"pixel\""));
        assert!(xml.contains("blendMode=\"normal\""));
        assert!(xml.contains("channelLayout=\"rgba\""));
        assert!(xml.contains("bitDepth=\"f16\""));
    }
}
