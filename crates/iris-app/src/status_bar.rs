// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use appthere_ui::AtStatusBar;
use dioxus::prelude::*;

use crate::state::AppState;

#[component]
pub fn IrisStatusBar(mut state: Signal<AppState>) -> Element {
    let (layer_count, canvas_dims, zoom) = {
        let s = state.read();
        let doc = s.document.as_ref();
        let layers = doc.map(|d| d.tree.root_layer_ids().len()).unwrap_or(0);
        let dims = doc.map(|d| {
            format!("{}×{}", d.tree.canvas_width, d.tree.canvas_height)
        }).unwrap_or_default();
        let zoom = doc.map(|d| d.zoom_percent()).unwrap_or(100);
        (layers, dims, zoom)
    };

    rsx! {
        AtStatusBar {
            page_label: format!("{layer_count} layer{}", if layer_count == 1 { "" } else { "s" }),
            word_count_label: canvas_dims,
            language_label: String::new(),
            zoom_percent: zoom,
            collaborator_count: 0,
            collaborator_label: String::new(),
            on_zoom_click: move |_| {},
            zoom_aria_label: "Zoom level".to_string(),
        }
    }
}
