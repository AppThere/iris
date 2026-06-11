# Malformed-input fixture corpus

Each `.aif` here is a deliberately corrupt container targeting one `AifError`
variant. `tests/malformed.rs` asserts the file → variant mapping; the
generator that produced these files is `tests/gen_fixtures.rs`.

Regenerate (overwrites all fixtures):

```
cargo test -p iris-aif --features gen-fixtures --test gen_fixtures
```

| Fixture | Triggers |
|---|---|
| `not_a_zip.aif` | `Opc` (not a ZIP container) |
| `missing_document_xml.aif` | `MissingRequiredPart` |
| `unsupported_major_version.aif` | `UnsupportedMajorVersion` |
| `malformed_document_xml.aif` | `XmlParse` (unparseable XML) |
| `missing_canvas.aif` | `MissingAttribute` |
| `oversized_canvas.aif` | `XmlParse` (canvas above `MAX_CANVAS_DIMENSION`) |
| `pixel_mode_no_artboard.aif` | `InvalidPixelModeArtboard` |
| `duplicate_layer_id.aif` | `DuplicateLayerId` |
| `missing_layer_meta.aif` | `MissingLayerMeta` |
| `unknown_blend_mode.aif` | `UnknownBlendMode` |
| `oversized_crop_bounds.aif` | `XmlParse` (crop bounds above limit; tile-loop DoS guard) |
| `corrupt_tile.aif` | `TileReadError` (tile part is not EXR) |
| `oversized_tile.aif` | `TileReadError` (EXR resolution ≠ 256×256; OOB-write regression) |

Variants with no fixture (not yet constructible from a file) are documented
in the header of `tests/malformed.rs`.
