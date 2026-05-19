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
        div {
            style: "margin: 0; padding: 0; width: 100%; height: 100vh; \
                    box-sizing: border-box; overflow: hidden;",
            AppLayout { state }
        }
    }
}
