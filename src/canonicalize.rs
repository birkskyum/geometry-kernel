use std::cmp::Ordering;

use crate::precision::PrecisionModel;
use crate::predicates::{is_ring_ccw, signed_ring_area};
use crate::types::{Coord, LinearRing, MultiPolygon, Polygon};

pub fn canonicalize_ring(
    ring: &LinearRing,
    desired_ccw: bool,
    precision: PrecisionModel,
) -> LinearRing {
    let mut coords = dedup_open_coords(&ring.coords, precision);
    if coords.len() < 3 {
        return LinearRing { coords: Vec::new() };
    }

    let mut closed = coords.clone();
    closed.push(coords[0]);
    let mut normalized = LinearRing::new(closed);

    if signed_ring_area(&normalized).abs() <= precision.epsilon() {
        return LinearRing { coords: Vec::new() };
    }

    if is_ring_ccw(&normalized) != desired_ccw {
        coords.reverse();
    }

    rotate_to_smallest_coord(&mut coords);
    coords.push(coords[0]);
    normalized = LinearRing::new(coords);
    normalized
}

pub fn canonicalize_polygon(polygon: &Polygon, precision: PrecisionModel) -> Polygon {
    let exterior = canonicalize_ring(&polygon.exterior, true, precision);
    if exterior.coords.is_empty() {
        return Polygon::empty();
    }

    let mut holes = polygon
        .holes
        .iter()
        .map(|ring| canonicalize_ring(ring, false, precision))
        .filter(|ring| !ring.coords.is_empty())
        .collect::<Vec<_>>();

    holes.sort_by(compare_rings);
    Polygon::new(exterior, holes)
}

pub fn canonicalize_multi_polygon(
    multi_polygon: &MultiPolygon,
    precision: PrecisionModel,
) -> MultiPolygon {
    let mut polygons = multi_polygon
        .polygons
        .iter()
        .map(|polygon| canonicalize_polygon(polygon, precision))
        .filter(|polygon| !polygon.is_empty())
        .collect::<Vec<_>>();

    polygons.sort_by(|left, right| {
        let area_cmp = signed_ring_area(&right.exterior)
            .abs()
            .partial_cmp(&signed_ring_area(&left.exterior).abs())
            .unwrap_or(Ordering::Equal);
        area_cmp.then_with(|| compare_rings(&left.exterior, &right.exterior))
    });

    MultiPolygon::new(polygons)
}

fn dedup_open_coords(coords: &[Coord], precision: PrecisionModel) -> Vec<Coord> {
    let mut out = Vec::new();
    for coord in coords {
        let snapped = precision.snap_coord(*coord);
        if out
            .last()
            .is_none_or(|last| !precision.same_coord(*last, snapped))
        {
            out.push(snapped);
        }
    }

    while out.len() > 1 && precision.same_coord(out[0], *out.last().expect("checked len")) {
        out.pop();
    }

    out
}

fn rotate_to_smallest_coord(coords: &mut [Coord]) {
    if coords.is_empty() {
        return;
    }

    let min_index = coords
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| compare_coords(left, right))
        .map(|(index, _)| index)
        .unwrap_or(0);

    coords.rotate_left(min_index);
}

fn compare_rings(left: &LinearRing, right: &LinearRing) -> Ordering {
    left.coords
        .iter()
        .zip(right.coords.iter())
        .map(|(left, right)| compare_coords(left, right))
        .find(|ordering| *ordering != Ordering::Equal)
        .unwrap_or_else(|| left.coords.len().cmp(&right.coords.len()))
}

fn compare_coords(left: &Coord, right: &Coord) -> Ordering {
    left.x
        .partial_cmp(&right.x)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.y.partial_cmp(&right.y).unwrap_or(Ordering::Equal))
}
