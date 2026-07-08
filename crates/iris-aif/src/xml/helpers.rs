// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Shared XML parsing helpers used across the `xml` sub-modules.
//!
//! All helpers convert quick-xml types to Rust primitives, mapping
//! parse errors to [`AifError::XmlParse`] or [`AifError::MissingAttribute`].

use quick_xml::events::{BytesRef, BytesStart, BytesText, Event};
use uuid::Uuid;

use crate::error::AifError;

// ── Text content extraction ───────────────────────────────────────────────────

/// Decodes and XML-entity-unescapes a text node's content.
///
/// `BytesText::unescape()` was removed in quick-xml 0.41 (COMPAT: split into
/// separate `decode()` + `escape::unescape()` steps); this restores the old
/// combined behaviour.
pub(crate) fn unescape_text(text: &BytesText<'_>, part: &str) -> Result<String, AifError> {
    let to_err = |e: quick_xml::Error| AifError::XmlParse {
        part: part.into(),
        message: e.to_string(),
    };
    let decoded = text
        .decode()
        .map_err(quick_xml::Error::from)
        .map_err(to_err)?;
    quick_xml::escape::unescape(&decoded)
        .map(|s| s.into_owned())
        .map_err(quick_xml::Error::from)
        .map_err(to_err)
}

/// Resolves a `&entity;` / `&#N;` general reference to its literal string.
///
/// COMPAT(quick-xml-0.41): entity/character refs no longer fold into the
/// surrounding `Event::Text`; they arrive as their own `Event::GeneralRef`
/// (verified: `&amp;` splits a run into Text, `GeneralRef`, Text, and is
/// dropped if unhandled) — every text loop here must match both. Named refs
/// resolve via the 5 predefined XML entities; anything else falls back to the
/// literal `&name;`, so nothing is silently lost.
pub(crate) fn resolve_general_ref(r: &BytesRef<'_>, part: &str) -> Result<String, AifError> {
    let to_err = |e: quick_xml::Error| AifError::XmlParse {
        part: part.into(),
        message: e.to_string(),
    };
    if let Some(ch) = r.resolve_char_ref().map_err(to_err)? {
        return Ok(ch.to_string());
    }
    let name = r.decode().map_err(quick_xml::Error::from).map_err(to_err)?;
    Ok(quick_xml::escape::resolve_predefined_entity(&name)
        .map_or_else(|| format!("&{name};"), ToString::to_string))
}

/// Extracts text from an `Event::Text` or `Event::GeneralRef`; empty string
/// for any other event kind. One-call replacement for the `unescape_text`/
/// `resolve_general_ref` pair, for call sites matching both in a single arm.
pub(crate) fn event_text(event: &Event<'_>, part: &str) -> Result<String, AifError> {
    match event {
        Event::Text(t) => unescape_text(t, part),
        Event::GeneralRef(r) => resolve_general_ref(r, part),
        _ => Ok(String::new()),
    }
}

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
            let val = attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|err| AifError::XmlParse {
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
            let val = attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|err| AifError::XmlParse {
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
