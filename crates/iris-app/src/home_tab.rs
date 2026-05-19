// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use appthere_ui::{AtHomeTab, BuiltinTemplate};
use dioxus::prelude::*;

use crate::state::{AppState, OpenDocument};

#[component]
pub fn IrisHomeTab(mut state: Signal<AppState>) -> Element {
    let templates = vec![
        BuiltinTemplate {
            name: "Blank Canvas".to_string(),
            description: String::new(),
            format_label: "AIF".to_string(),
        },
        BuiltinTemplate {
            name: "A4 Photo".to_string(),
            description: String::new(),
            format_label: "AIF".to_string(),
        },
        BuiltinTemplate {
            name: "Social Media".to_string(),
            description: String::new(),
            format_label: "AIF".to_string(),
        },
    ];

    // TODO(iris): SPEC.md §11 — measure actual window width for responsive layout
    let viewport_width_px = 1280.0_f32;

    rsx! {
        AtHomeTab {
            app_name: "Iris".to_string(),
            templates,
            recent_documents: vec![],
            templates_label: "Start from a template".to_string(),
            recent_label: "Recent documents".to_string(),
            browse_label: "Browse templates".to_string(),
            open_file_label: "Open file".to_string(),
            empty_recent_label: "No recent files".to_string(),
            pick_error_prefix: "Could not open file".to_string(),
            viewport_width_px,
            on_template_select: move |_idx| {
                let doc = OpenDocument::new_blank(800, 600, "Untitled");
                state.write().document = Some(doc);
                state.write().active_tab_index = 1;
            },
            on_browse_templates: move |_| {},
            on_recent_open: move |_| {},
            on_open_file: move |_| {
                // TODO(iris): Phase 3 — wire async file picker with appthere_file_access
                let doc = OpenDocument::new_blank(800, 600, "Untitled");
                state.write().document = Some(doc);
                state.write().active_tab_index = 1;
            },
        }
    }
}
