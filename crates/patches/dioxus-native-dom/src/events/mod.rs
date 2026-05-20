// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

mod form;
mod keyboard;
mod mouse;
mod touch;
mod wheel;

pub(crate) use form::{NativeFocusData, NativeFormData};
pub(crate) use keyboard::BlitzKeyboardData;
pub(crate) use mouse::NativeClickData;
use wheel::NativeWheelData;

use std::future::Future;
use std::pin::Pin;

use dioxus_html::{
    geometry::{euclid, Pixels, PixelsRect},
    AnimationData, CancelData, ClipboardData, CompositionData, DragData, FocusData, FormData,
    HtmlEventConverter, ImageData, KeyboardData, MediaData, MountedData, MountedResult,
    MouseData, PlatformEventData, PointerData, RenderedElementBacking, ResizeData, ScrollData,
    SelectionData, ToggleData, TouchData, TransitionData, VisibleData, WheelData,
};
use keyboard_types::Modifiers;
use touch::{NativeTouchData, NativeTouchPoint};

/// Platform-side mounted data for the Blitz renderer.
///
/// Created by [`DioxusDocument::poll`] after layout has been computed for the
/// mounted element, so [`get_client_rect`] returns the post-layout border-box
/// size. Origin is set to (0, 0); absolute position traversal is a TODO.
///
/// [`get_client_rect`]: BlitzMountedData::get_client_rect
#[derive(Clone)]
pub(crate) struct BlitzMountedData {
    /// Border-box size of the mounted element after the preceding layout pass.
    pub(crate) rect: PixelsRect,
}

impl RenderedElementBacking for BlitzMountedData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn get_client_rect(
        &self,
    ) -> Pin<Box<dyn Future<Output = MountedResult<PixelsRect>>>> {
        let rect = self.rect;
        Box::pin(async move { Ok(rect) })
    }
}

pub struct NativeConverter {}

impl HtmlEventConverter for NativeConverter {
    fn convert_cancel_data(&self, _: &PlatformEventData) -> CancelData { unimplemented!() }
    fn convert_animation_data(&self, _: &PlatformEventData) -> AnimationData { unimplemented!() }
    fn convert_clipboard_data(&self, _: &PlatformEventData) -> ClipboardData { unimplemented!() }
    fn convert_composition_data(&self, _: &PlatformEventData) -> CompositionData { unimplemented!() }
    fn convert_drag_data(&self, _: &PlatformEventData) -> DragData { unimplemented!() }
    fn convert_image_data(&self, _: &PlatformEventData) -> ImageData { unimplemented!() }
    fn convert_media_data(&self, _: &PlatformEventData) -> MediaData { unimplemented!() }

    fn convert_mounted_data(&self, event: &PlatformEventData) -> MountedData {
        // COMPAT(blitz): PlatformEventData wraps BlitzMountedData when fired from
        // DioxusDocument::poll after layout; downcast and forward the rect.
        event.downcast::<BlitzMountedData>()
            .map(|d| MountedData::from(d.clone()))
            .unwrap_or_else(|| MountedData::from(()))
    }

    fn convert_scroll_data(&self, _: &PlatformEventData) -> ScrollData { unimplemented!() }
    fn convert_selection_data(&self, _: &PlatformEventData) -> SelectionData { unimplemented!() }
    fn convert_toggle_data(&self, _: &PlatformEventData) -> ToggleData { unimplemented!() }
    fn convert_transition_data(&self, _: &PlatformEventData) -> TransitionData { unimplemented!() }
    fn convert_wheel_data(&self, event: &PlatformEventData) -> WheelData {
        // COMPAT(dioxus): blitz-shell 0.2.x does not route MouseWheel through
        // Dioxus events; this converter is invoked only if a future blitz
        // version adds wheel routing. Fall back to zero delta if no payload.
        if let Some(w) = event.downcast::<NativeWheelData>() {
            WheelData::new(w.clone())
        } else {
            WheelData::new(NativeWheelData { delta_x: 0.0, delta_y: 0.0 })
        }
    }
    fn convert_resize_data(&self, _: &PlatformEventData) -> ResizeData { unimplemented!() }
    fn convert_visible_data(&self, _: &PlatformEventData) -> VisibleData { unimplemented!() }

    fn convert_form_data(&self, event: &PlatformEventData) -> FormData {
        event.downcast::<NativeFormData>().unwrap().clone().into()
    }

    fn convert_mouse_data(&self, event: &PlatformEventData) -> MouseData {
        event.downcast::<NativeClickData>().unwrap().clone().into()
    }

    fn convert_keyboard_data(&self, event: &PlatformEventData) -> KeyboardData {
        event.downcast::<BlitzKeyboardData>().unwrap().clone().into()
    }

    fn convert_focus_data(&self, _: &PlatformEventData) -> FocusData {
        NativeFocusData {}.into()
    }

    fn convert_pointer_data(&self, event: &PlatformEventData) -> PointerData {
        PointerData::new(event.downcast::<NativeClickData>().unwrap().clone())
    }

    fn convert_touch_data(&self, event: &PlatformEventData) -> TouchData {
        // Touch events in blitz-shell 0.2.3 are synthesised as mouse events
        // (see patches/blitz-shell). NativeClickData carries the touch position.
        //
        // TODO(iris): SPEC.md §8.4 — only single touch points are forwarded.
        if let Some(click) = event.downcast::<NativeClickData>() {
            let pt = NativeTouchPoint { client_x: click.inner.x as f64, client_y: click.inner.y as f64 };
            TouchData::new(NativeTouchData {
                touches: vec![pt.clone()],
                changed_touches: vec![pt.clone()],
                target_touches: vec![pt],
                modifiers: click.inner.mods,
            })
        } else {
            let pt = NativeTouchPoint { client_x: 0.0, client_y: 0.0 };
            TouchData::new(NativeTouchData {
                touches: vec![pt.clone()],
                changed_touches: vec![pt.clone()],
                target_touches: vec![pt],
                modifiers: Modifiers::default(),
            })
        }
    }
}

/// Build a [`PixelsRect`] with origin (0, 0) and the given border-box dimensions.
///
/// Origin is set to zero because absolute-position traversal (summing parent
/// `final_layout.location` values) is not yet implemented.
/// TODO(iris): SPEC.md §11.3 — compute absolute origin by walking ancestor chain.
pub(crate) fn make_pixels_rect(width: f32, height: f32) -> PixelsRect {
    euclid::Rect::new(
        euclid::Point2D::<f64, Pixels>::new(0.0, 0.0),
        euclid::Size2D::<f64, Pixels>::new(width as f64, height as f64),
    )
}
