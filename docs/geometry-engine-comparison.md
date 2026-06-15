# Geometry Engine Comparison

Author: Birk Skyum. Last updated: 2026-06-15.

This note explains why `geometry-kernel` exists and how it compares with the
geometry libraries considered before publishing the standalone package.

The package is not trying to replace every GIS library. It targets deterministic
planar geometry operations where small differences in buffering, overlay, or
intersection behavior can materially change downstream application results.

## Current Package State

`geometry-kernel` currently ships:

- a pure Rust kernel as the default crate path
- a browser WebAssembly build for the npm package
- an optional native GEOS reference backend behind `geos-reference`

The portable path does not depend on GEOS-WASM, Turf, JSTS, or Rust `geo`.
GEOS is retained only as an optional native reference backend for tests and
compatibility work.

## Decision Summary

The package takes the compatibility-contract approach:

1. Keep the public API focused on a small set of geometry operations.
2. Keep the portable implementation pure Rust and WASM-compatible.
3. Keep GEOS as a native reference backend, not as a runtime dependency for the
   browser package.
4. Promote behavior by operation-level compatibility, not by visual similarity
   alone.

Application-specific validation belongs in downstream application repositories,
not in this public package.

## Why GEOS/JTS-Like Semantics Matter

Turf is not one single geometry engine. In Turf 7.3.5, important operations
come from different libraries:

- `@turf/buffer` uses `@turf/jsts` `BufferOp`.
- `@turf/difference` uses `polyclip-ts`.

JSTS is a JavaScript port of JTS, and GEOS follows the same JTS algorithm
family. That is why Turf buffer and GEOS tend to agree on offset behavior.

Rust `geo` is a separate geometry implementation. It is useful and WASM-safe,
but its buffer/offset semantics are not a drop-in match for Turf/JSTS/GEOS.
That difference matters for applications that rely on stable offset behavior.

## Compatibility Table

| Option | Runtime | Algorithm family | Browser/WASM fit | Bundle impact | Verdict |
| --- | --- | --- | --- | --- | --- |
| `geometry-kernel` | Native Rust and browser WASM | Focused pure Rust implementation with GEOS/JTS-style compatibility targets | Current browser package | Compact WASM package; exact size should be checked during release | Current package path for focused geometry operations |
| GEOS / JTS | Native C/C++ or JVM | Mature JTS topology family | Not browser portable directly | No browser bundle unless bridged | Best reference implementation, not the portable package path |
| Turf 7.3.5 | Browser JavaScript | Mixed: JSTS for buffer, polyclip-ts for difference | Excellent | Depends on selected Turf imports | Good JS baseline, broader and less focused than this package |
| GEOS-WASM | Browser WASM/JS bridge | GEOS/JTS family | Works | Larger browser payload because it ships a compiled GEOS runtime | Correct semantics, but too heavy for the package default |
| Rust `geo` | Native Rust and WASM | Independent Rust geometry algorithms | Excellent | Small and idiomatic Rust dependency | Useful library, but not a semantic drop-in for GEOS/JTS buffering |
| Clipper-style offsetting | Native Rust and WASM | Clipper polygon clipping and offsetting | Good | Potentially compact | Useful family, but different offset semantics from GEOS/JTS buffering |
| `wbtopology` | Pure Rust | JTS-inspired topology suite | Promising but should be evaluated per operation | Unknown | Interesting long-term candidate, not the current package kernel |
| `geo-buffer` | Pure Rust | Straight-skeleton polygon buffering | Likely WASM-safe | Unknown | Different semantics from GEOS/JTS buffer offset curves |

## Package Size

The npm package consists of a small JavaScript wrapper, TypeScript declarations,
README/docs, license files, and a generated WASM binary. Check size during
release with:

```bash
npm run build:wasm
npm pack --dry-run --json
gzip -c pkg/geometry_kernel_bg.wasm | wc -c
```

The portable package path should not include GEOS-WASM, Turf, JSTS, or Rust
`geo`.

## Why Not Use GEOS-WASM For Everything

GEOS-WASM gives good semantics, but it is not the ideal default for this
package:

- it adds a larger browser payload
- a JSON bridge creates serialization overhead
- a handle-based bridge would need explicit geometry lifetimes and more API
  complexity
- the package would still be tied to a compiled GEOS runtime instead of a
  compact Rust/WASM kernel

GEOS remains valuable as a reference implementation. It is just not the default
runtime for the portable package.

## Recommendation

Use `geometry-kernel` when the application needs a compact Rust/WASM geometry
backend with a narrow compatibility contract.

Use GEOS or JTS when the application needs a broad, mature topology engine and
native deployment is acceptable.

Use Turf when the application is already JavaScript/GeoJSON-centric and broad
browser GIS utilities matter more than a focused Rust/WASM kernel.

Do not substitute another geometry engine for this package unless the candidate
matches the operation contract and passes the downstream application's own
validation criteria.

## References

- Turf releases: https://github.com/Turfjs/turf/releases
- Turf buffer package: https://www.npmjs.com/package/@turf/buffer
- Turf JSTS package: https://www.npmjs.com/package/@turf/jsts
- GEOS-WASM: https://www.npmjs.com/package/geos-wasm
- Clipper2 Rust docs: https://docs.rs/clipper2-rust/latest/clipper2_rust/
- wbtopology docs: https://docs.rs/wbtopology/latest/wbtopology/
- geo-buffer docs: https://docs.rs/geo-buffer/latest/geo_buffer/
- geo-polygonize-core docs: https://docs.rs/geo-polygonize-core/latest/geo_polygonize_core/
