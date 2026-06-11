// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use dioxus::prelude::{ReadableExt, Signal, WritableExt};
use iris_canvas::ToolEvent;

use crate::state::{AppState, PixelTool, ToolMode};

/// Route a [`ToolEvent`] from the canvas to the active tool handler.
pub fn dispatch_tool_event(event: ToolEvent, state: Signal<AppState>) {
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
    let pixel_tool = state.read().active_pixel_tool;
    match &event {
        ToolEvent::Down { doc_pos, .. } => match pixel_tool {
            PixelTool::Brush => brush_down(event, state),
            PixelTool::Eraser => eraser_down(event, state),
            PixelTool::Eyedropper => {
                let mut s = state.write();
                let color = s.document.as_ref()
                    .map(|d| iris_tools::eyedropper::sample_color(*doc_pos, &d.tree))
                    .unwrap_or([0.0, 0.0, 0.0, 1.0]);
                s.foreground_color = color;
                s.pixel_tool_state.brush.settings.color = color;
                tracing::debug!(r=color[0], g=color[1], b=color[2], a=color[3], "eyedropper sample");
            }
            PixelTool::Fill => {
                let mut s = state.write();
                let layer_id = match s.validated_selected_layer() { Some(id) => id, None => return };
                let color = s.foreground_color;
                let tolerance = s.pixel_tool_state.fill_tolerance;
                let doc_rect = s.document.as_ref().map(|d| {
                    kurbo::Rect::new(0.0, 0.0, d.tree.canvas_width as f64, d.tree.canvas_height as f64)
                }).unwrap_or_default();
                if let Some(doc) = s.document.as_mut() {
                    if let Some(layer) = doc.tree.get_mut(layer_id) {
                        iris_tools::fill::flood_fill(*doc_pos, color, tolerance, layer, layer_id, doc_rect);
                        doc.dirty = true;
                    }
                }
                s.canvas_dirty = true;
            }
            PixelTool::Marquee => {
                let mut s = state.write();
                s.marquee_start = Some(*doc_pos);
                s.selection.rect = None;
            }
        },

        ToolEvent::Move { doc_pos, .. } => match pixel_tool {
            PixelTool::Brush => brush_move(event, state),
            PixelTool::Eraser => eraser_move(event, state),
            PixelTool::Marquee => {
                let mut s = state.write();
                if let Some(start) = s.marquee_start {
                    s.selection.rect = Some(kurbo::Rect::new(
                        start.x.min(doc_pos.x), start.y.min(doc_pos.y),
                        start.x.max(doc_pos.x), start.y.max(doc_pos.y),
                    ));
                    s.canvas_dirty = true;
                }
            }
            _ => {}
        },

        ToolEvent::Up { .. } => match pixel_tool {
            PixelTool::Brush => {
                let mut s = state.write();
                let dirty = s.pixel_tool_state.brush.on_up();
                s.canvas_dirty = true;
                tracing::debug!("brush stroke ended; {} tiles dirtied", dirty.len());
            }
            PixelTool::Eraser => {
                let mut s = state.write();
                let dirty = s.pixel_tool_state.eraser.on_up();
                s.canvas_dirty = true;
                tracing::debug!("eraser stroke ended; {} tiles dirtied", dirty.len());
            }
            PixelTool::Marquee => {
                state.write().marquee_start = None;
            }
            _ => {}
        },

        ToolEvent::Scroll { delta_x, delta_y } => {
            tracing::debug!("pixel Scroll dx={:.1} dy={:.1}", delta_x, delta_y);
        }
        ToolEvent::Pinch { scale_factor, anchor_screen } => {
            tracing::debug!(
                "pixel Pinch scale={:.3} anchor=({:.1},{:.1})",
                scale_factor, anchor_screen.x, anchor_screen.y
            );
        }
    }
}

fn brush_down(event: ToolEvent, mut state: Signal<AppState>) {
    let mut s = state.write();
    let layer_id = match s.validated_selected_layer() { Some(id) => id, None => return };
    let mut pts = std::mem::take(&mut s.pixel_tool_state);
    pts.brush.settings.selection = s.selection.rect;
    if let Some(doc) = s.document.as_mut() {
        if let Some(layer) = doc.tree.get_mut(layer_id) {
            pts.brush.on_down(&event, layer, layer_id);
            doc.dirty = true;
        }
    }
    s.pixel_tool_state = pts;
    s.canvas_dirty = true;
}

fn brush_move(event: ToolEvent, mut state: Signal<AppState>) {
    let mut s = state.write();
    let layer_id = match s.validated_selected_layer() { Some(id) => id, None => return };
    let mut pts = std::mem::take(&mut s.pixel_tool_state);
    pts.brush.settings.selection = s.selection.rect;
    if let Some(doc) = s.document.as_mut() {
        if let Some(layer) = doc.tree.get_mut(layer_id) {
            pts.brush.on_move(&event, layer, layer_id);
            doc.dirty = true;
        }
    }
    s.pixel_tool_state = pts;
    s.canvas_dirty = true;
}

fn eraser_down(event: ToolEvent, mut state: Signal<AppState>) {
    let mut s = state.write();
    let layer_id = match s.validated_selected_layer() { Some(id) => id, None => return };
    let mut pts = std::mem::take(&mut s.pixel_tool_state);
    pts.eraser.settings_mut().selection = s.selection.rect;
    if let Some(doc) = s.document.as_mut() {
        if let Some(layer) = doc.tree.get_mut(layer_id) {
            pts.eraser.on_down(&event, layer, layer_id);
            doc.dirty = true;
        }
    }
    s.pixel_tool_state = pts;
    s.canvas_dirty = true;
}

fn eraser_move(event: ToolEvent, mut state: Signal<AppState>) {
    let mut s = state.write();
    let layer_id = match s.validated_selected_layer() { Some(id) => id, None => return };
    let mut pts = std::mem::take(&mut s.pixel_tool_state);
    pts.eraser.settings_mut().selection = s.selection.rect;
    if let Some(doc) = s.document.as_mut() {
        if let Some(layer) = doc.tree.get_mut(layer_id) {
            pts.eraser.on_move(&event, layer, layer_id);
            doc.dirty = true;
        }
    }
    s.pixel_tool_state = pts;
    s.canvas_dirty = true;
}
