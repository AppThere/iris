## `iris-psd` — Photoshop PSD adapter

**Gate:** `iris-aif` milestone complete.

### Phase 1 milestone

Read a PSD file containing flat pixel layers (no groups, no adjustments) and produce
an `AifDocument`. Write is not in scope for Phase 1.

### Public API at milestone completion

```rust
pub use reader::{PsdReader, PsdError};

pub struct PsdReader;
impl PsdReader {
    pub fn read(path: &std::path::Path) -> Result<iris_aif::AifDocument, PsdError>;
    pub fn from_reader<R: std::io::Read>(r: R) -> Result<iris_aif::AifDocument, PsdError>;
}

pub enum PsdError {
    #[error("not a PSD file (bad signature)")]
    BadSignature,
    #[error("unsupported PSD version {0} (expected 1 or 2)")]
    UnsupportedVersion(u16),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    // ... additional variants per COMPAT(adobe) findings during audit
}
```

### Do not implement yet

- PSD write (Phase 2)
- Groups, adjustment layers, smart objects (Phase 2)
- PSB (large document) support (Phase 2)
- Layer effects and masks (Phase 4)

### Test requirements

- Round-trip: read a known PSD → layer count, names, dimensions match expected
- `BadSignature` triggered by a file with wrong magic bytes
- `// COMPAT(adobe):` annotation on every quirk path, with a test that exercises it
