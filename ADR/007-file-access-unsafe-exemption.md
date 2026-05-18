# ADR 007 — loki-file-access unsafe code exemption

**Status:** Accepted  
**Date:** 2026-05-18  
**Deciders:** AppThere core team

## Context

The iris workspace coding rules (CLAUDE.md) require `#![forbid(unsafe_code)]`
in all library crates. `loki-file-access` is an external MIT-licensed dependency
(path dep at `crates/loki-file-access`) that contains `unsafe` code in its
Android JNI and iOS Objective-C bridge layers.

## Decision

`loki-file-access` is exempt from the `#![forbid(unsafe_code)]` rule because:

1. **It is not a workspace member.** `CLAUDE.md`'s rule applies to crates
   under `iris/crates/` that are workspace members. `loki-file-access` is
   listed under `workspace.exclude` and is consumed as an external path dep.
   The rule does not bind externally authored dependencies.

2. **The unsafe surface is narrow.** All `unsafe` is confined to platform FFI
   boundaries: JNI calls into the Android runtime and Objective-C message
   sends into UIKit. There is no unsafe Rust memory manipulation or pointer
   arithmetic on Iris-owned data.

3. **Every unsafe block is annotated.** `// SAFETY:` comments were added to all
   existing unsafe blocks in the Android JNI layer as part of this session
   (PROMPT 2D Phase A7). New unsafe blocks added in the future must carry
   `// SAFETY:` comments as a condition of this exemption.

## Future direction

A `loki-file-access-sys` crate split — isolating all unsafe into a `-sys`
crate with a safe wrapper above — is desirable but deferred. Conditions for
opening a new ADR to govern the split:

- iOS `UIDocumentPickerViewController` implementation is complete and tested.
- Android multi-file support (`EXTRA_ALLOW_MULTIPLE` + `ClipData`) is complete.
- A clear stable API boundary between the safe wrapper and the unsafe `-sys`
  layer has been established.

## Consequences

- No source-level changes are required in iris workspace files.
- `// SAFETY:` comments in `loki-file-access` are maintained as part of
  normal upkeep whenever unsafe blocks are added or modified.
- Any new unsafe block added to `loki-file-access` without a `// SAFETY:`
  comment is a violation of this exemption and must be corrected before merge.
