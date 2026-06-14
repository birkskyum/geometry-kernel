# Geometry Engine Comparison

Author: Birk Skyum. Last updated: 2026-06-14.

This note captures the research that led to starting this standalone geometry
kernel. The motivating problem was a browser-portable geometry backend for a
system-design model where small polygon-buffer differences can change row
lengths, tree counts, strip counts, and area totals.

## Decision Summary

The model needs browser-side geometry that preserves native GEOS/JTS-style
results closely enough that downstream model output does not change.

The best short-term implementation found during research was a hybrid:

- Rust/WASM owns the layout algorithm, projection helpers, row generation,
  line/segment logic, and most local geometry plumbing.
- GEOS-WASM is used only for the parity-critical inward margin buffer.
- The TypeScript fallback remains Turf-based.

That hybrid is correct enough, but it carries a large lazy-loaded browser
payload. This repository exists to build the long-term alternative: a
Rust-native geometry kernel with GEOS/JTS-compatible semantics for the
operations the model actually needs.

## Why GEOS-Like Semantics Matter

Turf is not one single geometry engine. In Turf 7.3.5, important operations
come from different libraries:

- `@turf/buffer` uses `@turf/jsts` `BufferOp`.
- `@turf/difference` uses `polyclip-ts`.

JSTS is a JavaScript port of JTS, and GEOS follows the same JTS algorithm
family. That is why Turf buffer and GEOS tend to agree on offset behavior.

Rust `geo` is a separate geometry implementation. It is useful and WASM-safe,
but its buffer/offset semantics are not a drop-in match for Turf/JSTS/GEOS.
Small polygon-offset differences are enough to change row lengths and tree
counts in the motivating model.

## Compatibility Table

| Option | Runtime | Algorithm family | Browser/WASM fit | Bundle impact | Relevant operations | Fixture result | Verdict | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TypeScript + Turf 7.3.5 | Browser JS | Mixed: JSTS for buffer, polyclip-ts for difference | Existing browser fallback | Already in app bundle through Turf imports | Buffer, difference, intersections, areas, line helpers | Existing baseline | Good baseline, slower than Rust/WASM | Turf buffer aligns with GEOS because it uses JSTS `BufferOp`. |
| Native Rust + GEOS | Native server / desktop native | GEOS/JTS family | Not browser portable directly | No browser impact | Buffer, difference, polygon extraction | Native reference | Reference implementation | This is the behavior to match. |
| Rust/WASM + `geo` only | Browser WASM | Independent Rust geometry algorithms | Excellent | Smallest Rust/WASM-only design | Buffer, boolean ops, intersections, areas | Regressed on offset-sensitive fixtures | Not acceptable for model parity | Useful for plumbing and cheap local operations, but inward buffer behavior differs from GEOS/Turf. |
| Rust/WASM + GEOS-WASM only for margin buffer | Browser WASM + JS bridge | GEOS/JTS family for margin buffer | Works | Adds lazy GEOS-WASM payload | Margin buffer via GEOS-WASM; rest in Rust/`geo` | Count-safe across 85 fixtures; earlier acceptance run reached exact model metrics | Correct short-term compromise | Keeps GEOS-WASM off the main path and uses it only where parity requires it. |
| Rust/WASM + GEOS-WASM for all geometry | Browser WASM + JS bridge | GEOS/JTS family | Possible, but awkward with current bridge | Heavy | Could cover buffer, difference, intersection, area | Not implemented | Possible but not ideal | A JSON bridge would add serialization overhead and complexity for every geometry call. |
| Custom trimmed GEOS-WASM build | Browser WASM/JS | GEOS/JTS family | Likely viable | Potentially much smaller than package GEOS-WASM | Export only required operations | Not implemented | Best short-term bundle-size path | Keeps proven semantics while reducing unused GEOS API/runtime surface. |
| `clipper2-rust` | Pure Rust/WASM | Clipper2 polygon clipping and offsetting | Good | Small; experimental build removed GEOS-WASM chunk | Polygon offsetting, booleans | `Square` join changed counts in 9 fixtures; `Round` join changed 1 tree in `robinia-alley-croppping` | Not acceptable as drop-in replacement | Promising for bundle size, but not GEOS/JTS-compatible enough for this model. |
| `wbtopology` 0.2.0 | Pure Rust | JTS-inspired topology suite | Unknown for this app; crate is pure Rust but not proven here | Unknown | Buffer APIs, overlays, topology predicates | Returned empty output for the real inward-margin buffer fixture | Not acceptable today | Interesting long-term, but too immature for this regression-sensitive path. |
| `geo-buffer` | Pure Rust | Straight-skeleton polygon buffering | Likely WASM-safe | Unknown | Polygon inflate/deflate | Not fully integrated; algorithm family does not target GEOS/JTS parity | Not a priority | Straight-skeleton buffering is a different semantic model from GEOS/JTS buffer offset curves. |
| `geo-polygonize-core` | Pure Rust | JTS/GEOS-style polygonization only | Likely WASM-safe | Unknown | Polygonization from lines | Not applicable | Not a buffer replacement | Useful category, but it does not solve polygon buffering by itself. |

## Measured Bundle Impact

Measured during the initial browser-WASM spike.

| Build shape | Added/generated artifact | Raw size | Gzip size | Notes |
| --- | ---: | ---: | ---: | --- |
| Rust/WASM layout package with GEOS-WASM bridge | generated WASM package | about 676 KB package dir | n/a | Rust WASM artifact itself is modest. |
| Rust WASM binary | `gis_rs_bg.wasm` | about 644 KB | about 243-245 KB | Rust model code. |
| GEOS-WASM package chunk in flagged frontend build | generated JS chunk | about 2.58 MB | about 779-794 KB | Mostly `geos-wasm`, including embedded GEOS runtime and wrappers. |
| Total extra lazy-loaded payload when enabled | Rust WASM + GEOS-WASM chunk | about 3.2 MB raw | about 1.0 MB gzip | Only loaded on the WASM layout path when feature-flagged. |
| Experimental Clipper2 build | Rust WASM package | about 720 KB package dir / 684 KB `.wasm` | not measured in final build | Removed the large GEOS-WASM JS chunk, but regressed parity. |

## Fixture Results From The Spike

The important acceptance target is not visual similarity. It is behavioral
stability: row counts, tree counts, strip counts, species counts, and relevant
areas must stay compatible with native GEOS.

| Candidate | Test scope | Result | Outcome |
| --- | --- | --- | --- |
| GEOS-WASM margin buffer | 85 fixture comparison against native Rust/GEOS | 0 failures; no row/tree/strip count diffs; previous acceptance run reached 85/85 exact model metric matches | Keep as reference/short-term solution |
| Clipper2 `Square` join | 85 fixture comparison against native Rust/GEOS | 9 fixtures changed row/tree/strip counts | Reject |
| Clipper2 `Round` join, small arc tolerance | 85 fixture comparison against native Rust/GEOS | 1 fixture changed count: `robinia-alley-croppping` gained one tree | Reject |
| Clipper2 isolated inward-margin buffer | Local-meter inward margin buffer | Closest setting was `Square`, about 0.23 m2 area delta vs GEOS, but full model still regressed elsewhere | Reject as drop-in |
| `wbtopology` isolated inward-margin buffer | Local-meter inward margin buffer | Returned no polygon for the real inward buffer, even with reversed ring orientation | Reject |

## Why Clipper2 Was Not Enough

Clipper2 is a strong pure-Rust/WASM candidate because it supports offsetting
and polygon clipping with a compact payload. The problem is semantic parity:
Clipper2 offsetting is not the same algorithm family as JTS/GEOS buffering.

On the isolated failing margin geometry, Clipper2 could get close by area, but
the full model is sensitive to exact boundary shape. A small shape difference
can move one row endpoint across a spacing threshold and change a tree count.

That makes Clipper2 useful for future display/analysis paths, but not as a
drop-in replacement for the parity-critical layout model.

## Why Not Move Everything To GEOS-WASM

GEOS-WASM could probably do more of the work, but the current bridge shape is
not the right abstraction for a whole model. The spike bridge serialized
through JSON:

```text
Rust geometry -> JSON -> JS -> GEOS-WASM -> JSON -> Rust geometry
```

That is acceptable for a small number of expensive, parity-critical operations.
It is awkward for every segment intersection, area comparison, strip
construction, row clipping, and helper operation.

A cleaner all-GEOS browser design would need a real geometry-handle bridge:
reusable GEOS contexts, opaque geometry handles or WKB/typed-array transfer,
and explicit lifetime management.

## Recommendation

This standalone kernel should pursue a pure-Rust backend, but it must be held
against a GEOS/JTS compatibility contract:

1. Keep a GEOS reference backend for native test generation.
2. Build deterministic pure-Rust primitives, noding, polygonization, overlay,
   and buffer operations behind the same API.
3. Treat fixture-level model stability as the promotion gate.
4. Keep GEOS-WASM or native GEOS as a fallback until the pure-Rust backend
   passes the full suite.

Do not replace a proven GEOS/JTS buffer path with `geo`, Clipper2, or
`wbtopology` unless the candidate passes the full fixture suite without count
regressions.

## References

- Turf releases: https://github.com/Turfjs/turf/releases
- Turf buffer package: https://www.npmjs.com/package/@turf/buffer
- Turf JSTS package: https://www.npmjs.com/package/@turf/jsts
- GEOS-WASM: https://www.npmjs.com/package/geos-wasm
- Clipper2 Rust docs: https://docs.rs/clipper2-rust/latest/clipper2_rust/
- wbtopology docs: https://docs.rs/wbtopology/latest/wbtopology/
- geo-buffer docs: https://docs.rs/geo-buffer/latest/geo_buffer/
- geo-polygonize-core docs: https://docs.rs/geo-polygonize-core/latest/geo_polygonize_core/
