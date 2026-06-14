use crate::precision::PrecisionModel;
use crate::predicates::{point_on_segment, segment_intersection, SegmentIntersection};
use crate::types::{Coord, LineString};

#[derive(Debug, Clone, PartialEq)]
pub struct NodedLinework {
    pub lines: Vec<LineString>,
}

pub fn node_lines(lines: &[LineString], precision: PrecisionModel) -> NodedLinework {
    let segments = input_segments(lines, precision);
    let mut split_points = segments
        .iter()
        .map(|(a, b)| vec![*a, *b])
        .collect::<Vec<_>>();

    for left_index in 0..segments.len() {
        for right_index in left_index + 1..segments.len() {
            let (a1, a2) = segments[left_index];
            let (b1, b2) = segments[right_index];

            match segment_intersection(a1, a2, b1, b2, precision) {
                Some(SegmentIntersection::Point(point)) => {
                    push_split_point(&mut split_points[left_index], point, precision);
                    push_split_point(&mut split_points[right_index], point, precision);
                }
                Some(SegmentIntersection::Overlap(start, end)) => {
                    for point in [start, end] {
                        if point_on_segment(point, a1, a2, precision) {
                            push_split_point(&mut split_points[left_index], point, precision);
                        }
                        if point_on_segment(point, b1, b2, precision) {
                            push_split_point(&mut split_points[right_index], point, precision);
                        }
                    }
                }
                None => {}
            }
        }
    }

    let mut lines_out = Vec::new();
    for ((a, b), points) in segments.iter().zip(split_points.iter_mut()) {
        sort_along_segment(points, *a, *b);
        dedup_points(points, precision);

        for pair in points.windows(2) {
            if !precision.same_coord(pair[0], pair[1]) {
                lines_out.push(LineString::new(vec![pair[0], pair[1]]));
            }
        }
    }

    NodedLinework { lines: lines_out }
}

fn input_segments(lines: &[LineString], precision: PrecisionModel) -> Vec<(Coord, Coord)> {
    lines
        .iter()
        .flat_map(|line| precision.snap_line(line).segments().collect::<Vec<_>>())
        .filter(|(a, b)| !precision.same_coord(*a, *b))
        .collect()
}

fn push_split_point(points: &mut Vec<Coord>, point: Coord, precision: PrecisionModel) {
    let snapped = precision.snap_coord(point);
    if !points
        .iter()
        .any(|existing| precision.same_coord(*existing, snapped))
    {
        points.push(snapped);
    }
}

fn sort_along_segment(points: &mut [Coord], a: Coord, b: Coord) {
    let use_x = (b.x - a.x).abs() >= (b.y - a.y).abs();
    points.sort_by(|left, right| {
        let left_value = if use_x { left.x } else { left.y };
        let right_value = if use_x { right.x } else { right.y };
        left_value
            .partial_cmp(&right_value)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn dedup_points(points: &mut Vec<Coord>, precision: PrecisionModel) {
    let mut deduped = Vec::with_capacity(points.len());
    for point in points.iter().copied() {
        if deduped
            .last()
            .is_none_or(|last| !precision.same_coord(*last, point))
        {
            deduped.push(point);
        }
    }
    *points = deduped;
}
