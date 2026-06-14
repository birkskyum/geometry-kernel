#![cfg(feature = "geos-reference")]

use geometry_kernel::buffer::BufferOptions;
use geometry_kernel::{
    Coord, GeometryKernel, GeosReferenceKernel, LineString, LinearRing, MultiPolygon, Polygon,
    PrecisionModel, PureRustKernel,
};

fn square(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Polygon {
    Polygon::new(
        LinearRing::new(vec![
            Coord::new(min_x, min_y),
            Coord::new(max_x, min_y),
            Coord::new(max_x, max_y),
            Coord::new(min_x, max_y),
            Coord::new(min_x, min_y),
        ]),
        Vec::new(),
    )
}

fn multi(polygon: Polygon) -> MultiPolygon {
    MultiPolygon::new(vec![polygon])
}

#[test]
fn geos_reference_area_matches_pure_rust_for_simple_polygon() {
    let polygon = square(0.0, 0.0, 10.0, 10.0);

    assert_eq!(
        GeosReferenceKernel::default()
            .polygon_area(&polygon)
            .unwrap(),
        PureRustKernel::default().polygon_area(&polygon).unwrap()
    );
}

#[test]
fn geos_reference_intersection_matches_pure_rust_for_convex_polygons() {
    let subject = multi(square(0.0, 0.0, 10.0, 10.0));
    let clip = multi(square(5.0, 5.0, 15.0, 15.0));
    let geos = GeosReferenceKernel::new(PrecisionModel::fixed(1.0e-9));
    let pure = PureRustKernel::new(PrecisionModel::fixed(1.0e-9));

    let geos_intersection = geos.intersection(&subject, &clip).unwrap();
    let pure_intersection = pure.intersection(&subject, &clip).unwrap();

    assert_eq!(geos_intersection, pure_intersection);
}

#[test]
fn geos_reference_line_buffer_returns_areal_geometry() {
    let line = LineString::new(vec![Coord::new(0.0, 0.0), Coord::new(10.0, 0.0)]);
    let buffered = GeosReferenceKernel::default()
        .line_buffer(&line, 1.0, BufferOptions::default())
        .unwrap();

    assert!(!buffered.is_empty());
}
