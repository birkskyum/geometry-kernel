use crate::canonicalize::{canonicalize_multi_polygon, canonicalize_polygon};
use crate::error::{GeometryError, Result};
use crate::noding::node_lines;
use crate::precision::PrecisionModel;
use crate::predicates::{is_convex_ring, point_in_polygon, signed_area_coords, PointLocation};
use crate::types::{BBox, Coord, LineString, LinearRing, MultiPolygon, Polygon};

pub fn intersection(
    subject: &MultiPolygon,
    clip: &MultiPolygon,
    precision: PrecisionModel,
) -> Result<MultiPolygon> {
    let mut out = Vec::new();

    for subject_polygon in &subject.polygons {
        for clip_polygon in &clip.polygons {
            out.extend(polygon_intersection(
                subject_polygon,
                clip_polygon,
                precision,
            )?);
        }
    }

    Ok(canonicalize_multi_polygon(
        &MultiPolygon::new(out),
        precision,
    ))
}

pub fn difference(
    subject: &MultiPolygon,
    clip: &MultiPolygon,
    precision: PrecisionModel,
) -> Result<MultiPolygon> {
    let mut out = Vec::new();

    for subject_polygon in &subject.polygons {
        let subject_bbox = polygon_bbox(subject_polygon);
        let mut changed = false;
        let mut fully_covered = false;

        for clip_polygon in &clip.polygons {
            let Some(subject_bbox) = subject_bbox else {
                continue;
            };
            let Some(clip_bbox) = polygon_bbox(clip_polygon) else {
                continue;
            };

            if !subject_bbox.intersects(clip_bbox) {
                continue;
            }

            changed = true;
            if polygon_vertices_inside(subject_polygon, clip_polygon, precision) {
                fully_covered = true;
                break;
            }
        }

        if !changed {
            out.push(subject_polygon.clone());
        } else if !fully_covered {
            return Err(GeometryError::Unsupported(
                "pure Rust difference currently supports disjoint or fully covered polygons"
                    .to_owned(),
            ));
        }
    }

    Ok(canonicalize_multi_polygon(
        &MultiPolygon::new(out),
        precision,
    ))
}

fn polygon_intersection(
    subject: &Polygon,
    clip: &Polygon,
    precision: PrecisionModel,
) -> Result<Vec<Polygon>> {
    if subject.is_empty() || clip.is_empty() {
        return Ok(Vec::new());
    }
    if !subject.holes.is_empty() || !clip.holes.is_empty() {
        return Err(GeometryError::Unsupported(
            "pure Rust intersection currently supports polygons without holes".to_owned(),
        ));
    }
    if !is_convex_ring(&clip.exterior, precision) {
        return Err(GeometryError::Unsupported(
            "pure Rust intersection currently requires a convex clip polygon".to_owned(),
        ));
    }

    Ok(intersection_faces(subject, clip, precision))
}

fn polygon_vertices_inside(subject: &Polygon, clip: &Polygon, precision: PrecisionModel) -> bool {
    subject.exterior.coords[..subject.exterior.coords.len().saturating_sub(1)]
        .iter()
        .all(|point| {
            matches!(
                point_in_polygon(*point, clip, precision),
                PointLocation::Interior | PointLocation::Boundary
            )
        })
}

fn polygon_bbox(polygon: &Polygon) -> Option<BBox> {
    BBox::from_coords(&polygon.exterior.coords)
}

fn intersection_faces(
    subject: &Polygon,
    clip: &Polygon,
    precision: PrecisionModel,
) -> Vec<Polygon> {
    let lines = [
        LineString::new(subject.exterior.coords.clone()),
        LineString::new(clip.exterior.coords.clone()),
    ];
    let noded = node_lines(&lines, precision);
    let arrangement = Arrangement::from_lines(&noded.lines, precision);
    let mut out = Vec::new();

    for polygon in arrangement.polygonize_faces(precision) {
        let Some(point) = representative_point(&polygon, precision) else {
            continue;
        };

        if polygon_contains_point(subject, point, precision)
            && polygon_contains_point(clip, point, precision)
            && !out.iter().any(|existing| existing == &polygon)
        {
            out.push(polygon);
        }
    }

    out
}

#[derive(Debug, Clone)]
struct Arrangement {
    vertices: Vec<Coord>,
    adjacency: Vec<Vec<usize>>,
}

impl Arrangement {
    fn from_lines(lines: &[LineString], precision: PrecisionModel) -> Self {
        let mut vertices = Vec::new();
        let mut adjacency: Vec<Vec<usize>> = Vec::new();

        for line in lines {
            for (start, end) in line.segments() {
                let start_index = vertex_index(&mut vertices, &mut adjacency, start, precision);
                let end_index = vertex_index(&mut vertices, &mut adjacency, end, precision);
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
        let mut visited = Vec::<(usize, usize)>::new();
        let mut polygons = Vec::new();

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

                if signed_area_coords(&coords) <= precision.epsilon() {
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

        polygons
    }

    fn walk_face(&self, start: usize, end: usize, visited: &mut Vec<(usize, usize)>) -> Vec<usize> {
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

            visited.push((current_start, current_end));
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
    adjacency: &mut Vec<Vec<usize>>,
    coord: Coord,
    precision: PrecisionModel,
) -> usize {
    if let Some(index) = vertices
        .iter()
        .position(|existing| precision.same_coord(*existing, coord))
    {
        return index;
    }

    vertices.push(precision.snap_coord(coord));
    adjacency.push(Vec::new());
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

fn directed_edge_seen(visited: &[(usize, usize)], start: usize, end: usize) -> bool {
    visited.iter().any(|edge| edge.0 == start && edge.1 == end)
}

fn representative_point(polygon: &Polygon, precision: PrecisionModel) -> Option<Coord> {
    let extent = polygon_bbox(polygon)
        .map(|bbox| {
            let width = bbox.max.x - bbox.min.x;
            let height = bbox.max.y - bbox.min.y;
            (width * width + height * height).sqrt()
        })
        .unwrap_or(1.0);
    let offset = (extent * 1.0e-9).max(precision.epsilon() * 10.0);

    for (start, end) in polygon.exterior.segments() {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length = (dx * dx + dy * dy).sqrt();
        if length <= precision.epsilon() {
            continue;
        }

        let midpoint = Coord::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5);
        let normal = Coord::new(-dy / length, dx / length);
        let point = Coord::new(
            midpoint.x + normal.x * offset,
            midpoint.y + normal.y * offset,
        );
        if matches!(
            point_in_polygon(point, polygon, precision),
            PointLocation::Interior
        ) {
            return Some(point);
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
