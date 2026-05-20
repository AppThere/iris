// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

// COMPAT(dioxus): blitz-shell 0.2.x routes MouseWheel events directly to
// CSS scroll via scroll_node_by_has_changed — it never fires a Dioxus
// onwheel event. This type exists so convert_wheel_data does not panic
// when/if blitz adds wheel event routing to Dioxus. All coordinate and
// modifier fields default to zero/empty until blitz passes real data.

use dioxus_html::{
    geometry::{ClientPoint, ElementPoint, PagePoint, ScreenPoint, WheelDelta},
    input_data::{MouseButton, MouseButtonSet},
    point_interaction::{
        InteractionElementOffset, InteractionLocation, ModifiersInteraction, PointerInteraction,
    },
    HasMouseData, HasWheelData,
};
use keyboard_types::Modifiers;
use std::any::Any;

/// Stub wheel-event data produced when blitz routes a wheel event to Dioxus.
///
/// `delta_x` and `delta_y` are in screen pixels (positive = scroll down/right).
/// All pointer fields are zero because blitz does not include cursor position
/// in the raw wheel event payload.
#[derive(Clone, Debug)]
pub(crate) struct NativeWheelData {
    pub(crate) delta_x: f64,
    pub(crate) delta_y: f64,
}

impl InteractionLocation for NativeWheelData {
    fn client_coordinates(&self) -> ClientPoint { ClientPoint::new(0.0, 0.0) }
    fn screen_coordinates(&self) -> ScreenPoint { ScreenPoint::new(0.0, 0.0) }
    fn page_coordinates(&self) -> PagePoint { PagePoint::new(0.0, 0.0) }
}

impl InteractionElementOffset for NativeWheelData {
    fn element_coordinates(&self) -> ElementPoint { ElementPoint::new(0.0, 0.0) }
}

impl ModifiersInteraction for NativeWheelData {
    fn modifiers(&self) -> Modifiers { Modifiers::default() }
}

impl PointerInteraction for NativeWheelData {
    fn trigger_button(&self) -> Option<MouseButton> { None }
    fn held_buttons(&self) -> MouseButtonSet {
        dioxus_html::input_data::decode_mouse_button_set(0)
    }
}

impl HasMouseData for NativeWheelData {
    fn as_any(&self) -> &dyn Any { self as &dyn Any }
}

impl HasWheelData for NativeWheelData {
    fn delta(&self) -> WheelDelta {
        WheelDelta::pixels(self.delta_x, self.delta_y, 0.0)
    }

    fn as_any(&self) -> &dyn Any { self as &dyn Any }
}
