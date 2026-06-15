// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`OraReader`] — import an OpenRaster `.ora` file into an [`AifDocument`].

use std::io::{Cursor, Read};
use std::path::Path;

use iris_aif::{import_raster_image, AifArtboard, AifCanvas, AifDocument, CanvasMode};
use iris_pixel::{BitDepth, Layer, LayerContent, LayerId, LayerTree};
use uuid::Uuid;
use zip::ZipArchive;

use crate::composite_op::from_ora;
use crate::error::OraError;
use crate::stack::{self, OraNode};

type Archive<'a> = ZipArchive<Cursor<&'a [u8]>>;

/// Stateless reader that imports an OpenRaster document into Iris's native model.
pub struct OraReader;

impl OraReader {
    /// Read and convert an `.ora` file from disk.
    pub fn read(path: &Path) -> Result<AifDocument, OraError> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Read and convert an `.ora` document held in memory.
    pub fn from_bytes(bytes: &[u8]) -> Result<AifDocument, OraError> {
        let mut zip = ZipArchive::new(Cursor::new(bytes))?;
        check_mimetype(&mut zip)?;

        let xml = read_string(&mut zip, "stack.xml").ok_or(OraError::MissingStackXml)?;
        let image = stack::parse(&xml)?;

        let mut tree = LayerTree::new(image.width, image.height, 72.0, 72.0);
        for (i, node) in image.children.iter().enumerate() {
            add_node(&mut zip, &mut tree, None, i, node)?;
        }

        Ok(AifDocument {
            canvas: AifCanvas {
                mode: CanvasMode::Pixel,
                width_px: image.width,
                height_px: image.height,
                dpi_x: 72.0,
                dpi_y: 72.0,
                working_color_space: "linear-srgb".to_string(),
                bit_depth: BitDepth::F16,
            },
            artboards: vec![AifArtboard {
                id: Uuid::new_v4(),
                name: "Canvas".to_string(),
                x_px: 0,
                y_px: 0,
                width_px: image.width,
                height_px: image.height,
            }],
            layers: tree,
            format_version: (1, 0),
        })
    }
}

/// Add one ORA node (and its descendants) to the tree at `position` under
/// `parent`. ORA stacks are listed top-first, matching Iris's order, so the
/// caller's enumeration index is a valid insertion position.
fn add_node(
    zip: &mut Archive,
    tree: &mut LayerTree,
    parent: Option<LayerId>,
    position: usize,
    node: &OraNode,
) -> Result<(), OraError> {
    match node {
        OraNode::Layer(l) => {
            let png = read_bytes(zip, &l.src).ok_or_else(|| OraError::MissingLayerData(l.src.clone()))?;
            let mut layer = import_raster_image(&png, &l.name)?;
            if let LayerContent::Pixel(ref mut px) = layer.content {
                px.canvas_offset_x = l.x;
                px.canvas_offset_y = l.y;
            }
            layer.visible = l.visible;
            layer.opacity = l.opacity;
            layer.blend_mode = from_ora(&l.composite_op);
            // Position equals the parent's current child count, so this cannot fail.
            let _ = tree.add_layer(parent, position, layer);
        }
        OraNode::Stack(s) => {
            let group = Layer {
                id: Uuid::new_v4(),
                name: s.name.clone(),
                visible: s.visible,
                locked: false,
                opacity: s.opacity,
                blend_mode: from_ora(&s.composite_op),
                clipping_mask: false,
                mask: None,
                content: LayerContent::Group { children: Vec::new() },
            };
            let Ok(gid) = tree.add_layer(parent, position, group) else {
                return Ok(());
            };
            for (i, child) in s.children.iter().enumerate() {
                add_node(zip, tree, Some(gid), i, child)?;
            }
        }
    }
    Ok(())
}

/// Validate the `mimetype` entry required by the OpenRaster specification.
fn check_mimetype(zip: &mut Archive) -> Result<(), OraError> {
    match read_string(zip, "mimetype") {
        Some(s) if s.trim() == "image/openraster" => Ok(()),
        _ => Err(OraError::BadMimetype),
    }
}

fn read_string(zip: &mut Archive, name: &str) -> Option<String> {
    let mut s = String::new();
    zip.by_name(name).ok()?.read_to_string(&mut s).ok()?;
    Some(s)
}

fn read_bytes(zip: &mut Archive, name: &str) -> Option<Vec<u8>> {
    let mut v = Vec::new();
    zip.by_name(name).ok()?.read_to_end(&mut v).ok()?;
    Some(v)
}
