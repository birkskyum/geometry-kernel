#![cfg(feature = "geos-reference")]

use geometry_kernel::buffer::BufferOptions;
use geometry_kernel::{
    Coord, GeometryKernel, GeosReferenceKernel, LineString, LinearRing, MultiPolygon, Polygon,
    PrecisionModel, PureRustKernel,
};
use serde::Deserialize;

const HAZZLE_OVERLAY_GRID_SIZE: f64 = 1.0e-11;
const HAZZLE_AREA_TOLERANCE: f64 = 1.0e-11;

#[derive(Deserialize)]
struct OverlayFixture {
    subject: MultiPolygon,
    clip: MultiPolygon,
}

#[derive(Deserialize)]
struct OverlaySequenceFixture {
    subject: MultiPolygon,
    clips: Vec<MultiPolygon>,
}

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

#[test]
fn geos_reference_difference_matches_hazzle_headland_subtraction() {
    let fixture: OverlayFixture =
        serde_json::from_str(include_str!("fixtures/hazzle-headland-first-diff.json")).unwrap();
    let geos = GeosReferenceKernel::new(PrecisionModel::floating());
    let pure = PureRustKernel::new(PrecisionModel::fixed(HAZZLE_OVERLAY_GRID_SIZE));

    let geos_difference = geos.difference(&fixture.subject, &fixture.clip).unwrap();
    let pure_difference = pure.difference(&fixture.subject, &fixture.clip).unwrap();
    let geos_area = multi_area(&geos, &geos_difference);
    let pure_area = multi_area(&pure, &pure_difference);

    assert_eq!(
        pure_difference.polygons.len(),
        geos_difference.polygons.len()
    );
    assert!(
        (pure_area - geos_area).abs() < HAZZLE_AREA_TOLERANCE,
        "area mismatch: geos={geos_area}, pure={pure_area}, diff={}",
        pure_area - geos_area
    );
}

#[test]
fn geos_reference_difference_matches_hazzle_headland_fourth_subtraction() {
    let fixture: OverlayFixture =
        serde_json::from_str(include_str!("fixtures/hazzle-headland-step-3-diff.json")).unwrap();
    let geos = GeosReferenceKernel::new(PrecisionModel::floating());
    let pure = PureRustKernel::new(PrecisionModel::fixed(HAZZLE_OVERLAY_GRID_SIZE));

    let geos_difference = geos.difference(&fixture.subject, &fixture.clip).unwrap();
    let pure_difference = pure.difference(&fixture.subject, &fixture.clip).unwrap();
    let geos_area = multi_area(&geos, &geos_difference);
    let pure_area = multi_area(&pure, &pure_difference);

    assert_eq!(
        pure_difference.polygons.len(),
        geos_difference.polygons.len()
    );
    assert!(
        (pure_area - geos_area).abs() < HAZZLE_AREA_TOLERANCE,
        "area mismatch: geos={geos_area}, pure={pure_area}, diff={}",
        pure_area - geos_area
    );
}

#[test]
fn geos_reference_difference_matches_hazzle_headland_sixth_subtraction() {
    let fixture: OverlayFixture =
        serde_json::from_str(include_str!("fixtures/hazzle-headland-step-5-diff.json")).unwrap();
    let geos = GeosReferenceKernel::new(PrecisionModel::floating());
    let pure = PureRustKernel::new(PrecisionModel::fixed(HAZZLE_OVERLAY_GRID_SIZE));

    let geos_difference = geos.difference(&fixture.subject, &fixture.clip).unwrap();
    let pure_difference = pure.difference(&fixture.subject, &fixture.clip).unwrap();
    let geos_area = multi_area(&geos, &geos_difference);
    let pure_area = multi_area(&pure, &pure_difference);

    assert_eq!(
        pure_difference.polygons.len(),
        geos_difference.polygons.len()
    );
    assert!(
        (pure_area - geos_area).abs() < HAZZLE_AREA_TOLERANCE,
        "area mismatch: geos={geos_area}, pure={pure_area}, diff={}",
        pure_area - geos_area
    );
}

#[test]
fn geos_reference_difference_matches_hazzle_headland_sequence() {
    let fixture: OverlaySequenceFixture =
        serde_json::from_str(include_str!("fixtures/hazzle-headland-sequence.json")).unwrap();
    let geos = GeosReferenceKernel::new(PrecisionModel::floating());
    let pure = PureRustKernel::new(PrecisionModel::fixed(HAZZLE_OVERLAY_GRID_SIZE));
    let mut geos_subject = fixture.subject.clone();
    let mut pure_subject = fixture.subject;

    for (index, clip) in fixture.clips.iter().enumerate() {
        geos_subject = geos.difference(&geos_subject, clip).unwrap();
        pure_subject = pure.difference(&pure_subject, clip).unwrap();

        let geos_area = multi_area(&geos, &geos_subject);
        let pure_area = multi_area(&pure, &pure_subject);
        assert_eq!(
            pure_subject.polygons.len(),
            geos_subject.polygons.len(),
            "polygon count diverged after clip {index}"
        );
        assert!(
            (pure_area - geos_area).abs() < HAZZLE_AREA_TOLERANCE,
            "area diverged after clip {index}: geos={geos_area}, pure={pure_area}"
        );
    }
}

#[test]
fn geos_reference_difference_matches_rwefwefwe_headland_subtraction() {
    let fixture: OverlayFixture = serde_json::from_str(include_str!(
        "fixtures/rwefwefwe-headland-step-21-diff.json"
    ))
    .unwrap();
    let geos = GeosReferenceKernel::new(PrecisionModel::floating());
    let pure = PureRustKernel::new(PrecisionModel::fixed(HAZZLE_OVERLAY_GRID_SIZE));

    let geos_difference = geos.difference(&fixture.subject, &fixture.clip).unwrap();
    let pure_difference = pure.difference(&fixture.subject, &fixture.clip).unwrap();
    let geos_area = multi_area(&geos, &geos_difference);
    let pure_area = multi_area(&pure, &pure_difference);

    assert_eq!(
        pure_difference.polygons.len(),
        geos_difference.polygons.len()
    );
    assert!(
        (pure_area - geos_area).abs() < HAZZLE_AREA_TOLERANCE,
        "area mismatch: geos={geos_area}, pure={pure_area}, diff={}",
        pure_area - geos_area
    );
}

#[test]
fn geos_reference_difference_matches_rwefwefwe_mid_headland_subtraction() {
    let fixture: OverlayFixture = serde_json::from_str(include_str!(
        "fixtures/rwefwefwe-headland-step-13-diff.json"
    ))
    .unwrap();
    let geos = GeosReferenceKernel::new(PrecisionModel::floating());
    let pure = PureRustKernel::new(PrecisionModel::fixed(HAZZLE_OVERLAY_GRID_SIZE));

    let geos_difference = geos.difference(&fixture.subject, &fixture.clip).unwrap();
    let pure_difference = pure.difference(&fixture.subject, &fixture.clip).unwrap();
    let geos_area = multi_area(&geos, &geos_difference);
    let pure_area = multi_area(&pure, &pure_difference);

    assert_eq!(
        pure_difference.polygons.len(),
        geos_difference.polygons.len()
    );
    assert!(
        (pure_area - geos_area).abs() < HAZZLE_AREA_TOLERANCE,
        "area mismatch: geos={geos_area}, pure={pure_area}, diff={}",
        pure_area - geos_area
    );
}

#[test]
fn geos_reference_difference_matches_rwefwefwe_late_headland_subtraction() {
    let fixture: OverlayFixture = serde_json::from_str(include_str!(
        "fixtures/rwefwefwe-headland-step-22-diff.json"
    ))
    .unwrap();
    let geos = GeosReferenceKernel::new(PrecisionModel::floating());
    let pure = PureRustKernel::new(PrecisionModel::fixed(HAZZLE_OVERLAY_GRID_SIZE));

    let geos_difference = geos.difference(&fixture.subject, &fixture.clip).unwrap();
    let pure_difference = pure.difference(&fixture.subject, &fixture.clip).unwrap();
    let geos_area = multi_area(&geos, &geos_difference);
    let pure_area = multi_area(&pure, &pure_difference);

    assert_eq!(
        pure_difference.polygons.len(),
        geos_difference.polygons.len()
    );
    assert!(
        (pure_area - geos_area).abs() < HAZZLE_AREA_TOLERANCE,
        "area mismatch: geos={geos_area}, pure={pure_area}, diff={}",
        pure_area - geos_area
    );
}

fn multi_area(kernel: &impl GeometryKernel, multi_polygon: &MultiPolygon) -> f64 {
    multi_polygon
        .polygons
        .iter()
        .map(|polygon| kernel.polygon_area(polygon).unwrap())
        .sum()
}
