// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! XML serialisation and deserialisation for AIF document parts.
//!
//! Sub-modules:
//! - [`document`] — `iris/document.xml` (§4.4)
//! - [`meta_doc`] — `iris/metadata.xml` (§4.5)
//! - [`layer_meta`] — types and read path for `iris/layers/{id}/meta.xml` (§4.6)
//! - [`layer_meta_write`] — write path and `layer_from_spec` (§4.6)
//! - [`helpers`] — shared parsing utilities (no public API surface)
//!
//! All parsing uses event-based `quick_xml::Reader` (SAX-style); no DOM tree
//! is constructed. Attribute errors from quick-xml 0.36's `AttrError` are
//! mapped to [`crate::AifError::XmlParse`].

pub(crate) mod document;
pub(crate) mod helpers;
pub(crate) mod layer_meta;
pub(crate) mod layer_meta_write;
pub(crate) mod meta_doc;

// ── Re-exports used by reader.rs and writer.rs (PROMPT 3C) ───────────────────

pub(crate) use document::{read_document_xml, write_document_xml, LayerTreeEntry};
pub(crate) use layer_meta::{read_layer_meta_xml, LayerMetaSpec};
pub(crate) use layer_meta_write::{layer_from_spec, write_layer_meta_xml};
pub(crate) use meta_doc::{read_metadata_xml, write_metadata_xml, DocumentMetadata, SessionInfo};
