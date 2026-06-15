// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Building the Iris layer tree from a parsed PSD: group hierarchy, stacking
//! order, per-layer rasterisation, and layer-bounds cropping.

use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

use iris_aif::layer_from_rgba8;
use iris_pixel::{Layer, LayerContent, LayerId, LayerTree};
use psd::{Psd, PsdGroup, PsdLayer};
use uuid::Uuid;

/// A child slot within a parent (group or root), tagged with the flat layer
/// index used to recover Photoshop's stacking order.
enum Child {
    /// Index into `psd.layers()`.
    Layer(usize),
    /// PSD group id (key into `psd.groups()`).
    Group(u32),
}

/// Populate `tree` with the PSD's layers, preserving group nesting and order.
pub(crate) fn populate_tree(psd: &Psd, tree: &mut LayerTree, canvas_w: u32, canvas_h: u32) {
    // COMPAT(adobe): a PSD that was never given an explicit layer has an empty
    // layer-and-mask section; its pixels live only in the merged image. Fall
    // back to the composite as a single "Background" layer.
    if psd.layers().is_empty() {
        if let Some(layer) = composite_layer(psd, canvas_w, canvas_h) {
            let _ = tree.add_layer(None, 0, layer);
        }
        return;
    }

    let children = build_children(psd);
    add_children(psd, tree, None, None, &children, canvas_w, canvas_h);
}

/// Group every layer and group under its parent group id (`None` = root),
/// sorted bottom-to-top by flat stacking index.
fn build_children(psd: &Psd) -> BTreeMap<Option<u32>, Vec<(usize, Child)>> {
    // The lowest flat index reachable inside a group fixes where that group
    // sits relative to its sibling layers.
    let mut group_min: BTreeMap<u32, usize> = BTreeMap::new();
    for (i, layer) in psd.layers().iter().enumerate() {
        let mut parent = layer.parent_id();
        while let Some(gid) = parent {
            let slot = group_min.entry(gid).or_insert(usize::MAX);
            *slot = (*slot).min(i);
            parent = psd.groups().get(&gid).and_then(|g| g.parent_id());
        }
    }

    let mut map: BTreeMap<Option<u32>, Vec<(usize, Child)>> = BTreeMap::new();
    for (i, layer) in psd.layers().iter().enumerate() {
        map.entry(layer.parent_id()).or_default().push((i, Child::Layer(i)));
    }
    for (gid, group) in psd.groups() {
        let key = group_min.get(gid).copied().unwrap_or(usize::MAX);
        map.entry(group.parent_id()).or_default().push((key, Child::Group(*gid)));
    }
    for items in map.values_mut() {
        items.sort_by_key(|(key, _)| *key);
    }
    map
}

/// Recursively add the children of `parent_psd` (already created as
/// `parent_iris`) to the tree. Inserting each child at position 0 while walking
/// bottom-to-top reproduces Photoshop's top-first panel order.
fn add_children(
    psd: &Psd,
    tree: &mut LayerTree,
    parent_psd: Option<u32>,
    parent_iris: Option<LayerId>,
    children: &BTreeMap<Option<u32>, Vec<(usize, Child)>>,
    canvas_w: u32,
    canvas_h: u32,
) {
    let Some(items) = children.get(&parent_psd) else {
        return;
    };
    for (_key, child) in items {
        match child {
            Child::Layer(idx) => {
                if let Some(layer) = convert_layer(&psd.layers()[*idx], canvas_w, canvas_h) {
                    let _ = tree.add_layer(parent_iris, 0, layer);
                }
            }
            Child::Group(gid) => {
                let Some(group) = psd.groups().get(gid) else {
                    continue;
                };
                if let Ok(group_iris) = tree.add_layer(parent_iris, 0, group_layer(group)) {
                    add_children(psd, tree, Some(*gid), Some(group_iris), children, canvas_w, canvas_h);
                }
            }
        }
    }
}

/// Convert one PSD layer into an Iris pixel [`Layer`], cropped to its on-canvas
/// bounds. Returns `None` if the layer is entirely off-canvas or fails to
/// rasterise.
fn convert_layer(psd_layer: &PsdLayer, canvas_w: u32, canvas_h: u32) -> Option<Layer> {
    // COMPAT(adobe): the `psd` crate unwraps internally when a layer is missing
    // an expected channel (e.g. divider/section layers). Isolate the panic so a
    // single odd layer cannot crash the host application.
    let rgba = match catch_unwind(AssertUnwindSafe(|| psd_layer.rgba())) {
        Ok(rgba) => rgba,
        Err(_) => {
            tracing::warn!(name = psd_layer.name(), "skipping PSD layer that failed to rasterise");
            return None;
        }
    };

    // `rgba()` returns a full-canvas buffer; crop to the layer's bounding box
    // (clamped to the canvas) and record the offset so storage is proportional
    // to the layer, not the document.
    let left = psd_layer.layer_left();
    let top = psd_layer.layer_top();
    let cx0 = left.max(0);
    let cy0 = top.max(0);
    let cx1 = (left + psd_layer.width() as i32).min(canvas_w as i32);
    let cy1 = (top + psd_layer.height() as i32).min(canvas_h as i32);
    if cx1 <= cx0 || cy1 <= cy0 {
        return None;
    }
    let crop_w = (cx1 - cx0) as u32;
    let crop_h = (cy1 - cy0) as u32;

    let cropped = crop_rgba(&rgba, canvas_w, cx0 as u32, cy0 as u32, crop_w, crop_h);
    let mut layer = layer_from_rgba8(crop_w, crop_h, &cropped, psd_layer.name());
    if let LayerContent::Pixel(ref mut px) = layer.content {
        px.canvas_offset_x = cx0;
        px.canvas_offset_y = cy0;
    }
    layer.visible = psd_layer.visible();
    layer.opacity = psd_layer.opacity() as f32 / 255.0;
    layer.blend_mode = crate::blend::map_blend_mode(&format!("{:?}", psd_layer.blend_mode()));
    layer.clipping_mask = psd_layer.is_clipping_mask();
    Some(layer)
}

/// Copy a `crop_w × crop_h` sub-rectangle out of a full-canvas RGBA buffer.
fn crop_rgba(rgba: &[u8], canvas_w: u32, x: u32, y: u32, crop_w: u32, crop_h: u32) -> Vec<u8> {
    let row_len = crop_w as usize * 4;
    let mut out = vec![0u8; row_len * crop_h as usize];
    for row in 0..crop_h as usize {
        let src = ((y as usize + row) * canvas_w as usize + x as usize) * 4;
        let dst = row * row_len;
        if src + row_len <= rgba.len() {
            out[dst..dst + row_len].copy_from_slice(&rgba[src..src + row_len]);
        }
    }
    out
}

/// Build an Iris group [`Layer`] from a PSD group folder.
// TODO(iris): SPEC.md §6.2 — Phase 3+: the compositor does not yet apply group
// opacity/blend/visibility to children; group nodes are structural for now.
fn group_layer(group: &PsdGroup) -> Layer {
    Layer {
        id: Uuid::new_v4(),
        name: group.name().to_string(),
        visible: group.visible(),
        locked: false,
        opacity: group.opacity() as f32 / 255.0,
        blend_mode: crate::blend::map_blend_mode(&format!("{:?}", group.blend_mode())),
        clipping_mask: group.is_clipping_mask(),
        mask: None,
        content: LayerContent::Group { children: vec![] },
    }
}

/// Build a single "Background" layer from the merged composite image.
fn composite_layer(psd: &Psd, width: u32, height: u32) -> Option<Layer> {
    let rgba = catch_unwind(AssertUnwindSafe(|| psd.rgba())).ok()?;
    Some(layer_from_rgba8(width, height, &rgba, "Background"))
}
