use robust::{orient2d, Coord as RobustCoord};

use crate::precision::PrecisionModel;
use crate::types::{Coord, LinearRing, Polygon};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointLocation {
    Exterior,
    Boundary,
    Interior,
}

pub fn orientation(a: Coord, b: Coord, c: Coord) -> f64 {
    orient2d(
        RobustCoord { x: a.x, y: a.y },
        RobustCoord { x: b.x, y: b.y },
        RobustCoord { x: c.x, y: c.y },
    )
}

pub fn signed_ring_area(ring: &LinearRing) -> f64 {
    signed_area_coords(&ring.coords)
}

pub fn signed_area_coords(coords: &[Coord]) -> f64 {
    coords
        .windows(2)
        .map(|pair| pair[0].x * pair[1].y - pair[1].x * pair[0].y)
        .sum::<f64>()
        * 0.5
}

pub fn polygon_area(polygon: &Polygon) -> f64 {
    if polygon.is_empty() {
        return 0.0;
    }
    let holes = polygon
        .holes
        .iter()
        .map(|ring| signed_ring_area(ring).abs())
        .sum::<f64>();
    signed_ring_area(&polygon.exterior).abs() - holes
}

pub fn is_ring_ccw(ring: &LinearRing) -> bool {
    signed_ring_area(ring) > 0.0
}

pub fn point_on_segment(point: Coord, a: Coord, b: Coord, precision: PrecisionModel) -> bool {
    if orientation(a, b, point).abs() > precision.epsilon() {
        return false;
    }

    point.x >= a.x.min(b.x) - precision.epsilon()
        && point.x <= a.x.max(b.x) + precision.epsilon()
        && point.y >= a.y.min(b.y) - precision.epsilon()
        && point.y <= a.y.max(b.y) + precision.epsilon()
}

pub fn point_in_ring(point: Coord, ring: &LinearRing, precision: PrecisionModel) -> PointLocation {
    if ring.coords.len() < 4 {
        return PointLocation::Exterior;
    }

    let mut inside = false;
    for (a, b) in ring.segments() {
        if point_on_segment(point, a, b, precision) {
            return PointLocation::Boundary;
        }

        let crosses = (a.y > point.y) != (b.y > point.y);
        if crosses {
            let x_at_y = (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x;
            if x_at_y > point.x {
                inside = !inside;
            }
        }
    }

    if inside {
        PointLocation::Interior
    } else {
        PointLocation::Exterior
    }
}

pub fn point_in_polygon(
    point: Coord,
    polygon: &Polygon,
    precision: PrecisionModel,
) -> PointLocation {
    match point_in_ring(point, &polygon.exterior, precision) {
        PointLocation::Exterior => PointLocation::Exterior,
        PointLocation::Boundary => PointLocation::Boundary,
        PointLocation::Interior => {
            for hole in &polygon.holes {
                match point_in_ring(point, hole, precision) {
                    PointLocation::Interior => return PointLocation::Exterior,
                    PointLocation::Boundary => return PointLocation::Boundary,
                    PointLocation::Exterior => {}
                }
            }
            PointLocation::Interior
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SegmentIntersection {
    Point(Coord),
    Overlap(Coord, Coord),
}

pub fn segment_intersection(
    a1: Coord,
    a2: Coord,
    b1: Coord,
    b2: Coord,
    precision: PrecisionModel,
) -> Option<SegmentIntersection> {
    let o1 = orientation(a1, a2, b1);
    let o2 = orientation(a1, a2, b2);
    let o3 = orientation(b1, b2, a1);
    let o4 = orientation(b1, b2, a2);
    let eps = precision.epsilon();

    if o1.abs() <= eps && o2.abs() <= eps && o3.abs() <= eps && o4.abs() <= eps {
        return collinear_overlap(a1, a2, b1, b2, precision);
    }

    if (o1 > eps && o2 > eps) || (o1 < -eps && o2 < -eps) {
        return None;
    }
    if (o3 > eps && o4 > eps) || (o3 < -eps && o4 < -eps) {
        return None;
    }

    let denom = (a1.x - a2.x) * (b1.y - b2.y) - (a1.y - a2.y) * (b1.x - b2.x);
    if denom.abs() <= eps {
        return None;
    }

    let a_cross = a1.x * a2.y - a1.y * a2.x;
    let b_cross = b1.x * b2.y - b1.y * b2.x;
    let x = (a_cross * (b1.x - b2.x) - (a1.x - a2.x) * b_cross) / denom;
    let y = (a_cross * (b1.y - b2.y) - (a1.y - a2.y) * b_cross) / denom;
    let point = precision.snap_coord(Coord::new(x, y));

    if point_on_segment(point, a1, a2, precision) && point_on_segment(point, b1, b2, precision) {
        Some(SegmentIntersection::Point(point))
    } else {
        None
    }
}

fn collinear_overlap(
    a1: Coord,
    a2: Coord,
    b1: Coord,
    b2: Coord,
    precision: PrecisionModel,
) -> Option<SegmentIntersection> {
    let use_x = (a2.x - a1.x).abs() >= (a2.y - a1.y).abs();
    let mut points = [a1, a2, b1, b2];
    points.sort_by(|left, right| {
        let l = if use_x { left.x } else { left.y };
        let r = if use_x { right.x } else { right.y };
        l.partial_cmp(&r).unwrap_or(std::cmp::Ordering::Equal)
    });

    let start = points[1];
    let end = points[2];
    if point_on_segment(start, a1, a2, precision)
        && point_on_segment(start, b1, b2, precision)
        && point_on_segment(end, a1, a2, precision)
        && point_on_segment(end, b1, b2, precision)
    {
        if precision.same_coord(start, end) {
            Some(SegmentIntersection::Point(precision.snap_coord(start)))
        } else {
            Some(SegmentIntersection::Overlap(
                precision.snap_coord(start),
                precision.snap_coord(end),
            ))
        }
    } else {
        None
    }
}

pub fn is_convex_ring(ring: &LinearRing, precision: PrecisionModel) -> bool {
    let coords = &ring.coords;
    if coords.len() < 4 {
        return false;
    }
    let mut sign = 0i8;
    for i in 0..coords.len() - 1 {
        let a = coords[i];
        let b = coords[(i + 1) % (coords.len() - 1)];
        let c = coords[(i + 2) % (coords.len() - 1)];
        let o = orientation(a, b, c);
        if o.abs() <= precision.epsilon() {
            continue;
        }
        let current = if o > 0.0 { 1 } else { -1 };
        if sign == 0 {
            sign = current;
        } else if sign != current {
            return false;
        }
    }
    true
}
