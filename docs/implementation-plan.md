# Implementation Plan

Author: Birk Skyum. Last updated: 2026-06-14.

This project is a long-term effort to build a Rust-native geometry kernel with
GEOS/JTS-compatible behavior for a focused modelling API. The plan is staged so
the repository becomes useful before the hardest algorithms are complete.

## Goal

Build a standalone Rust-first geometry package that can eventually replace
heavy browser GEOS-WASM usage while preserving model behavior.

The intended end state:

```text
One geometry contract.
One stable kernel API.
Multiple backends while developing.
Pure Rust backend promoted only after fixture parity.
```

## Non-Goals

- Do not build a full GIS library.
- Do not expose every GEOS operation.
- Do not optimize for visual similarity alone.
- Do not promote a smaller backend if it changes downstream model output.

## Package Shape

```text
geometry-kernel/
  Cargo.toml
  package.json
  README.md
  docs/
    compatibility-contract.md
    geometry-engine-comparison.md
    implementation-plan.md
  src/
    lib.rs
    error.rs
    types.rs
    precision.rs
    predicates.rs
    canonicalize.rs
    noding.rs
    polygonize.rs
    overlay.rs
    buffer.rs
    kernel.rs
    pure_rust.rs
    geos_reference.rs
    wasm.rs
  tests/
    fixtures/
    parity/
```

## Public API Scope

The package should expose model-level operations, not low-level internals:

```text
polygon_area
largest_polygon
canonicalize_polygon
buffer_polygon
line_buffer
intersection
difference
line_polygon_intersections
```

Inputs should be planar meter coordinates. Projection and WGS84 geodesic
concerns should stay outside the kernel.

## Phase 1: Contract Before Engine

Define the compatibility contract before optimizing algorithms.

Deliverables:

- operation fixture format
- model-level fixture format
- tolerance policy
- canonical output policy
- parity test harness
- docs explaining promotion criteria

Acceptance criteria:

- fixture tests can compare a candidate backend against a reference backend
- test failures clearly identify operation, fixture, metric, and tolerance
- model-level counts are exact, not tolerance-based

## Phase 2: Repository And API Scaffold

Create a standalone repository with neutral project identity and local git
history.

Deliverables:

- Rust crate
- package metadata for optional JS/WASM distribution
- core geometry types
- error types
- `GeometryKernel` trait
- `PureRustKernel` implementation skeleton
- feature flags:
  - `pure-rust`
  - `geos-reference`
  - `wasm`

Acceptance criteria:

- `cargo test` passes
- `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown` passes
- the public API is documented

## Phase 3: Robust Primitives

Implement deterministic primitive geometry operations.

Deliverables:

- coordinate, line, ring, polygon, multi-polygon types
- signed and unsigned area
- ring orientation
- ring closure and duplicate-point cleanup
- bounding boxes
- robust orientation predicate
- segment intersection
- point-on-segment and point-in-ring
- precision model with grid snapping

Acceptance criteria:

- unit tests for degenerate and near-degenerate cases
- deterministic canonical output for equivalent rings
- no panics on empty or malformed input; return typed errors instead

## Phase 4: Noding

Implement segment noding: split linework at all intersections.

Deliverables:

- segment collection
- sweep or indexed pair detection
- exact split point insertion
- deduplication under precision model
- deterministic output ordering

Acceptance criteria:

- overlapping and crossing segment fixtures pass
- output linework has no un-noded intersections
- repeated points and collinear overlaps are handled intentionally

## Phase 5: Polygonization

Build polygons from noded linework.

Deliverables:

- planar graph construction
- directed edge traversal
- ring extraction
- shell/hole classification
- invalid ring rejection
- canonical polygon ordering

Acceptance criteria:

- polygonization fixtures match the GEOS reference within tolerance
- holes are assigned to the correct shells
- output is stable across input line ordering

## Phase 6: Overlay

Implement the operations needed by the model.

Deliverables:

- polygon intersection
- polygon difference
- optional union if useful for buffer cleanup
- face labeling
- precision-aware overlay variant

Acceptance criteria:

- operation fixtures match GEOS reference topology and area tolerances
- model fixture counts do not regress when overlay is used in shadow mode

## Phase 7: Buffer

Implement GEOS/JTS-compatible-enough buffering.

Deliverables:

- offset curve generation
- round, mitre, and bevel joins
- line end caps
- polygon exterior inward/outward handling
- hole handling
- collapsed polygon handling
- split erosion components
- offset-curve noding
- polygonization of buffer curves
- face/depth selection

Acceptance criteria:

- buffer operation fixtures match GEOS reference within tolerance
- 85-fixture model suite has zero count regressions
- known sensitive cases, including the inward-margin fixture, pass exactly on
  row/tree/strip/species counts

## Phase 8: GEOS Reference Backend

Add a native-only reference backend for test generation and parity comparison.

Deliverables:

- `GeosReferenceKernel`
- WKT/WKB conversion helpers
- fixture generation command or test helper
- side-by-side pure Rust vs GEOS tests

Acceptance criteria:

- `cargo test --features geos-reference` passes
- reference output can be regenerated intentionally
- reference backend is never required for WASM builds

## Phase 9: WASM Bindings

Expose the stable API to browser clients.

Deliverables:

- `wasm-bindgen` module
- JSON convenience API for early usage
- later typed-array or WKB API for large geometries
- panic hook and structured errors

Acceptance criteria:

- `wasm32-unknown-unknown` check passes
- generated WASM package avoids GEOS dependencies
- API roundtrip tests pass in Node or browser test harness

## Phase 10: Shadow Integration

Use the package next to an existing production backend without changing output.

Deliverables:

- shadow-run adapter
- comparison logs
- fixture import/export scripts
- dashboard or report for parity gaps

Acceptance criteria:

- candidate backend can be run on real fixtures without affecting behavior
- parity failures produce actionable diagnostics
- no default behavior changes until promotion criteria are met

## Phase 11: Promotion

Promote the pure-Rust backend only after it proves itself.

Required promotion gates:

- all operation fixtures pass
- all model fixtures pass
- zero row count diffs
- zero tree count diffs
- zero strip count diffs
- zero species count diffs
- area tolerances accepted and documented
- WASM build passes
- browser payload is meaningfully smaller than the GEOS-WASM path

## Suggested Commit Milestones

1. Project scaffold and docs.
2. Geometry types, errors, precision model.
3. Canonicalization, area, predicates.
4. Segment intersection and noding.
5. Polygonization.
6. Overlay MVP.
7. Buffer MVP.
8. GEOS reference backend.
9. Fixture parity harness.
10. WASM bindings.
11. Shadow integration example.

## Risk Register

| Risk | Why it matters | Mitigation |
| --- | --- | --- |
| Buffer semantics drift | Tiny boundary differences can change model counts | Keep GEOS reference fixtures and model-level count gates |
| Numeric robustness | Overlay and noding can fail on near-collinear inputs | Use robust predicates and explicit precision models |
| Scope creep | A full GEOS clone is too large | Keep public API limited to model operations |
| Premature integration | Incomplete backend could destabilize apps | Shadow mode only until promotion gates pass |
| WASM payload grows again | Long-term goal includes payload reduction | Track size in CI once WASM package exists |

## First Implementation Slice

The first useful slice should include:

- project scaffold
- docs in this directory
- core types
- precision/canonicalization
- area and predicates
- MVP pure-Rust kernel trait implementation
- GEOS reference feature stub or implementation
- initial fixtures for buffer-sensitive polygons
- tests and initial git commit

That creates a stable base for the harder algorithms without tying the project
to any one application repository.
