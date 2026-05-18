# ADR 002 — Artisan Interchange Format (AIF)

**Status:** Accepted  
**Date:** 2024-11-01  
**Deciders:** AppThere core team

## Context

Iris needs a native file format that:
- Stores both pixel tile data and vector path data
- Supports CRDT-based collaboration history
- Is inspectable without proprietary tools
- Does not require Iris to be installed to validate or examine
- Follows the same container architecture as Loki's native format (reducing custom infrastructure)

## Decision

AIF is an OPC/ZIP container (`appthere-opc`) using:
- XML for all structural metadata (`document.xml`, `meta.xml`)
- OpenEXR for all pixel tile data (f16/f32 precision, industry-standard compression)
- FlatBuffers for vector path data (zero-copy read, strongly typed)
- Loro CRDT binary for the op log

The full format specification is in SPEC.md §4. That section is the normative reference; this ADR records the reasoning for the major encoding choices.

**Why OpenEXR for tiles, not PNG or raw:**  
EXR is the VFX industry standard for HDR pixel data. It provides f16/f32 precision natively, has multiple internal compression codecs (ZIP, PIZ, DWAB) avoiding the need for double-compression inside the ZIP container, supports arbitrary named channels (enabling CMYK and multichannel without format changes), and produces files directly readable by Blender, Nuke, and DaVinci Resolve.

**Why FlatBuffers for paths, not JSON or MessagePack:**  
FlatBuffers provides zero-copy read access (the `PathStore` can be memory-mapped without deserialisation), has a stable binary schema with versioning built in (`PathStore.version`), and generates strongly typed Rust code from the `.fbs` schema. JSON paths would be human-readable but prohibitively slow for large vector documents.

**Why OPC/ZIP, not a custom binary container:**  
OPC is a well-specified ECMA standard. The container is inspectable with any ZIP tool. The `appthere-opc` crate is shared with Loki, reducing the surface area of custom infrastructure. OPC's content type and relationship model gives us typed part references without inventing our own addressing scheme.

## Consequences

- `iris-aif` depends on `appthere-opc`; it must not begin implementation until `appthere-opc` is published.
- EXR tiles must use `Stored` ZIP compression (no double-compression). This is enforced in `iris-aif`'s write path and validated on read.
- The FlatBuffers `.fbs` schema (`iris_paths.fbs`) is versioned (`PathStore.version`). Changes to path encoding require bumping this version, not the AIF format version.
