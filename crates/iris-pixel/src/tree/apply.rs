// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Reading and applying scalar [`LayerProp`] values — the building blocks of
//! undoable `LayerOp::SetProp` operations (iris-ops). Kept separate from the
//! structural tree ops (`tree/ops.rs`) to stay within the file-length ceiling.

use crate::{BlendMode, LayerId, LayerProp, PropValue};

use super::{LayerTree, LayerTreeError};

impl LayerTree {
    /// Read the current value of a scalar layer property as a [`PropValue`].
    ///
    /// Returns `None` if no layer with `id` exists. Callers capture this as the
    /// `before` value when building an undoable `SetProp` op.
    pub fn layer_prop(&self, id: LayerId, prop: &LayerProp) -> Option<PropValue> {
        let layer = self.get(id)?;
        Some(match prop {
            LayerProp::Name => PropValue::Str(layer.name.clone()),
            LayerProp::Visible => PropValue::Bool(layer.visible),
            LayerProp::Locked => PropValue::Bool(layer.locked),
            LayerProp::Opacity => PropValue::Float(layer.opacity),
            // Blend mode travels as its AIF string id so iris-ops need not name
            // the iris-pixel `BlendMode` type (see iris_ops::LayerProp docs).
            LayerProp::BlendMode => PropValue::Str(layer.blend_mode.to_aif_str().to_string()),
        })
    }

    /// Apply a scalar property `value` to layer `id`. This is the primitive used
    /// for both `SetProp` redo (apply `after`) and undo (apply `before`).
    ///
    /// A `prop`/`value` type mismatch or an unknown blend-mode string is logged
    /// and ignored rather than panicking — library code never panics on data.
    pub fn set_layer_prop(
        &mut self,
        id: LayerId,
        prop: &LayerProp,
        value: &PropValue,
    ) -> Result<(), LayerTreeError> {
        let layer = self.get_mut(id).ok_or(LayerTreeError::NotFound(id))?;
        match (prop, value) {
            (LayerProp::Name, PropValue::Str(s)) => layer.name = s.clone(),
            (LayerProp::Visible, PropValue::Bool(b)) => layer.visible = *b,
            (LayerProp::Locked, PropValue::Bool(b)) => layer.locked = *b,
            (LayerProp::Opacity, PropValue::Float(f)) => layer.opacity = f.clamp(0.0, 1.0),
            (LayerProp::BlendMode, PropValue::Str(s)) => match BlendMode::from_aif_str(s) {
                Some(m) => layer.blend_mode = m,
                None => {
                    tracing::warn!(value = %s, "set_layer_prop: unknown blend mode id; ignored")
                }
            },
            _ => tracing::warn!(?prop, "set_layer_prop: value type mismatch; ignored"),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::blend::BlendMode;
    use crate::color_space::LINEAR_SRGB;
    use crate::layer::{Layer, LayerContent};
    use crate::pixel_layer::{BitDepth, ChannelLayout, ExrCompression, PixelLayer};
    use crate::tile::TileCache;

    fn tree_with_layer() -> (LayerTree, LayerId) {
        let layer = Layer {
            id: Uuid::new_v4(), name: "L".into(),
            visible: true, locked: false, opacity: 1.0,
            blend_mode: BlendMode::Normal, clipping_mask: false, mask: None,
            content: LayerContent::Pixel(PixelLayer {
                channel_layout: ChannelLayout::Rgba, bit_depth: BitDepth::F16,
                color_space: LINEAR_SRGB, compression: ExrCompression::Zip,
                canvas_offset_x: 0, canvas_offset_y: 0,
                crop_bounds: None, tiles: TileCache::new(4),
            }),
        };
        let id = layer.id;
        let mut t = LayerTree::new(64, 64, 72.0, 72.0);
        t.add_layer(None, 0, layer).expect("add");
        (t, id)
    }

    #[test]
    fn opacity_read_apply_round_trip() {
        let (mut t, id) = tree_with_layer();
        let before = t.layer_prop(id, &LayerProp::Opacity).expect("read");
        assert_eq!(before, PropValue::Float(1.0));
        t.set_layer_prop(id, &LayerProp::Opacity, &PropValue::Float(0.4)).expect("set");
        assert_eq!(t.get(id).unwrap().opacity, 0.4);
        // Undo by re-applying the captured `before`.
        t.set_layer_prop(id, &LayerProp::Opacity, &before).expect("undo");
        assert_eq!(t.get(id).unwrap().opacity, 1.0);
    }

    #[test]
    fn opacity_is_clamped() {
        let (mut t, id) = tree_with_layer();
        t.set_layer_prop(id, &LayerProp::Opacity, &PropValue::Float(2.0)).expect("set");
        assert_eq!(t.get(id).unwrap().opacity, 1.0);
    }

    #[test]
    fn blend_mode_via_aif_string() {
        let (mut t, id) = tree_with_layer();
        let before = t.layer_prop(id, &LayerProp::BlendMode).expect("read");
        assert_eq!(before, PropValue::Str("normal".into()));
        let after = PropValue::Str("multiply".into());
        t.set_layer_prop(id, &LayerProp::BlendMode, &after).expect("set");
        assert_eq!(t.get(id).unwrap().blend_mode, BlendMode::Multiply);
        t.set_layer_prop(id, &LayerProp::BlendMode, &before).expect("undo");
        assert_eq!(t.get(id).unwrap().blend_mode, BlendMode::Normal);
    }

    #[test]
    fn unknown_blend_mode_string_ignored() {
        let (mut t, id) = tree_with_layer();
        t.set_layer_prop(id, &LayerProp::BlendMode, &PropValue::Str("bogus".into())).expect("ok");
        assert_eq!(t.get(id).unwrap().blend_mode, BlendMode::Normal); // unchanged
    }

    #[test]
    fn missing_layer_errors() {
        let (mut t, _) = tree_with_layer();
        let missing = Uuid::new_v4();
        assert!(t.layer_prop(missing, &LayerProp::Opacity).is_none());
        assert!(matches!(
            t.set_layer_prop(missing, &LayerProp::Opacity, &PropValue::Float(0.5)),
            Err(LayerTreeError::NotFound(_))
        ));
    }

    #[test]
    fn type_mismatch_is_ignored() {
        let (mut t, id) = tree_with_layer();
        // Bool value for an Opacity prop: ignored, layer unchanged, no panic.
        t.set_layer_prop(id, &LayerProp::Opacity, &PropValue::Bool(true)).expect("ok");
        assert_eq!(t.get(id).unwrap().opacity, 1.0);
    }
}
