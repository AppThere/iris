// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Properties panel for the selected layer: opacity and blend mode.
//!
//! The Blitz / dioxus-native renderer in use does not provide form inputs
//! (`<input type=range>`, `<select>`), so these are button-driven: opacity is a
//! ±10% stepper and the blend mode is a button that expands a list of all 27
//! modes (SPEC.md §4.8).
//!
//! Edits are recorded as undoable `SetProp` ops via
//! [`OpenDocument::set_layer_prop`], so they participate in the document's
//! undo/redo history (see [`crate::history_bar`]).

use appthere_ui::tokens::colors::{
    COLOR_ACCENT_PRIMARY, COLOR_BORDER_CHROME, COLOR_SURFACE_1, COLOR_SURFACE_2,
    COLOR_TEXT_ON_CHROME,
};
use appthere_ui::tokens::spacing::{RADIUS_SM, SPACE_2, SPACE_3};
use appthere_ui::tokens::typography::{FONT_SIZE_BODY, FONT_SIZE_LABEL, FONT_WEIGHT_SEMIBOLD};
use dioxus::prelude::*;
use iris_pixel::{BlendMode, LayerProp, PropValue};

use crate::state::AppState;

/// Opacity step applied by the −/+ buttons.
const OPACITY_STEP: f32 = 0.1;

/// All 27 blend modes in SPEC.md §4.8 order, with display labels.
const BLEND_MODES: &[(BlendMode, &str)] = &[
    (BlendMode::Normal, "Normal"),
    (BlendMode::Dissolve, "Dissolve"),
    (BlendMode::Darken, "Darken"),
    (BlendMode::Multiply, "Multiply"),
    (BlendMode::ColorBurn, "Color Burn"),
    (BlendMode::LinearBurn, "Linear Burn"),
    (BlendMode::DarkerColor, "Darker Color"),
    (BlendMode::Lighten, "Lighten"),
    (BlendMode::Screen, "Screen"),
    (BlendMode::ColorDodge, "Color Dodge"),
    (BlendMode::LinearDodge, "Linear Dodge"),
    (BlendMode::LighterColor, "Lighter Color"),
    (BlendMode::Overlay, "Overlay"),
    (BlendMode::SoftLight, "Soft Light"),
    (BlendMode::HardLight, "Hard Light"),
    (BlendMode::VividLight, "Vivid Light"),
    (BlendMode::LinearLight, "Linear Light"),
    (BlendMode::PinLight, "Pin Light"),
    (BlendMode::HardMix, "Hard Mix"),
    (BlendMode::Difference, "Difference"),
    (BlendMode::Exclusion, "Exclusion"),
    (BlendMode::Subtract, "Subtract"),
    (BlendMode::Divide, "Divide"),
    (BlendMode::Hue, "Hue"),
    (BlendMode::Saturation, "Saturation"),
    (BlendMode::Color, "Color"),
    (BlendMode::Luminosity, "Luminosity"),
];

/// Display label for a blend mode (falls back to "Normal", which never occurs
/// because [`BLEND_MODES`] is exhaustive).
fn blend_label(mode: BlendMode) -> &'static str {
    BLEND_MODES
        .iter()
        .find(|(m, _)| *m == mode)
        .map(|(_, l)| *l)
        .unwrap_or("Normal")
}

/// Set a property on the selected layer through the document's undo-tracked
/// [`crate::state::OpenDocument::set_layer_prop`], then flag the canvas dirty so
/// the compositor re-renders.
fn set_prop(mut state: Signal<AppState>, prop: LayerProp, value: PropValue) {
    let id = state.read().selected_layer;
    if let Some(id) = id {
        if let Some(doc) = state.write().document.as_mut() {
            doc.set_layer_prop(id, prop, value);
        }
        state.write().canvas_dirty = true;
    }
}

#[component]
pub fn LayerProperties(mut state: Signal<AppState>) -> Element {
    let mut expanded = use_signal(|| false);

    // Current selected-layer properties, or render nothing if none selected.
    let current = {
        let s = state.read();
        s.selected_layer
            .and_then(|id| s.document.as_ref().and_then(|d| d.tree.get(id)))
            .map(|l| (l.opacity, l.blend_mode))
    };
    let Some((opacity, blend_mode)) = current else {
        return rsx! {};
    };
    let pct = (opacity * 100.0).round() as i32;

    let step_style = format!(
        "background-color: {COLOR_SURFACE_2}; color: {COLOR_TEXT_ON_CHROME}; \
         border: none; border-radius: {RADIUS_SM}px; cursor: pointer; \
         width: 28px; height: 24px; font-size: {FONT_SIZE_BODY}px;"
    );

    rsx! {
        div {
            style: "padding: {SPACE_2}px {SPACE_3}px; background-color: {COLOR_SURFACE_1}; \
                    border-top: 1px solid {COLOR_BORDER_CHROME}; display: flex; \
                    flex-direction: column; gap: {SPACE_2}px;",

            // --- Opacity stepper ---
            div {
                style: "display: flex; align-items: center; gap: {SPACE_2}px;",
                span {
                    style: "flex: 1; color: {COLOR_TEXT_ON_CHROME}; \
                            font-size: {FONT_SIZE_LABEL}px; font-weight: {FONT_WEIGHT_SEMIBOLD};",
                    "Opacity"
                }
                button {
                    style: "{step_style}",
                    title: "Decrease opacity",
                    onclick: move |_| set_prop(
                        state,
                        LayerProp::Opacity,
                        PropValue::Float((opacity - OPACITY_STEP).clamp(0.0, 1.0)),
                    ),
                    "−"
                }
                span {
                    style: "min-width: 40px; text-align: center; \
                            color: {COLOR_TEXT_ON_CHROME}; font-size: {FONT_SIZE_BODY}px;",
                    "{pct}%"
                }
                button {
                    style: "{step_style}",
                    title: "Increase opacity",
                    onclick: move |_| set_prop(
                        state,
                        LayerProp::Opacity,
                        PropValue::Float((opacity + OPACITY_STEP).clamp(0.0, 1.0)),
                    ),
                    "+"
                }
            }

            // --- Blend mode selector ---
            button {
                style: "width: 100%; text-align: left; padding: {SPACE_2}px; \
                        background-color: {COLOR_SURFACE_2}; color: {COLOR_TEXT_ON_CHROME}; \
                        border: none; border-radius: {RADIUS_SM}px; cursor: pointer; \
                        font-size: {FONT_SIZE_BODY}px;",
                title: "Change blend mode",
                onclick: move |_| {
                    let now = expanded();
                    expanded.set(!now);
                },
                "Blend: {blend_label(blend_mode)}  ▾"
            }
            if expanded() {
                div {
                    style: "max-height: 220px; overflow-y: auto; display: flex; \
                            flex-direction: column; border: 1px solid {COLOR_BORDER_CHROME}; \
                            border-radius: {RADIUS_SM}px;",
                    for (mode, label) in BLEND_MODES.iter().copied() {
                        button {
                            key: "{label}",
                            style: {
                                let bg = if mode == blend_mode { COLOR_ACCENT_PRIMARY } else { COLOR_SURFACE_1 };
                                format!("text-align: left; padding: {SPACE_2}px {SPACE_3}px; \
                                         background-color: {bg}; color: {COLOR_TEXT_ON_CHROME}; \
                                         border: none; cursor: pointer; font-size: {FONT_SIZE_BODY}px;")
                            },
                            onclick: move |_| {
                                set_prop(
                                    state,
                                    LayerProp::BlendMode,
                                    PropValue::Str(mode.to_aif_str().to_string()),
                                );
                                expanded.set(false);
                            },
                            "{label}"
                        }
                    }
                }
            }
        }
    }
}
