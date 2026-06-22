// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use appthere_ui::Platform;
use iris_canvas::CanvasViewport;
use iris_ops::{LayerOp, LayerProp, Op, PropValue, UndoStack};
use iris_pixel::{
    BitDepth, BlendMode, ChannelLayout, ExrCompression, Layer, LayerContent, LayerId, LayerTree,
    PixelLayer, TileCache, LINEAR_SRGB,
};
use iris_tools::{BrushEngine, BrushSettings, EraserEngine};

/// Per-document undo depth (ADR-004 specifies a UI default of 200).
const UNDO_DEPTH: usize = 200;

/// Which pixel sub-tool is active within [`ToolMode::Pixel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PixelTool {
    #[default]
    Brush,
    Eraser,
    Eyedropper,
    Fill,
    Marquee,
}

/// Active marquee selection in document space. `None` = no selection (paint everywhere).
#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub rect: Option<kurbo::Rect>,
}

/// Active pixel-tool state, stored in AppState so dispatch_tool_event can mutate it.
#[derive(Clone)]
pub struct PixelToolState {
    pub brush: BrushEngine,
    pub eraser: EraserEngine,
    /// Flood-fill colour-similarity threshold (0.0–1.0 Euclidean RGB distance).
    pub fill_tolerance: f32,
}

impl Default for PixelToolState {
    fn default() -> Self {
        Self { brush: BrushEngine::default(), eraser: EraserEngine::default(), fill_tolerance: 0.1 }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub document: Option<OpenDocument>,
    pub tool_mode: ToolMode,
    pub active_pixel_tool: PixelTool,
    pub selected_layer: Option<LayerId>,
    pub pixel_tool_state: PixelToolState,
    /// Foreground (paint) colour in linear RGBA. Default: opaque black.
    pub foreground_color: [f32; 4],
    /// Background colour in linear RGBA. Default: opaque white.
    pub background_color: [f32; 4],
    /// Current marquee selection (document pixels). Updated by the Marquee tool.
    pub selection: Selection,
    /// Drag origin for the marquee tool; cleared on ToolEvent::Up.
    pub marquee_start: Option<kurbo::Vec2>,
    /// Set to `true` when a stroke dirties tiles; canvas_area.rs syncs tree_signal.
    pub canvas_dirty: bool,
    pub active_tab_index: usize,
    pub platform: Platform,
}

#[derive(Clone)]
pub struct OpenDocument {
    pub title: String,
    // TODO(iris): SPEC.md §11 — Phase 3: populated by file-picker integration
    #[allow(dead_code)] // Phase 3 — file picker not yet wired
    pub path: Option<std::path::PathBuf>,
    pub tree: LayerTree,
    pub viewport: CanvasViewport,
    pub dirty: bool,
    /// Undo/redo history for this document (layer-property edits today).
    pub undo: UndoStack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolMode {
    Pixel,
    Vector,
}

impl Default for AppState {
    fn default() -> Self {
        let doc = OpenDocument::new_blank(800, 600, "Untitled");
        let selected_layer = doc.tree.root_layer_ids().first().copied();
        Self {
            document: Some(doc),
            tool_mode: ToolMode::Pixel,
            active_pixel_tool: PixelTool::Brush,
            selected_layer,
            pixel_tool_state: PixelToolState::default(),
            foreground_color: [0.0, 0.0, 0.0, 1.0],
            background_color: [1.0, 1.0, 1.0, 1.0],
            selection: Selection::default(),
            marquee_start: None,
            canvas_dirty: false,
            active_tab_index: 1,
            platform: detect_platform(),
        }
    }
}

impl AppState {
    /// Replace the open document and reset all per-document UI state.
    ///
    /// Always use this instead of assigning `self.document` directly:
    /// `selected_layer`, the marquee selection, and drag state refer to the
    /// outgoing document and would silently misdirect tool events otherwise.
    pub fn set_document(&mut self, doc: OpenDocument) {
        self.selected_layer = doc.tree.root_layer_ids().first().copied();
        self.selection = Selection::default();
        self.marquee_start = None;
        self.canvas_dirty = true;
        self.document = Some(doc);
    }

    /// The selected layer id, revalidated against the current document.
    ///
    /// A stale id (left over from a previous document or a deleted layer)
    /// heals to the first root layer so tools act on something visible
    /// instead of silently doing nothing.
    pub fn validated_selected_layer(&mut self) -> Option<LayerId> {
        let doc = self.document.as_ref()?;
        match self.selected_layer {
            Some(id) if doc.tree.get(id).is_some() => Some(id),
            stale => {
                if let Some(id) = stale {
                    tracing::warn!(%id, "stale layer selection; selecting first root layer");
                }
                let fallback = doc.tree.root_layer_ids().first().copied();
                self.selected_layer = fallback;
                fallback
            }
        }
    }
}

impl OpenDocument {
    pub fn new_blank(width_px: u32, height_px: u32, title: &str) -> Self {
        let mut tree = LayerTree::new(width_px, height_px, 96.0, 96.0);
        let layer = Layer {
            id: uuid::Uuid::new_v4(),
            name: "Background".to_string(),
            visible: true,
            locked: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            clipping_mask: false,
            mask: None,
            content: LayerContent::Pixel(PixelLayer {
                channel_layout: ChannelLayout::Rgba,
                bit_depth: BitDepth::F16,
                color_space: LINEAR_SRGB,
                compression: ExrCompression::Zip,
                canvas_offset_x: 0,
                canvas_offset_y: 0,
                crop_bounds: None,
                tiles: TileCache::default(),
            }),
        };
        tree.add_layer(None, 0, layer)
            .expect("new_blank: adding initial layer to empty tree cannot fail");
        let mut viewport = CanvasViewport::new();
        viewport.pan = kurbo::Vec2::new(width_px as f64 / 2.0, height_px as f64 / 2.0);
        Self {
            title: title.to_string(), path: None, tree, viewport, dirty: false,
            undo: UndoStack::new(UNDO_DEPTH),
        }
    }

    /// Build a document from a parsed [`iris_aif::AifDocument`], as produced by
    /// a format adapter (e.g. `iris-psd`). The layer tree is adopted directly
    /// and the viewport is centred on the canvas.
    pub fn from_aif(doc: iris_aif::AifDocument, title: &str) -> Self {
        let tree = doc.layers;
        let mut viewport = CanvasViewport::new();
        viewport.pan =
            kurbo::Vec2::new(tree.canvas_width as f64 / 2.0, tree.canvas_height as f64 / 2.0);
        Self {
            title: title.to_string(), path: None, tree, viewport, dirty: false,
            undo: UndoStack::new(UNDO_DEPTH),
        }
    }

    pub fn zoom_percent(&self) -> u32 {
        (self.viewport.zoom * 100.0).round() as u32
    }

    /// Set a scalar layer property and record an undoable op. No-op when the
    /// layer is gone or the value is unchanged (avoids empty history entries).
    pub fn set_layer_prop(&mut self, id: LayerId, prop: LayerProp, after: PropValue) {
        let Some(before) = self.tree.layer_prop(id, &prop) else {
            return;
        };
        if before == after {
            return;
        }
        if self.tree.set_layer_prop(id, &prop, &after).is_ok() {
            self.undo
                .push(Op::Layer(LayerOp::SetProp { layer_id: id, prop, before, after }));
            self.dirty = true;
        }
    }

    /// Undo the most recent recorded op. Returns `true` if the tree changed.
    pub fn undo(&mut self) -> bool {
        match self.undo.undo() {
            Some(op) => {
                self.apply_op(&op, false);
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    /// Redo the most recently undone op. Returns `true` if the tree changed.
    pub fn redo(&mut self) -> bool {
        match self.undo.redo() {
            Some(op) => {
                self.apply_op(&op, true);
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    /// Apply a recorded op in the given direction (`redo` → `after`, else
    /// `before`). Today the app only records `SetProp`; structural and tile
    /// undo are deferred.
    // TODO(iris): SPEC.md §3 — record/replay Add/Remove/Move and Tile ops too.
    fn apply_op(&mut self, op: &Op, redo: bool) {
        if let Op::Layer(LayerOp::SetProp { layer_id, prop, before, after }) = op {
            let value = if redo { after } else { before };
            let _ = self.tree.set_layer_prop(*layer_id, prop, value);
        }
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;

pub fn detect_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::MacOs
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "android") {
        Platform::Android
    } else if cfg!(target_os = "ios") {
        Platform::Ios
    } else {
        Platform::Linux
    }
}
