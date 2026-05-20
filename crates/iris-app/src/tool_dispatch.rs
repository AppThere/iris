// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use dioxus::prelude::{ReadableExt, Signal, WritableExt};
use iris_canvas::ToolEvent;

use crate::state::{AppState, PixelToolState, ToolMode};

/// Route a [`ToolEvent`] from the canvas to the active tool handler.
pub fn dispatch_tool_event(event: ToolEvent, state: Signal<AppState>) {
    // Copy tool_mode out before matching so the read-guard is dropped immediately,
    // allowing state.write() inside dispatch_pixel_tool to succeed.
    let tool_mode = state.read().tool_mode;
    match tool_mode {
        ToolMode::Pixel => dispatch_pixel_tool(event, state),
        ToolMode::Vector => {
            // TODO(iris): SPEC.md §7.2 — Phase 3C: vector tool dispatch
            tracing::debug!("vector tool event (Phase 3C+): {:?}", event);
        }
    }
}

fn dispatch_pixel_tool(event: ToolEvent, mut state: Signal<AppState>) {
    match &event {
        ToolEvent::Down { .. } => {
            let mut s = state.write();
            let layer_id = match s.selected_layer {
                Some(id) => id,
                None => return,
            };
            // Take pixel_tool_state out to allow simultaneous mutable borrow of document.
            let mut pts = std::mem::take(&mut s.pixel_tool_state);
            if let Some(doc) = s.document.as_mut() {
                if let Some(layer) = doc.tree.get_mut(layer_id) {
                    pts.brush.on_down(&event, layer, layer_id);
                    doc.dirty = true;
                }
            }
            s.pixel_tool_state = pts;
            s.canvas_dirty = true;
        }

        ToolEvent::Move { .. } => {
            let mut s = state.write();
            let layer_id = match s.selected_layer {
                Some(id) => id,
                None => return,
            };
            let mut pts = std::mem::take(&mut s.pixel_tool_state);
            if let Some(doc) = s.document.as_mut() {
                if let Some(layer) = doc.tree.get_mut(layer_id) {
                    pts.brush.on_move(&event, layer, layer_id);
                    doc.dirty = true;
                }
            }
            s.pixel_tool_state = pts;
            s.canvas_dirty = true;
        }

        ToolEvent::Up { .. } => {
            let mut s = state.write();
            let _dirty_tiles = s.pixel_tool_state.brush.on_up();
            s.canvas_dirty = true;
            // TODO(iris): SPEC.md §8 — Phase 3: push PaintTile ops to UndoStack
            tracing::debug!("stroke ended; {} tiles dirtied", _dirty_tiles.len());
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
