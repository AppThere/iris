// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use appthere_ui::icons::lucide;
use appthere_ui::{AtRibbon, AtRibbonGroup, AtRibbonIconButton, RibbonTabDesc, RibbonTabIndex};
use dioxus::prelude::*;

use crate::state::AppState;

#[component]
pub fn IrisRibbon(state: Signal<AppState>) -> Element {
    let tabs = vec![
        RibbonTabDesc { label: "Edit".to_string(),        is_contextual: false, aria_label: None },
        RibbonTabDesc { label: "Adjustments".to_string(), is_contextual: false, aria_label: None },
        RibbonTabDesc { label: "Export".to_string(),      is_contextual: false, aria_label: None },
    ];
    let mut active_tab = use_signal(|| 0_usize);

    rsx! {
        AtRibbon {
            tabs,
            active_tab: *active_tab.read(),
            on_tab_select: move |idx: RibbonTabIndex| *active_tab.write() = idx,
            tab_content: rsx! {
                if *active_tab.read() == 0 {
                    AtRibbonGroup {
                        label: Some("Tools".to_string()),
                        aria_label: "Tools group".to_string(),
                        AtRibbonIconButton {
                            icon_label: "B".to_string(),
                            icon: Some(lucide::BRUSH),
                            aria_label: "Brush".to_string(),
                            is_active: false,
                            is_disabled: true,
                            on_click: move |_| {},
                            // TODO(iris): Phase 3 — activate brush tool
                        }
                        AtRibbonIconButton {
                            icon_label: "E".to_string(),
                            icon: Some(lucide::ERASER),
                            aria_label: "Eraser".to_string(),
                            is_active: false,
                            is_disabled: true,
                            on_click: move |_| {},
                        }
                    }
                    AtRibbonGroup {
                        label: Some("Selection".to_string()),
                        aria_label: "Selection group".to_string(),
                        AtRibbonIconButton {
                            icon_label: "M".to_string(),
                            icon: Some(lucide::SQUARE_DASHED),
                            aria_label: "Rectangular marquee".to_string(),
                            is_active: false,
                            is_disabled: true,
                            on_click: move |_| {},
                        }
                    }
                }
                if *active_tab.read() == 1 {
                    AtRibbonGroup {
                        label: Some("Colour".to_string()),
                        aria_label: "Colour adjustment group".to_string(),
                        AtRibbonIconButton {
                            icon_label: "Lv".to_string(),
                            icon: Some(lucide::SLIDERS_HORIZONTAL),
                            aria_label: "Levels".to_string(),
                            is_active: false,
                            is_disabled: true,
                            on_click: move |_| {},
                            // TODO(iris): Phase 4 — adjustment layers
                        }
                    }
                }
                if *active_tab.read() == 2 {
                    AtRibbonGroup {
                        label: Some("Export".to_string()),
                        aria_label: "Export group".to_string(),
                        AtRibbonIconButton {
                            icon_label: "Ex".to_string(),
                            icon: Some(lucide::FILE_OUTPUT),
                            aria_label: "Export".to_string(),
                            is_active: false,
                            is_disabled: true,
                            on_click: move |_| {},
                            // TODO(iris): Phase 4 — export pipeline
                        }
                    }
                }
            },
        }
    }
}
