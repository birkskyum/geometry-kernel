//! Rust-first geometry kernel focused on deterministic geometry operations.

pub mod buffer;
pub mod canonicalize;
pub mod error;
#[cfg(feature = "geos-reference")]
pub mod geos_reference;
pub mod kernel;
pub mod noding;
pub mod overlay;
pub mod polygonize;
pub mod precision;
pub mod predicates;
pub mod pure_rust;
pub mod types;
#[cfg(feature = "wasm")]
pub mod wasm;

pub use error::{GeometryError, Result};
pub use kernel::{GeometryKernel, KernelBackend};
pub use precision::PrecisionModel;
pub use pure_rust::PureRustKernel;
pub use types::{Coord, LineString, LinearRing, MultiPolygon, Polygon};

#[cfg(feature = "geos-reference")]
pub use geos_reference::GeosReferenceKernel;
