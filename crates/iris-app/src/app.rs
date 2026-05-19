// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use appthere_ui::AtThemeContext;
use dioxus::prelude::*;

use crate::layout::AppLayout;
use crate::state::AppState;

#[component]
pub fn App() -> Element {
    provide_context(AtThemeContext::default());
    let state = use_signal(AppState::default);
    rsx! {
        AppLayout { state }
    }
}
