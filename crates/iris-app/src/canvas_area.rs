// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use appthere_ui::tokens::colors::{COLOR_SURFACE_BASE, COLOR_TEXT_ON_CHROME_SECONDARY};
use appthere_ui::tokens::layout::{
    RIBBON_TOTAL_HEIGHT, STATUS_BAR_HEIGHT, TAB_BAR_HEIGHT, TITLE_BAR_HEIGHT_MACOS,
};
use appthere_ui::tokens::typography::FONT_SIZE_BODY;
use dioxus::prelude::*;
use iris_canvas::IrisCanvas;
use iris_pixel::LayerTree;

use crate::state::AppState;

// Chrome dimensions used to derive the canvas rendered area from window size.
// Left/right panel widths are defined in tool_palette.rs / layers_panel.rs;
// duplicated here until they are promoted to appthere-ui layout tokens.
// TODO(iris): SPEC.md §11.3 — move to appthere-ui layout tokens
const CHROME_LEFT: u32 = 56;   // TOOL_PALETTE_WIDTH
const CHROME_RIGHT: u32 = 240; // LAYERS_PANEL_WIDTH
const CHROME_TOP: u32 = (TITLE_BAR_HEIGHT_MACOS + TAB_BAR_HEIGHT + RIBBON_TOTAL_HEIGHT) as u32;
const CHROME_BOTTOM: u32 = STATUS_BAR_HEIGHT as u32;

#[component]
pub fn CanvasArea(mut state: Signal<AppState>) -> Element {
    // All hooks called unconditionally before any early return (Dioxus rules).
    let mut tree_signal = use_signal(move || {
        state
            .read()
            .document
            .as_ref()
            .map(|d| d.tree.clone())
            .unwrap_or_else(|| LayerTree::new(1, 1, 96.0, 96.0))
    });

    let viewport_signal = use_signal(move || {
        state
            .read()
            .document
            .as_ref()
            .map(|d| d.viewport.clone())
            .unwrap_or_default()
    });

    use_effect(move || {
        let vp = viewport_signal.read().clone();
        if let Some(doc) = state.write().document.as_mut() {
            doc.viewport = vp;
        }
    });

    // Sync tree_signal → IrisCanvas when a stroke has painted new tile data.
    // canvas_dirty is set by tool_dispatch after each Down/Move/Up event.
    // TODO(iris): SPEC.md §13 Phase 4 — this deep-clones every painted tile
    // (512 KB each) twice per pointer event (here and in canvas_widget's
    // shared_tree sync). Replace with Arc-shared tiles (copy-on-write) or
    // dirty-rect updates when the GPU compositor path lands.
    use_effect(move || {
        if state.read().canvas_dirty {
            let new_tree = state
                .read()
                .document
                .as_ref()
                .map(|d| d.tree.clone())
                .unwrap_or_else(|| LayerTree::new(1, 1, 96.0, 96.0));
            *tree_signal.write() = new_tree;
            state.write().canvas_dirty = false;
        }
    });

    // Compute canvas rendered size = window size minus shell chrome.
    // TODO(iris): SPEC.md §11.3 — replace hardcoded window size with a real
    // window-size hook once dioxus-native exposes one (winit PhysicalSize is
    // available but no Dioxus hook wraps it yet).
    let window_w = use_signal(|| 1280u32);
    let window_h = use_signal(|| 800u32);
    let canvas_w = use_memo(move || {
        window_w().saturating_sub(CHROME_LEFT + CHROME_RIGHT).max(100)
    });
    let canvas_h = use_memo(move || {
        window_h().saturating_sub(CHROME_TOP + CHROME_BOTTOM).max(100)
    });

    let doc_exists = state.read().document.is_some();

    // Early return AFTER all hooks.
    if !doc_exists {
        return rsx! {
            div {
                style: "flex: 1; display: flex; align-items: center; \
                        justify-content: center; background-color: {COLOR_SURFACE_BASE}; \
                        color: {COLOR_TEXT_ON_CHROME_SECONDARY}; \
                        font-size: {FONT_SIZE_BODY}px;",
                "No document open"
            }
        };
    }

    rsx! {
        div {
            style: "flex: 1; overflow: hidden; background-color: {COLOR_SURFACE_BASE};",
            IrisCanvas {
                tree: tree_signal,
                viewport: viewport_signal,
                width: canvas_w(),
                height: canvas_h(),
                on_tool_event: move |evt| {
                    crate::tool_dispatch::dispatch_tool_event(evt, state);
                },
            }
        }
    }
}
