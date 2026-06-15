# Implementation Plan

Author: Birk Skyum. Last updated: 2026-06-15.

This project is a long-term effort to build a Rust-native geometry kernel with
GEOS/JTS-compatible behavior for a focused geometry API. The plan is staged so
the repository stays useful before the hardest algorithms are complete.

## Goal

Build a standalone Rust-first geometry package that can reduce browser
GEOS-WASM usage while preserving documented geometry behavior.

The intended end state:

```text
One geometry contract.
One stable kernel API.
Multiple reference paths while developing.
Pure Rust backend promoted only after operation parity.
```

## Non-Goals

- Do not build a full GIS library.
- Do not expose every GEOS operation.
- Do not include application-specific logic or validation data.
- Do not optimize for visual similarity alone.
- Do not promote a smaller backend if it changes documented geometry behavior.

## Public API Scope

The package should expose focused geometry operations, not broad GIS internals:

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

Inputs should be planar coordinates. Projection and WGS84 geodesic concerns
should stay outside the kernel.

## Phase 1: Contract Before Engine

Define the compatibility contract before optimizing algorithms.

Deliverables:

- operation case format
- tolerance policy
- canonical output policy
- parity test harness
- docs explaining promotion criteria

Acceptance criteria:

- tests can compare a candidate backend against a reference backend
- failures clearly identify operation, case, metric, and tolerance
- operation-level output is canonical and deterministic

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

- overlapping and crossing segment cases pass
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

- polygonization cases match the GEOS reference within tolerance
- holes are assigned to the correct shells
- output is stable across input line ordering

## Phase 6: Overlay

Implement the required overlay operations.

Deliverables:

- polygon intersection
- polygon difference
- optional union if useful for buffer cleanup
- face labeling
- precision-aware overlay variant

Acceptance criteria:

- operation cases match GEOS reference topology and area tolerances
- unsupported cases fail with typed errors instead of incorrect geometry

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

- buffer operation cases match GEOS reference within tolerance
- collapsed and near-degenerate inputs are handled deterministically
- output topology is canonicalized

## Phase 8: GEOS Reference Backend

Add a native-only reference backend for test generation and parity comparison.

Deliverables:

- `GeosReferenceKernel`
- WKT/WKB conversion helpers
- case generation command or test helper
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

## Phase 10: Downstream Validation

Use the package next to downstream application code without moving
application-specific logic or data into this repository.

Deliverables:

- clear extension points for application adapters
- deterministic operation outputs
- diagnostics that make compatibility gaps debuggable

Acceptance criteria:

- this repository remains application-neutral
- downstream validation can happen outside the public package
- no default behavior changes until public operation contracts pass

## Suggested Commit Milestones

1. Project scaffold and docs.
2. Geometry types, errors, precision model.
3. Canonicalization, area, predicates.
4. Segment intersection and noding.
5. Polygonization.
6. Overlay MVP.
7. Buffer MVP.
8. GEOS reference backend.
9. WASM bindings.
10. Downstream adapter examples that contain no application-specific data.

## Risk Register

| Risk | Why it matters | Mitigation |
| --- | --- | --- |
| Buffer semantics drift | Tiny boundary differences can materially affect downstream outputs | Keep GEOS reference cases and strict operation contracts |
| Numeric robustness | Overlay and noding can fail on near-collinear inputs | Use robust predicates and explicit precision models |
| Scope creep | A full GEOS clone is too large | Keep public API limited to focused geometry operations |
| Application leakage | Domain-specific logic may not belong in a public package | Keep application adapters and validation data outside this repository |
| WASM payload grows again | Long-term goal includes payload reduction | Track package size during release |
