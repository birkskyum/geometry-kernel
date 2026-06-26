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
        .map(|segment| vec![segment.start, segment.end])
        .collect::<Vec<_>>();
    let mut segment_order = (0..segments.len()).collect::<Vec<_>>();
    segment_order.sort_by(|left, right| {
        segments[*left]
            .min_x
            .partial_cmp(&segments[*right].min_x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (order_index, &left_index) in segment_order.iter().enumerate() {
        let left = segments[left_index];
        for &right_index in &segment_order[order_index + 1..] {
            let right = segments[right_index];
            if right.min_x > left.max_x + precision.epsilon() {
                break;
            }
            if left.bbox_disjoint(right, precision) {
                continue;
            }

            match segment_intersection(left.start, left.end, right.start, right.end, precision) {
                Some(SegmentIntersection::Point(point)) => {
                    push_split_point(&mut split_points[left_index], point, precision);
                    push_split_point(&mut split_points[right_index], point, precision);
                }
                Some(SegmentIntersection::Overlap(start, end)) => {
                    for point in [start, end] {
                        if point_on_segment(point, left.start, left.end, precision) {
                            push_split_point(&mut split_points[left_index], point, precision);
                        }
                        if point_on_segment(point, right.start, right.end, precision) {
                            push_split_point(&mut split_points[right_index], point, precision);
                        }
                    }
                }
                None => {}
            }
        }
    }

    let mut lines_out = Vec::new();
    for (segment, points) in segments.iter().zip(split_points.iter_mut()) {
        sort_along_segment(points, segment.start, segment.end);
        dedup_points(points, precision);

        for pair in points.windows(2) {
            if !precision.same_coord(pair[0], pair[1]) {
                lines_out.push(LineString::new(vec![pair[0], pair[1]]));
            }
        }
    }

    NodedLinework { lines: lines_out }
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    start: Coord,
    end: Coord,
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Segment {
    fn new(start: Coord, end: Coord) -> Self {
        Self {
            start,
            end,
            min_x: start.x.min(end.x),
            min_y: start.y.min(end.y),
            max_x: start.x.max(end.x),
            max_y: start.y.max(end.y),
        }
    }

    fn bbox_disjoint(self, other: Self, precision: PrecisionModel) -> bool {
        let eps = precision.epsilon();
        self.min_x > other.max_x + eps
            || other.min_x > self.max_x + eps
            || self.min_y > other.max_y + eps
            || other.min_y > self.max_y + eps
    }
}

fn input_segments(lines: &[LineString], precision: PrecisionModel) -> Vec<Segment> {
    lines
        .iter()
        .flat_map(|line| precision.snap_line(line).segments().collect::<Vec<_>>())
        .filter(|(a, b)| !precision.same_coord(*a, *b))
        .map(|(start, end)| Segment::new(start, end))
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
