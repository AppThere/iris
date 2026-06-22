// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

mod app;
mod canvas_area;
mod home_tab;
mod layer_properties;
mod layers_panel;
mod layout;
mod ribbon;
mod state;
mod status_bar;
mod tool_dispatch;
mod tool_palette;

use app::App;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("AppThere Iris starting");
    dioxus::launch(App);
}
