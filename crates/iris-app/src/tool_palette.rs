// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use appthere_ui::tokens::colors::{
    COLOR_ACCENT_PRIMARY, COLOR_BORDER_CHROME, COLOR_SURFACE_1, COLOR_TEXT_ON_CHROME,
    COLOR_TEXT_ON_CHROME_SECONDARY,
};
use appthere_ui::tokens::spacing::{RADIUS_SM, SPACE_2};
use appthere_ui::tokens::typography::{FONT_SIZE_LABEL, FONT_WEIGHT_SEMIBOLD};
use dioxus::prelude::*;

use crate::state::{AppState, ToolMode};

// TODO(iris): move TOOL_PALETTE_WIDTH to appthere_ui layout tokens
const TOOL_PALETTE_WIDTH: f32 = 56.0;

#[component]
pub fn ToolPalette(mut state: Signal<AppState>) -> Element {
    let tool_mode = state.read().tool_mode;

    let pixel_bg = if tool_mode == ToolMode::Pixel { COLOR_ACCENT_PRIMARY } else { COLOR_SURFACE_1 };
    let vector_bg = if tool_mode == ToolMode::Vector { COLOR_ACCENT_PRIMARY } else { COLOR_SURFACE_1 };

    let btn_style = |bg: &str| {
        format!(
            "width: 100%; padding: {p}px 0; background-color: {bg}; \
             color: {fg}; border: none; border-radius: {r}px; cursor: pointer; \
             font-size: {size}px; font-weight: {weight}; margin-bottom: {p}px;",
            p      = SPACE_2,
            bg     = bg,
            fg     = COLOR_TEXT_ON_CHROME,
            r      = RADIUS_SM,
            size   = FONT_SIZE_LABEL,
            weight = FONT_WEIGHT_SEMIBOLD,
        )
    };

    let disabled_btn_style = format!(
        "width: 100%; padding: {p}px 0; background-color: {bg}; \
         color: {fg}; border: none; border-radius: {r}px; cursor: not-allowed; \
         font-size: {size}px; font-weight: {weight}; margin-bottom: {p}px; opacity: 0.5;",
        p      = SPACE_2,
        bg     = COLOR_SURFACE_1,
        fg     = COLOR_TEXT_ON_CHROME_SECONDARY,
        r      = RADIUS_SM,
        size   = FONT_SIZE_LABEL,
        weight = FONT_WEIGHT_SEMIBOLD,
    );

    rsx! {
        div {
            style: "width: {TOOL_PALETTE_WIDTH}px; background-color: {COLOR_SURFACE_1}; \
                    display: flex; flex-direction: column; padding: {SPACE_2}px; \
                    border-right: 1px solid {COLOR_BORDER_CHROME}; flex-shrink: 0; box-sizing: border-box;",
            button {
                style: btn_style(pixel_bg),
                onclick: move |_| state.write().tool_mode = ToolMode::Pixel,
                "Px"
                // TODO(iris): Phase 3 — SVG icon when appthere_ui ships Tabler Icons
            }
            button {
                style: btn_style(vector_bg),
                onclick: move |_| state.write().tool_mode = ToolMode::Vector,
                "Vc"
            }
            button { style: disabled_btn_style.clone(), disabled: true, "B" }
            button { style: disabled_btn_style.clone(), disabled: true, "E" }
            button { style: disabled_btn_style.clone(), disabled: true, "P" }
            button { style: disabled_btn_style.clone(), disabled: true, "T" }
        }
    }
}
