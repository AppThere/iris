// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use appthere_ui::Platform;
use iris_canvas::CanvasViewport;
use iris_pixel::{
    BitDepth, BlendMode, ChannelLayout, ExrCompression, Layer, LayerContent, LayerId, LayerTree,
    PixelLayer, TileCache, LINEAR_SRGB,
};
use iris_tools::{BrushEngine, EraserEngine};

/// Active pixel-tool state, stored in AppState so dispatch_tool_event can mutate it.
#[derive(Clone, Default)]
pub struct PixelToolState {
    pub brush: BrushEngine,
    pub eraser: EraserEngine,
}

#[derive(Clone)]
pub struct AppState {
    pub document: Option<OpenDocument>,
    pub tool_mode: ToolMode,
    pub selected_layer: Option<LayerId>,
    pub pixel_tool_state: PixelToolState,
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
        // Auto-select the first (background) layer so the brush has a target immediately.
        let selected_layer = doc.tree.root_layer_ids().first().copied();
        Self {
            document: Some(doc),
            tool_mode: ToolMode::Pixel,
            selected_layer,
            pixel_tool_state: PixelToolState::default(),
            canvas_dirty: false,
            active_tab_index: 1,
            platform: detect_platform(),
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
        // Centre the document in the canvas view at zoom 1:
        // pan = doc centre so that (doc_w/2, doc_h/2) appears at screen centre.
        // This ensures doc (0,0) is at the canvas top-left and all tile coords
        // are positive when painting within the document bounds.
        let mut viewport = CanvasViewport::new();
        viewport.pan = kurbo::Vec2::new(width_px as f64 / 2.0, height_px as f64 / 2.0);
        Self {
            title: title.to_string(),
            path: None,
            tree,
            viewport,
            dirty: false,
        }
    }

    pub fn zoom_percent(&self) -> u32 {
        (self.viewport.zoom * 100.0).round() as u32
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
