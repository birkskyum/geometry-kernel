use crate::canonicalize::canonicalize_polygon;
use crate::precision::PrecisionModel;
use crate::predicates::signed_ring_area;
use crate::types::{LineString, LinearRing, Polygon};

pub fn polygonize_closed_lines(lines: &[LineString], precision: PrecisionModel) -> Vec<Polygon> {
    lines
        .iter()
        .filter_map(|line| polygon_from_closed_line(line, precision))
        .collect()
}

pub fn polygonize_noded_segments(lines: &[LineString], precision: PrecisionModel) -> Vec<Polygon> {
    polygonize_closed_lines(lines, precision)
}

fn polygon_from_closed_line(line: &LineString, precision: PrecisionModel) -> Option<Polygon> {
    if line.coords.len() < 4 {
        return None;
    }

    let first = *line.coords.first()?;
    let last = *line.coords.last()?;
    if !precision.same_coord(first, last) {
        return None;
    }

    let ring = LinearRing::new(
        line.coords
            .iter()
            .map(|c| precision.snap_coord(*c))
            .collect(),
    );
    if signed_ring_area(&ring).abs() <= precision.epsilon() {
        return None;
    }

    Some(canonicalize_polygon(
        &Polygon::new(ring, Vec::new()),
        precision,
    ))
}
