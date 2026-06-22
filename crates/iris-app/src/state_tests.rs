// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Tests for `state.rs` (kept in a sibling file to hold `state.rs` under the
//! 300-line ceiling).

use iris_pixel::{BlendMode, LayerProp, PropValue};

use super::{AppState, OpenDocument};

#[test]
fn set_document_selects_first_root_layer() {
    let mut state = AppState::default();
    state.selection.rect = Some(kurbo::Rect::new(0.0, 0.0, 5.0, 5.0));
    let doc = OpenDocument::new_blank(64, 64, "Next");
    let expected = doc.tree.root_layer_ids().first().copied();
    state.set_document(doc);
    assert_eq!(state.selected_layer, expected);
    assert!(state.selection.rect.is_none(), "selection must reset");
    assert!(state.marquee_start.is_none(), "drag state must reset");
}

#[test]
fn validated_selected_layer_heals_stale_id() {
    let mut state = AppState::default();
    state.selected_layer = Some(uuid::Uuid::new_v4()); // not in the tree
    let healed = state.validated_selected_layer();
    let first_root = state
        .document
        .as_ref()
        .and_then(|d| d.tree.root_layer_ids().first().copied());
    assert_eq!(healed, first_root);
    assert_eq!(state.selected_layer, first_root, "state must be rewritten");
}

#[test]
fn validated_selected_layer_keeps_valid_id() {
    let mut state = AppState::default();
    let valid = state.selected_layer;
    assert!(valid.is_some(), "default state selects a layer");
    assert_eq!(state.validated_selected_layer(), valid);
}

#[test]
fn set_layer_prop_records_undoable_op() {
    let mut doc = OpenDocument::new_blank(64, 64, "Doc");
    let id = doc.tree.root_layer_ids()[0];

    // Opacity edit is applied and recorded.
    doc.set_layer_prop(id, LayerProp::Opacity, PropValue::Float(0.5));
    assert_eq!(doc.tree.get(id).unwrap().opacity, 0.5);
    assert!(doc.undo.can_undo());

    // Undo restores the original value; redo re-applies the edit.
    assert!(doc.undo());
    assert_eq!(doc.tree.get(id).unwrap().opacity, 1.0);
    assert!(doc.redo());
    assert_eq!(doc.tree.get(id).unwrap().opacity, 0.5);
}

#[test]
fn blend_mode_edit_is_undoable() {
    let mut doc = OpenDocument::new_blank(64, 64, "Doc");
    let id = doc.tree.root_layer_ids()[0];

    doc.set_layer_prop(id, LayerProp::BlendMode, PropValue::Str("multiply".into()));
    assert_eq!(doc.tree.get(id).unwrap().blend_mode, BlendMode::Multiply);
    assert!(doc.undo());
    assert_eq!(doc.tree.get(id).unwrap().blend_mode, BlendMode::Normal);
}

#[test]
fn unchanged_value_records_no_history() {
    let mut doc = OpenDocument::new_blank(64, 64, "Doc");
    let id = doc.tree.root_layer_ids()[0];
    // Layer opacity is already 1.0; setting it again must not create history.
    doc.set_layer_prop(id, LayerProp::Opacity, PropValue::Float(1.0));
    assert!(!doc.undo.can_undo());
}
