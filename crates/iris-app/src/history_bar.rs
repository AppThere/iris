// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Undo / redo bar for the active document's [`iris_ops::UndoStack`].
//!
//! Records today come only from layer-property edits (opacity, blend mode,
//! visibility); brush strokes and structural changes are not yet undoable
//! (gap-audit / SPEC.md §3 follow-up).
//
// COMPAT: text buttons (not an icon set) because the shared lucide icons have
// no undo/redo glyphs; disabled state is shown by dimming rather than the
// `disabled` attribute, which the Blitz renderer does not honour.

use appthere_ui::tokens::colors::{
    COLOR_BORDER_CHROME, COLOR_SURFACE_1, COLOR_SURFACE_2, COLOR_TEXT_ON_CHROME,
};
use appthere_ui::tokens::spacing::{RADIUS_SM, SPACE_2};
use appthere_ui::tokens::typography::FONT_SIZE_LABEL;
use dioxus::prelude::*;

use crate::state::AppState;

fn button_style(enabled: bool) -> String {
    format!(
        "flex: 1; padding: {SPACE_2}px; border: none; border-radius: {RADIUS_SM}px; \
         background-color: {COLOR_SURFACE_2}; color: {COLOR_TEXT_ON_CHROME}; \
         font-size: {FONT_SIZE_LABEL}px; opacity: {}; cursor: {};",
        if enabled { "1.0" } else { "0.4" },
        if enabled { "pointer" } else { "default" },
    )
}

#[component]
pub fn HistoryBar(mut state: Signal<AppState>) -> Element {
    let (can_undo, can_redo) = state
        .read()
        .document
        .as_ref()
        .map(|d| (d.undo.can_undo(), d.undo.can_redo()))
        .unwrap_or((false, false));

    rsx! {
        div {
            style: "display: flex; gap: {SPACE_2}px; padding: {SPACE_2}px; \
                    background-color: {COLOR_SURFACE_1}; \
                    border-bottom: 1px solid {COLOR_BORDER_CHROME};",
            button {
                style: button_style(can_undo),
                title: "Undo",
                onclick: move |_| {
                    let changed =
                        state.write().document.as_mut().map(|d| d.undo()).unwrap_or(false);
                    if changed {
                        state.write().canvas_dirty = true;
                    }
                },
                "Undo"
            }
            button {
                style: button_style(can_redo),
                title: "Redo",
                onclick: move |_| {
                    let changed =
                        state.write().document.as_mut().map(|d| d.redo()).unwrap_or(false);
                    if changed {
                        state.write().canvas_dirty = true;
                    }
                },
                "Redo"
            }
        }
    }
}
