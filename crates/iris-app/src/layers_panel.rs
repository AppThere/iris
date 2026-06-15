// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use appthere_ui::tokens::colors::{
    COLOR_ACCENT_PRIMARY, COLOR_BORDER_CHROME, COLOR_SURFACE_1, COLOR_SURFACE_2,
    COLOR_SURFACE_CHROME, COLOR_TEXT_ON_CHROME,
};
use appthere_ui::tokens::spacing::{RADIUS_SM, SPACE_2, SPACE_3};
use appthere_ui::tokens::typography::{FONT_SIZE_BODY, FONT_SIZE_LABEL, FONT_WEIGHT_SEMIBOLD};
use dioxus::prelude::*;
use iris_pixel::{
    BitDepth, BlendMode, ChannelLayout, ExrCompression, Layer, LayerContent, LayerId, PixelLayer,
    TileCache, LINEAR_SRGB,
};

use crate::state::{AppState, OpenDocument};

// TODO(iris): move LAYERS_PANEL_WIDTH to appthere_ui layout tokens
const LAYERS_PANEL_WIDTH: f32 = 240.0;

#[component]
pub fn LayersPanel(mut state: Signal<AppState>) -> Element {
    rsx! {
        div {
            style: "width: {LAYERS_PANEL_WIDTH}px; background-color: {COLOR_SURFACE_1}; \
                    display: flex; flex-direction: column; flex-shrink: 0; \
                    border-left: 1px solid {COLOR_BORDER_CHROME}; box-sizing: border-box;",
            div {
                style: "padding: {SPACE_2}px {SPACE_3}px; \
                        background-color: {COLOR_SURFACE_CHROME}; \
                        color: {COLOR_TEXT_ON_CHROME}; \
                        font-size: {FONT_SIZE_LABEL}px; \
                        font-weight: {FONT_WEIGHT_SEMIBOLD}; \
                        border-bottom: 1px solid {COLOR_BORDER_CHROME};",
                "Layers"
            }
            div {
                style: "flex: 1; overflow-y: auto;",
                {
                    let layer_ids: Vec<LayerId> = state.read().document.as_ref()
                        .map(|d| d.tree.iter_depth_first().map(|l| l.id).collect())
                        .unwrap_or_default();
                    rsx! {
                        for id in layer_ids {
                            {
                                let name = state.read().document.as_ref()
                                    .and_then(|d| d.tree.get(id))
                                    .map(|l| l.name.clone())
                                    .unwrap_or_default();
                                let visible = state.read().document.as_ref()
                                    .and_then(|d| d.tree.get(id))
                                    .map(|l| l.visible)
                                    .unwrap_or(true);
                                let selected = state.read().selected_layer == Some(id);
                                rsx! {
                                    LayerRow {
                                        key: "{id}",
                                        layer_id: id,
                                        name,
                                        visible,
                                        selected,
                                        state,
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div {
                style: "padding: {SPACE_2}px; display: flex; gap: {SPACE_2}px; \
                        border-top: 1px solid {COLOR_BORDER_CHROME};",
                button {
                    style: "flex: 1; padding: {SPACE_2}px; \
                            background-color: {COLOR_ACCENT_PRIMARY}; \
                            color: {COLOR_TEXT_ON_CHROME}; border: none; \
                            border-radius: {RADIUS_SM}px; cursor: pointer; \
                            font-size: {FONT_SIZE_LABEL}px;",
                    onclick: move |_| {
                        if let Some(doc) = state.write().document.as_mut() {
                            let layer = build_blank_pixel_layer("Layer");
                            let _ = doc.tree.add_layer(None, 0, layer);
                            doc.dirty = true;
                        }
                    },
                    "+"
                }
                button {
                    style: "flex: 1; padding: {SPACE_2}px; \
                            background-color: {COLOR_SURFACE_2}; \
                            color: {COLOR_TEXT_ON_CHROME}; border: none; \
                            border-radius: {RADIUS_SM}px; cursor: pointer; \
                            font-size: {FONT_SIZE_LABEL}px;",
                    onclick: move |_| {
                        let selected_id = state.read().selected_layer;
                        if let Some(sid) = selected_id {
                            if let Some(doc) = state.write().document.as_mut() {
                                let _ = doc.tree.remove_layer(sid);
                                doc.dirty = true;
                            }
                            state.write().selected_layer = None;
                        }
                    },
                    "−"
                }
                button {
                    style: "flex: 1; padding: {SPACE_2}px; \
                            background-color: {COLOR_SURFACE_2}; \
                            color: {COLOR_TEXT_ON_CHROME}; border: none; \
                            border-radius: {RADIUS_SM}px; cursor: pointer; \
                            font-size: {FONT_SIZE_LABEL}px;",
                    onclick: move |_| {
                        spawn(async move {
                            let picker = iris_aif::FilePicker::new();
                            let options = iris_aif::PickOptions {
                                mime_types: vec![
                                    "image/png".to_string(),
                                    "image/jpeg".to_string(),
                                    "image/webp".to_string(),
                                    "image/x-exr".to_string(),
                                    "image/vnd.adobe.photoshop".to_string(),
                                ],
                                filter_label: Some("Images & PSD".to_string()),
                                ..Default::default()
                            };
                            match picker.pick_file_to_open(options).await {
                                Ok(Some(token)) => {
                                    let name = token.display_name().to_string();
                                    match token.open_read() {
                                        Ok(mut reader) => {
                                            let mut bytes = Vec::new();
                                            use std::io::Read;
                                            if let Err(e) = reader.read_to_end(&mut bytes) {
                                                tracing::error!("Failed to read imported file: {}", e);
                                                return;
                                            }
                                            // PSD files (signature "8BPS") open as a
                                            // new multi-layer document; other formats
                                            // import as a single layer.
                                            if bytes.starts_with(b"8BPS") {
                                                match iris_psd::PsdReader::from_bytes(&bytes) {
                                                    Ok(aif) => {
                                                        let doc = OpenDocument::from_aif(aif, &name);
                                                        let first =
                                                            doc.tree.root_layer_ids().first().copied();
                                                        let mut s = state.write();
                                                        s.document = Some(doc);
                                                        s.selected_layer = first;
                                                        s.canvas_dirty = true;
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Failed to open PSD: {}", e);
                                                    }
                                                }
                                                return;
                                            }
                                            match iris_aif::import_raster_image(&bytes, &name) {
                                                Ok(layer) => {
                                                    if let Some(doc) = state.write().document.as_mut() {
                                                        let _ = doc.tree.add_layer(None, 0, layer);
                                                        doc.dirty = true;
                                                    }
                                                    state.write().canvas_dirty = true;
                                                }
                                                Err(e) => {
                                                    tracing::error!("Failed to import image: {}", e);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!("Failed to open file for import: {}", e);
                                        }
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    tracing::error!("File picker error: {}", e);
                                }
                            }
                        });
                    },
                    "Import"
                }
            }
        }
    }
}

#[component]
fn LayerRow(
    layer_id: LayerId,
    name: String,
    visible: bool,
    selected: bool,
    mut state: Signal<AppState>,
) -> Element {
    let bg = if selected { COLOR_ACCENT_PRIMARY } else { COLOR_SURFACE_1 };
    rsx! {
        div {
            style: "display: flex; align-items: center; padding: {SPACE_2}px {SPACE_3}px; \
                    background-color: {bg}; cursor: pointer; \
                    border-bottom: 1px solid {COLOR_BORDER_CHROME};",
            onclick: move |_| state.write().selected_layer = Some(layer_id),
            button {
                style: "background: none; border: none; cursor: pointer; \
                        color: {COLOR_TEXT_ON_CHROME}; margin-right: {SPACE_2}px; \
                        font-size: {FONT_SIZE_LABEL}px; padding: 0;",
                onclick: move |evt| {
                    evt.stop_propagation();
                    if let Some(doc) = state.write().document.as_mut() {
                        if let Some(layer) = doc.tree.get_mut(layer_id) {
                            layer.visible = !layer.visible;
                            doc.dirty = true;
                        }
                    }
                },
                if visible { "●" } else { "○" }
            }
            span {
                style: "flex: 1; overflow: hidden; color: {COLOR_TEXT_ON_CHROME}; \
                        font-size: {FONT_SIZE_BODY}px;",
                "{name}"
            }
        }
    }
}

fn build_blank_pixel_layer(name: &str) -> Layer {
    Layer {
        id: uuid::Uuid::new_v4(),
        name: name.to_string(),
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
    }
}
