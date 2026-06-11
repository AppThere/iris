// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use appthere_ui::Platform;
use iris_canvas::CanvasViewport;
use iris_pixel::{
    BitDepth, BlendMode, ChannelLayout, ExrCompression, Layer, LayerContent, LayerId, LayerTree,
    PixelLayer, TileCache, LINEAR_SRGB,
};
use iris_tools::{BrushEngine, BrushSettings, EraserEngine};

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
        Self { title: title.to_string(), path: None, tree, viewport, dirty: false }
    }

    pub fn zoom_percent(&self) -> u32 {
        (self.viewport.zoom * 100.0).round() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_document_selects_first_root_layer() {
        let mut state = AppState::default();
        state.selection.rect = Some(kurbo::Rect::new(0.0, 0.0, 5.0, 5.0));
        let doc = OpenDocument::new_blank(64, 64, "Next");
        let expected = doc.tree.root_layer_ids().first().copied();
        state.set_document(doc);
        assert_eq!(state.selected_layer, expected);
        assert!(state.selection.rect.is_none(), "selection must reset");
        assert!(state.marquee_start.is_none(), "drag state must reset");
    }

    #[test]
    fn validated_selected_layer_heals_stale_id() {
        let mut state = AppState::default();
        state.selected_layer = Some(uuid::Uuid::new_v4()); // not in the tree
        let healed = state.validated_selected_layer();
        let first_root = state
            .document
            .as_ref()
            .and_then(|d| d.tree.root_layer_ids().first().copied());
        assert_eq!(healed, first_root);
        assert_eq!(state.selected_layer, first_root, "state must be rewritten");
    }

    #[test]
    fn validated_selected_layer_keeps_valid_id() {
        let mut state = AppState::default();
        let valid = state.selected_layer;
        assert!(valid.is_some(), "default state selects a layer");
        assert_eq!(state.validated_selected_layer(), valid);
    }
}

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
