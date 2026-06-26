use std::collections::{HashMap, HashSet};

use crate::canonicalize::{canonicalize_multi_polygon, canonicalize_polygon};
use crate::error::Result;
use crate::noding::node_lines;
use crate::precision::PrecisionModel;
use crate::predicates::{
    point_in_polygon, point_in_ring, polygon_area, signed_area_coords, PointLocation,
};
use crate::types::{BBox, Coord, LineString, LinearRing, MultiPolygon, Polygon};

pub fn intersection(
    subject: &MultiPolygon,
    clip: &MultiPolygon,
    precision: PrecisionModel,
) -> Result<MultiPolygon> {
    overlay(subject, clip, precision, OverlayOperation::Intersection)
}

pub fn difference(
    subject: &MultiPolygon,
    clip: &MultiPolygon,
    precision: PrecisionModel,
) -> Result<MultiPolygon> {
    overlay(subject, clip, precision, OverlayOperation::Difference)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayOperation {
    Intersection,
    Difference,
}

fn overlay(
    subject: &MultiPolygon,
    clip: &MultiPolygon,
    precision: PrecisionModel,
    operation: OverlayOperation,
) -> Result<MultiPolygon> {
    if subject.is_empty() {
        return Ok(MultiPolygon::empty());
    }

    if clip.is_empty() {
        return Ok(match operation {
            OverlayOperation::Intersection => MultiPolygon::empty(),
            OverlayOperation::Difference => canonicalize_multi_polygon(subject, precision),
        });
    }

    let faces = overlay_faces(subject, clip, precision);
    let mut selected = Vec::new();
    let mut unselected = Vec::new();
    for face in faces {
        let points = representative_points(&face, precision);
        if points.is_empty() {
            continue;
        };

        let is_selected = match operation {
            OverlayOperation::Intersection => points.iter().any(|point| {
                multi_polygon_contains_point(subject, *point, precision)
                    && multi_polygon_contains_point(clip, *point, precision)
            }),
            OverlayOperation::Difference => points.iter().any(|point| {
                multi_polygon_contains_point(subject, *point, precision)
                    && !multi_polygon_contains_point(clip, *point, precision)
            }),
        };

        if is_selected {
            selected.push(face);
        } else {
            unselected.push(face);
        }
    }

    Ok(assemble_selected_faces(selected, &unselected, precision))
}

fn overlay_faces(
    subject: &MultiPolygon,
    clip: &MultiPolygon,
    precision: PrecisionModel,
) -> Vec<Polygon> {
    let lines = multi_polygon_lines(subject)
        .into_iter()
        .chain(multi_polygon_lines(clip))
        .collect::<Vec<_>>();
    let noded = node_lines(&lines, precision);
    let arrangement = Arrangement::from_lines(&noded.lines, precision);
    arrangement.polygonize_faces(precision)
}

fn multi_polygon_lines(multi_polygon: &MultiPolygon) -> Vec<LineString> {
    multi_polygon
        .polygons
        .iter()
        .flat_map(|polygon| {
            std::iter::once(&polygon.exterior)
                .chain(polygon.holes.iter())
                .filter(|ring| ring.coords.len() >= 4)
                .map(|ring| LineString::new(ring.coords.clone()))
        })
        .collect()
}

fn assemble_selected_faces(
    selected: Vec<Polygon>,
    unselected: &[Polygon],
    precision: PrecisionModel,
) -> MultiPolygon {
    let mut output = merge_selected_faces(&selected, precision);
    if output.is_empty() {
        output = selected;
    }
    output = remove_repeated_exterior_loops(output, precision);
    output = attach_nested_shells_as_holes(output, precision);

    for hole_face in unselected {
        let Some(point) = representative_point(hole_face, precision) else {
            continue;
        };
        let Some(output_index) = containing_polygon_index(&output, point, precision) else {
            continue;
        };

        if polygon_area(hole_face) >= polygon_area(&output[output_index]) {
            continue;
        }

        if !ring_strictly_inside_polygon(&hole_face.exterior, &output[output_index], precision) {
            continue;
        }

        output[output_index].holes.push(hole_face.exterior.clone());
    }

    canonicalize_multi_polygon(&MultiPolygon::new(output), precision)
}

fn remove_repeated_exterior_loops(
    polygons: Vec<Polygon>,
    precision: PrecisionModel,
) -> Vec<Polygon> {
    polygons
        .into_iter()
        .map(|mut polygon| {
            polygon.exterior = remove_same_orientation_repeated_loops(&polygon.exterior, precision);
            polygon
        })
        .collect()
}

fn remove_same_orientation_repeated_loops(
    ring: &LinearRing,
    precision: PrecisionModel,
) -> LinearRing {
    let mut out = Vec::<Coord>::new();
    let mut indexes = HashMap::<CoordKey, usize>::new();

    for coord in ring.coords.iter().copied() {
        let coord = precision.snap_coord(coord);
        if out
            .last()
            .is_some_and(|previous| precision.same_coord(*previous, coord))
        {
            continue;
        }
        if !out.is_empty() && precision.same_coord(out[0], coord) {
            continue;
        }

        let key = CoordKey::new(coord);
        if let Some(previous_index) = indexes.get(&key).copied() {
            let mut loop_coords = out[previous_index..].to_vec();
            loop_coords.push(out[previous_index]);
            if signed_area_coords(&loop_coords) > precision.epsilon() {
                for removed in out.drain(previous_index + 1..) {
                    indexes.remove(&CoordKey::new(removed));
                }
                continue;
            }
        }

        indexes.insert(key, out.len());
        out.push(coord);
    }

    if out.len() < 3 {
        return LinearRing { coords: Vec::new() };
    }

    out.push(out[0]);
    LinearRing::new(out)
}

fn ring_strictly_inside_polygon(
    ring: &LinearRing,
    polygon: &Polygon,
    precision: PrecisionModel,
) -> bool {
    ring.coords
        .iter()
        .take(ring.coords.len().saturating_sub(1))
        .all(|coord| {
            matches!(
                point_in_polygon(*coord, polygon, precision),
                PointLocation::Interior
            )
        })
}

fn attach_nested_shells_as_holes(
    mut polygons: Vec<Polygon>,
    precision: PrecisionModel,
) -> Vec<Polygon> {
    if polygons.len() <= 1 {
        return polygons;
    }

    let mut remove = vec![false; polygons.len()];
    let mut hole_assignments = Vec::<(usize, LinearRing)>::new();

    for inner_index in 0..polygons.len() {
        let inner_area = polygon_area(&polygons[inner_index]);
        let Some(point) = representative_point(&polygons[inner_index], precision) else {
            continue;
        };

        let Some(outer_index) = polygons
            .iter()
            .enumerate()
            .filter(|(outer_index, outer)| {
                *outer_index != inner_index
                    && polygon_area(outer) > inner_area
                    && matches!(
                        point_in_ring(point, &outer.exterior, precision),
                        PointLocation::Interior
                    )
            })
            .min_by(|(_, left), (_, right)| {
                polygon_area(left)
                    .partial_cmp(&polygon_area(right))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(outer_index, _)| outer_index)
        else {
            continue;
        };

        remove[inner_index] = true;
        hole_assignments.push((outer_index, polygons[inner_index].exterior.clone()));
    }

    for (outer_index, hole) in hole_assignments {
        if !remove[outer_index] {
            polygons[outer_index].holes.push(hole);
        }
    }

    polygons
        .into_iter()
        .enumerate()
        .filter_map(|(index, polygon)| (!remove[index]).then_some(polygon))
        .collect()
}

fn merge_selected_faces(selected: &[Polygon], precision: PrecisionModel) -> Vec<Polygon> {
    if selected.len() <= 1 {
        return selected.to_vec();
    }

    let mut edge_counts = HashMap::<EdgeKey, BoundaryEdge>::new();
    for face in selected {
        for (start, end) in face.exterior.segments() {
            let key = EdgeKey::new(start, end);
            edge_counts
                .entry(key)
                .and_modify(|edge| edge.count += 1)
                .or_insert(BoundaryEdge {
                    start,
                    end,
                    count: 1,
                });
        }
    }

    let boundary_lines = edge_counts
        .into_values()
        .filter(|edge| edge.count == 1)
        .map(|edge| LineString::new(vec![edge.start, edge.end]))
        .collect::<Vec<_>>();

    if boundary_lines.is_empty() {
        return Vec::new();
    }

    Arrangement::from_lines(&boundary_lines, precision).polygonize_faces(precision)
}

#[derive(Debug, Clone, Copy)]
struct BoundaryEdge {
    start: Coord,
    end: Coord,
    count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct EdgeKey(CoordKey, CoordKey);

impl EdgeKey {
    fn new(start: Coord, end: Coord) -> Self {
        let start = CoordKey::new(start);
        let end = CoordKey::new(end);
        if start <= end {
            Self(start, end)
        } else {
            Self(end, start)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CoordKey(u64, u64);

impl CoordKey {
    fn new(coord: Coord) -> Self {
        Self(
            normalize_zero(coord.x).to_bits(),
            normalize_zero(coord.y).to_bits(),
        )
    }
}

fn containing_polygon_index(
    polygons: &[Polygon],
    point: Coord,
    precision: PrecisionModel,
) -> Option<usize> {
    polygons
        .iter()
        .enumerate()
        .filter(|(_, polygon)| {
            matches!(
                point_in_polygon(point, polygon, precision),
                PointLocation::Interior
            )
        })
        .min_by(|(_, left), (_, right)| {
            polygon_area(left)
                .partial_cmp(&polygon_area(right))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(index, _)| index)
}

fn multi_polygon_contains_point(
    multi_polygon: &MultiPolygon,
    point: Coord,
    precision: PrecisionModel,
) -> bool {
    multi_polygon
        .polygons
        .iter()
        .any(|polygon| polygon_contains_point(polygon, point, precision))
}

fn polygon_bbox(polygon: &Polygon) -> Option<BBox> {
    BBox::from_coords(&polygon.exterior.coords)
}

#[derive(Debug, Clone)]
struct Arrangement {
    vertices: Vec<Coord>,
    adjacency: Vec<Vec<usize>>,
}

impl Arrangement {
    fn from_lines(lines: &[LineString], precision: PrecisionModel) -> Self {
        let mut vertices = Vec::new();
        let mut vertex_indexes = HashMap::<CoordKey, usize>::new();
        let mut adjacency: Vec<Vec<usize>> = Vec::new();

        for line in lines {
            for (start, end) in line.segments() {
                let start_index = vertex_index(
                    &mut vertices,
                    &mut vertex_indexes,
                    &mut adjacency,
                    start,
                    precision,
                );
                let end_index = vertex_index(
                    &mut vertices,
                    &mut vertex_indexes,
                    &mut adjacency,
                    end,
                    precision,
                );
                if start_index == end_index {
                    continue;
                }

                push_unique_neighbor(&mut adjacency[start_index], end_index);
                push_unique_neighbor(&mut adjacency[end_index], start_index);
            }
        }

        for index in 0..vertices.len() {
            let origin = vertices[index];
            adjacency[index].sort_by(|left, right| {
                edge_angle(origin, vertices[*left])
                    .partial_cmp(&edge_angle(origin, vertices[*right]))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        Self {
            vertices,
            adjacency,
        }
    }

    fn polygonize_faces(&self, precision: PrecisionModel) -> Vec<Polygon> {
        let mut visited = HashSet::<(usize, usize)>::new();
        let mut polygons = Vec::new();
        let mut reversed_polygons = Vec::new();

        for start in 0..self.vertices.len() {
            for &end in &self.adjacency[start] {
                if directed_edge_seen(&visited, start, end) {
                    continue;
                }

                let ring = self.walk_face(start, end, &mut visited);
                if ring.len() < 3 {
                    continue;
                }

                let mut coords = ring
                    .iter()
                    .map(|index| self.vertices[*index])
                    .collect::<Vec<_>>();
                coords.push(coords[0]);

                let signed_area = signed_area_coords(&coords);
                if signed_area.abs() <= precision.epsilon() {
                    continue;
                }

                if signed_area < 0.0 {
                    coords.reverse();
                    let polygon = canonicalize_polygon(
                        &Polygon::new(LinearRing::new(coords), Vec::new()),
                        precision,
                    );
                    if !polygon.is_empty()
                        && !reversed_polygons
                            .iter()
                            .any(|existing| existing == &polygon)
                    {
                        reversed_polygons.push(polygon);
                    }
                    continue;
                }

                let polygon = canonicalize_polygon(
                    &Polygon::new(LinearRing::new(coords), Vec::new()),
                    precision,
                );
                if !polygon.is_empty() && !polygons.iter().any(|existing| existing == &polygon) {
                    polygons.push(polygon);
                }
            }
        }

        if polygons.is_empty() {
            reversed_polygons
        } else {
            polygons
        }
    }

    fn walk_face(
        &self,
        start: usize,
        end: usize,
        visited: &mut HashSet<(usize, usize)>,
    ) -> Vec<usize> {
        let mut ring = Vec::new();
        let mut current_start = start;
        let mut current_end = end;
        let max_steps = self
            .adjacency
            .iter()
            .map(Vec::len)
            .sum::<usize>()
            .saturating_add(1);

        for _ in 0..max_steps {
            if directed_edge_seen(visited, current_start, current_end) {
                break;
            }

            visited.insert((current_start, current_end));
            ring.push(current_start);

            let Some(next) = self.next_face_vertex(current_start, current_end) else {
                break;
            };
            current_start = current_end;
            current_end = next;

            if current_start == start && current_end == end {
                break;
            }
        }

        ring
    }

    fn next_face_vertex(&self, previous: usize, current: usize) -> Option<usize> {
        let neighbors = self.adjacency.get(current)?;
        let reverse_index = neighbors
            .iter()
            .position(|neighbor| *neighbor == previous)?;
        Some(neighbors[(reverse_index + neighbors.len() - 1) % neighbors.len()])
    }
}

fn vertex_index(
    vertices: &mut Vec<Coord>,
    vertex_indexes: &mut HashMap<CoordKey, usize>,
    adjacency: &mut Vec<Vec<usize>>,
    coord: Coord,
    precision: PrecisionModel,
) -> usize {
    let snapped = precision.snap_coord(coord);
    let key = CoordKey::new(snapped);
    if let Some(index) = vertex_indexes.get(&key).copied() {
        return index;
    }

    if let Some(index) = vertices
        .iter()
        .position(|existing| precision.same_coord(*existing, snapped))
    {
        vertex_indexes.insert(key, index);
        return index;
    }

    vertices.push(snapped);
    adjacency.push(Vec::new());
    vertex_indexes.insert(key, vertices.len() - 1);
    vertices.len() - 1
}

fn push_unique_neighbor(neighbors: &mut Vec<usize>, neighbor: usize) {
    if !neighbors.contains(&neighbor) {
        neighbors.push(neighbor);
    }
}

fn edge_angle(origin: Coord, target: Coord) -> f64 {
    (target.y - origin.y).atan2(target.x - origin.x)
}

fn directed_edge_seen(visited: &HashSet<(usize, usize)>, start: usize, end: usize) -> bool {
    visited.contains(&(start, end))
}

fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn representative_point(polygon: &Polygon, precision: PrecisionModel) -> Option<Coord> {
    representative_points(polygon, precision).into_iter().next()
}

fn representative_points(polygon: &Polygon, precision: PrecisionModel) -> Vec<Coord> {
    let mut points = Vec::new();

    if let Some(point) = triangle_fan_representative_point(polygon, precision) {
        push_unique_point(&mut points, point, precision);
    }

    if let Some(point) = polygon_centroid(polygon) {
        if matches!(
            point_in_polygon(point, polygon, precision),
            PointLocation::Interior
        ) {
            push_unique_point(&mut points, point, precision);
        }
    }

    if let Some(bbox) = polygon_bbox(polygon) {
        let point = Coord::new(
            (bbox.min.x + bbox.max.x) * 0.5,
            (bbox.min.y + bbox.max.y) * 0.5,
        );
        if matches!(
            point_in_polygon(point, polygon, precision),
            PointLocation::Interior
        ) {
            push_unique_point(&mut points, point, precision);
        }
    }

    for point in edge_probe_representative_points(polygon, precision) {
        push_unique_point(&mut points, point, precision);
    }

    points
}

fn edge_probe_representative_points(polygon: &Polygon, precision: PrecisionModel) -> Vec<Coord> {
    let mut points = Vec::new();
    let extent = polygon_bbox(polygon)
        .map(|bbox| {
            let width = bbox.max.x - bbox.min.x;
            let height = bbox.max.y - bbox.min.y;
            (width * width + height * height).sqrt()
        })
        .unwrap_or(1.0);
    let offsets = [
        (extent * 1.0e-9).max(precision.epsilon() * 10.0),
        (extent * 1.0e-7).max(precision.epsilon() * 10.0),
        (extent * 1.0e-5).max(precision.epsilon() * 10.0),
    ];

    for (start, end) in polygon.exterior.segments() {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length = (dx * dx + dy * dy).sqrt();
        if length <= precision.epsilon() {
            continue;
        }

        let midpoint = Coord::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5);
        let normal = Coord::new(-dy / length, dx / length);
        for offset in offsets {
            for direction in [1.0, -1.0] {
                let point = Coord::new(
                    midpoint.x + normal.x * offset * direction,
                    midpoint.y + normal.y * offset * direction,
                );
                if matches!(
                    point_in_polygon(point, polygon, precision),
                    PointLocation::Interior
                ) {
                    push_unique_point(&mut points, point, precision);
                }
            }
        }
    }

    points
}

fn push_unique_point(points: &mut Vec<Coord>, point: Coord, precision: PrecisionModel) {
    if points
        .iter()
        .all(|existing| !precision.same_coord(*existing, point))
    {
        points.push(point);
    }
}

fn polygon_centroid(polygon: &Polygon) -> Option<Coord> {
    let coords = &polygon.exterior.coords;
    if coords.len() < 4 {
        return None;
    }

    let mut twice_area = 0.0;
    let mut centroid_x = 0.0;
    let mut centroid_y = 0.0;
    for pair in coords.windows(2) {
        let cross = pair[0].x * pair[1].y - pair[1].x * pair[0].y;
        twice_area += cross;
        centroid_x += (pair[0].x + pair[1].x) * cross;
        centroid_y += (pair[0].y + pair[1].y) * cross;
    }

    if twice_area.abs() <= f64::EPSILON {
        return None;
    }

    Some(Coord::new(
        centroid_x / (3.0 * twice_area),
        centroid_y / (3.0 * twice_area),
    ))
}

fn triangle_fan_representative_point(
    polygon: &Polygon,
    precision: PrecisionModel,
) -> Option<Coord> {
    let coords = &polygon.exterior.coords;
    if coords.len() < 4 {
        return None;
    }

    let anchor = coords[0];
    for pair in coords[1..coords.len() - 1].windows(2) {
        let centroid = Coord::new(
            (anchor.x + pair[0].x + pair[1].x) / 3.0,
            (anchor.y + pair[0].y + pair[1].y) / 3.0,
        );
        if matches!(
            point_in_polygon(centroid, polygon, precision),
            PointLocation::Interior
        ) {
            return Some(centroid);
        }
    }

    None
}

fn polygon_contains_point(polygon: &Polygon, point: Coord, precision: PrecisionModel) -> bool {
    matches!(
        point_in_polygon(point, polygon, precision),
        PointLocation::Interior | PointLocation::Boundary
    )
}
