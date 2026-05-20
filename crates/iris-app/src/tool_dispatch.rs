// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use dioxus::prelude::{ReadableExt, Signal};
use iris_canvas::ToolEvent;

use crate::state::{AppState, ToolMode};

/// Route a [`ToolEvent`] from the canvas to the active tool handler.
///
/// Called on every pointer, scroll, and pinch event from [`IrisCanvas`].
/// The active tool is determined by [`AppState::tool_mode`].
pub fn dispatch_tool_event(event: ToolEvent, state: Signal<AppState>) {
    match state.read().tool_mode {
        ToolMode::Pixel => dispatch_pixel_tool(event, state),
        ToolMode::Vector => {
            // TODO(iris): SPEC.md §7.2 — Phase 3C: vector tool dispatch
            tracing::debug!("vector tool event (Phase 3C+): {:?}", event);
        }
    }
}

fn dispatch_pixel_tool(event: ToolEvent, _state: Signal<AppState>) {
    match event {
        ToolEvent::Down { doc_pos, button, .. } => {
            tracing::debug!(
                "pixel Down  btn={:?} doc=({:.1},{:.1})",
                button,
                doc_pos.x,
                doc_pos.y
            );
            // TODO(iris): SPEC.md §7.1 — Phase 3C: route to BrushEngine / EraserEngine
        }
        ToolEvent::Move { doc_pos, .. } => {
            tracing::debug!(
                "pixel Move  doc=({:.1},{:.1})",
                doc_pos.x,
                doc_pos.y,
            );
            // TODO(iris): SPEC.md §7.1 — Phase 3C: append stroke point to BrushEngine
        }
        ToolEvent::Up { doc_pos, button } => {
            tracing::debug!(
                "pixel Up    btn={:?} doc=({:.1},{:.1})",
                button,
                doc_pos.x,
                doc_pos.y
            );
            // TODO(iris): SPEC.md §7.1 — Phase 3C: finalise stroke, commit to LayerTree
        }
        ToolEvent::Scroll { delta_x, delta_y } => {
            tracing::debug!("pixel Scroll dx={:.1} dy={:.1}", delta_x, delta_y);
        }
        ToolEvent::Pinch { scale_factor, anchor_screen } => {
            tracing::debug!(
                "pixel Pinch scale={:.3} anchor=({:.1},{:.1})",
                scale_factor,
                anchor_screen.x,
                anchor_screen.y
            );
        }
    }
}
