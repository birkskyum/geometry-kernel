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
    if orientation(a, b, point).abs() > orientation_epsilon(a, b, point, precision) {
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
    let s1 = orientation_sign(o1, orientation_epsilon(a1, a2, b1, precision));
    let s2 = orientation_sign(o2, orientation_epsilon(a1, a2, b2, precision));
    let s3 = orientation_sign(o3, orientation_epsilon(b1, b2, a1, precision));
    let s4 = orientation_sign(o4, orientation_epsilon(b1, b2, a2, precision));

    if s1 == 0 && s2 == 0 && s3 == 0 && s4 == 0 {
        return collinear_overlap(a1, a2, b1, b2, precision);
    }

    if s1 != 0 && s1 == s2 {
        return None;
    }
    if s3 != 0 && s3 == s4 {
        return None;
    }

    let r = Coord::new(a2.x - a1.x, a2.y - a1.y);
    let s = Coord::new(b2.x - b1.x, b2.y - b1.y);
    let denom = cross(r, s);
    if denom.abs() <= cross_epsilon(r, s, precision) {
        return None;
    }

    let q_minus_p = Coord::new(b1.x - a1.x, b1.y - a1.y);
    let t = cross(q_minus_p, s) / denom;
    let point = precision.snap_coord(Coord::new(a1.x + t * r.x, a1.y + t * r.y));

    if point_on_segment(point, a1, a2, precision) && point_on_segment(point, b1, b2, precision) {
        Some(SegmentIntersection::Point(point))
    } else {
        None
    }
}

fn cross(a: Coord, b: Coord) -> f64 {
    a.x * b.y - a.y * b.x
}

fn orientation_epsilon(a: Coord, b: Coord, c: Coord, precision: PrecisionModel) -> f64 {
    let ab = a.distance(b);
    let ac = a.distance(c);
    precision.epsilon() * (ab + ac).max(f64::EPSILON)
}

fn cross_epsilon(a: Coord, b: Coord, precision: PrecisionModel) -> f64 {
    precision.epsilon() * (vector_length(a) + vector_length(b)).max(f64::EPSILON)
}

fn vector_length(vector: Coord) -> f64 {
    (vector.x * vector.x + vector.y * vector.y).sqrt()
}

fn orientation_sign(value: f64, epsilon: f64) -> i8 {
    if value > epsilon {
        1
    } else if value < -epsilon {
        -1
    } else {
        0
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
        if o.abs() <= orientation_epsilon(a, b, c, precision) {
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
