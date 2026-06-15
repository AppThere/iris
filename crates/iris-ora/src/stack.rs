// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! `stack.xml` data model and parser.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::error::OraError;

/// Parsed `stack.xml`: canvas dimensions and the top-level node list.
pub(crate) struct OraImage {
    pub width: u32,
    pub height: u32,
    /// Top-first list of root nodes (index 0 is the topmost).
    pub children: Vec<OraNode>,
}

/// A node in an ORA stack: either a raster layer or a nested group.
pub(crate) enum OraNode {
    Layer(OraLayer),
    Stack(OraStack),
}

/// A raster layer entry (`<layer>`).
pub(crate) struct OraLayer {
    pub name: String,
    pub src: String,
    pub x: i32,
    pub y: i32,
    pub opacity: f32,
    pub visible: bool,
    pub composite_op: String,
}

/// A group entry (`<stack>`).
pub(crate) struct OraStack {
    pub name: String,
    pub opacity: f32,
    pub visible: bool,
    pub composite_op: String,
    pub children: Vec<OraNode>,
}

/// A frame on the parse stack: a partially-built group accumulating children.
struct Frame {
    name: String,
    opacity: f32,
    visible: bool,
    composite_op: String,
    children: Vec<OraNode>,
}

/// Parse `stack.xml` into an [`OraImage`].
pub(crate) fn parse(xml: &str) -> Result<OraImage, OraError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().expand_empty_elements = true;
    reader.config_mut().trim_text(true);

    let mut width = 0u32;
    let mut height = 0u32;
    // The `<image>` holds exactly one root `<stack>` whose children are the
    // top-level layers; `frames` tracks the currently-open stacks. The first
    // stack to open is the root, and its children become the result.
    let mut frames: Vec<Frame> = Vec::new();
    let mut root_children: Vec<OraNode> = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(OraError::Xml(e.to_string())),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                b"image" => {
                    width = attr_u32(&e, "w").unwrap_or(0);
                    height = attr_u32(&e, "h").unwrap_or(0);
                }
                b"stack" => frames.push(frame_from(&e)),
                b"layer" => {
                    if let Some(frame) = frames.last_mut() {
                        frame.children.push(OraNode::Layer(layer_from(&e)));
                    }
                }
                _ => {}
            },
            Ok(Event::End(e)) => {
                if e.local_name().as_ref() == b"stack" {
                    if let Some(done) = frames.pop() {
                        match frames.last_mut() {
                            // A nested stack closes into its parent as a group.
                            Some(parent) => parent.children.push(OraNode::Stack(OraStack {
                                name: done.name,
                                opacity: done.opacity,
                                visible: done.visible,
                                composite_op: done.composite_op,
                                children: done.children,
                            })),
                            // The root stack closes: its children are the result.
                            None => root_children = done.children,
                        }
                    }
                }
            }
            _ => {}
        }
        buf.clear();
    }

    if width == 0 || height == 0 {
        return Err(OraError::MissingImageDimensions);
    }
    Ok(OraImage { width, height, children: root_children })
}

fn frame_from(e: &BytesStart) -> Frame {
    Frame {
        name: attr_str(e, "name").unwrap_or_default(),
        opacity: attr_f32(e, "opacity").unwrap_or(1.0),
        visible: attr_str(e, "visibility").map(|v| v != "hidden").unwrap_or(true),
        composite_op: attr_str(e, "composite-op").unwrap_or_else(|| "svg:src-over".to_string()),
        children: Vec::new(),
    }
}

fn layer_from(e: &BytesStart) -> OraLayer {
    OraLayer {
        name: attr_str(e, "name").unwrap_or_default(),
        src: attr_str(e, "src").unwrap_or_default(),
        x: attr_i32(e, "x").unwrap_or(0),
        y: attr_i32(e, "y").unwrap_or(0),
        opacity: attr_f32(e, "opacity").unwrap_or(1.0),
        visible: attr_str(e, "visibility").map(|v| v != "hidden").unwrap_or(true),
        composite_op: attr_str(e, "composite-op").unwrap_or_else(|| "svg:src-over".to_string()),
    }
}

fn attr_str(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.as_ref() == key.as_bytes()).map(|a| {
        String::from_utf8_lossy(&a.value).into_owned()
    })
}
fn attr_u32(e: &BytesStart, key: &str) -> Option<u32> {
    attr_str(e, key).and_then(|v| v.trim().parse().ok())
}
fn attr_i32(e: &BytesStart, key: &str) -> Option<i32> {
    attr_str(e, key).and_then(|v| v.trim().parse().ok())
}
fn attr_f32(e: &BytesStart, key: &str) -> Option<f32> {
    attr_str(e, key).and_then(|v| v.trim().parse().ok())
}
