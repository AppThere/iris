// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use appthere_ui::tokens::colors::COLOR_SURFACE_BASE;
use appthere_ui::tokens::typography::FONT_FAMILY_UI;
use appthere_ui::{AtDocumentTabData, AtTabBar, AtTitleBar};
use dioxus::prelude::*;

use crate::canvas_area::CanvasArea;
use crate::home_tab::IrisHomeTab;
use crate::layers_panel::LayersPanel;
use crate::ribbon::IrisRibbon;
use crate::state::{AppState, OpenDocument};
use crate::status_bar::IrisStatusBar;
use crate::tool_palette::ToolPalette;

#[component]
pub fn AppLayout(mut state: Signal<AppState>) -> Element {
    let tabs: Vec<AtDocumentTabData> = state.read().document.as_ref().map(|doc| {
        vec![AtDocumentTabData {
            title: doc.title.clone(),
            is_dirty: doc.dirty,
            is_discarded: false,
        }]
    }).unwrap_or_default();

    let active_idx = state.read().active_tab_index;
    let platform = state.read().platform;
    let title = state.read().document.as_ref()
        .map(|d| d.title.clone());
    let is_dirty = state.read().document.as_ref()
        .map(|d| d.dirty)
        .unwrap_or(false);

    rsx! {
        div {
            style: "display: flex; flex-direction: column; width: 100%; height: 100vh; \
                    background-color: {COLOR_SURFACE_BASE}; font-family: {FONT_FAMILY_UI};",
            AtTitleBar {
                document_title: title,
                is_dirty,
                app_name: "Iris",
                collaborator_count: 0,
                collaborator_label: String::new(),
                platform,
                on_icon_press: move |_| {},
            }
            AtTabBar {
                tabs,
                active_index: active_idx,
                home_tab_label: "Home".to_string(),
                aria_label: "Document tabs".to_string(),
                on_tab_select: move |idx| state.write().active_tab_index = idx,
                on_tab_close: move |_| {},
                on_new_tab: move |_| {
                    let doc = OpenDocument::new_blank(800, 600, "Untitled");
                    state.write().document = Some(doc);
                    state.write().active_tab_index = 1;
                },
                new_tab_aria_label: "New document".to_string(),
            }
            if active_idx == 0 {
                IrisHomeTab { state }
            } else {
                IrisRibbon { state }
                div {
                    style: "display: flex; flex: 1; overflow: hidden;",
                    ToolPalette { state }
                    CanvasArea { state }
                    LayersPanel { state }
                }
                IrisStatusBar { state }
            }
        }
    }
}
