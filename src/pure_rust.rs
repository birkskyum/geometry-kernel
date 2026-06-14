use crate::buffer::{self, BufferOptions};
use crate::canonicalize::canonicalize_polygon;
use crate::error::Result;
use crate::kernel::{GeometryKernel, KernelBackend};
use crate::overlay;
use crate::precision::PrecisionModel;
use crate::predicates::{polygon_area, segment_intersection, SegmentIntersection};
use crate::types::{Coord, LineString, MultiPolygon, Polygon};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PureRustKernel {
    precision: PrecisionModel,
}

impl PureRustKernel {
    pub const fn new(precision: PrecisionModel) -> Self {
        Self { precision }
    }
}

impl Default for PureRustKernel {
    fn default() -> Self {
        Self::new(PrecisionModel::floating())
    }
}

impl GeometryKernel for PureRustKernel {
    fn backend(&self) -> KernelBackend {
        KernelBackend::PureRust
    }

    fn precision(&self) -> PrecisionModel {
        self.precision
    }

    fn canonicalize_polygon(&self, polygon: &Polygon) -> Polygon {
        canonicalize_polygon(polygon, self.precision)
    }

    fn polygon_area(&self, polygon: &Polygon) -> Result<f64> {
        Ok(polygon_area(polygon))
    }

    fn largest_polygon(&self, multi_polygon: &MultiPolygon) -> Result<Option<Polygon>> {
        Ok(multi_polygon
            .polygons
            .iter()
            .max_by(|left, right| {
                polygon_area(left)
                    .partial_cmp(&polygon_area(right))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned())
    }

    fn buffer_polygon(
        &self,
        polygon: &Polygon,
        distance: f64,
        options: BufferOptions,
    ) -> Result<MultiPolygon> {
        buffer::buffer_polygon(polygon, distance, options, self.precision)
    }

    fn line_buffer(
        &self,
        line: &LineString,
        distance: f64,
        options: BufferOptions,
    ) -> Result<MultiPolygon> {
        buffer::line_buffer(line, distance, options, self.precision)
    }

    fn intersection(&self, subject: &MultiPolygon, clip: &MultiPolygon) -> Result<MultiPolygon> {
        overlay::intersection(subject, clip, self.precision)
    }

    fn difference(&self, subject: &MultiPolygon, clip: &MultiPolygon) -> Result<MultiPolygon> {
        overlay::difference(subject, clip, self.precision)
    }

    fn line_polygon_intersections(
        &self,
        line: &LineString,
        polygon: &Polygon,
    ) -> Result<Vec<Coord>> {
        let mut points = Vec::new();

        for (line_start, line_end) in line.segments() {
            for ring in std::iter::once(&polygon.exterior).chain(polygon.holes.iter()) {
                for (ring_start, ring_end) in ring.segments() {
                    match segment_intersection(
                        line_start,
                        line_end,
                        ring_start,
                        ring_end,
                        self.precision,
                    ) {
                        Some(SegmentIntersection::Point(point)) => {
                            push_unique(&mut points, point, self.precision);
                        }
                        Some(SegmentIntersection::Overlap(start, end)) => {
                            push_unique(&mut points, start, self.precision);
                            push_unique(&mut points, end, self.precision);
                        }
                        None => {}
                    }
                }
            }
        }

        Ok(points)
    }
}

fn push_unique(points: &mut Vec<Coord>, point: Coord, precision: PrecisionModel) {
    let point = precision.snap_coord(point);
    if !points
        .iter()
        .any(|existing| precision.same_coord(*existing, point))
    {
        points.push(point);
    }
}
