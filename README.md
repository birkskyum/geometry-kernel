# Geometry Kernel

Author: Birk Skyum.

`geometry-kernel` is a Rust-first geometry project focused on deterministic,
GEOS/JTS-compatible model operations. The long-term goal is a browser-portable
kernel that can replace heavyweight GEOS-WASM usage for application-specific
geometry flows without changing model behavior.

This is deliberately not a general GIS kitchen sink. The public API is scoped
to the operations needed by layout and modelling engines:

- polygon buffering
- line buffering
- polygon intersection and difference
- line/polygon intersection points
- polygon area and largest-polygon selection
- canonicalization, snapping, and precision handling

## Current State

The repository is structured around a compatibility contract:

- `PureRustKernel` is the long-term backend under active development.
- `GeosReferenceKernel` is an optional native reference backend enabled with
  `--features geos-reference`.
- Fixture tests define expected semantics before new algorithms are trusted.

The first pure-Rust implementation is intentionally conservative. It provides
working primitives, canonicalization, noding, simple polygonization helpers,
and MVP buffer/overlay paths for simple geometries. It should not be promoted
as a full GEOS replacement until the parity suite says so.

## Commands

```bash
cargo test
cargo test --features geos-reference
cargo check --no-default-features --features wasm --target wasm32-unknown-unknown
```

## Design Rule

GEOS compatibility is measured by behavior, not by implementation style. A
backend is eligible only when it preserves fixture-level model outcomes:

- exact row counts
- exact tree counts
- exact strip counts
- exact species counts
- area differences inside explicit tolerances
- deterministic canonical output
