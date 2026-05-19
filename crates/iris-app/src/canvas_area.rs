// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use appthere_ui::tokens::colors::{COLOR_SURFACE_BASE, COLOR_TEXT_ON_CHROME_SECONDARY};
use appthere_ui::tokens::typography::FONT_SIZE_BODY;
use dioxus::prelude::*;
use iris_canvas::IrisCanvas;
use iris_pixel::LayerTree;

use crate::state::AppState;

// TODO(iris): SPEC.md §11.3 — measure actual CanvasArea dimensions via onmounted
const CANVAS_WIDTH: u32 = 800;
const CANVAS_HEIGHT: u32 = 600;

#[component]
pub fn CanvasArea(mut state: Signal<AppState>) -> Element {
    // All hooks called unconditionally before any early return (Dioxus rules).
    let tree_signal = use_signal(move || {
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
                width: CANVAS_WIDTH,
                height: CANVAS_HEIGHT,
            }
        }
    }
}
