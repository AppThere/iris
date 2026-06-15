// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! [`PathObject`] — a single fillable/strokable path in a vector layer.

use kurbo::{Affine, BezPath};
use uuid::Uuid;

use crate::paint::Paint;
use crate::stroke::{FillRule, StrokePaint};

/// Stable identifier for a path object within a document.
pub type ObjectId = Uuid;

/// A vector path with its paint, stroke, fill rule, and transform.
#[derive(Debug, Clone)]
pub struct PathObject {
    /// Stable identifier.
    pub id: ObjectId,
    /// Geometry as a kurbo Bézier path, in object-local coordinates.
    pub path: BezPath,
    /// Optional fill paint; `None` = unfilled.
    pub fill: Option<Paint>,
    /// Optional stroke; `None` = unstroked.
    pub stroke: Option<StrokePaint>,
    /// Fill winding rule.
    pub fill_rule: FillRule,
    /// Affine transform from object-local space to layer space.
    pub transform: Affine,
    /// Display name.
    pub name: String,
    /// Whether the object is drawn.
    pub visible: bool,
}

impl PathObject {
    /// Create a path object with a fresh id, identity transform, no stroke, and
    /// the given fill.
    pub fn new(path: BezPath, fill: Option<Paint>) -> Self {
        Self {
            id: Uuid::new_v4(),
            path,
            fill,
            stroke: None,
            fill_rule: FillRule::default(),
            transform: Affine::IDENTITY,
            name: String::new(),
            visible: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{Color, Paint};

    #[test]
    fn new_object_has_identity_transform_and_fresh_id() {
        let p = PathObject::new(BezPath::new(), Some(Paint::Solid(Color::BLACK)));
        assert_eq!(p.transform, Affine::IDENTITY);
        assert!(p.visible);
        assert_ne!(p.id, Uuid::nil());
    }
}
