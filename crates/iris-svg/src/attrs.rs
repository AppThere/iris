// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Element attribute map with presentation-attribute + inline-`style` merging.

use std::collections::BTreeMap;

use quick_xml::events::BytesStart;

/// A merged map of an element's styling properties (presentation attributes
/// overlaid with `style="…"` declarations, which take precedence).
pub(crate) type Attrs = BTreeMap<String, String>;

/// Collect an element's attributes, then overlay any `style` declarations.
pub(crate) fn collect(e: &BytesStart) -> Attrs {
    let mut map = Attrs::new();
    for attr in e.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let val = String::from_utf8_lossy(&attr.value).into_owned();
        map.insert(key, val);
    }
    if let Some(style) = map.get("style").cloned() {
        for decl in style.split(';') {
            if let Some((k, v)) = decl.split_once(':') {
                map.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    map
}

/// Look up a string property.
pub(crate) fn str_of<'a>(a: &'a Attrs, key: &str) -> Option<&'a str> {
    a.get(key).map(|s| s.as_str())
}

/// Look up a numeric property, parsing a leading float (ignores unit suffixes
/// like `px`).
pub(crate) fn f64_of(a: &Attrs, key: &str) -> Option<f64> {
    a.get(key).and_then(|s| parse_len(s))
}

/// Parse a length, tolerating a trailing unit (`px`, `pt`, …) by reading the
/// leading numeric portion.
pub(crate) fn parse_len(s: &str) -> Option<f64> {
    let s = s.trim();
    let end = s
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+' || *c == 'e' || *c == 'E'))
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    s[..end].parse().ok()
}
