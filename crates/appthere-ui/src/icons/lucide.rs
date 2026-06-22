// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Vendored Lucide icon markup (inner SVG elements, 24×24 viewBox).
//!
//! Source: <https://github.com/lucide-icons/lucide> — ISC License,
//! Copyright (c) Lucide Contributors. Vendored as string constants instead of
//! an icon crate dependency (CLAUDE.md: no unflagged dependencies); rendered
//! by [`AtIcon`][crate::AtIcon] via `dangerous_inner_html` and parsed by
//! Blitz's inline-SVG (usvg) path.
//!
//! Regenerate by fetching `icons/{name}.svg` from the Lucide repository and
//! stripping the outer `<svg>` element.

/// Lucide `aperture` icon.
pub const APERTURE: &str = "<circle cx=\"12\" cy=\"12\" r=\"10\" /> <path d=\"m14.31 8 5.74 9.94\" /> <path d=\"M9.69 8h11.48\" /> <path d=\"m7.38 12 5.74-9.94\" /> <path d=\"M9.69 16 3.95 6.06\" /> <path d=\"M14.31 16H2.83\" /> <path d=\"m16.62 12-5.74 9.94\" />";

/// Lucide `brush` icon.
pub const BRUSH: &str = "<path d=\"m11 10 3 3\" /> <path d=\"M6.5 21A3.5 3.5 0 1 0 3 17.5a2.62 2.62 0 0 1-.708 1.792A1 1 0 0 0 3 21z\" /> <path d=\"M9.969 17.031 21.378 5.624a1 1 0 0 0-3.002-3.002L6.967 14.031\" />";

/// Lucide `eraser` icon.
pub const ERASER: &str = "<path d=\"M21 21H8a2 2 0 0 1-1.42-.587l-3.994-3.999a2 2 0 0 1 0-2.828l10-10a2 2 0 0 1 2.829 0l5.999 6a2 2 0 0 1 0 2.828L12.834 21\" /> <path d=\"m5.082 11.09 8.828 8.828\" />";

/// Lucide `file-output` icon.
pub const FILE_OUTPUT: &str = "<path d=\"M4.226 20.925A2 2 0 0 0 6 22h12a2 2 0 0 0 2-2V8a2.4 2.4 0 0 0-.706-1.706l-3.588-3.588A2.4 2.4 0 0 0 14 2H6a2 2 0 0 0-2 2v3.127\" /> <path d=\"M14 2v5a1 1 0 0 0 1 1h5\" /> <path d=\"m5 11-3 3\" /> <path d=\"m5 17-3-3h10\" />";

/// Lucide `file-plus` icon.
pub const FILE_PLUS: &str = "<path d=\"M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z\" /> <path d=\"M14 2v5a1 1 0 0 0 1 1h5\" /> <path d=\"M9 15h6\" /> <path d=\"M12 18v-6\" />";

/// Lucide `folder-open` icon.
pub const FOLDER_OPEN: &str = "<path d=\"m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2\" />";

/// Lucide `grid-2x2` icon.
pub const GRID_2X2: &str = "<path d=\"M12 3v18\" /> <path d=\"M3 12h18\" /> <rect x=\"3\" y=\"3\" width=\"18\" height=\"18\" rx=\"2\" />";

/// Lucide `image-plus` icon.
pub const IMAGE_PLUS: &str = "<path d=\"M16 5h6\" /> <path d=\"M19 2v6\" /> <path d=\"M21 11.5V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7.5\" /> <path d=\"m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21\" /> <circle cx=\"9\" cy=\"9\" r=\"2\" />";

/// Lucide `layers` icon.
pub const LAYERS: &str = "<path d=\"M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83z\" /> <path d=\"M2 12a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 12\" /> <path d=\"M2 17a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 17\" />";

/// Lucide `paint-bucket` icon.
pub const PAINT_BUCKET: &str = "<path d=\"M11 7 6 2\" /> <path d=\"M18.992 12H2.041\" /> <path d=\"M21.145 18.38A3.34 3.34 0 0 1 20 16.5a3.3 3.3 0 0 1-1.145 1.88c-.575.46-.855 1.02-.855 1.595A2 2 0 0 0 20 22a2 2 0 0 0 2-2.025c0-.58-.285-1.13-.855-1.595\" /> <path d=\"m8.5 4.5 2.148-2.148a1.205 1.205 0 0 1 1.704 0l7.296 7.296a1.205 1.205 0 0 1 0 1.704l-7.592 7.592a3.615 3.615 0 0 1-5.112 0l-3.888-3.888a3.615 3.615 0 0 1 0-5.112L5.67 7.33\" />";

/// Lucide `pen-tool` icon.
pub const PEN_TOOL: &str = "<path d=\"M15.707 21.293a1 1 0 0 1-1.414 0l-1.586-1.586a1 1 0 0 1 0-1.414l5.586-5.586a1 1 0 0 1 1.414 0l1.586 1.586a1 1 0 0 1 0 1.414z\" /> <path d=\"m18 13-1.375-6.874a1 1 0 0 0-.746-.776L3.235 2.028a1 1 0 0 0-1.207 1.207L5.35 15.879a1 1 0 0 0 .776.746L13 18\" /> <path d=\"m2.3 2.3 7.286 7.286\" /> <circle cx=\"11\" cy=\"11\" r=\"2\" />";

/// Lucide `pipette` icon.
pub const PIPETTE: &str = "<path d=\"m12 9-8.414 8.414A2 2 0 0 0 3 18.828v1.344a2 2 0 0 1-.586 1.414A2 2 0 0 1 3.828 21h1.344a2 2 0 0 0 1.414-.586L15 12\" /> <path d=\"m18 9 .4.4a1 1 0 1 1-3 3l-3.8-3.8a1 1 0 1 1 3-3l.4.4 3.4-3.4a1 1 0 1 1 3 3z\" /> <path d=\"m2 22 .414-.414\" />";

/// Lucide `plus` icon.
pub const PLUS: &str = "<path d=\"M5 12h14\" /> <path d=\"M12 5v14\" />";

/// Lucide `sliders-horizontal` icon.
pub const SLIDERS_HORIZONTAL: &str = "<path d=\"M10 5H3\" /> <path d=\"M12 19H3\" /> <path d=\"M14 3v4\" /> <path d=\"M16 17v4\" /> <path d=\"M21 12h-9\" /> <path d=\"M21 19h-5\" /> <path d=\"M21 5h-7\" /> <path d=\"M8 10v4\" /> <path d=\"M8 12H3\" />";

/// Lucide `square-dashed` icon.
pub const SQUARE_DASHED: &str = "<path d=\"M5 3a2 2 0 0 0-2 2\" /> <path d=\"M19 3a2 2 0 0 1 2 2\" /> <path d=\"M21 19a2 2 0 0 1-2 2\" /> <path d=\"M5 21a2 2 0 0 1-2-2\" /> <path d=\"M9 3h1\" /> <path d=\"M9 21h1\" /> <path d=\"M14 3h1\" /> <path d=\"M14 21h1\" /> <path d=\"M3 9v1\" /> <path d=\"M21 9v1\" /> <path d=\"M3 14v1\" /> <path d=\"M21 14v1\" />";

/// Lucide `trash-2` icon.
pub const TRASH_2: &str = "<path d=\"M10 11v6\" /> <path d=\"M14 11v6\" /> <path d=\"M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6\" /> <path d=\"M3 6h18\" /> <path d=\"M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2\" />";

/// Lucide `type` icon.
pub const TYPE: &str = "<path d=\"M12 4v16\" /> <path d=\"M4 7V5a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v2\" /> <path d=\"M9 20h6\" />";
