## `iris-ora` — OpenRaster adapter

**Gate:** `iris-aif` milestone complete.

### Phase 1 milestone

Full read and write of ORA files containing pixel layers and groups.
ORA maps cleanly to AIF — no known compatibility traps.

### Public API

```rust
pub use reader::{OraReader, OraError};
pub use writer::{OraWriter};

impl OraReader {
    pub fn read(path: &std::path::Path) -> Result<iris_aif::AifDocument, OraError>;
}
impl OraWriter {
    pub fn write(doc: &iris_aif::AifDocument, path: &std::path::Path) -> Result<(), OraError>;
}
```

### Test requirements

- Round-trip: AIF → ORA → AIF, layer tree structure and tile data match
- Compatibility: read a Krita-exported ORA file (fixture provided in `tests/fixtures/`)
- `// COMPAT(krita):` annotations on any Krita-specific quirks
