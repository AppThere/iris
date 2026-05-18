// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Shared XML parsing helpers used across the `xml` sub-modules.
//!
//! All helpers convert quick-xml types to Rust primitives, mapping
//! parse errors to [`AifError::XmlParse`] or [`AifError::MissingAttribute`].

use quick_xml::events::BytesStart;
use uuid::Uuid;

use crate::error::AifError;

// ── Attribute extraction ──────────────────────────────────────────────────────

/// Extract a required attribute value by local name, without namespace prefix.
///
/// Returns `AifError::MissingAttribute` if the attribute is absent.
/// Returns `AifError::XmlParse` if an `AttrError` occurs during iteration.
pub(crate) fn required_attr(
    e: &BytesStart<'_>,
    name: &str,
    part: &str,
) -> Result<String, AifError> {
    for attr in e.attributes() {
        let attr = attr.map_err(|err| AifError::XmlParse {
            part: part.into(),
            message: err.to_string(),
        })?;
        let local = attr.key.local_name();
        let key = std::str::from_utf8(local.as_ref()).unwrap_or("");
        if key == name {
            let val = attr.unescape_value().map_err(|err| AifError::XmlParse {
                part: part.into(),
                message: err.to_string(),
            })?;
            return Ok(val.into_owned());
        }
    }
    Err(AifError::MissingAttribute {
        element: local_name(e),
        attr: name.into(),
    })
}

/// Extract an optional attribute value by local name. Returns `None` if absent.
pub(crate) fn optional_attr(
    e: &BytesStart<'_>,
    name: &str,
    part: &str,
) -> Result<Option<String>, AifError> {
    for attr in e.attributes() {
        let attr = attr.map_err(|err| AifError::XmlParse {
            part: part.into(),
            message: err.to_string(),
        })?;
        let local = attr.key.local_name();
        let key = std::str::from_utf8(local.as_ref()).unwrap_or("");
        if key == name {
            let val = attr.unescape_value().map_err(|err| AifError::XmlParse {
                part: part.into(),
                message: err.to_string(),
            })?;
            return Ok(Some(val.into_owned()));
        }
    }
    Ok(None)
}

// ── Type-converting parsers ───────────────────────────────────────────────────

pub(crate) fn parse_uuid(s: &str, part: &str) -> Result<Uuid, AifError> {
    Uuid::parse_str(s).map_err(|e| AifError::XmlParse {
        part: part.into(),
        message: format!("invalid UUID '{s}': {e}"),
    })
}

pub(crate) fn parse_u32(s: &str, attr: &str, part: &str) -> Result<u32, AifError> {
    s.parse::<u32>().map_err(|e| AifError::XmlParse {
        part: part.into(),
        message: format!("attribute '{attr}' is not a valid u32: {e}"),
    })
}

pub(crate) fn parse_i32(s: &str, attr: &str, part: &str) -> Result<i32, AifError> {
    s.parse::<i32>().map_err(|e| AifError::XmlParse {
        part: part.into(),
        message: format!("attribute '{attr}' is not a valid i32: {e}"),
    })
}

pub(crate) fn parse_f32(s: &str, attr: &str, part: &str) -> Result<f32, AifError> {
    s.parse::<f32>().map_err(|e| AifError::XmlParse {
        part: part.into(),
        message: format!("attribute '{attr}' is not a valid f32: {e}"),
    })
}

pub(crate) fn parse_bool(s: &str, attr: &str, part: &str) -> Result<bool, AifError> {
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(AifError::XmlParse {
            part: part.into(),
            message: format!("attribute '{attr}' must be 'true' or 'false', got '{other}'"),
        }),
    }
}

// ── Element name helpers ──────────────────────────────────────────────────────

/// Return the local name of an element (without namespace prefix).
pub(crate) fn local_name(e: &BytesStart<'_>) -> String {
    std::str::from_utf8(e.local_name().as_ref())
        .unwrap_or("")
        .into()
}

/// Return the raw qualified name (e.g. `"iris:Canvas"` or `"x:Foo"`).
pub(crate) fn qualified_name(e: &BytesStart<'_>) -> String {
    std::str::from_utf8(e.name().as_ref()).unwrap_or("").into()
}

// ── XML output helpers ────────────────────────────────────────────────────────

/// Escape a string value for use as an XML attribute value.
pub(crate) fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
