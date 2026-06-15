# geometry-kernel

`geometry-kernel` is a Rust-first geometry engine for deterministic planar
geometry operations. It is designed for applications where small differences in
buffering, overlay, or intersection behavior can materially change downstream
results.

The crate is intentionally scoped. It is not a full GIS toolkit and it is not a
Rust port of GEOS. The goal is a compact, portable kernel with clear behavior
for a focused set of geometry operations.

## Status

This is a `1.0.0` package with a focused public API. The API is intended to be
stable for core geometry entrypoints, while the implementation can continue
improving under the compatibility contract.

Currently supported areas:

- planar geometry primitives
- polygon area
- largest-polygon selection
- polygon and line buffering
- polygon intersection and difference
- line/polygon intersection points
- canonicalization and precision handling
- a WebAssembly build for browser use

The pure Rust kernel is the default path. An optional GEOS reference backend is
available for tests and compatibility work.

## Install

Rust:

```bash
cargo add geometry-kernel
```

JavaScript / WebAssembly:

```bash
npm install geometry-kernel
```

## Rust Usage

```rust
use geometry_kernel::{
    GeometryKernel, LinearRing, Polygon, PrecisionModel, PureRustKernel,
};

fn main() -> geometry_kernel::Result<()> {
    let polygon = Polygon::new(
        LinearRing::new(vec![
            [0.0, 0.0].into(),
            [10.0, 0.0].into(),
            [10.0, 10.0].into(),
            [0.0, 10.0].into(),
        ]),
        vec![],
    );

    let kernel = PureRustKernel::new(PrecisionModel::floating());
    let area = kernel.polygon_area(&polygon)?;

    assert_eq!(area, 100.0);
    Ok(())
}
```

## JavaScript / WASM Usage

The npm package exposes the WebAssembly build generated from the Rust crate.
Initialize the module once, then call the exported JSON APIs.

```js
import init, { polygon_area_json } from "geometry-kernel";

await init();

const polygon = {
  exterior: {
    coords: [
      { x: 0, y: 0 },
      { x: 10, y: 0 },
      { x: 10, y: 10 },
      { x: 0, y: 10 },
      { x: 0, y: 0 },
    ],
  },
  holes: [],
};

const response = JSON.parse(polygon_area_json(JSON.stringify(polygon)));

if (!response.ok) {
  throw new Error(response.error);
}

console.log(response.value);
```

## Feature Flags

| Feature | Purpose |
| --- | --- |
| `pure-rust` | Default pure Rust kernel path. |
| `geos-reference` | Optional GEOS reference backend for tests and comparison. Requires GEOS. |
| `wasm` | WebAssembly bindings for core geometry operations. |

## Development

```bash
cargo test
cargo test --features geos-reference
cargo test --features wasm
cargo check --no-default-features --features wasm --target wasm32-unknown-unknown
npm run build:wasm
```

## Compatibility

`geometry-kernel` aims for deterministic behavior on the operations it
supports. Compatibility is measured by operation tests and reference comparisons,
not by claiming to match every behavior of a larger geometry library.

The package keeps a GEOS reference backend available for comparison, but the
portable path is pure Rust and WebAssembly-compatible.

Applications with domain-specific output requirements should validate those
requirements in their own repositories. This package intentionally does not
include application-specific data or acceptance criteria.

## Comparison To Other Libraries

| Library | Best fit | How `geometry-kernel` differs |
| --- | --- | --- |
| GEOS / JTS | Mature, broad geometry operations with well-known topology behavior. | `geometry-kernel` is narrower, Rust-first, and designed to run natively or in WebAssembly without shipping GEOS. |
| Turf / JSTS | Browser JavaScript GIS utilities and GeoJSON workflows. | `geometry-kernel` exposes a compact Rust/WASM API for deterministic geometry operations instead of a broad JS toolbox. |
| Rust `geo` | General-purpose Rust geospatial primitives and algorithms. | `geometry-kernel` is focused on a stricter compatibility contract for buffer, overlay, and deterministic output. |
| GEOS-WASM | GEOS semantics in the browser. | `geometry-kernel` avoids the larger GEOS-WASM payload on the portable path, while keeping a GEOS reference backend for tests. |
| Clipper-style kernels | Polygon clipping and offsetting with a compact algorithm family. | `geometry-kernel` treats semantic compatibility as the promotion gate, not just local polygon similarity. |

For more detail, see [docs/geometry-engine-comparison.md](docs/geometry-engine-comparison.md).

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT license

at your option.

Copyright (c) Birk Skyum.
