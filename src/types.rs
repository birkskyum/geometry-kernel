use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Coord {
    pub x: f64,
    pub y: f64,
}

impl Coord {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance_squared(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    pub fn distance(self, other: Self) -> f64 {
        self.distance_squared(other).sqrt()
    }
}

impl From<[f64; 2]> for Coord {
    fn from(value: [f64; 2]) -> Self {
        Self::new(value[0], value[1])
    }
}

impl From<Coord> for [f64; 2] {
    fn from(value: Coord) -> Self {
        [value.x, value.y]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineString {
    pub coords: Vec<Coord>,
}

impl LineString {
    pub fn new(coords: Vec<Coord>) -> Self {
        Self { coords }
    }

    pub fn is_empty(&self) -> bool {
        self.coords.is_empty()
    }

    pub fn segments(&self) -> impl Iterator<Item = (Coord, Coord)> + '_ {
        self.coords.windows(2).map(|pair| (pair[0], pair[1]))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinearRing {
    pub coords: Vec<Coord>,
}

impl LinearRing {
    pub fn new(mut coords: Vec<Coord>) -> Self {
        close_coords(&mut coords);
        Self { coords }
    }

    pub fn is_empty(&self) -> bool {
        self.coords.is_empty()
    }

    pub fn segments(&self) -> impl Iterator<Item = (Coord, Coord)> + '_ {
        self.coords.windows(2).map(|pair| (pair[0], pair[1]))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Polygon {
    pub exterior: LinearRing,
    #[serde(default)]
    pub holes: Vec<LinearRing>,
}

impl Polygon {
    pub fn new(exterior: LinearRing, holes: Vec<LinearRing>) -> Self {
        Self { exterior, holes }
    }

    pub fn empty() -> Self {
        Self::new(LinearRing { coords: Vec::new() }, Vec::new())
    }

    pub fn is_empty(&self) -> bool {
        self.exterior.coords.len() < 4
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MultiPolygon {
    #[serde(default)]
    pub polygons: Vec<Polygon>,
}

impl MultiPolygon {
    pub fn new(polygons: Vec<Polygon>) -> Self {
        Self { polygons }
    }

    pub fn empty() -> Self {
        Self {
            polygons: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.polygons.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BBox {
    pub min: Coord,
    pub max: Coord,
}

impl BBox {
    pub fn from_coords(coords: &[Coord]) -> Option<Self> {
        let first = *coords.first()?;
        let mut bbox = Self {
            min: first,
            max: first,
        };
        for coord in &coords[1..] {
            bbox.min.x = bbox.min.x.min(coord.x);
            bbox.min.y = bbox.min.y.min(coord.y);
            bbox.max.x = bbox.max.x.max(coord.x);
            bbox.max.y = bbox.max.y.max(coord.y);
        }
        Some(bbox)
    }

    pub fn intersects(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }
}

pub fn close_coords(coords: &mut Vec<Coord>) {
    if let (Some(first), Some(last)) = (coords.first().copied(), coords.last().copied()) {
        if first != last {
            coords.push(first);
        }
    }
}
