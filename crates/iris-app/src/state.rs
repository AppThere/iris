// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use appthere_ui::Platform;
use iris_canvas::CanvasViewport;
use iris_pixel::{
    BitDepth, BlendMode, ChannelLayout, ExrCompression, Layer, LayerContent, LayerId, LayerTree,
    PixelLayer, TileCache, LINEAR_SRGB,
};

#[derive(Clone)]
pub struct AppState {
    pub document: Option<OpenDocument>,
    pub tool_mode: ToolMode,
    pub selected_layer: Option<LayerId>,
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
        Self {
            document: Some(OpenDocument::new_blank(800, 600, "Untitled")),
            tool_mode: ToolMode::Pixel,
            selected_layer: None,
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
        Self {
            title: title.to_string(),
            path: None,
            tree,
            viewport: CanvasViewport::new(),
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
