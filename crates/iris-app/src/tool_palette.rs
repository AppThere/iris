// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use appthere_ui::tokens::colors::{
    COLOR_ACCENT_PRIMARY, COLOR_BORDER_CHROME, COLOR_SURFACE_1, COLOR_TEXT_ON_CHROME,
    COLOR_TEXT_ON_CHROME_SECONDARY,
};
use appthere_ui::icons::lucide;
use appthere_ui::tokens::spacing::{RADIUS_SM, SPACE_2};
use appthere_ui::tokens::typography::{FONT_SIZE_LABEL, FONT_WEIGHT_SEMIBOLD};
use appthere_ui::AtIcon;
use dioxus::prelude::*;

use crate::state::{AppState, PixelTool, ToolMode};

const TOOL_PALETTE_WIDTH: f32 = 56.0;

#[component]
pub fn ToolPalette(mut state: Signal<AppState>) -> Element {
    let tool_mode    = state.read().tool_mode;
    let pixel_tool   = state.read().active_pixel_tool;
    let fg = state.read().foreground_color;
    let bg = state.read().background_color;

    let active_bg  = COLOR_ACCENT_PRIMARY;
    let inactive_bg = COLOR_SURFACE_1;

    let btn = |active: bool| -> String {
        let bg = if active { active_bg } else { inactive_bg };
        format!(
            "width: 100%; padding: {p}px 0; background-color: {bg}; \
             color: {fg}; border: none; border-radius: {r}px; cursor: pointer; \
             font-size: {size}px; font-weight: {weight}; margin-bottom: {p}px; \
             display: flex; align-items: center; justify-content: center;",
            p = SPACE_2, bg = bg, fg = COLOR_TEXT_ON_CHROME,
            r = RADIUS_SM, size = FONT_SIZE_LABEL, weight = FONT_WEIGHT_SEMIBOLD,
        )
    };
    let disabled = format!(
        "width: 100%; padding: {p}px 0; background-color: {bg}; \
         color: {fg}; border: none; border-radius: {r}px; cursor: not-allowed; \
         font-size: {size}px; font-weight: {weight}; margin-bottom: {p}px; opacity: 0.5; \
         display: flex; align-items: center; justify-content: center;",
        p = SPACE_2, bg = COLOR_SURFACE_1, fg = COLOR_TEXT_ON_CHROME_SECONDARY,
        r = RADIUS_SM, size = FONT_SIZE_LABEL, weight = FONT_WEIGHT_SEMIBOLD,
    );

    let fg_css = srgb_css(fg);
    let bg_css = srgb_css(bg);

    let swatch_style = |color_css: &str| -> String {
        format!(
            "width: 36px; height: 18px; background-color: {color_css}; \
             border: 1px solid {border}; border-radius: 2px; cursor: pointer; \
             margin: 1px auto; display: block;",
            color_css = color_css, border = COLOR_BORDER_CHROME,
        )
    };

    rsx! {
        div {
            style: "width: {TOOL_PALETTE_WIDTH}px; background-color: {COLOR_SURFACE_1}; \
                    display: flex; flex-direction: column; padding: {SPACE_2}px; \
                    border-right: 1px solid {COLOR_BORDER_CHROME}; flex-shrink: 0; box-sizing: border-box;",

            // Mode selectors
            button {
                style: btn(tool_mode == ToolMode::Pixel),
                onclick: move |_| state.write().tool_mode = ToolMode::Pixel,
                title: "Pixel mode",
                AtIcon { icon: lucide::GRID_2X2, color: COLOR_TEXT_ON_CHROME.to_string() }
            }
            button {
                style: btn(tool_mode == ToolMode::Vector),
                onclick: move |_| state.write().tool_mode = ToolMode::Vector,
                title: "Vector mode",
                AtIcon { icon: lucide::PEN_TOOL, color: COLOR_TEXT_ON_CHROME.to_string() }
            }

            // Pixel sub-tool selectors
            button {
                style: btn(tool_mode == ToolMode::Pixel && pixel_tool == PixelTool::Brush),
                onclick: move |_| {
                    state.write().tool_mode = ToolMode::Pixel;
                    state.write().active_pixel_tool = PixelTool::Brush;
                },
                title: "Brush",
                AtIcon { icon: lucide::BRUSH, color: COLOR_TEXT_ON_CHROME.to_string() }
            }
            button {
                style: btn(tool_mode == ToolMode::Pixel && pixel_tool == PixelTool::Eraser),
                onclick: move |_| {
                    state.write().tool_mode = ToolMode::Pixel;
                    state.write().active_pixel_tool = PixelTool::Eraser;
                },
                title: "Eraser",
                AtIcon { icon: lucide::ERASER, color: COLOR_TEXT_ON_CHROME.to_string() }
            }
            button {
                style: btn(tool_mode == ToolMode::Pixel && pixel_tool == PixelTool::Eyedropper),
                onclick: move |_| {
                    state.write().tool_mode = ToolMode::Pixel;
                    state.write().active_pixel_tool = PixelTool::Eyedropper;
                },
                title: "Eyedropper",
                AtIcon { icon: lucide::PIPETTE, color: COLOR_TEXT_ON_CHROME.to_string() }
            }
            button {
                style: btn(tool_mode == ToolMode::Pixel && pixel_tool == PixelTool::Fill),
                onclick: move |_| {
                    state.write().tool_mode = ToolMode::Pixel;
                    state.write().active_pixel_tool = PixelTool::Fill;
                },
                title: "Fill",
                AtIcon { icon: lucide::PAINT_BUCKET, color: COLOR_TEXT_ON_CHROME.to_string() }
            }
            button {
                style: btn(tool_mode == ToolMode::Pixel && pixel_tool == PixelTool::Marquee),
                onclick: move |_| {
                    state.write().tool_mode = ToolMode::Pixel;
                    state.write().active_pixel_tool = PixelTool::Marquee;
                },
                title: "Rectangular marquee",
                AtIcon { icon: lucide::SQUARE_DASHED, color: COLOR_TEXT_ON_CHROME.to_string() }
            }
            button {
                style: disabled.clone(),
                disabled: true,
                title: "Text (Phase 4)",
                AtIcon { icon: lucide::TYPE, color: COLOR_TEXT_ON_CHROME_SECONDARY.to_string() }
            }

            // Colour swatches: foreground over background.
            // Click either swatch to swap foreground ↔ background.
            // TODO(iris): Phase 4 — click foreground swatch opens full colour picker
            div {
                style: "margin-top: {SPACE_2}px;",
                button {
                    style: swatch_style(&bg_css),
                    title: "Background colour (click to swap)",
                    onclick: move |_| swap_colors(&mut state),
                }
                button {
                    style: swatch_style(&fg_css),
                    title: "Foreground colour (click to swap)",
                    onclick: move |_| swap_colors(&mut state),
                }
            }
        }
    }
}

/// Approximate linear RGBA → sRGB CSS `rgb(r,g,b)` for display in swatches.
fn srgb_css(linear: [f32; 4]) -> String {
    let to_u8 = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
    format!("rgb({},{},{})", to_u8(linear[0]), to_u8(linear[1]), to_u8(linear[2]))
}

fn swap_colors(state: &mut Signal<AppState>) {
    let mut s = state.write();
    let tmp = s.foreground_color;
    s.foreground_color = s.background_color;
    s.background_color = tmp;
    s.pixel_tool_state.brush.settings.color = s.foreground_color;
}
