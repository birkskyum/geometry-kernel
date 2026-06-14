use crate::canonicalize::{canonicalize_multi_polygon, canonicalize_polygon};
use crate::error::{GeometryError, Result};
use crate::precision::PrecisionModel;
use crate::predicates::{is_convex_ring, is_ring_ccw, point_in_polygon, PointLocation};
use crate::types::{BBox, Coord, LinearRing, MultiPolygon, Polygon};

pub fn intersection(
    subject: &MultiPolygon,
    clip: &MultiPolygon,
    precision: PrecisionModel,
) -> Result<MultiPolygon> {
    let mut out = Vec::new();

    for subject_polygon in &subject.polygons {
        for clip_polygon in &clip.polygons {
            if let Some(polygon) = polygon_intersection(subject_polygon, clip_polygon, precision)? {
                out.push(polygon);
            }
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
) -> Result<Option<Polygon>> {
    if subject.is_empty() || clip.is_empty() {
        return Ok(None);
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

    let mut output = subject.exterior.coords[..subject.exterior.coords.len() - 1].to_vec();
    let clip_coords = &clip.exterior.coords;
    let clip_ccw = is_ring_ccw(&clip.exterior);

    for edge in clip_coords.windows(2) {
        if output.is_empty() {
            break;
        }

        let input = output;
        output = Vec::new();
        let mut previous = *input.last().expect("non-empty input");
        let mut previous_inside = inside_clip_edge(previous, edge[0], edge[1], clip_ccw, precision);

        for current in input {
            let current_inside = inside_clip_edge(current, edge[0], edge[1], clip_ccw, precision);
            match (previous_inside, current_inside) {
                (true, true) => output.push(current),
                (true, false) => {
                    output.push(line_intersection(
                        previous, current, edge[0], edge[1], precision,
                    ));
                }
                (false, true) => {
                    output.push(line_intersection(
                        previous, current, edge[0], edge[1], precision,
                    ));
                    output.push(current);
                }
                (false, false) => {}
            }
            previous = current;
            previous_inside = current_inside;
        }
    }

    dedup_ring_coords(&mut output, precision);
    if output.len() < 3 {
        return Ok(None);
    }

    output.push(output[0]);
    let polygon = canonicalize_polygon(
        &Polygon::new(LinearRing::new(output), Vec::new()),
        precision,
    );
    if polygon.is_empty() {
        Ok(None)
    } else {
        Ok(Some(polygon))
    }
}

fn inside_clip_edge(
    point: Coord,
    edge_start: Coord,
    edge_end: Coord,
    clip_ccw: bool,
    precision: PrecisionModel,
) -> bool {
    let cross = (edge_end.x - edge_start.x) * (point.y - edge_start.y)
        - (edge_end.y - edge_start.y) * (point.x - edge_start.x);
    if clip_ccw {
        cross >= -precision.epsilon()
    } else {
        cross <= precision.epsilon()
    }
}

fn line_intersection(
    a1: Coord,
    a2: Coord,
    b1: Coord,
    b2: Coord,
    precision: PrecisionModel,
) -> Coord {
    let denom = (a1.x - a2.x) * (b1.y - b2.y) - (a1.y - a2.y) * (b1.x - b2.x);
    if denom.abs() <= precision.epsilon() {
        return precision.snap_coord(a2);
    }

    let a_cross = a1.x * a2.y - a1.y * a2.x;
    let b_cross = b1.x * b2.y - b1.y * b2.x;
    precision.snap_coord(Coord::new(
        (a_cross * (b1.x - b2.x) - (a1.x - a2.x) * b_cross) / denom,
        (a_cross * (b1.y - b2.y) - (a1.y - a2.y) * b_cross) / denom,
    ))
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

fn dedup_ring_coords(coords: &mut Vec<Coord>, precision: PrecisionModel) {
    let mut deduped = Vec::with_capacity(coords.len());
    for coord in coords.iter().copied() {
        if deduped
            .last()
            .is_none_or(|last| !precision.same_coord(*last, coord))
        {
            deduped.push(coord);
        }
    }
    if deduped.len() > 1 && precision.same_coord(deduped[0], *deduped.last().unwrap()) {
        deduped.pop();
    }
    *coords = deduped;
}
