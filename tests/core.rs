use geometry_kernel::buffer::BufferOptions;
use geometry_kernel::noding::node_lines;
use geometry_kernel::polygonize::polygonize_closed_lines;
use geometry_kernel::predicates::{
    is_ring_ccw, point_in_polygon, segment_intersection, PointLocation, SegmentIntersection,
};
use geometry_kernel::{
    Coord, GeometryKernel, LineString, LinearRing, MultiPolygon, Polygon, PrecisionModel,
    PureRustKernel,
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

fn rectangle(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Polygon {
    square(min_x, min_y, max_x, max_y)
}

fn multi(polygon: Polygon) -> MultiPolygon {
    MultiPolygon::new(vec![polygon])
}

fn multi_area(kernel: &PureRustKernel, multi_polygon: &MultiPolygon) -> f64 {
    multi_polygon
        .polygons
        .iter()
        .map(|polygon| kernel.polygon_area(polygon).unwrap())
        .sum()
}

#[test]
fn computes_polygon_area_with_holes() {
    let polygon = Polygon::new(
        square(0.0, 0.0, 10.0, 10.0).exterior,
        vec![square(2.0, 2.0, 4.0, 4.0).exterior],
    );

    assert_eq!(
        PureRustKernel::default().polygon_area(&polygon).unwrap(),
        96.0
    );
}

#[test]
fn canonicalizes_ring_orientation_and_start_coordinate() {
    let polygon = Polygon::new(
        LinearRing::new(vec![
            Coord::new(10.0, 0.0),
            Coord::new(0.0, 0.0),
            Coord::new(0.0, 10.0),
            Coord::new(10.0, 10.0),
            Coord::new(10.0, 0.0),
        ]),
        Vec::new(),
    );

    let canonical = PureRustKernel::default().canonicalize_polygon(&polygon);

    assert!(is_ring_ccw(&canonical.exterior));
    assert_eq!(canonical.exterior.coords[0], Coord::new(0.0, 0.0));
}

#[test]
fn detects_point_and_overlap_segment_intersections() {
    let precision = PrecisionModel::floating();

    assert_eq!(
        segment_intersection(
            Coord::new(0.0, 0.0),
            Coord::new(10.0, 10.0),
            Coord::new(0.0, 10.0),
            Coord::new(10.0, 0.0),
            precision
        ),
        Some(SegmentIntersection::Point(Coord::new(5.0, 5.0)))
    );

    assert_eq!(
        segment_intersection(
            Coord::new(0.0, 0.0),
            Coord::new(10.0, 0.0),
            Coord::new(5.0, 0.0),
            Coord::new(15.0, 0.0),
            precision
        ),
        Some(SegmentIntersection::Overlap(
            Coord::new(5.0, 0.0),
            Coord::new(10.0, 0.0)
        ))
    );
}

#[test]
fn detects_near_endpoint_crossing_with_fixed_precision() {
    let precision = PrecisionModel::fixed(1.0e-11);

    let intersection = segment_intersection(
        Coord::new(9.97643221387, 55.970333086009994),
        Coord::new(9.97643308689, 55.97033359586),
        Coord::new(9.97643270445989, 55.97033374354527),
        Coord::new(9.976433617511823, 55.970333256150404),
        precision,
    );

    assert!(matches!(intersection, Some(SegmentIntersection::Point(_))));
}

#[test]
fn classifies_points_in_polygon() {
    let polygon = square(0.0, 0.0, 10.0, 10.0);

    assert_eq!(
        point_in_polygon(Coord::new(5.0, 5.0), &polygon, PrecisionModel::floating()),
        PointLocation::Interior
    );
    assert_eq!(
        point_in_polygon(Coord::new(0.0, 5.0), &polygon, PrecisionModel::floating()),
        PointLocation::Boundary
    );
    assert_eq!(
        point_in_polygon(Coord::new(11.0, 5.0), &polygon, PrecisionModel::floating()),
        PointLocation::Exterior
    );
}

#[test]
fn classifies_points_against_polygon_holes() {
    let polygon = Polygon::new(
        square(0.0, 0.0, 10.0, 10.0).exterior,
        vec![square(2.0, 2.0, 4.0, 4.0).exterior],
    );

    assert_eq!(
        point_in_polygon(Coord::new(1.0, 1.0), &polygon, PrecisionModel::floating()),
        PointLocation::Interior
    );
    assert_eq!(
        point_in_polygon(Coord::new(3.0, 3.0), &polygon, PrecisionModel::floating()),
        PointLocation::Exterior
    );
    assert_eq!(
        point_in_polygon(Coord::new(2.0, 3.0), &polygon, PrecisionModel::floating()),
        PointLocation::Boundary
    );
}

#[test]
fn fixed_precision_snaps_canonicalized_coordinates() {
    let kernel = PureRustKernel::new(PrecisionModel::fixed(0.5));
    let polygon = Polygon::new(
        LinearRing::new(vec![
            Coord::new(0.24, 0.24),
            Coord::new(10.24, 0.24),
            Coord::new(10.24, 9.76),
            Coord::new(0.24, 9.76),
            Coord::new(0.24, 0.24),
        ]),
        Vec::new(),
    );

    let canonical = kernel.canonicalize_polygon(&polygon);

    assert_eq!(canonical.exterior.coords[0], Coord::new(0.0, 0.0));
    assert!(canonical
        .exterior
        .coords
        .iter()
        .all(|coord| [0.0, 10.0].contains(&coord.x) && [0.0, 10.0].contains(&coord.y)));
}

#[test]
fn nodes_crossing_linework() {
    let lines = vec![
        LineString::new(vec![Coord::new(0.0, 0.0), Coord::new(10.0, 10.0)]),
        LineString::new(vec![Coord::new(0.0, 10.0), Coord::new(10.0, 0.0)]),
    ];

    let noded = node_lines(&lines, PrecisionModel::floating());

    assert_eq!(noded.lines.len(), 4);
    assert!(noded
        .lines
        .iter()
        .all(|line| line.coords.contains(&Coord::new(5.0, 5.0))));
}

#[test]
fn polygonizes_closed_lines() {
    let lines = vec![LineString::new(
        square(0.0, 0.0, 10.0, 10.0).exterior.coords,
    )];

    let polygons = polygonize_closed_lines(&lines, PrecisionModel::floating());

    assert_eq!(polygons.len(), 1);
    assert_eq!(
        PureRustKernel::default()
            .polygon_area(&polygons[0])
            .unwrap(),
        100.0
    );
}

#[test]
fn intersects_convex_polygons() {
    let kernel = PureRustKernel::default();
    let subject = multi(square(0.0, 0.0, 10.0, 10.0));
    let clip = multi(square(5.0, 5.0, 15.0, 15.0));

    let intersection = kernel.intersection(&subject, &clip).unwrap();

    assert_eq!(intersection.polygons.len(), 1);
    assert_eq!(
        kernel.polygon_area(&intersection.polygons[0]).unwrap(),
        25.0
    );
}

#[test]
fn intersects_concave_subject_as_multiple_polygons() {
    let kernel = PureRustKernel::default();
    let subject = multi(Polygon::new(
        LinearRing::new(vec![
            Coord::new(0.0, 0.0),
            Coord::new(6.0, 0.0),
            Coord::new(6.0, 2.0),
            Coord::new(2.0, 2.0),
            Coord::new(2.0, 4.0),
            Coord::new(6.0, 4.0),
            Coord::new(6.0, 6.0),
            Coord::new(0.0, 6.0),
            Coord::new(0.0, 0.0),
        ]),
        Vec::new(),
    ));
    let clip = multi(rectangle(3.0, -1.0, 5.0, 7.0));

    let intersection = kernel.intersection(&subject, &clip).unwrap();

    assert_eq!(intersection.polygons.len(), 2);
    let area = intersection
        .polygons
        .iter()
        .map(|polygon| kernel.polygon_area(polygon).unwrap())
        .sum::<f64>();
    assert_eq!(area, 8.0);
}

#[test]
fn difference_keeps_disjoint_subject() {
    let kernel = PureRustKernel::default();
    let subject = multi(square(0.0, 0.0, 10.0, 10.0));
    let clip = multi(square(20.0, 20.0, 30.0, 30.0));

    let difference = kernel.difference(&subject, &clip).unwrap();

    assert_eq!(difference.polygons.len(), 1);
    assert_eq!(kernel.polygon_area(&difference.polygons[0]).unwrap(), 100.0);
}

#[test]
fn difference_removes_subject_when_fully_covered() {
    let kernel = PureRustKernel::default();
    let subject = multi(square(0.0, 0.0, 10.0, 10.0));
    let clip = multi(square(-1.0, -1.0, 11.0, 11.0));

    let difference = kernel.difference(&subject, &clip).unwrap();

    assert!(difference.is_empty());
}

#[test]
fn difference_subtracts_contained_polygon_as_hole() {
    let kernel = PureRustKernel::default();
    let subject = multi(square(0.0, 0.0, 10.0, 10.0));
    let clip = multi(square(2.0, 2.0, 8.0, 8.0));

    let difference = kernel.difference(&subject, &clip).unwrap();

    assert_eq!(difference.polygons.len(), 1);
    assert_eq!(difference.polygons[0].holes.len(), 1);
    assert_eq!(kernel.polygon_area(&difference.polygons[0]).unwrap(), 64.0);
}

#[test]
fn difference_can_split_subject() {
    let kernel = PureRustKernel::default();
    let subject = multi(square(0.0, 0.0, 10.0, 10.0));
    let clip = multi(rectangle(4.0, -1.0, 6.0, 11.0));

    let difference = kernel.difference(&subject, &clip).unwrap();

    assert_eq!(difference.polygons.len(), 2);
    assert_eq!(multi_area(&kernel, &difference), 80.0);
}

#[test]
fn difference_merges_adjacent_faces_after_partial_clip() {
    let kernel = PureRustKernel::default();
    let subject = multi(square(0.0, 0.0, 10.0, 10.0));
    let clip = multi(rectangle(5.0, 2.0, 15.0, 8.0));

    let difference = kernel.difference(&subject, &clip).unwrap();

    assert_eq!(difference.polygons.len(), 1);
    assert_eq!(kernel.polygon_area(&difference.polygons[0]).unwrap(), 70.0);
}

#[test]
fn intersection_preserves_clip_holes() {
    let kernel = PureRustKernel::default();
    let subject = multi(square(-1.0, -1.0, 11.0, 11.0));
    let clip = multi(Polygon::new(
        square(0.0, 0.0, 10.0, 10.0).exterior,
        vec![square(2.0, 2.0, 8.0, 8.0).exterior],
    ));

    let intersection = kernel.intersection(&subject, &clip).unwrap();

    assert_eq!(intersection.polygons.len(), 1);
    assert_eq!(intersection.polygons[0].holes.len(), 1);
    assert_eq!(
        kernel.polygon_area(&intersection.polygons[0]).unwrap(),
        64.0
    );
}

#[test]
fn line_buffer_difference_creates_annulus() {
    let kernel = PureRustKernel::default();
    let line = LineString::new(vec![Coord::new(0.0, 0.0), Coord::new(10.0, 0.0)]);
    let outer = kernel
        .line_buffer(&line, 2.0, BufferOptions::default())
        .unwrap();
    let inner = kernel
        .line_buffer(&line, 1.0, BufferOptions::default())
        .unwrap();

    let annulus = kernel.difference(&outer, &inner).unwrap();
    let expected_area = multi_area(&kernel, &outer) - multi_area(&kernel, &inner);

    assert_eq!(annulus.polygons.len(), 1);
    assert_eq!(annulus.polygons[0].holes.len(), 1);
    assert!((multi_area(&kernel, &annulus) - expected_area).abs() < 1.0e-6);
}

#[test]
fn selects_largest_polygon_by_area() {
    let kernel = PureRustKernel::default();
    let multi_polygon = MultiPolygon::new(vec![
        square(0.0, 0.0, 2.0, 2.0),
        square(0.0, 0.0, 10.0, 10.0),
        square(0.0, 0.0, 4.0, 4.0),
    ]);

    let largest = kernel.largest_polygon(&multi_polygon).unwrap().unwrap();

    assert_eq!(kernel.polygon_area(&largest).unwrap(), 100.0);
}

#[test]
fn buffers_simple_square_inward_and_outward() {
    let kernel = PureRustKernel::default();
    let polygon = square(0.0, 0.0, 10.0, 10.0);

    let outward = kernel
        .buffer_polygon(&polygon, 1.0, BufferOptions::default())
        .unwrap();
    let inward = kernel
        .buffer_polygon(&polygon, -1.0, BufferOptions::default())
        .unwrap();

    assert!(kernel.polygon_area(&outward.polygons[0]).unwrap() > 100.0);
    assert!(kernel.polygon_area(&inward.polygons[0]).unwrap() < 100.0);
}

#[test]
fn inward_buffer_can_erode_polygon_completely() {
    let kernel = PureRustKernel::default();
    let polygon = square(0.0, 0.0, 10.0, 10.0);

    let eroded = kernel
        .buffer_polygon(&polygon, -20.0, BufferOptions::default())
        .unwrap();

    assert!(eroded.is_empty());
}

#[test]
fn buffers_single_segment_line() {
    let kernel = PureRustKernel::default();
    let line = LineString::new(vec![Coord::new(0.0, 0.0), Coord::new(10.0, 0.0)]);

    let buffered = kernel
        .line_buffer(&line, 1.0, BufferOptions::default())
        .unwrap();

    assert_eq!(buffered.polygons.len(), 1);
    assert!(kernel.polygon_area(&buffered.polygons[0]).unwrap() > 0.0);
}

#[test]
fn finds_line_polygon_boundary_intersections() {
    let kernel = PureRustKernel::default();
    let polygon = square(0.0, 0.0, 10.0, 10.0);
    let line = LineString::new(vec![Coord::new(-1.0, 5.0), Coord::new(11.0, 5.0)]);

    let intersections = kernel.line_polygon_intersections(&line, &polygon).unwrap();

    assert_eq!(intersections.len(), 2);
    assert!(intersections.contains(&Coord::new(0.0, 5.0)));
    assert!(intersections.contains(&Coord::new(10.0, 5.0)));
}

#[test]
fn deduplicates_line_polygon_intersections_on_overlapping_edge() {
    let kernel = PureRustKernel::default();
    let polygon = square(0.0, 0.0, 10.0, 10.0);
    let line = LineString::new(vec![Coord::new(-1.0, 0.0), Coord::new(11.0, 0.0)]);

    let intersections = kernel.line_polygon_intersections(&line, &polygon).unwrap();

    assert_eq!(intersections.len(), 2);
    assert!(intersections.contains(&Coord::new(0.0, 0.0)));
    assert!(intersections.contains(&Coord::new(10.0, 0.0)));
}
