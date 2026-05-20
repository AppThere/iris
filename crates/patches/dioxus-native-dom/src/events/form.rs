// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use dioxus_html::{FormValue, HasFileData, HasFocusData, HasFormData};
use std::any::Any;

#[derive(Clone, Debug)]
pub struct NativeFormData {
    pub value: String,
    pub values: Vec<(String, FormValue)>,
}

impl HasFormData for NativeFormData {
    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }

    fn value(&self) -> String {
        self.value.clone()
    }

    fn values(&self) -> Vec<(String, FormValue)> {
        self.values.clone()
    }

    fn valid(&self) -> bool {
        true
    }
}

impl HasFileData for NativeFormData {
    fn files(&self) -> Vec<dioxus_html::FileData> {
        vec![]
    }
}

#[derive(Clone)]
pub struct NativeFocusData {}

impl HasFocusData for NativeFocusData {
    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}
