## `iris-plugin-api` — WASI plugin sandbox

**Gate:** Phase 5 only.

### Phase 1 milestone (pre-gate stub only)

No implementation. Crate defines the trait surface only:

```rust
pub trait IrisPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
}
```

Everything else is a stub with `// TODO(iris): SPEC.md §9 — Phase 5`.
