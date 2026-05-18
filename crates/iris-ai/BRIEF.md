## `iris-ai` — Adobe Illustrator adapter

**Gate:** `iris-vector` milestone complete (Phase 3).

### Phase 1 milestone (pre-gate stub only)

No implementation in Phase 1. Crate exists as stub.

### Phase 3 milestone

Read modern AI files (PDF-based, CS/CC era). Write as SVG + AI namespace extensions.
This is the highest-risk adapter — allocate extra audit time.
Every quirk path requires `// COMPAT(adobe):` annotation and a fixture test.
