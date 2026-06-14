use geos::{
    CapStyle as GeosCapStyle, CoordSeq, Geom, Geometry, GeometryTypes, JoinStyle as GeosJoinStyle,
};

use crate::buffer::{BufferOptions, CapStyle, JoinStyle};
use crate::canonicalize::canonicalize_polygon;
use crate::error::{GeometryError, Result};
use crate::kernel::{GeometryKernel, KernelBackend};
use crate::precision::PrecisionModel;
use crate::types::{Coord, LineString, LinearRing, MultiPolygon, Polygon};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeosReferenceKernel {
    precision: PrecisionModel,
}

impl GeosReferenceKernel {
    pub const fn new(precision: PrecisionModel) -> Self {
        Self { precision }
    }
}

impl Default for GeosReferenceKernel {
    fn default() -> Self {
        Self::new(PrecisionModel::floating())
    }
}

impl GeometryKernel for GeosReferenceKernel {
    fn backend(&self) -> KernelBackend {
        KernelBackend::GeosReference
    }

    fn precision(&self) -> PrecisionModel {
        self.precision
    }

    fn canonicalize_polygon(&self, polygon: &Polygon) -> Polygon {
        canonicalize_polygon(polygon, self.precision)
    }

    fn polygon_area(&self, polygon: &Polygon) -> Result<f64> {
        polygon_to_geos(polygon)?.area().map_err(geos_error)
    }

    fn largest_polygon(&self, multi_polygon: &MultiPolygon) -> Result<Option<Polygon>> {
        Ok(multi_polygon
            .polygons
            .iter()
            .max_by(|left, right| {
                self.polygon_area(left)
                    .unwrap_or(0.0)
                    .partial_cmp(&self.polygon_area(right).unwrap_or(0.0))
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
        let geometry = polygon_to_geos(polygon)?;
        let buffered = geometry
            .buffer_with_style(
                distance,
                options.quadrant_segments.max(1) as i32,
                geos_cap_style(options.cap_style),
                geos_join_style(options.join_style),
                options.mitre_limit,
            )
            .map_err(geos_error)?;
        geos_to_multi_polygon(&buffered, self.precision)
    }

    fn line_buffer(
        &self,
        line: &LineString,
        distance: f64,
        options: BufferOptions,
    ) -> Result<MultiPolygon> {
        let geometry = line_to_geos(line)?;
        let buffered = geometry
            .buffer_with_style(
                distance,
                options.quadrant_segments.max(1) as i32,
                geos_cap_style(options.cap_style),
                geos_join_style(options.join_style),
                options.mitre_limit,
            )
            .map_err(geos_error)?;
        geos_to_multi_polygon(&buffered, self.precision)
    }

    fn intersection(&self, subject: &MultiPolygon, clip: &MultiPolygon) -> Result<MultiPolygon> {
        let subject = multi_polygon_to_geos(subject)?;
        let clip = multi_polygon_to_geos(clip)?;
        let intersection = subject.intersection(&clip).map_err(geos_error)?;
        geos_to_multi_polygon(&intersection, self.precision)
    }

    fn difference(&self, subject: &MultiPolygon, clip: &MultiPolygon) -> Result<MultiPolygon> {
        let subject = multi_polygon_to_geos(subject)?;
        let clip = multi_polygon_to_geos(clip)?;
        let difference = subject.difference(&clip).map_err(geos_error)?;
        geos_to_multi_polygon(&difference, self.precision)
    }

    fn line_polygon_intersections(
        &self,
        line: &LineString,
        polygon: &Polygon,
    ) -> Result<Vec<Coord>> {
        let line = line_to_geos(line)?;
        let polygon = polygon_to_geos(polygon)?;
        let boundary = polygon.boundary().map_err(geos_error)?;
        let intersections = line.intersection(&boundary).map_err(geos_error)?;
        geos_to_points(&intersections, self.precision)
    }
}

fn polygon_to_geos(polygon: &Polygon) -> Result<Geometry> {
    let exterior = ring_to_geos(&polygon.exterior)?;
    let holes = polygon
        .holes
        .iter()
        .map(ring_to_geos)
        .collect::<Result<Vec<_>>>()?;
    Geometry::create_polygon(exterior, holes).map_err(geos_error)
}

fn multi_polygon_to_geos(multi_polygon: &MultiPolygon) -> Result<Geometry> {
    if multi_polygon.polygons.is_empty() {
        return Geometry::create_empty_collection(GeometryTypes::MultiPolygon).map_err(geos_error);
    }

    let polygons = multi_polygon
        .polygons
        .iter()
        .map(polygon_to_geos)
        .collect::<Result<Vec<_>>>()?;
    Geometry::create_multipolygon(polygons).map_err(geos_error)
}

fn ring_to_geos(ring: &LinearRing) -> Result<Geometry> {
    let seq = coord_seq(&ring.coords)?;
    Geometry::create_linear_ring(seq).map_err(geos_error)
}

fn line_to_geos(line: &LineString) -> Result<Geometry> {
    if line.coords.is_empty() {
        return Geometry::create_empty_line_string().map_err(geos_error);
    }
    let seq = coord_seq(&line.coords)?;
    Geometry::create_line_string(seq).map_err(geos_error)
}

fn coord_seq(coords: &[Coord]) -> Result<CoordSeq> {
    let rows = coords
        .iter()
        .map(|coord| vec![coord.x, coord.y])
        .collect::<Vec<_>>();
    let refs = rows.iter().map(Vec::as_slice).collect::<Vec<&[f64]>>();
    CoordSeq::new_from_vec(&refs).map_err(geos_error)
}

fn geos_to_multi_polygon<G: Geom>(geometry: &G, precision: PrecisionModel) -> Result<MultiPolygon> {
    if geometry.is_empty().map_err(geos_error)? {
        return Ok(MultiPolygon::empty());
    }

    let polygons = match geometry.geometry_type() {
        GeometryTypes::Polygon => vec![geos_to_polygon(geometry, precision)?],
        GeometryTypes::MultiPolygon | GeometryTypes::GeometryCollection => {
            let mut out = Vec::new();
            for index in 0..geometry.get_num_geometries().map_err(geos_error)? {
                let child = geometry.get_geometry_n(index).map_err(geos_error)?;
                match child.geometry_type() {
                    GeometryTypes::Polygon => out.push(geos_to_polygon(&child, precision)?),
                    GeometryTypes::MultiPolygon | GeometryTypes::GeometryCollection => {
                        out.extend(geos_to_multi_polygon(&child, precision)?.polygons);
                    }
                    _ => {}
                }
            }
            out
        }
        _ => Vec::new(),
    };

    Ok(MultiPolygon::new(polygons))
}

fn geos_to_polygon<G: Geom>(geometry: &G, precision: PrecisionModel) -> Result<Polygon> {
    let exterior = geometry.get_exterior_ring().map_err(geos_error)?;
    let holes = (0..geometry.get_num_interior_rings().map_err(geos_error)?)
        .map(|index| {
            geometry
                .get_interior_ring_n(index as u32)
                .map_err(geos_error)
                .and_then(|ring| geos_to_ring(&ring, precision))
        })
        .collect::<Result<Vec<_>>>()?;

    let polygon = Polygon::new(geos_to_ring(&exterior, precision)?, holes);
    Ok(canonicalize_polygon(&polygon, precision))
}

fn geos_to_ring<G: Geom>(geometry: &G, precision: PrecisionModel) -> Result<LinearRing> {
    Ok(LinearRing::new(geos_to_coords(geometry, precision)?))
}

fn geos_to_coords<G: Geom>(geometry: &G, precision: PrecisionModel) -> Result<Vec<Coord>> {
    let seq = geometry.get_coord_seq().map_err(geos_error)?;
    let size = seq.size().map_err(geos_error)?;
    let mut coords = Vec::with_capacity(size);
    for index in 0..size {
        coords.push(precision.snap_coord(Coord::new(
            seq.get_x(index).map_err(geos_error)?,
            seq.get_y(index).map_err(geos_error)?,
        )));
    }
    Ok(coords)
}

fn geos_to_points<G: Geom>(geometry: &G, precision: PrecisionModel) -> Result<Vec<Coord>> {
    if geometry.is_empty().map_err(geos_error)? {
        return Ok(Vec::new());
    }

    match geometry.geometry_type() {
        GeometryTypes::Point => Ok(vec![precision.snap_coord(Coord::new(
            geometry.get_x().map_err(geos_error)?,
            geometry.get_y().map_err(geos_error)?,
        ))]),
        GeometryTypes::MultiPoint | GeometryTypes::GeometryCollection => {
            let mut out = Vec::new();
            for index in 0..geometry.get_num_geometries().map_err(geos_error)? {
                let child = geometry.get_geometry_n(index).map_err(geos_error)?;
                for point in geos_to_points(&child, precision)? {
                    push_unique(&mut out, point, precision);
                }
            }
            Ok(out)
        }
        GeometryTypes::LineString | GeometryTypes::LinearRing => {
            let mut out = Vec::new();
            for point in geos_to_coords(geometry, precision)? {
                push_unique(&mut out, point, precision);
            }
            Ok(out)
        }
        _ => Ok(Vec::new()),
    }
}

fn push_unique(points: &mut Vec<Coord>, point: Coord, precision: PrecisionModel) {
    if !points
        .iter()
        .any(|existing| precision.same_coord(*existing, point))
    {
        points.push(point);
    }
}

fn geos_cap_style(style: CapStyle) -> GeosCapStyle {
    match style {
        CapStyle::Round => GeosCapStyle::Round,
        CapStyle::Flat => GeosCapStyle::Flat,
        CapStyle::Square => GeosCapStyle::Square,
    }
}

fn geos_join_style(style: JoinStyle) -> GeosJoinStyle {
    match style {
        JoinStyle::Round => GeosJoinStyle::Round,
        JoinStyle::Mitre => GeosJoinStyle::Mitre,
        JoinStyle::Bevel => GeosJoinStyle::Bevel,
    }
}

fn geos_error(error: geos::Error) -> GeometryError {
    GeometryError::Backend(error.to_string())
}
