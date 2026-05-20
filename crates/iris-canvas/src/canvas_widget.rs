// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`IrisCanvas`] — Dioxus component that composites a [`LayerTree`] into a
//! wgpu-rendered canvas element via Dioxus Native's `use_wgpu` hook.
//!
//! # Architecture
//!
//! Dioxus Native (Blitz) owns the GPU device and calls the paint source on
//! every frame. The bridge:
//!   1. `IrisCanvas` holds shared `Arc<Mutex<…>>` state for the viewport and
//!      layer tree, updated on each Dioxus re-render.
//!   2. `use_wgpu` registers an [`IrisCanvasPaintSource`] with the Blitz
//!      renderer; it calls `Compositor::composite()` each frame, hands the
//!      resulting `wgpu::Texture` to `CustomPaintCtx::register_texture`, and
//!      returns the `TextureHandle` for `<canvas src="…">`.
//!   3. `IrisPageSource` implements `appthere_canvas::PageSource` for
//!      non-Dioxus contexts (e.g. headless rendering, tests).
//!
//! Hooks rule: every `use_*` call is unconditional at the top level of the
//! component body — no hooks inside closures, conditions, or loops.

use std::sync::{Arc, Mutex};

use dioxus::html::input_data::MouseButton;
use dioxus::native::use_wgpu;
use dioxus::prelude::*;
use iris_pixel::LayerTree;

use crate::compositor::Compositor;
use crate::paint_bridge::IrisCanvasPaintSource;
use crate::tool_event::{PointerButton, ToolEvent};
use crate::viewport::CanvasViewport;

/// Props for the [`IrisCanvas`] component.
#[derive(Props, Clone, PartialEq)]
pub struct IrisCanvasProps {
    /// Reactive document layer tree. Component re-renders when the signal changes.
    pub tree: Signal<LayerTree>,
    /// Reactive viewport state (pan, zoom, rotation). Component re-renders on change.
    pub viewport: Signal<CanvasViewport>,
    /// Canvas width in CSS pixels.
    pub width: u32,
    /// Canvas height in CSS pixels.
    pub height: u32,
    /// Called for every normalised tool event (down, move, up, scroll, pinch).
    pub on_tool_event: EventHandler<ToolEvent>,
}

/// Convert CSS element-local coordinates from a mouse event to a `kurbo::Vec2`.
///
/// `element_coordinates()` returns the position relative to the element's
/// top-left corner — the same coordinate space that `CanvasViewport::screen_to_doc`
/// expects as its `screen` argument.
fn screen_pos(evt: &Event<MouseData>) -> kurbo::Vec2 {
    let p = evt.element_coordinates();
    kurbo::Vec2::new(p.x, p.y)
}

/// Map a Dioxus `MouseButton` to the canvas-internal `PointerButton`.
fn to_pointer_button(btn: MouseButton) -> PointerButton {
    match btn {
        MouseButton::Primary => PointerButton::Primary,
        MouseButton::Secondary => PointerButton::Secondary,
        MouseButton::Auxiliary => PointerButton::Middle,
        _ => PointerButton::Primary,
    }
}

/// Dioxus component for the Iris infinite canvas.
///
/// Renders via Dioxus Native's `use_wgpu` + Blitz `<canvas src="{id}">`.
/// Reactive signals keep viewport and layer tree in sync with the component tree.
#[component]
pub fn IrisCanvas(props: IrisCanvasProps) -> Element {
    // Lazily-initialised compositor, shared with the paint source.
    let compositor = use_hook(|| Arc::new(Mutex::new(Compositor::new())));

    // Shared viewport and tree: initialised once, updated each re-render so the
    // paint source always sees the latest state without needing Dioxus context.
    let shared_viewport = use_hook(|| Arc::new(Mutex::new(props.viewport.peek().clone())));
    let shared_tree = use_hook(|| Arc::new(Mutex::new(props.tree.peek().clone())));

    // Sync signal values into shared state on every re-render.
    if let Ok(mut vp) = shared_viewport.try_lock() {
        *vp = props.viewport.read().clone();
    }
    if let Ok(mut tr) = shared_tree.try_lock() {
        *tr = props.tree.read().clone();
    }

    // Register the paint source with Blitz. `use_wgpu` uses `use_hook_with_cleanup`
    // internally — auto-unregisters when the component is dropped. FnOnce captures
    // cloned Arcs, so the shared state remains live for the component's lifetime.
    let canvas_id = use_wgpu(|| {
        IrisCanvasPaintSource::new(
            compositor.clone(),
            shared_viewport.clone(),
            shared_tree.clone(),
        )
    });

    // Clone EventHandler and copy lightweight values before moving into closures.
    let mut vp = props.viewport;
    let (w, h) = (props.width, props.height);
    let on_down  = props.on_tool_event.clone();
    let on_move  = props.on_tool_event.clone();
    let on_up    = props.on_tool_event.clone();
    let on_wheel = props.on_tool_event.clone();

    rsx! {
        canvas {
            // "src" is not in Dioxus's canvas element schema but blitz-dom reads
            // it to associate a registered CustomPaintSource with this element.
            "src": "{canvas_id}",
            width: "{props.width}",
            height: "{props.height}",
            style: "display: block; width: {props.width}px; height: {props.height}px;",

            onmousedown: move |evt| {
                let doc = vp.read().screen_to_doc(screen_pos(&evt), w, h);
                let button = evt.trigger_button().map(to_pointer_button)
                    .unwrap_or(PointerButton::Primary);
                on_down.call(ToolEvent::Down {
                    doc_pos: doc, pressure: 1.0, tilt_x: 0.0, tilt_y: 0.0, button,
                });
            },

            onmousemove: move |evt| {
                if evt.held_buttons().contains(MouseButton::Primary) {
                    let doc = vp.read().screen_to_doc(screen_pos(&evt), w, h);
                    on_move.call(ToolEvent::Move {
                        doc_pos: doc, pressure: 1.0, tilt_x: 0.0, tilt_y: 0.0,
                    });
                }
            },

            onmouseup: move |evt| {
                let doc = vp.read().screen_to_doc(screen_pos(&evt), w, h);
                let button = evt.trigger_button().map(to_pointer_button)
                    .unwrap_or(PointerButton::Primary);
                on_up.call(ToolEvent::Up { doc_pos: doc, button });
            },

            // TODO(iris): SPEC.md §11.2 — onwheel requires blitz-shell to route
            // MouseWheel events to Dioxus (currently they go to CSS scroll only).
            // The handler logic below is correct; it will activate once blitz-shell
            // is patched to call handle_ui_event for wheel events.
            onwheel: move |evt| {
                let delta = evt.delta().strip_units();
                if evt.modifiers().ctrl() {
                    // Ctrl + scroll → zoom anchored at cursor.
                    let anchor = kurbo::Vec2::new(
                        evt.element_coordinates().x,
                        evt.element_coordinates().y,
                    );
                    let new_zoom = vp.read().zoom * (1.0 - delta.y as f32 * 0.001);
                    vp.write().zoom_to(new_zoom, anchor, w, h);
                } else {
                    // Plain scroll → pan.
                    let zoom = vp.read().zoom as f64;
                    let pan_delta = kurbo::Vec2::new(-delta.x / zoom, -delta.y / zoom);
                    vp.write().pan += pan_delta;
                    on_wheel.call(ToolEvent::Scroll {
                        delta_x: delta.x as f32,
                        delta_y: delta.y as f32,
                    });
                }
            },

            // TODO(iris): Phase 3 — touch/stylus events require dioxus-native-dom patch
            // and blitz-shell multi-touch support. Add ontouchstart, ontouchmove,
            // ontouchend + ToolEvent::Pinch when available.
        }
    }
}

/// [`appthere_canvas::PageSource`] implementation for the Iris compositor.
///
/// The unit key `()` represents the single composited viewport. Useful for
/// headless rendering and non-Dioxus contexts. Dioxus Native rendering uses
/// [`IrisCanvasPaintSource`] via `use_wgpu` instead.
pub struct IrisPageSource {
    compositor: Arc<Mutex<Compositor>>,
    width: u32,
    height: u32,
    viewport: CanvasViewport,
    tree: LayerTree,
}

impl IrisPageSource {
    /// Create a new page source from current component state.
    pub fn new(
        compositor: Arc<Mutex<Compositor>>,
        width: u32,
        height: u32,
        viewport: CanvasViewport,
        tree: LayerTree,
    ) -> Self {
        Self { compositor, width, height, viewport, tree }
    }
}

impl appthere_canvas::PageSource for IrisPageSource {
    type Key = ();

    fn page_size_px(&self, _index: ()) -> (u32, u32) {
        (self.width, self.height)
    }

    fn render(
        &self,
        _index: (),
        _scale: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<appthere_canvas::GpuTexture, appthere_canvas::RenderError> {
        let guard = self.compositor.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .composite(&self.tree, &self.viewport, self.width, self.height, device, queue)
            .map_err(|e| appthere_canvas::RenderError::Wgpu(e.to_string()))
    }
}
