// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`LayerTree`] — authoritative in-memory document layer tree.

use std::collections::BTreeMap;

use crate::layer::{Layer, LayerContent, LayerId};

/// Internal: where a layer is attached in the tree.
enum ParentLocation {
    Root,
    Group(LayerId),
}

/// In-memory document layer tree with canvas metadata.
///
/// All layers live in a flat [`BTreeMap`]; tree structure is encoded by
/// `LayerContent::Group` child-ID lists. Root-level layers are in `root_ids`.
// BTreeMap chosen over HashMap for deterministic iteration order (CLAUDE.md).
#[derive(Debug)]
pub struct LayerTree {
    /// Canvas width in pixels.
    pub canvas_width: u32,
    /// Canvas height in pixels.
    pub canvas_height: u32,
    /// Horizontal dots-per-inch.
    pub dpi_x: f32,
    /// Vertical dots-per-inch.
    pub dpi_y: f32,
    layers: BTreeMap<LayerId, Layer>,
    root_ids: Vec<LayerId>,
}

impl LayerTree {
    /// Create an empty layer tree with the given canvas dimensions.
    pub fn new(canvas_width: u32, canvas_height: u32, dpi_x: f32, dpi_y: f32) -> Self {
        Self {
            canvas_width,
            canvas_height,
            dpi_x,
            dpi_y,
            layers: BTreeMap::new(),
            root_ids: Vec::new(),
        }
    }

    /// Ordered slice of root-level layer IDs (index 0 = topmost).
    pub fn root_layer_ids(&self) -> &[LayerId] {
        &self.root_ids
    }

    /// Immutable access to a layer by ID.
    pub fn get(&self, id: LayerId) -> Option<&Layer> {
        self.layers.get(&id)
    }

    /// Mutable access to a layer by ID.
    pub fn get_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.get_mut(&id)
    }

    /// Find where `id` is parented. O(n) — no back-references in Phase 1.
    fn find_parent(&self, id: LayerId) -> Option<ParentLocation> {
        if self.root_ids.contains(&id) {
            return Some(ParentLocation::Root);
        }
        for (&pid, layer) in &self.layers {
            if let LayerContent::Group { children } = &layer.content {
                if children.contains(&id) {
                    return Some(ParentLocation::Group(pid));
                }
            }
        }
        None
    }

    /// Returns `true` if `candidate` is anywhere inside the subtree rooted at `ancestor`.
    fn is_in_subtree(&self, ancestor: LayerId, candidate: LayerId) -> bool {
        let mut stack = vec![ancestor];
        while let Some(id) = stack.pop() {
            if id == candidate {
                return true;
            }
            if let Some(LayerContent::Group { children }) =
                self.layers.get(&id).map(|l| &l.content)
            {
                stack.extend(children.iter().copied());
            }
        }
        false
    }

    /// Add a layer to the tree, returning its ID on success.
    ///
    /// `parent = None` inserts at the document root; `parent = Some(id)` requires
    /// the parent to be a `Group`. `position = 0` is the topmost slot.
    pub fn add_layer(
        &mut self,
        parent: Option<LayerId>,
        position: usize,
        layer: Layer,
    ) -> Result<LayerId, LayerTreeError> {
        let id = layer.id;
        match parent {
            None => {
                if position > self.root_ids.len() {
                    return Err(LayerTreeError::PositionOutOfRange(position));
                }
                self.root_ids.insert(position, id);
            }
            Some(pid) => {
                let parent_layer =
                    self.layers.get_mut(&pid).ok_or(LayerTreeError::NotFound(pid))?;
                let LayerContent::Group { children } = &mut parent_layer.content else {
                    return Err(LayerTreeError::NotFound(pid));
                };
                if position > children.len() {
                    return Err(LayerTreeError::PositionOutOfRange(position));
                }
                children.insert(position, id);
            }
        }
        self.layers.insert(id, layer);
        Ok(id)
    }

    /// Remove a layer and all its descendants, returning the root layer.
    ///
    /// Descendants are collected recursively so no orphaned entries remain.
    // TODO(iris): SPEC.md §3 — Phase 5: snapshot full subtree for iris-ops undo.
    pub fn remove_layer(&mut self, id: LayerId) -> Result<Layer, LayerTreeError> {
        // BFS to collect the full subtree.
        let mut to_remove = vec![id];
        let mut i = 0;
        while i < to_remove.len() {
            if let Some(LayerContent::Group { children }) =
                self.layers.get(&to_remove[i]).map(|l| &l.content)
            {
                to_remove.extend(children.iter().copied());
            }
            i += 1;
        }
        // Detach the root of the removed subtree from its parent.
        match self.find_parent(id).ok_or(LayerTreeError::NotFound(id))? {
            ParentLocation::Root => self.root_ids.retain(|&x| x != id),
            ParentLocation::Group(pid) => {
                if let Some(LayerContent::Group { children }) =
                    self.layers.get_mut(&pid).map(|l| &mut l.content)
                {
                    children.retain(|&x| x != id);
                }
            }
        }
        for &did in to_remove.iter().skip(1) {
            self.layers.remove(&did);
        }
        self.layers.remove(&id).ok_or(LayerTreeError::NotFound(id))
    }

    /// Move a layer to a new parent and/or position.
    ///
    /// Returns [`LayerTreeError::CircularMove`] when `new_parent` is a
    /// descendant of `id`, which would create a cycle.
    pub fn move_layer(
        &mut self,
        id: LayerId,
        new_parent: Option<LayerId>,
        new_position: usize,
    ) -> Result<(), LayerTreeError> {
        if !self.layers.contains_key(&id) {
            return Err(LayerTreeError::NotFound(id));
        }
        if let Some(np) = new_parent {
            if self.is_in_subtree(id, np) {
                return Err(LayerTreeError::CircularMove(id));
            }
            if !self.layers.contains_key(&np) {
                return Err(LayerTreeError::NotFound(np));
            }
        }
        // Detach from current parent.
        match self.find_parent(id).ok_or(LayerTreeError::NotFound(id))? {
            ParentLocation::Root => self.root_ids.retain(|&x| x != id),
            ParentLocation::Group(pid) => {
                if let Some(LayerContent::Group { children }) =
                    self.layers.get_mut(&pid).map(|l| &mut l.content)
                {
                    children.retain(|&x| x != id);
                }
            }
        }
        // Attach at new location.
        match new_parent {
            None => {
                if new_position > self.root_ids.len() {
                    return Err(LayerTreeError::PositionOutOfRange(new_position));
                }
                self.root_ids.insert(new_position, id);
            }
            Some(np) => {
                let np_layer =
                    self.layers.get_mut(&np).ok_or(LayerTreeError::NotFound(np))?;
                let LayerContent::Group { children } = &mut np_layer.content else {
                    return Err(LayerTreeError::NotFound(np));
                };
                if new_position > children.len() {
                    return Err(LayerTreeError::PositionOutOfRange(new_position));
                }
                children.insert(new_position, id);
            }
        }
        Ok(())
    }

    /// Collect all layer IDs in depth-first pre-order.
    fn dfs_ids(&self) -> Vec<LayerId> {
        let mut out = Vec::new();
        let mut stack: Vec<LayerId> = self.root_ids.iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            out.push(id);
            if let Some(LayerContent::Group { children }) =
                self.layers.get(&id).map(|l| &l.content)
            {
                stack.extend(children.iter().rev().copied());
            }
        }
        out
    }

    /// Iterate over all layers in depth-first pre-order (root → children → grandchildren).
    pub fn iter_depth_first(&self) -> impl Iterator<Item = &Layer> + '_ {
        self.dfs_ids().into_iter().filter_map(|id| self.layers.get(&id))
    }
}

/// Errors produced by [`LayerTree`] mutation operations.
#[derive(Debug, thiserror::Error)]
pub enum LayerTreeError {
    /// No layer with the given ID exists in the tree.
    #[error("layer {0} not found")]
    NotFound(LayerId),
    /// The requested move would create a cycle in the tree.
    #[error("cannot move layer {0} into its own descendant")]
    CircularMove(LayerId),
    /// The requested insertion index exceeds the current child-list length.
    #[error("position {0} out of range")]
    PositionOutOfRange(usize),
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::blend::BlendMode;
    use crate::color_space::LINEAR_SRGB;
    use crate::layer::{Layer, LayerContent};
    use crate::pixel_layer::{BitDepth, ChannelLayout, ExrCompression, PixelLayer};
    use crate::tile::TileCache;

    fn pixel_layer() -> Layer {
        Layer {
            id: Uuid::new_v4(), name: "layer".into(),
            visible: true, locked: false, opacity: 1.0,
            blend_mode: BlendMode::Normal, clipping_mask: false, mask: None,
            content: LayerContent::Pixel(PixelLayer {
                channel_layout: ChannelLayout::Rgba, bit_depth: BitDepth::F16,
                color_space: LINEAR_SRGB, compression: ExrCompression::Zip,
                canvas_offset_x: 0, canvas_offset_y: 0,
                crop_bounds: None, tiles: TileCache::new(8),
            }),
        }
    }

    fn group_layer() -> Layer {
        Layer {
            id: Uuid::new_v4(), name: "group".into(),
            visible: true, locked: false, opacity: 1.0,
            blend_mode: BlendMode::Normal, clipping_mask: false, mask: None,
            content: LayerContent::Group { children: vec![] },
        }
    }

    #[test]
    fn add_then_get_round_trip() {
        let mut t = LayerTree::new(1920, 1080, 72.0, 72.0);
        let layer = pixel_layer();
        let id = layer.id;
        assert_eq!(t.add_layer(None, 0, layer).expect("add"), id);
        assert_eq!(t.get(id).map(|l| l.id), Some(id));
        assert_eq!(t.root_layer_ids(), &[id]);
    }

    #[test]
    fn remove_layer_removes_from_parent_children() {
        let mut t = LayerTree::new(1920, 1080, 72.0, 72.0);
        let g = group_layer();
        let gid = g.id;
        t.add_layer(None, 0, g).expect("add group");
        let child = pixel_layer();
        let cid = child.id;
        t.add_layer(Some(gid), 0, child).expect("add child");
        t.remove_layer(cid).expect("remove");
        assert!(t.get(cid).is_none());
        if let LayerContent::Group { children } = &t.get(gid).expect("parent").content {
            assert!(!children.contains(&cid));
        }
    }

    #[test]
    fn move_layer_circular_detected() {
        let mut t = LayerTree::new(1920, 1080, 72.0, 72.0);
        let a = group_layer();
        let aid = a.id;
        t.add_layer(None, 0, a).expect("add A");
        let b = pixel_layer();
        let bid = b.id;
        t.add_layer(Some(aid), 0, b).expect("add B under A");
        assert!(matches!(
            t.move_layer(aid, Some(bid), 0),
            Err(LayerTreeError::CircularMove(_))
        ));
    }

    #[test]
    fn add_layer_position_out_of_range() {
        let mut t = LayerTree::new(1920, 1080, 72.0, 72.0);
        assert!(matches!(
            t.add_layer(None, 999, pixel_layer()),
            Err(LayerTreeError::PositionOutOfRange(999))
        ));
    }

    #[test]
    fn iter_depth_first_visits_all_layers() {
        let mut t = LayerTree::new(1920, 1080, 72.0, 72.0);
        let g = group_layer();
        let gid = g.id;
        t.add_layer(None, 0, g).expect("add group");
        let c1 = pixel_layer();
        let c1id = c1.id;
        t.add_layer(Some(gid), 0, c1).expect("add c1");
        let c2 = pixel_layer();
        let c2id = c2.id;
        t.add_layer(Some(gid), 1, c2).expect("add c2");
        let ids: Vec<LayerId> = t.iter_depth_first().map(|l| l.id).collect();
        assert_eq!(ids, vec![gid, c1id, c2id]);
    }
}
