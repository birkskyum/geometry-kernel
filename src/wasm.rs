use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::buffer::BufferOptions;
use crate::kernel::GeometryKernel;
use crate::types::{MultiPolygon, Polygon};
use crate::{PrecisionModel, PureRustKernel};

#[derive(Serialize)]
struct WasmResponse<T: Serialize> {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn polygon_area_json(polygon_json: &str) -> String {
    encode_result(|| {
        let polygon: Polygon = serde_json::from_str(polygon_json)?;
        PureRustKernel::new(PrecisionModel::floating()).polygon_area(&polygon)
    })
}

#[wasm_bindgen]
pub fn buffer_polygon_json(polygon_json: &str, distance: f64) -> String {
    encode_result(|| {
        let polygon: Polygon = serde_json::from_str(polygon_json)?;
        PureRustKernel::new(PrecisionModel::floating()).buffer_polygon(
            &polygon,
            distance,
            BufferOptions::default(),
        )
    })
}

#[wasm_bindgen]
pub fn intersection_json(subject_json: &str, clip_json: &str) -> String {
    encode_result(|| {
        let subject: MultiPolygon = serde_json::from_str(subject_json)?;
        let clip: MultiPolygon = serde_json::from_str(clip_json)?;
        PureRustKernel::new(PrecisionModel::floating()).intersection(&subject, &clip)
    })
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn encode_result<T, F>(operation: F) -> String
where
    T: Serialize,
    F: FnOnce() -> crate::Result<T>,
{
    let response = match operation() {
        Ok(value) => WasmResponse {
            ok: true,
            value: Some(value),
            error: None,
        },
        Err(error) => WasmResponse::<T> {
            ok: false,
            value: None,
            error: Some(error.to_string()),
        },
    };

    serde_json::to_string(&response).unwrap_or_else(|error| {
        format!(
            "{{\"ok\":false,\"error\":\"failed to serialize response: {}\"}}",
            error
        )
    })
}
