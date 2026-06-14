use crate::canonicalize::canonicalize_polygon;
use crate::error::{GeometryError, Result};
use crate::precision::PrecisionModel;
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
}

impl Default for BufferOptions {
    fn default() -> Self {
        Self {
            quadrant_segments: 8,
            join_style: JoinStyle::Round,
            cap_style: CapStyle::Round,
            mitre_limit: 5.0,
        }
    }
}

pub fn buffer_polygon(
    polygon: &Polygon,
    distance: f64,
    _options: BufferOptions,
    precision: PrecisionModel,
) -> Result<MultiPolygon> {
    if polygon.is_empty() || distance == 0.0 {
        return Ok(MultiPolygon::new(vec![canonicalize_polygon(
            polygon, precision,
        )]));
    }
    if !polygon.holes.is_empty() {
        return Err(GeometryError::Unsupported(
            "pure Rust polygon buffer currently supports polygons without holes".to_owned(),
        ));
    }

    let polygon = canonicalize_polygon(polygon, precision);
    let ring = offset_ring(&polygon.exterior, distance, precision)?;
    let buffered = canonicalize_polygon(&Polygon::new(ring, Vec::new()), precision);
    if buffered.is_empty() {
        Ok(MultiPolygon::empty())
    } else {
        Ok(MultiPolygon::new(vec![buffered]))
    }
}

pub fn line_buffer(
    line: &LineString,
    distance: f64,
    _options: BufferOptions,
    precision: PrecisionModel,
) -> Result<MultiPolygon> {
    if line.coords.len() < 2 || distance <= 0.0 {
        return Ok(MultiPolygon::empty());
    }

    let snapped = precision.snap_line(line);
    if snapped.coords.len() != 2 {
        return Err(GeometryError::Unsupported(
            "pure Rust line buffer currently supports single-segment lines".to_owned(),
        ));
    }

    let a = snapped.coords[0];
    let b = snapped.coords[1];
    let length = a.distance(b);
    if length <= precision.epsilon() {
        return Ok(MultiPolygon::empty());
    }

    let normal = Coord::new(-(b.y - a.y) / length, (b.x - a.x) / length);
    let d = distance;
    let coords = vec![
        precision.snap_coord(Coord::new(a.x + normal.x * d, a.y + normal.y * d)),
        precision.snap_coord(Coord::new(b.x + normal.x * d, b.y + normal.y * d)),
        precision.snap_coord(Coord::new(b.x - normal.x * d, b.y - normal.y * d)),
        precision.snap_coord(Coord::new(a.x - normal.x * d, a.y - normal.y * d)),
        precision.snap_coord(Coord::new(a.x + normal.x * d, a.y + normal.y * d)),
    ];
    let polygon = canonicalize_polygon(
        &Polygon::new(LinearRing::new(coords), Vec::new()),
        precision,
    );
    Ok(MultiPolygon::new(vec![polygon]))
}

fn offset_ring(ring: &LinearRing, distance: f64, precision: PrecisionModel) -> Result<LinearRing> {
    let open = &ring.coords[..ring.coords.len().saturating_sub(1)];
    if open.len() < 3 {
        return Err(GeometryError::InvalidGeometry(
            "buffer requires a ring with at least three coordinates".to_owned(),
        ));
    }

    let mut out = Vec::with_capacity(open.len() + 1);
    for index in 0..open.len() {
        let prev = open[(index + open.len() - 1) % open.len()];
        let current = open[index];
        let next = open[(index + 1) % open.len()];

        let incoming = edge_outward_normal(prev, current, distance, precision)?;
        let outgoing = edge_outward_normal(current, next, distance, precision)?;

        let p1a = Coord::new(prev.x + incoming.x, prev.y + incoming.y);
        let p1b = Coord::new(current.x + incoming.x, current.y + incoming.y);
        let p2a = Coord::new(current.x + outgoing.x, current.y + outgoing.y);
        let p2b = Coord::new(next.x + outgoing.x, next.y + outgoing.y);

        let joined =
            infinite_line_intersection(p1a, p1b, p2a, p2b, precision).unwrap_or_else(|| {
                let blended = Coord::new(incoming.x + outgoing.x, incoming.y + outgoing.y);
                precision.snap_coord(Coord::new(current.x + blended.x, current.y + blended.y))
            });
        out.push(joined);
    }

    out.push(out[0]);
    Ok(LinearRing::new(out))
}

fn edge_outward_normal(
    start: Coord,
    end: Coord,
    distance: f64,
    precision: PrecisionModel,
) -> Result<Coord> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length <= precision.epsilon() {
        return Err(GeometryError::InvalidGeometry(
            "buffer encountered a zero-length edge".to_owned(),
        ));
    }
    Ok(Coord::new(dy / length * distance, -dx / length * distance))
}

fn infinite_line_intersection(
    a1: Coord,
    a2: Coord,
    b1: Coord,
    b2: Coord,
    precision: PrecisionModel,
) -> Option<Coord> {
    let denom = (a1.x - a2.x) * (b1.y - b2.y) - (a1.y - a2.y) * (b1.x - b2.x);
    if denom.abs() <= precision.epsilon() {
        return None;
    }

    let a_cross = a1.x * a2.y - a1.y * a2.x;
    let b_cross = b1.x * b2.y - b1.y * b2.x;
    Some(precision.snap_coord(Coord::new(
        (a_cross * (b1.x - b2.x) - (a1.x - a2.x) * b_cross) / denom,
        (a_cross * (b1.y - b2.y) - (a1.y - a2.y) * b_cross) / denom,
    )))
}
