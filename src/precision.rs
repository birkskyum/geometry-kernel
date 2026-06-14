use crate::types::{Coord, LineString, LinearRing, MultiPolygon, Polygon};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrecisionModel {
    grid_size: Option<f64>,
    epsilon: f64,
}

impl PrecisionModel {
    pub const fn floating() -> Self {
        Self {
            grid_size: None,
            epsilon: 1.0e-9,
        }
    }

    pub fn fixed(grid_size: f64) -> Self {
        assert!(grid_size.is_finite() && grid_size > 0.0);
        Self {
            grid_size: Some(grid_size),
            epsilon: grid_size * 0.5,
        }
    }

    pub const fn epsilon(self) -> f64 {
        self.epsilon
    }

    pub fn snap_value(self, value: f64) -> f64 {
        match self.grid_size {
            Some(grid) => (value / grid).round() * grid,
            None => value,
        }
    }

    pub fn snap_coord(self, coord: Coord) -> Coord {
        Coord::new(self.snap_value(coord.x), self.snap_value(coord.y))
    }

    pub fn same_coord(self, a: Coord, b: Coord) -> bool {
        a.distance_squared(b) <= self.epsilon * self.epsilon
    }

    pub fn snap_line(self, line: &LineString) -> LineString {
        LineString::new(line.coords.iter().map(|c| self.snap_coord(*c)).collect())
    }

    pub fn snap_ring(self, ring: &LinearRing) -> LinearRing {
        LinearRing::new(ring.coords.iter().map(|c| self.snap_coord(*c)).collect())
    }

    pub fn snap_polygon(self, polygon: &Polygon) -> Polygon {
        Polygon::new(
            self.snap_ring(&polygon.exterior),
            polygon
                .holes
                .iter()
                .map(|ring| self.snap_ring(ring))
                .collect(),
        )
    }

    pub fn snap_multi_polygon(self, multi_polygon: &MultiPolygon) -> MultiPolygon {
        MultiPolygon::new(
            multi_polygon
                .polygons
                .iter()
                .map(|polygon| self.snap_polygon(polygon))
                .collect(),
        )
    }
}

impl Default for PrecisionModel {
    fn default() -> Self {
        Self::floating()
    }
}
