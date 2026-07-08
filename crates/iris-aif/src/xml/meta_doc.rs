// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Read and write `iris/metadata.xml` (SPEC.md §4.5).
//!
//! `metadata.xml` is optional on read: a missing part is a [`tracing::warn!`],
//! not an error. Writers always emit a fresh `metadata.xml` with a single
//! `<iris:Session>` entry appended (do not read–modify–write; the reader
//! returns an empty struct when the part is absent).

use quick_xml::{Reader, events::Event};

use crate::{
    error::AifError,
    parts::{IRIS_EXT_NS, IRIS_NS},
    xml::helpers::{event_text, local_name},
};

/// Decoded contents of `iris/metadata.xml`.
#[derive(Debug, Default, Clone)]
pub(crate) struct DocumentMetadata {
    /// Document title (e.g. filename without extension).
    pub title: Option<String>,
    /// Primary author.
    pub author: Option<String>,
    /// Free-form description.
    pub description: Option<String>,
    /// Comma-separated keyword list.
    pub keywords: Option<String>,
}

/// Information about the current editing session, written into
/// `<iris:Session>` during save.
#[derive(Debug, Clone)]
pub(crate) struct SessionInfo {
    /// ISO 8601 UTC timestamp when the session started.
    pub started_at: String,
    /// ISO 8601 UTC timestamp when the document was saved.
    pub ended_at: String,
    /// Application version string (e.g. `"0.1.0"`).
    pub app_version: String,
    /// `std::env::consts::OS` + `-` + `std::env::consts::ARCH`.
    pub platform: String,
}

// ── Read ──────────────────────────────────────────────────────────────────────

/// Parse `iris/metadata.xml` bytes into [`DocumentMetadata`].
///
/// Returns `Ok(Default::default())` on an empty slice.
/// The caller should emit `tracing::warn!` if the part is absent before
/// calling this function.
pub(crate) fn read_metadata_xml(bytes: &[u8]) -> Result<DocumentMetadata, AifError> {
    if bytes.is_empty() {
        return Ok(DocumentMetadata::default());
    }
    let part = crate::parts::METADATA_XML;
    let mut reader = Reader::from_reader(bytes);
    // COMPAT(quick-xml-0.41): `trim_text(true)` trims each individual
    // `Event::Text` fragment, and an `&entity;` now splits one run into
    // several such fragments — trimming per-fragment would strip whitespace
    // adjacent to the entity (e.g. "Alice &amp; Bob" -> "Alice&Bob"). Read
    // untrimmed and trim the fully-accumulated string once, below.
    reader.config_mut().trim_text(false);

    let mut meta = DocumentMetadata::default();
    let mut current_tag: Option<String> = None;
    let mut current_text = String::new();

    loop {
        match reader.read_event() {
            Err(e) => {
                return Err(AifError::XmlParse {
                    part: part.into(),
                    message: e.to_string(),
                });
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(ref e)) => {
                current_tag = Some(local_name(e));
                current_text.clear();
            }
            Ok(ref ev @ (Event::Text(_) | Event::GeneralRef(_))) => {
                // A tag's content can arrive as several Text/GeneralRef
                // fragments (e.g. an `&amp;` splits the run) — accumulate
                // rather than overwrite, or only the last fragment survives.
                current_text.push_str(&event_text(ev, part)?);
            }
            Ok(Event::End(_)) => {
                let text = std::mem::take(&mut current_text);
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    let text = trimmed.to_string();
                    match current_tag.as_deref() {
                        Some("Title") => meta.title = Some(text),
                        Some("Author") => meta.author = Some(text),
                        Some("Description") => meta.description = Some(text),
                        Some("Keywords") => meta.keywords = Some(text),
                        _ => {}
                    }
                }
                current_tag = None;
            }
            Ok(_) => {}
        }
    }
    Ok(meta)
}

// ── Write ─────────────────────────────────────────────────────────────────────

/// Serialise `iris/metadata.xml` bytes with a single `<iris:Session>` entry.
///
/// Previous session history is not preserved in Phase 1 (the op log owns
/// authoritative session data; `metadata.xml` is a human-readable summary).
pub(crate) fn write_metadata_xml(
    meta: &DocumentMetadata,
    session: &SessionInfo,
) -> Result<Vec<u8>, AifError> {
    let title = meta.title.as_deref().unwrap_or("");
    let author = meta.author.as_deref().unwrap_or("");
    let description = meta.description.as_deref().unwrap_or("");
    let keywords = meta.keywords.as_deref().unwrap_or("");

    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <iris:Metadata xmlns:iris=\"{IRIS_NS}\" xmlns:x=\"{IRIS_EXT_NS}\">\n\
         \x20\x20<iris:Title>{title}</iris:Title>\n\
         \x20\x20<iris:Author>{author}</iris:Author>\n\
         \x20\x20<iris:Description>{description}</iris:Description>\n\
         \x20\x20<iris:Keywords>{keywords}</iris:Keywords>\n\
         \n\
         \x20\x20<iris:EditingHistory>\n\
         \x20\x20\x20\x20<iris:Session\n\
         \x20\x20\x20\x20\x20\x20\x20\x20startedAt=\"{started}\"\n\
         \x20\x20\x20\x20\x20\x20\x20\x20endedAt=\"{ended}\"\n\
         \x20\x20\x20\x20\x20\x20\x20\x20appVersion=\"{app}\"\n\
         \x20\x20\x20\x20\x20\x20\x20\x20platform=\"{platform}\" />\n\
         \x20\x20</iris:EditingHistory>\n\
         \n\
         </iris:Metadata>\n",
        started = session.started_at,
        ended = session.ended_at,
        app = session.app_version,
        platform = session.platform,
    );
    Ok(xml.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_metadata() {
        let meta = DocumentMetadata {
            title: Some("Test Document".into()),
            author: Some("Alice".into()),
            description: None,
            keywords: None,
        };
        let session = SessionInfo {
            started_at: "2024-01-01T00:00:00Z".into(),
            ended_at: "2024-01-01T01:00:00Z".into(),
            app_version: "0.1.0".into(),
            platform: "linux-x86_64".into(),
        };
        let bytes = write_metadata_xml(&meta, &session).unwrap();
        let parsed = read_metadata_xml(&bytes).unwrap();
        assert_eq!(parsed.title.as_deref(), Some("Test Document"));
        assert_eq!(parsed.author.as_deref(), Some("Alice"));
    }

    #[test]
    fn empty_bytes_returns_default() {
        let meta = read_metadata_xml(&[]).unwrap();
        assert!(meta.title.is_none());
    }

    #[test]
    fn entity_reference_mid_run_is_not_dropped() {
        // Regression test: quick-xml 0.41 reports an `&amp;` as its own
        // `Event::GeneralRef` between two `Event::Text` fragments rather than
        // folding it into a single Text event. A reader that only matches
        // `Event::Text` silently drops the entity and any Text fragment after
        // it if the code overwrites instead of accumulating.
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<iris:Metadata xmlns:iris="urn:iris" xmlns:x="urn:iris-ext">
  <iris:Author>Alice &amp; Bob</iris:Author>
</iris:Metadata>
"#;
        let meta = read_metadata_xml(xml).unwrap();
        assert_eq!(meta.author.as_deref(), Some("Alice & Bob"));
    }
}
