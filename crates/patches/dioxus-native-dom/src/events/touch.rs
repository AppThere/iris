// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use dioxus_html::{
    geometry::{ClientPoint, PagePoint, ScreenPoint},
    point_interaction::{InteractionLocation, ModifiersInteraction},
    HasTouchData, HasTouchPointData, TouchPoint,
};
use keyboard_types::Modifiers;
use std::any::Any;

/// A single touch contact point, carrying client-coordinate position.
#[derive(Clone, Debug)]
pub(crate) struct NativeTouchPoint {
    pub(crate) client_x: f64,
    pub(crate) client_y: f64,
}

impl InteractionLocation for NativeTouchPoint {
    fn client_coordinates(&self) -> ClientPoint {
        ClientPoint::new(self.client_x, self.client_y)
    }

    fn screen_coordinates(&self) -> ScreenPoint {
        // Screen coordinates unavailable through synthesised mouse event path;
        // return client coordinates as a reasonable fallback.
        ScreenPoint::new(self.client_x, self.client_y)
    }

    fn page_coordinates(&self) -> PagePoint {
        PagePoint::new(self.client_x, self.client_y)
    }
}

impl HasTouchPointData for NativeTouchPoint {
    fn identifier(&self) -> i32 {
        0
    }

    fn force(&self) -> f64 {
        1.0
    }

    fn radius(&self) -> ScreenPoint {
        ScreenPoint::new(1.0, 1.0)
    }

    fn rotation(&self) -> f64 {
        0.0
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

/// Touch event data for a single synthesised touch contact.
#[derive(Clone, Debug)]
pub(crate) struct NativeTouchData {
    pub(crate) touches: Vec<NativeTouchPoint>,
    pub(crate) changed_touches: Vec<NativeTouchPoint>,
    pub(crate) target_touches: Vec<NativeTouchPoint>,
    pub(crate) modifiers: Modifiers,
}

impl ModifiersInteraction for NativeTouchData {
    fn modifiers(&self) -> Modifiers {
        self.modifiers
    }
}

impl HasTouchData for NativeTouchData {
    fn touches(&self) -> Vec<TouchPoint> {
        self.touches.iter().cloned().map(TouchPoint::new).collect()
    }

    fn touches_changed(&self) -> Vec<TouchPoint> {
        self.changed_touches.iter().cloned().map(TouchPoint::new).collect()
    }

    fn target_touches(&self) -> Vec<TouchPoint> {
        self.target_touches.iter().cloned().map(TouchPoint::new).collect()
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}
