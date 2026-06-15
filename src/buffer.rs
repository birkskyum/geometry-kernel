use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

use crate::canonicalize::canonicalize_polygon;
use crate::error::{GeometryError, Result};
use crate::noding::node_lines;
use crate::precision::PrecisionModel;
use crate::predicates::{
    is_ring_ccw, orientation, point_in_ring, segment_intersection, signed_ring_area, PointLocation,
    SegmentIntersection,
};
use crate::types::{Coord, LineString, LinearRing, MultiPolygon, Polygon};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinStyle {
    Round,
    Mitre,
    Bevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapStyle {
    Round,
    Flat,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BufferOptions {
    pub quadrant_segments: u32,
    pub join_style: JoinStyle,
    pub cap_style: CapStyle,
    pub mitre_limit: f64,
    pub simplify_factor: f64,
}

impl Default for BufferOptions {
    fn default() -> Self {
        Self {
            quadrant_segments: 8,
            join_style: JoinStyle::Round,
            cap_style: CapStyle::Round,
            mitre_limit: 5.0,
            simplify_factor: 0.01,
        }
    }
}

pub fn buffer_polygon(
    polygon: &Polygon,
    distance: f64,
    options: BufferOptions,
    precision: PrecisionModel,
) -> Result<MultiPolygon> {
    if polygon.is_empty() || distance == 0.0 {
        return Ok(MultiPolygon::new(vec![canonicalize_polygon(
            polygon, precision,
        )]));
    }

    let polygon = precision.snap_polygon(polygon);
    let shell_coords = remove_repeated_points(&polygon.exterior.coords, precision);
    if shell_coords.len() < 4 {
        return Ok(MultiPolygon::empty());
    }
    if distance < 0.0 && is_eroded_completely(&shell_coords, distance) {
        return Ok(MultiPolygon::empty());
    }

    let offset_distance = distance.abs();
    let shell_side = if distance < 0.0 {
        Position::Right
    } else {
        Position::Left
    };
    let exterior_curve = ring_curve(
        &shell_coords,
        shell_side,
        offset_distance,
        options,
        precision,
    )?;

    let mut curves = vec![exterior_curve.clone()];
    let mut direct_holes = Vec::new();
    for hole in &polygon.holes {
        let hole_coords = remove_repeated_points(&hole.coords, precision);
        if hole_coords.len() < 4 {
            continue;
        }
        if distance > 0.0 && is_eroded_completely(&hole_coords, -distance) {
            continue;
        }
        let hole_curve = ring_curve(
            &hole_coords,
            shell_side.opposite(),
            offset_distance,
            options,
            precision,
        )?;
        direct_holes.push(LinearRing::new(hole_curve.clone()));
        curves.push(hole_curve);
    }

    let polygonized = polygonize_buffer_curves(&curves, precision);
    if !polygonized.is_empty() {
        return Ok(MultiPolygon::new(polygonized));
    }

    let direct = canonicalize_polygon(
        &Polygon::new(LinearRing::new(exterior_curve), direct_holes),
        precision,
    );
    if direct.is_empty() {
        Ok(MultiPolygon::empty())
    } else {
        Ok(MultiPolygon::new(vec![direct]))
    }
}

pub fn line_buffer(
    line: &LineString,
    distance: f64,
    options: BufferOptions,
    precision: PrecisionModel,
) -> Result<MultiPolygon> {
    if line.coords.len() < 2 || distance <= 0.0 {
        return Ok(MultiPolygon::empty());
    }

    let snapped = precision.snap_line(line);
    let coords = remove_repeated_points(&snapped.coords, precision);
    if coords.len() < 2 {
        let Some(center) = snapped.coords.first().copied() else {
            return Ok(MultiPolygon::empty());
        };
        let polygon = canonicalize_polygon(
            &Polygon::new(
                LinearRing::new(point_buffer_ring(center, distance, options, precision)),
                Vec::new(),
            ),
            precision,
        );
        return if polygon.is_empty() {
            Ok(MultiPolygon::empty())
        } else {
            Ok(MultiPolygon::new(vec![polygon]))
        };
    }

    let coords = if coords.len() == 2 && options.join_style == JoinStyle::Round {
        single_segment_line_buffer_ring(&coords, distance, options, precision)?
    } else {
        line_buffer_curve(&coords, distance, options, precision)?
    };
    let polygonized = polygonize_buffer_curves(std::slice::from_ref(&coords), precision);
    if !polygonized.is_empty() {
        return Ok(MultiPolygon::new(polygonized));
    }

    let polygon = canonicalize_polygon(
        &Polygon::new(LinearRing::new(coords), Vec::new()),
        precision,
    );
    if polygon.is_empty() {
        Ok(MultiPolygon::empty())
    } else {
        Ok(MultiPolygon::new(vec![polygon]))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Position {
    Left,
    Right,
}

impl Position {
    const fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

const CLOCKWISE: i8 = -1;
const COUNTERCLOCKWISE: i8 = 1;
const COLLINEAR: i8 = 0;
const OFFSET_SEGMENT_SEPARATION_FACTOR: f64 = 1.0e-3;
const INSIDE_TURN_VERTEX_SNAP_DISTANCE_FACTOR: f64 = 1.0e-3;
const CURVE_VERTEX_SNAP_DISTANCE_FACTOR: f64 = 1.0e-6;
const MAX_CLOSING_SEG_LEN_FACTOR: f64 = 80.0;
const NUM_SIMPLIFY_PTS_TO_CHECK: usize = 10;

fn ring_curve(
    coords: &[Coord],
    mut side: Position,
    distance: f64,
    options: BufferOptions,
    precision: PrecisionModel,
) -> Result<Vec<Coord>> {
    if coords.len() <= 2 {
        return line_buffer_curve(coords, distance, options, precision);
    }

    if coords.len() >= 4 && is_ccw_jts(coords) {
        side = side.opposite();
    }

    let simplify_tolerance = simplify_tolerance(distance, options, side == Position::Right);
    let simplified = simplify_buffer_input_line(coords, simplify_tolerance);
    if simplified.len() < 4 {
        return Err(GeometryError::InvalidGeometry(
            "buffer simplification produced an invalid ring".to_owned(),
        ));
    }

    let n = simplified.len() - 1;
    let mut generator = OffsetSegmentGenerator::new(distance, options, precision);
    generator.init_side_segments(simplified[n - 1], simplified[0], side)?;
    for i in 1..=n {
        generator.add_next_segment(simplified[i], i != 1)?;
    }
    generator.close_ring();
    Ok(generator.coordinates())
}

fn line_buffer_curve(
    coords: &[Coord],
    distance: f64,
    options: BufferOptions,
    precision: PrecisionModel,
) -> Result<Vec<Coord>> {
    let dist_tol = simplify_tolerance(distance, options, false);
    let simp1 = simplify_buffer_input_line(coords, dist_tol);
    if simp1.len() < 2 {
        return Ok(Vec::new());
    }

    let mut generator = OffsetSegmentGenerator::new(distance, options, precision);
    let n1 = simp1.len() - 1;
    generator.init_side_segments(simp1[0], simp1[1], Position::Left)?;
    for point in simp1.iter().take(n1 + 1).skip(2) {
        generator.add_next_segment(*point, true)?;
    }
    generator.add_last_segment();
    generator.add_line_end_cap(simp1[n1 - 1], simp1[n1])?;

    let simp2 = simplify_buffer_input_line(coords, -dist_tol);
    let n2 = simp2.len() - 1;
    generator.init_side_segments(simp2[n2], simp2[n2 - 1], Position::Left)?;
    if n2 >= 2 {
        for i in (0..=n2 - 2).rev() {
            generator.add_next_segment(simp2[i], true)?;
        }
    }
    generator.add_last_segment();
    generator.add_line_end_cap(simp2[1], simp2[0])?;
    generator.close_ring();

    Ok(generator.coordinates())
}

fn single_segment_line_buffer_ring(
    coords: &[Coord],
    distance: f64,
    options: BufferOptions,
    precision: PrecisionModel,
) -> Result<Vec<Coord>> {
    let start = coords[0];
    let end = coords[1];
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length <= precision.epsilon() {
        return Ok(Vec::new());
    }

    let unit = Coord::new(dx / length, dy / length);
    let normal = Coord::new(-unit.y, unit.x);
    let cap_steps = options.quadrant_segments.max(1) as usize * 2;
    let angle_step = PI / cap_steps as f64;
    let mut ring = Vec::with_capacity(2 * cap_steps + 3);

    ring.push(offset_point(start, normal, distance, precision));
    ring.push(offset_point(end, normal, distance, precision));

    for step in 1..=cap_steps {
        let rotated = rotate_vector(normal, -angle_step * step as f64);
        ring.push(offset_point(end, rotated, distance, precision));
    }

    let opposite = Coord::new(-normal.x, -normal.y);
    ring.push(offset_point(start, opposite, distance, precision));

    for step in 1..=cap_steps {
        let rotated = rotate_vector(opposite, -angle_step * step as f64);
        ring.push(offset_point(start, rotated, distance, precision));
    }

    Ok(ring)
}

fn point_buffer_ring(
    center: Coord,
    distance: f64,
    options: BufferOptions,
    precision: PrecisionModel,
) -> Vec<Coord> {
    let steps = options.quadrant_segments.max(1) as usize * 4;
    (0..=steps)
        .map(|step| {
            let angle = 2.0 * PI * step as f64 / steps as f64;
            precision.snap_coord(Coord::new(
                center.x + distance * angle.cos(),
                center.y + distance * angle.sin(),
            ))
        })
        .collect()
}

fn offset_point(point: Coord, unit: Coord, distance: f64, precision: PrecisionModel) -> Coord {
    precision.snap_coord(Coord::new(
        point.x + unit.x * distance,
        point.y + unit.y * distance,
    ))
}

fn rotate_vector(vector: Coord, angle: f64) -> Coord {
    let cos = angle.cos();
    let sin = angle.sin();
    Coord::new(
        vector.x * cos - vector.y * sin,
        vector.x * sin + vector.y * cos,
    )
}

fn simplify_tolerance(distance: f64, options: BufferOptions, right_side: bool) -> f64 {
    let tolerance = distance * options.simplify_factor.max(0.0);
    if right_side {
        -tolerance
    } else {
        tolerance
    }
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    p0: Coord,
    p1: Coord,
}

impl Segment {
    const fn new(p0: Coord, p1: Coord) -> Self {
        Self { p0, p1 }
    }
}

struct OffsetSegmentGenerator {
    distance: f64,
    options: BufferOptions,
    precision: PrecisionModel,
    fillet_angle_quantum: f64,
    closing_seg_length_factor: f64,
    minimum_vertex_distance: f64,
    points: Vec<Coord>,
    s0: Coord,
    s1: Coord,
    s2: Coord,
    offset0: Segment,
    offset1: Segment,
    side: Position,
}

impl OffsetSegmentGenerator {
    fn new(distance: f64, options: BufferOptions, precision: PrecisionModel) -> Self {
        let quadrant_segments = options.quadrant_segments.max(1) as f64;
        let closing_seg_length_factor =
            if options.quadrant_segments >= 8 && options.join_style == JoinStyle::Round {
                MAX_CLOSING_SEG_LEN_FACTOR
            } else {
                1.0
            };

        Self {
            distance,
            options,
            precision,
            fillet_angle_quantum: PI / 2.0 / quadrant_segments,
            closing_seg_length_factor,
            minimum_vertex_distance: distance * CURVE_VERTEX_SNAP_DISTANCE_FACTOR,
            points: Vec::new(),
            s0: Coord::default(),
            s1: Coord::default(),
            s2: Coord::default(),
            offset0: Segment::new(Coord::default(), Coord::default()),
            offset1: Segment::new(Coord::default(), Coord::default()),
            side: Position::Left,
        }
    }

    fn coordinates(self) -> Vec<Coord> {
        self.points
    }

    fn init_side_segments(&mut self, s1: Coord, s2: Coord, side: Position) -> Result<()> {
        self.s1 = s1;
        self.s2 = s2;
        self.side = side;
        self.offset1 = compute_offset_segment(Segment::new(s1, s2), side, self.distance)?;
        Ok(())
    }

    fn add_next_segment(&mut self, point: Coord, add_start_point: bool) -> Result<()> {
        self.s0 = self.s1;
        self.s1 = self.s2;
        self.s2 = point;
        self.offset0 =
            compute_offset_segment(Segment::new(self.s0, self.s1), self.side, self.distance)?;
        self.offset1 =
            compute_offset_segment(Segment::new(self.s1, self.s2), self.side, self.distance)?;

        if same_coord_exact(self.s1, self.s2) {
            return Ok(());
        }

        let orient = orientation_index(self.s0, self.s1, self.s2);
        let outside_turn = (orient == CLOCKWISE && self.side == Position::Left)
            || (orient == COUNTERCLOCKWISE && self.side == Position::Right);

        if orient == COLLINEAR {
            self.add_collinear(add_start_point);
        } else if outside_turn {
            self.add_outside_turn(orient, add_start_point);
        } else {
            self.add_inside_turn();
        }
        Ok(())
    }

    fn add_last_segment(&mut self) {
        self.add_point(self.offset1.p1);
    }

    fn add_line_end_cap(&mut self, p0: Coord, p1: Coord) -> Result<()> {
        let segment = Segment::new(p0, p1);
        let offset_left = compute_offset_segment(segment, Position::Left, self.distance)?;
        let offset_right = compute_offset_segment(segment, Position::Right, self.distance)?;
        let angle = (p1.y - p0.y).atan2(p1.x - p0.x);

        match self.options.cap_style {
            CapStyle::Round => {
                self.add_point(offset_left.p1);
                self.add_directed_fillet(
                    p1,
                    angle + PI / 2.0,
                    angle - PI / 2.0,
                    CLOCKWISE,
                    self.distance,
                );
                self.add_point(offset_right.p1);
            }
            CapStyle::Flat => {
                self.add_point(offset_left.p1);
                self.add_point(offset_right.p1);
            }
            CapStyle::Square => {
                let square_offset = Coord::new(
                    self.distance.abs() * angle.cos(),
                    self.distance.abs() * angle.sin(),
                );
                self.add_point(Coord::new(
                    offset_left.p1.x + square_offset.x,
                    offset_left.p1.y + square_offset.y,
                ));
                self.add_point(Coord::new(
                    offset_right.p1.x + square_offset.x,
                    offset_right.p1.y + square_offset.y,
                ));
            }
        }

        Ok(())
    }

    fn add_outside_turn(&mut self, orient: i8, add_start_point: bool) {
        if self.offset0.p1.distance(self.offset1.p0)
            < self.distance * OFFSET_SEGMENT_SEPARATION_FACTOR
        {
            self.add_point(self.offset0.p1);
            return;
        }

        match self.options.join_style {
            JoinStyle::Mitre => self.add_mitre_join(),
            JoinStyle::Bevel => {
                self.add_point(self.offset0.p1);
                self.add_point(self.offset1.p0);
            }
            JoinStyle::Round => {
                if add_start_point {
                    self.add_point(self.offset0.p1);
                }
                self.add_corner_fillet(
                    self.s1,
                    self.offset0.p1,
                    self.offset1.p0,
                    orient,
                    self.distance,
                );
                self.add_point(self.offset1.p0);
            }
        }
    }

    fn add_inside_turn(&mut self) {
        if let Some(point) = segment_intersection_point(self.offset0, self.offset1, self.precision)
        {
            self.add_point(point);
            return;
        }

        self.add_point(self.offset0.p1);
        if self.offset0.p1.distance(self.offset1.p0)
            >= self.distance * INSIDE_TURN_VERTEX_SNAP_DISTANCE_FACTOR
        {
            if self.closing_seg_length_factor > 0.0 {
                self.add_point(Coord::new(
                    (self.closing_seg_length_factor * self.offset0.p1.x + self.s1.x)
                        / (self.closing_seg_length_factor + 1.0),
                    (self.closing_seg_length_factor * self.offset0.p1.y + self.s1.y)
                        / (self.closing_seg_length_factor + 1.0),
                ));
                self.add_point(Coord::new(
                    (self.closing_seg_length_factor * self.offset1.p0.x + self.s1.x)
                        / (self.closing_seg_length_factor + 1.0),
                    (self.closing_seg_length_factor * self.offset1.p0.y + self.s1.y)
                        / (self.closing_seg_length_factor + 1.0),
                ));
            } else {
                self.add_point(self.s1);
            }
            self.add_point(self.offset1.p0);
        }
    }

    fn add_collinear(&mut self, add_start_point: bool) {
        let dx0 = self.s1.x - self.s0.x;
        let dy0 = self.s1.y - self.s0.y;
        let dx1 = self.s2.x - self.s1.x;
        let dy1 = self.s2.y - self.s1.y;
        if dx0 * dx1 + dy0 * dy1 >= 0.0 {
            return;
        }

        if self.options.join_style == JoinStyle::Bevel
            || self.options.join_style == JoinStyle::Mitre
        {
            if add_start_point {
                self.add_point(self.offset0.p1);
            }
            self.add_point(self.offset1.p0);
            return;
        }

        self.add_corner_fillet(
            self.s1,
            self.offset0.p1,
            self.offset1.p0,
            CLOCKWISE,
            self.distance,
        );
    }

    fn add_mitre_join(&mut self) {
        if let Some(point) = line_intersection_centered(self.offset0, self.offset1, self.precision)
        {
            let mitre_ratio = if self.distance <= 0.0 {
                1.0
            } else {
                point.distance(self.s1) / self.distance.abs()
            };
            if mitre_ratio <= self.options.mitre_limit {
                self.add_point(point);
                return;
            }
        }

        self.add_point(self.offset0.p1);
        self.add_point(self.offset1.p0);
    }

    fn add_corner_fillet(
        &mut self,
        center: Coord,
        start: Coord,
        end: Coord,
        direction: i8,
        radius: f64,
    ) {
        let dx0 = start.x - center.x;
        let dy0 = start.y - center.y;
        let mut start_angle = dy0.atan2(dx0);
        let dx1 = end.x - center.x;
        let dy1 = end.y - center.y;
        let end_angle = dy1.atan2(dx1);

        if direction == CLOCKWISE {
            if start_angle <= end_angle {
                start_angle += 2.0 * PI;
            }
        } else if start_angle >= end_angle {
            start_angle -= 2.0 * PI;
        }

        self.add_point(start);
        self.add_directed_fillet(center, start_angle, end_angle, direction, radius);
        self.add_point(end);
    }

    fn add_directed_fillet(
        &mut self,
        center: Coord,
        start_angle: f64,
        end_angle: f64,
        direction: i8,
        radius: f64,
    ) {
        let direction_factor = if direction == CLOCKWISE { -1.0 } else { 1.0 };
        let total_angle = (start_angle - end_angle).abs();
        let n_segments = (total_angle / self.fillet_angle_quantum + 0.5).trunc() as usize;
        if n_segments < 1 {
            return;
        }

        let angle_inc = total_angle / n_segments as f64;
        for i in 0..n_segments {
            let angle = start_angle + direction_factor * i as f64 * angle_inc;
            self.add_point(Coord::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            ));
        }
    }

    fn add_point(&mut self, point: Coord) {
        let point = self.precision.snap_coord(point);
        if self
            .points
            .last()
            .is_some_and(|last| point.distance(*last) < self.minimum_vertex_distance)
        {
            return;
        }
        self.points.push(point);
    }

    fn close_ring(&mut self) {
        let Some(first) = self.points.first().copied() else {
            return;
        };
        if self
            .points
            .last()
            .is_none_or(|last| !same_coord_exact(first, *last))
        {
            self.points.push(first);
        }
    }
}

fn compute_offset_segment(segment: Segment, side: Position, distance: f64) -> Result<Segment> {
    let side_sign = if side == Position::Left { 1.0 } else { -1.0 };
    let dx = segment.p1.x - segment.p0.x;
    let dy = segment.p1.y - segment.p0.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length == 0.0 {
        return Err(GeometryError::InvalidGeometry(
            "buffer encountered a zero-length segment".to_owned(),
        ));
    }
    let ux = side_sign * distance * dx / length;
    let uy = side_sign * distance * dy / length;
    Ok(Segment::new(
        Coord::new(segment.p0.x - uy, segment.p0.y + ux),
        Coord::new(segment.p1.x - uy, segment.p1.y + ux),
    ))
}

fn segment_intersection_point(
    left: Segment,
    right: Segment,
    precision: PrecisionModel,
) -> Option<Coord> {
    match segment_intersection(left.p0, left.p1, right.p0, right.p1, precision) {
        Some(SegmentIntersection::Point(point)) => Some(point),
        _ => None,
    }
}

fn line_intersection_centered(
    left: Segment,
    right: Segment,
    precision: PrecisionModel,
) -> Option<Coord> {
    let min_x = left.p0.x.min(left.p1.x).max(right.p0.x.min(right.p1.x));
    let max_x = left.p0.x.max(left.p1.x).min(right.p0.x.max(right.p1.x));
    let min_y = left.p0.y.min(left.p1.y).max(right.p0.y.min(right.p1.y));
    let max_y = left.p0.y.max(left.p1.y).min(right.p0.y.max(right.p1.y));
    let mid_x = (min_x + max_x) / 2.0;
    let mid_y = (min_y + max_y) / 2.0;

    let p1 = Coord::new(left.p0.x - mid_x, left.p0.y - mid_y);
    let p2 = Coord::new(left.p1.x - mid_x, left.p1.y - mid_y);
    let q1 = Coord::new(right.p0.x - mid_x, right.p0.y - mid_y);
    let q2 = Coord::new(right.p1.x - mid_x, right.p1.y - mid_y);

    let px = p1.y - p2.y;
    let py = p2.x - p1.x;
    let pw = p1.x * p2.y - p2.x * p1.y;
    let qx = q1.y - q2.y;
    let qy = q2.x - q1.x;
    let qw = q1.x * q2.y - q2.x * q1.y;
    let x = py * qw - qy * pw;
    let y = qx * pw - px * qw;
    let w = px * qy - qx * py;

    if w == 0.0 {
        return None;
    }

    let x_int = x / w;
    let y_int = y / w;
    if !x_int.is_finite() || !y_int.is_finite() {
        return None;
    }

    Some(precision.snap_coord(Coord::new(x_int + mid_x, y_int + mid_y)))
}

fn simplify_buffer_input_line(input: &[Coord], distance_tol: f64) -> Vec<Coord> {
    let distance_tol_abs = distance_tol.abs();
    let angle_orientation = if distance_tol < 0.0 {
        CLOCKWISE
    } else {
        COUNTERCLOCKWISE
    };
    let mut is_deleted = vec![false; input.len()];

    loop {
        let changed =
            delete_shallow_concavities(input, &mut is_deleted, distance_tol_abs, angle_orientation);
        if !changed {
            break;
        }
    }

    input
        .iter()
        .zip(is_deleted)
        .filter_map(|(coord, deleted)| (!deleted).then_some(*coord))
        .collect()
}

fn delete_shallow_concavities(
    input: &[Coord],
    is_deleted: &mut [bool],
    distance_tol: f64,
    angle_orientation: i8,
) -> bool {
    let mut index = 1;
    let mut mid_index = find_next_non_deleted_index(index, is_deleted);
    let mut last_index = find_next_non_deleted_index(mid_index, is_deleted);
    let mut changed = false;

    while last_index < input.len() {
        let mut middle_deleted = false;
        if is_deletable(
            input,
            index,
            mid_index,
            last_index,
            distance_tol,
            angle_orientation,
        ) {
            is_deleted[mid_index] = true;
            middle_deleted = true;
            changed = true;
        }

        if middle_deleted {
            index = last_index;
        } else {
            index = mid_index;
        }
        mid_index = find_next_non_deleted_index(index, is_deleted);
        last_index = find_next_non_deleted_index(mid_index, is_deleted);
    }

    changed
}

fn find_next_non_deleted_index(index: usize, is_deleted: &[bool]) -> usize {
    let mut next = index + 1;
    while next < is_deleted.len() && is_deleted[next] {
        next += 1;
    }
    next
}

fn is_deletable(
    input: &[Coord],
    i0: usize,
    i1: usize,
    i2: usize,
    distance_tol: f64,
    angle_orientation: i8,
) -> bool {
    let p0 = input[i0];
    let p1 = input[i1];
    let p2 = input[i2];
    orientation_index(p0, p1, p2) == angle_orientation
        && point_to_segment_distance(p1, p0, p2) < distance_tol
        && is_shallow_sampled(input, p0, p1, i0, i2, distance_tol)
}

fn is_shallow_sampled(
    input: &[Coord],
    p0: Coord,
    p1: Coord,
    i0: usize,
    i2: usize,
    distance_tol: f64,
) -> bool {
    let mut increment = (i2 - i0) / NUM_SIMPLIFY_PTS_TO_CHECK;
    if increment == 0 {
        increment = 1;
    }

    let mut i = i0;
    while i < i2 {
        if point_to_segment_distance(p1, p0, input[i]) >= distance_tol {
            return false;
        }
        i += increment;
    }
    true
}

fn point_to_segment_distance(point: Coord, a: Coord, b: Coord) -> f64 {
    if same_coord_exact(a, b) {
        return point.distance(a);
    }

    let len2 = b.distance_squared(a);
    let r = ((point.x - a.x) * (b.x - a.x) + (point.y - a.y) * (b.y - a.y)) / len2;
    if r <= 0.0 {
        return point.distance(a);
    }
    if r >= 1.0 {
        return point.distance(b);
    }

    let s = ((a.y - point.y) * (b.x - a.x) - (a.x - point.x) * (b.y - a.y)) / len2;
    s.abs() * len2.sqrt()
}

fn is_eroded_completely(ring: &[Coord], buffer_distance: f64) -> bool {
    if ring.len() < 4 {
        return buffer_distance < 0.0;
    }
    let Some(bbox) = crate::types::BBox::from_coords(ring) else {
        return true;
    };
    let min_dimension = (bbox.max.y - bbox.min.y).min(bbox.max.x - bbox.min.x);
    buffer_distance < 0.0 && 2.0 * buffer_distance.abs() > min_dimension
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CoordKey(u64, u64);

#[derive(Debug, Clone)]
struct HalfEdge {
    from: usize,
    to: usize,
    reverse: usize,
    angle: f64,
    visited: bool,
}

fn polygonize_buffer_curves(curves: &[Vec<Coord>], precision: PrecisionModel) -> Vec<Polygon> {
    let lines = curves
        .iter()
        .filter(|curve| curve.len() >= 4)
        .map(|curve| LineString::new(curve.clone()))
        .collect::<Vec<_>>();
    let noded = node_lines(&lines, precision);
    if noded.lines.is_empty() {
        return Vec::new();
    }

    let mut vertices = Vec::<Coord>::new();
    let mut vertex_ids = HashMap::<CoordKey, usize>::new();
    let mut undirected_edges = HashSet::<(usize, usize)>::new();
    let mut edges = Vec::<HalfEdge>::new();
    let mut outgoing = Vec::<Vec<usize>>::new();

    for line in noded.lines {
        if line.coords.len() != 2 || precision.same_coord(line.coords[0], line.coords[1]) {
            continue;
        }
        let from = vertex_id(
            line.coords[0],
            &mut vertices,
            &mut vertex_ids,
            &mut outgoing,
        );
        let to = vertex_id(
            line.coords[1],
            &mut vertices,
            &mut vertex_ids,
            &mut outgoing,
        );
        let edge_key = if from < to { (from, to) } else { (to, from) };
        if !undirected_edges.insert(edge_key) {
            continue;
        }

        let forward = edges.len();
        let reverse = forward + 1;
        edges.push(HalfEdge {
            from,
            to,
            reverse,
            angle: angle_between(vertices[from], vertices[to]),
            visited: false,
        });
        edges.push(HalfEdge {
            from: to,
            to: from,
            reverse: forward,
            angle: angle_between(vertices[to], vertices[from]),
            visited: false,
        });
        outgoing[from].push(forward);
        outgoing[to].push(reverse);
    }

    for edge_ids in &mut outgoing {
        edge_ids.sort_by(|left, right| {
            edges[*left]
                .angle
                .partial_cmp(&edges[*right].angle)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    let mut shells = Vec::<LinearRing>::new();
    let mut holes = Vec::<LinearRing>::new();
    for edge_id in 0..edges.len() {
        if edges[edge_id].visited {
            continue;
        }
        let Some(ring) = trace_face(edge_id, &mut edges, &outgoing, &vertices, precision) else {
            continue;
        };
        let area = signed_ring_area(&ring);
        if area > precision.epsilon() {
            shells.push(ring);
        } else if area < -precision.epsilon() {
            holes.push(ring);
        }
    }
    assemble_polygons(shells, holes, precision)
}

fn vertex_id(
    coord: Coord,
    vertices: &mut Vec<Coord>,
    vertex_ids: &mut HashMap<CoordKey, usize>,
    outgoing: &mut Vec<Vec<usize>>,
) -> usize {
    let key = coord_key(coord);
    if let Some(id) = vertex_ids.get(&key) {
        return *id;
    }

    let id = vertices.len();
    vertices.push(coord);
    vertex_ids.insert(key, id);
    outgoing.push(Vec::new());
    id
}

fn trace_face(
    start_edge: usize,
    edges: &mut [HalfEdge],
    outgoing: &[Vec<usize>],
    vertices: &[Coord],
    precision: PrecisionModel,
) -> Option<LinearRing> {
    let mut edge_id = start_edge;
    let mut coords = Vec::new();

    loop {
        if edges[edge_id].visited {
            if edge_id == start_edge {
                break;
            }
            return None;
        }

        edges[edge_id].visited = true;
        coords.push(vertices[edges[edge_id].from]);

        let reverse = edges[edge_id].reverse;
        let destination = edges[edge_id].to;
        let edge_ids = &outgoing[destination];
        let reverse_index = edge_ids.iter().position(|id| *id == reverse)?;
        let next_index = if reverse_index == 0 {
            edge_ids.len() - 1
        } else {
            reverse_index - 1
        };
        edge_id = edge_ids[next_index];

        if edge_id == start_edge {
            break;
        }
    }

    if coords.len() < 3 {
        return None;
    }
    coords.push(coords[0]);
    let ring = LinearRing::new(coords);
    (signed_ring_area(&ring).abs() > precision.epsilon()).then_some(ring)
}

fn assemble_polygons(
    shells: Vec<LinearRing>,
    holes: Vec<LinearRing>,
    precision: PrecisionModel,
) -> Vec<Polygon> {
    let mut polygons = shells
        .into_iter()
        .map(|shell| Polygon::new(shell, Vec::new()))
        .collect::<Vec<_>>();

    for hole in holes {
        let Some(sample) = hole.coords.first().copied() else {
            continue;
        };
        let mut target_index = None;
        let mut target_area = f64::INFINITY;
        for (index, polygon) in polygons.iter().enumerate() {
            if matches!(
                point_in_ring(sample, &polygon.exterior, precision),
                PointLocation::Interior
            ) {
                let area = signed_ring_area(&polygon.exterior).abs();
                if area < target_area {
                    target_area = area;
                    target_index = Some(index);
                }
            }
        }
        if let Some(index) = target_index {
            polygons[index].holes.push(hole);
        }
    }

    polygons
        .into_iter()
        .filter(|polygon| !polygon.is_empty())
        .collect()
}

fn angle_between(from: Coord, to: Coord) -> f64 {
    (to.y - from.y).atan2(to.x - from.x)
}

fn coord_key(coord: Coord) -> CoordKey {
    CoordKey(
        normalize_zero(coord.x).to_bits(),
        normalize_zero(coord.y).to_bits(),
    )
}

fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn remove_repeated_points(coords: &[Coord], precision: PrecisionModel) -> Vec<Coord> {
    let mut out = Vec::with_capacity(coords.len());
    for coord in coords {
        let coord = precision.snap_coord(*coord);
        if out
            .last()
            .is_none_or(|last| !same_coord_exact(*last, coord))
        {
            out.push(coord);
        }
    }
    out
}

fn is_ccw_jts(ring: &[Coord]) -> bool {
    if ring.len() < 4 {
        return is_ring_ccw(&LinearRing::new(ring.to_vec()));
    }

    let n = ring.len() - 1;
    let mut high_point = ring[0];
    let mut high_index = 0;
    for (index, point) in ring.iter().enumerate().take(n + 1).skip(1) {
        if point.y > high_point.y {
            high_point = *point;
            high_index = index;
        }
    }

    let mut prev_index = high_index;
    loop {
        prev_index = if prev_index == 0 { n } else { prev_index - 1 };
        if !same_coord_exact(ring[prev_index], high_point) || prev_index == high_index {
            break;
        }
    }

    let mut next_index = high_index;
    loop {
        next_index = (next_index + 1) % n;
        if !same_coord_exact(ring[next_index], high_point) || next_index == high_index {
            break;
        }
    }

    let prev = ring[prev_index];
    let next = ring[next_index];
    if same_coord_exact(prev, high_point)
        || same_coord_exact(next, high_point)
        || same_coord_exact(prev, next)
    {
        return false;
    }

    let disc = orientation_index(prev, high_point, next);
    if disc == COLLINEAR {
        prev.x > next.x
    } else {
        disc > 0
    }
}

fn orientation_index(a: Coord, b: Coord, c: Coord) -> i8 {
    let value = orientation(a, b, c);
    if value > 0.0 {
        COUNTERCLOCKWISE
    } else if value < 0.0 {
        CLOCKWISE
    } else {
        COLLINEAR
    }
}

fn same_coord_exact(a: Coord, b: Coord) -> bool {
    a.x == b.x && a.y == b.y
}
