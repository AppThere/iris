// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

//! Bounds-checked reader for the FlatBuffers binary wire format.
//!
//! # Why this exists
//!
//! SPEC.md §4.10 specifies `paths.bin` as FlatBuffers. The `flatbuffers` crate's
//! read path (`Table::get`, `Follow::follow`) is `unsafe fn`, and `flatc` is not
//! available in this workspace to generate typed accessors — either route would
//! require `unsafe` blocks in `iris-aif`, which carries `#![forbid(unsafe_code)]`
//! (CLAUDE.md forbids removing it without an ADR).
//!
//! This module therefore decodes the wire format directly with checked slice
//! indexing. Every read is bounds-validated and returns [`WireError`] rather
//! than panicking, which also satisfies SPEC.md §4.1 rule 2 — a reader must fail
//! loudly on malformed input rather than produce corrupt output. Writing still
//! uses the upstream `FlatBufferBuilder`, whose builder API is entirely safe.
//!
//! Layout reference (FlatBuffers internals):
//! - Buffer head: `uoffset` (u32 LE) to the root table; bytes 4..8 hold the
//!   optional file identifier.
//! - Table at `t`: `soffset` (i32 LE) at `t`; `vtable = t - soffset`.
//! - vtable: `u16` vtable byte length, `u16` table byte length, then one `u16`
//!   per field slot. Slot `n` lives at vtable offset `4 + 2n`; a zero voffset
//!   (or a vtable too short to contain the slot) means the field is absent.
//! - Offsets to tables, strings, and vectors are `uoffset`s relative to the
//!   position of the offset itself.
//! - Strings and vectors begin with a `u32` element count.

mod scalar;
mod table;

pub(crate) use table::{root, Table};

/// Structural failure while decoding a FlatBuffers buffer.
#[derive(Debug, thiserror::Error)]
pub(crate) enum WireError {
    /// A read would have run past the end of the buffer.
    #[error("flatbuffers read out of bounds at offset {offset}")]
    OutOfBounds {
        /// Byte offset at which the read was attempted.
        offset: usize,
    },
    /// An offset field pointed outside the buffer or wrapped around.
    #[error("flatbuffers offset at {offset} does not resolve to a valid position")]
    BadOffset {
        /// Byte offset of the offending offset field.
        offset: usize,
    },
    /// The buffer's file identifier is not the one this reader expects.
    #[error("expected file identifier {expected:?}")]
    BadFileIdentifier {
        /// The identifier required by the caller.
        expected: &'static str,
    },
    /// A string field contained bytes that are not valid UTF-8.
    #[error("string field is not valid UTF-8")]
    InvalidUtf8,
}

/// Shorthand for fallible wire reads.
pub(crate) type Result<T> = core::result::Result<T, WireError>;
