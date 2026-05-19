// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use blitz_traits::events::{BlitzMouseButtonEvent, MouseEventButton};
use dioxus_html::{
    geometry::{ClientPoint, ElementPoint, PagePoint, ScreenPoint},
    input_data::{MouseButton, MouseButtonSet},
    point_interaction::{
        InteractionElementOffset, InteractionLocation, ModifiersInteraction, PointerInteraction,
    },
    HasMouseData, HasPointerData,
};
use keyboard_types::Modifiers;
use std::any::Any;

#[derive(Clone)]
pub struct NativeClickData {
    pub(crate) inner: BlitzMouseButtonEvent,
    /// Element-local x coordinate (offset from target element's top-left corner).
    /// Computed via blitz-dom `Node::absolute_position` at event dispatch time.
    pub(crate) element_x: f32,
    /// Element-local y coordinate (offset from target element's top-left corner).
    /// Computed via blitz-dom `Node::absolute_position` at event dispatch time.
    pub(crate) element_y: f32,
}

impl InteractionLocation for NativeClickData {
    fn client_coordinates(&self) -> ClientPoint {
        ClientPoint::new(self.inner.x as _, self.inner.y as _)
    }

    fn screen_coordinates(&self) -> ScreenPoint {
        // Screen coordinates unavailable through the synthesised mouse event path.
        ScreenPoint::new(self.inner.x as _, self.inner.y as _)
    }

    fn page_coordinates(&self) -> PagePoint {
        PagePoint::new(self.inner.x as _, self.inner.y as _)
    }
}

impl InteractionElementOffset for NativeClickData {
    fn element_coordinates(&self) -> ElementPoint {
        ElementPoint::new(self.element_x as _, self.element_y as _)
    }
}

impl ModifiersInteraction for NativeClickData {
    fn modifiers(&self) -> Modifiers {
        self.inner.mods
    }
}

impl PointerInteraction for NativeClickData {
    fn trigger_button(&self) -> Option<MouseButton> {
        Some(match self.inner.button {
            MouseEventButton::Main => MouseButton::Primary,
            MouseEventButton::Auxiliary => MouseButton::Auxiliary,
            MouseEventButton::Secondary => MouseButton::Secondary,
            MouseEventButton::Fourth => MouseButton::Fourth,
            MouseEventButton::Fifth => MouseButton::Fifth,
        })
    }

    fn held_buttons(&self) -> MouseButtonSet {
        dioxus_html::input_data::decode_mouse_button_set(self.inner.buttons.bits() as u16)
    }
}

impl HasMouseData for NativeClickData {
    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

// COMPAT(dioxus): blitz-shell 0.2.x synthesises pointer events from mouse
// events; we forward the click data as a mouse-type primary pointer.
impl HasPointerData for NativeClickData {
    fn pointer_id(&self) -> i32 {
        1
    }

    fn width(&self) -> f64 {
        1.0
    }

    fn height(&self) -> f64 {
        1.0
    }

    fn pressure(&self) -> f32 {
        if self.inner.buttons.is_empty() { 0.0 } else { 0.5 }
    }

    fn tangential_pressure(&self) -> f32 {
        0.0
    }

    fn tilt_x(&self) -> i32 {
        0
    }

    fn tilt_y(&self) -> i32 {
        0
    }

    fn twist(&self) -> i32 {
        0
    }

    fn pointer_type(&self) -> String {
        "mouse".to_string()
    }

    fn is_primary(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}
