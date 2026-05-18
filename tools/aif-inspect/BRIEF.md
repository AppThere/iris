## `tools/aif-inspect` — CLI inspector

**Gate:** `iris-aif` milestone complete.

### Phase 1 milestone

```
aif-inspect <file.aif>
```

Prints to stdout:
- Format version
- Canvas dimensions, DPI, working colour space
- Layer tree (indented, with layer type and name)
- Asset inventory (ICC profiles, brushes, patterns)
- Op log presence / size

Exit code 0 on valid file, 1 on any `AifError`.

No dependencies on `iris-canvas` or `iris-app`.
## `tools/aif-inspect` — CLI inspector

**Gate:** `iris-aif` milestone complete.

### Phase 1 milestone

```
aif-inspect <file.aif>
```

Prints to stdout:
- Format version
- Canvas dimensions, DPI, working colour space
- Layer tree (indented, with layer type and name)
- Asset inventory (ICC profiles, brushes, patterns)
- Op log presence / size

Exit code 0 on valid file, 1 on any `AifError`.

No dependencies on `iris-canvas` or `iris-app`.
