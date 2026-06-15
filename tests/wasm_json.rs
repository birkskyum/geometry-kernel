#![cfg(feature = "wasm")]

use geometry_kernel::wasm::{buffer_polygon_json, intersection_json, polygon_area_json};
use serde_json::{json, Value};

fn square(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Value {
    json!({
        "exterior": {
            "coords": [
                { "x": min_x, "y": min_y },
                { "x": max_x, "y": min_y },
                { "x": max_x, "y": max_y },
                { "x": min_x, "y": max_y },
                { "x": min_x, "y": min_y }
            ]
        },
        "holes": []
    })
}

fn multi(polygon: Value) -> Value {
    json!({ "polygons": [polygon] })
}

#[test]
fn polygon_area_json_returns_value_response() {
    let response: Value = serde_json::from_str(&polygon_area_json(
        &square(0.0, 0.0, 10.0, 10.0).to_string(),
    ))
    .unwrap();

    assert_eq!(response["ok"], true);
    assert_eq!(response["value"], 100.0);
}

#[test]
fn polygon_area_json_returns_error_response_for_invalid_json() {
    let response: Value = serde_json::from_str(&polygon_area_json("{not valid json")).unwrap();

    assert_eq!(response["ok"], false);
    assert!(!response["error"].as_str().unwrap().is_empty());
}

#[test]
fn buffer_polygon_json_returns_geometry_response() {
    let response: Value = serde_json::from_str(&buffer_polygon_json(
        &square(0.0, 0.0, 10.0, 10.0).to_string(),
        1.0,
    ))
    .unwrap();

    assert_eq!(response["ok"], true);
    assert_eq!(response["value"]["polygons"].as_array().unwrap().len(), 1);
}

#[test]
fn intersection_json_returns_geometry_response() {
    let subject = multi(square(0.0, 0.0, 10.0, 10.0));
    let clip = multi(square(5.0, 5.0, 15.0, 15.0));

    let response: Value =
        serde_json::from_str(&intersection_json(&subject.to_string(), &clip.to_string())).unwrap();

    assert_eq!(response["ok"], true);
    assert_eq!(response["value"]["polygons"].as_array().unwrap().len(), 1);
}
