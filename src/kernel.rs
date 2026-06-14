use crate::buffer::BufferOptions;
use crate::error::Result;
use crate::precision::PrecisionModel;
use crate::types::{Coord, LineString, MultiPolygon, Polygon};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelBackend {
    PureRust,
    GeosReference,
}

pub trait GeometryKernel {
    fn backend(&self) -> KernelBackend;
    fn precision(&self) -> PrecisionModel;
    fn canonicalize_polygon(&self, polygon: &Polygon) -> Polygon;
    fn polygon_area(&self, polygon: &Polygon) -> Result<f64>;
    fn largest_polygon(&self, multi_polygon: &MultiPolygon) -> Result<Option<Polygon>>;
    fn buffer_polygon(
        &self,
        polygon: &Polygon,
        distance: f64,
        options: BufferOptions,
    ) -> Result<MultiPolygon>;
    fn line_buffer(
        &self,
        line: &LineString,
        distance: f64,
        options: BufferOptions,
    ) -> Result<MultiPolygon>;
    fn intersection(&self, subject: &MultiPolygon, clip: &MultiPolygon) -> Result<MultiPolygon>;
    fn difference(&self, subject: &MultiPolygon, clip: &MultiPolygon) -> Result<MultiPolygon>;
    fn line_polygon_intersections(
        &self,
        line: &LineString,
        polygon: &Polygon,
    ) -> Result<Vec<Coord>>;
}
